//! Plan verbs. They mutate [`State`] and record [`Write`]s; the caller
//! persists the writes in the same transaction.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::admission::{self, Cond};
use crate::clock::{add_duration, parse_stamp, stamp};
use crate::error::FlowError;
use crate::explain;
use crate::model::{Attempt, Outcome, Pair, Plan, PlanKind, Step, StepState, Write};
use crate::revision::{self, Absent, Carry};
use crate::slug::is_slug;
use crate::store::State;
use crate::validate;

pub const DEFAULT_LEASE: Duration = Duration::from_secs(15 * 60);
pub const MAX_LEASE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone)]
pub struct AddStep {
    pub id: String,
    pub title: String,
    pub done_when: String,
    pub produces: Option<String>,
    pub results: Vec<String>,
    pub needs: Vec<String>,
    pub when: Vec<Pair>,
    pub when_any: Vec<Vec<Pair>>,
    pub actor: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SetStep {
    pub id: String,
    pub title: Option<String>,
    pub done_when: Option<String>,
    pub produces: Option<String>,
    pub no_produces: bool,
    pub needs: Option<Vec<String>>,
    pub no_needs: bool,
    pub results: Option<Vec<String>>,
    pub no_results: bool,
    pub when: Option<Vec<Pair>>,
    pub when_any: Option<Vec<Vec<Pair>>>,
    pub no_when: bool,
    pub actor: Option<String>,
    pub no_actor: bool,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub step: String,
    pub agent: String,
    pub outcome: Outcome,
    pub result: Option<String>,
    pub note: Option<String>,
    pub because: Option<String>,
    pub artifact: Option<String>,
    pub artifact_hash: Option<String>,
}

pub fn goal(
    state: &mut State,
    now: DateTime<Utc>,
    text: Option<String>,
    retries: Option<u32>,
) -> Result<Value, FlowError> {
    if state.draft().is_some() {
        if text.is_none() && retries.is_none() {
            return Err(FlowError::usage(
                "goal needs --text or --retries",
                "pass --text TEXT, --retries N, or both",
            ));
        }
        let plan = state.draft_mut().expect("draft");
        let next_text = text.unwrap_or_else(|| plan.goal.clone());
        let next_retries = retries.unwrap_or(plan.retries);
        if next_text.trim().is_empty() {
            return Err(empty("goal"));
        }
        if next_text == plan.goal && next_retries == plan.retries {
            return Ok(json!({"goal": plan.goal, "retries": plan.retries}));
        }
        plan.goal = next_text;
        plan.retries = next_retries;
        let plan = plan.clone();
        state.writes.push(Write::UpdatePlan(plan.clone()));
        return Ok(json!({"goal": plan.goal, "retries": plan.retries}));
    }
    if state.committed().is_some() {
        return Err(FlowError::refusal(
            "needs_revise",
            "a plan is committed and no draft is open",
            "call revise",
        ));
    }
    let Some(text) = text else {
        return Err(FlowError::usage("goal needs --text", "pass --text TEXT"));
    };
    if text.trim().is_empty() {
        return Err(empty("goal"));
    }
    let plan = Plan {
        uid: fresh("plan"),
        name: state.plan_name.clone(),
        kind: PlanKind::Template,
        inputs: Vec::new(),
        template: None,
        template_version: None,
        version: None,
        based_on: None,
        goal: text,
        retries: retries.unwrap_or(2),
        created_at: stamp(now),
        steps: Vec::new(),
        supersedes: None,
    };
    let body = json!({"goal": plan.goal, "retries": plan.retries});
    state.writes.push(Write::CreatePlan(plan.clone()));
    state.plans.push(plan);
    Ok(body)
}

pub fn add(state: &mut State, spec: AddStep) -> Result<Value, FlowError> {
    require_draft_edit(state)?;
    check_place(spec.before.as_deref(), spec.after.as_deref())?;
    check_condition_flags(!spec.when.is_empty(), !spec.when_any.is_empty())?;
    validate_identity(
        &spec.id,
        spec.produces.as_deref(),
        spec.actor.as_deref(),
        &spec.results,
        &spec.needs,
    )?;
    if spec.title.trim().is_empty() {
        return Err(empty("title"));
    }
    if spec.done_when.trim().is_empty() {
        return Err(empty("done-when"));
    }
    if !spec.results.is_empty() && spec.produces.is_none() {
        return Err(results_need_produce(&spec.id));
    }
    reject_dups(&spec.results, "result")?;
    reject_dups(&spec.needs, "need")?;
    let plan = state.draft().expect("draft");
    if let Some(existing) = plan.steps.iter().find(|s| s.id == spec.id) {
        if same_step(existing, &spec)
            && place_matches(
                &plan.steps,
                &spec.id,
                spec.before.as_deref(),
                spec.after.as_deref(),
            )
        {
            return Ok(add_body(plan, &spec.id));
        }
        return Err(FlowError::refusal(
            "step_differs",
            format!("{} already exists with different flags", spec.id),
            format!("call set {}", spec.id),
        ));
    }
    if let Some(produce) = &spec.produces {
        if plan
            .steps
            .iter()
            .any(|s| s.produces.as_deref() == Some(produce.as_str()))
        {
            return Err(FlowError::refusal(
                "duplicate_value",
                format!("produce name '{produce}' is already used"),
                "choose a produce name no other step uses",
            ));
        }
    }
    ensure_known_place(&plan.steps, spec.before.as_deref(), spec.after.as_deref())?;
    let uid = fresh("step");
    let new_uid = uid.clone();
    let step = Step {
        uid,
        id: spec.id.clone(),
        title: spec.title,
        done_when: spec.done_when,
        produces: spec.produces,
        results: spec.results,
        needs: spec.needs,
        when: spec.when,
        when_any: spec.when_any,
        actor: spec.actor,
        state: StepState::Pending,
        position: 0,
    };
    let plan_uid = plan.uid.clone();
    let index = state.draft_index().expect("draft");
    insert_step(
        &mut state.plans[index].steps,
        step,
        spec.before.as_deref(),
        spec.after.as_deref(),
    );
    persist_steps(state, index, &plan_uid, false, Some(&new_uid));
    let plan = state.draft().expect("draft");
    Ok(add_body(plan, &spec.id))
}

pub fn set_step(state: &mut State, spec: SetStep) -> Result<Value, FlowError> {
    require_draft_edit(state)?;
    check_place(spec.before.as_deref(), spec.after.as_deref())?;
    if spec.no_produces && spec.produces.is_some()
        || spec.no_needs && spec.needs.is_some()
        || spec.no_results && spec.results.is_some()
        || spec.no_actor && spec.actor.is_some()
        || spec.no_when && (spec.when.is_some() || spec.when_any.is_some())
    {
        return Err(FlowError::usage(
            "a --no-* flag was combined with the flag it clears",
            "pass either the value or the --no-* flag, not both",
        ));
    }
    if spec.when.is_some() && spec.when_any.is_some() {
        return Err(FlowError::usage(
            "--when and --when-any cannot be combined",
            "pass only one of --when or --when-any",
        ));
    }
    let committed_reopen = state.committed().and_then(|p| {
        p.steps
            .iter()
            .find(|s| s.id == spec.id && s.state == StepState::Succeeded)
            .map(|s| s.done_when.clone())
    });
    let index = state.draft_index().expect("draft");
    let plan_uid = state.plans[index].uid.clone();
    let Some(pos) = state.plans[index]
        .steps
        .iter()
        .position(|s| s.id == spec.id)
    else {
        return Err(unknown(&spec.id));
    };
    if let Some(title) = &spec.title {
        if title.trim().is_empty() {
            return Err(empty("title"));
        }
    }
    if let Some(done) = &spec.done_when {
        if done.trim().is_empty() {
            return Err(empty("done-when"));
        }
    }
    if let Some(produce) = &spec.produces {
        validate_identity(&spec.id, Some(produce), None, &[], &[])?;
        if state.plans[index]
            .steps
            .iter()
            .any(|s| s.id != spec.id && s.produces.as_deref() == Some(produce.as_str()))
        {
            return Err(FlowError::refusal(
                "duplicate_value",
                format!("produce name '{produce}' is already used"),
                "choose a produce name no other step uses",
            ));
        }
    }
    if let Some(actor) = &spec.actor {
        if !is_slug(actor) {
            return Err(bad_slug(actor));
        }
    }
    if let Some(results) = &spec.results {
        validate_identity(&spec.id, None, None, results, &[])?;
        reject_dups(results, "result")?;
    }
    if let Some(needs) = &spec.needs {
        validate_identity(&spec.id, None, None, &[], needs)?;
        reject_dups(needs, "need")?;
    }
    ensure_known_place(
        &state.plans[index].steps,
        spec.before.as_deref(),
        spec.after.as_deref(),
    )?;

    let mut step = state.plans[index].steps[pos].clone();
    if let Some(title) = spec.title.clone() {
        step.title = title;
    }
    if let Some(done) = spec.done_when.clone() {
        step.done_when = done;
    }
    if spec.no_produces {
        step.produces = None;
    } else if let Some(produce) = spec.produces.clone() {
        step.produces = Some(produce);
    }
    if spec.no_needs {
        step.needs.clear();
    } else if let Some(needs) = spec.needs.clone() {
        step.needs = needs;
    }
    if spec.no_results {
        step.results.clear();
    } else if let Some(results) = spec.results.clone() {
        step.results = results;
    }
    if spec.no_when {
        step.when.clear();
        step.when_any.clear();
    } else {
        if let Some(when) = spec.when.clone() {
            step.when = when;
            step.when_any.clear();
        }
        if let Some(when_any) = spec.when_any.clone() {
            step.when_any = when_any;
            step.when.clear();
        }
    }
    if spec.no_actor {
        step.actor = None;
    } else if let Some(actor) = spec.actor.clone() {
        step.actor = Some(actor);
    }
    if !step.results.is_empty() && step.produces.is_none() {
        return Err(results_need_produce(&step.id));
    }
    let changed =
        step != state.plans[index].steps[pos] || spec.before.is_some() || spec.after.is_some();
    let reopens = spec
        .done_when
        .as_ref()
        .is_some_and(|done| committed_reopen.as_ref().is_some_and(|old| old != done));
    if !changed && !reopens {
        return Ok(json!({"step": spec.id}));
    }
    let before = spec.before.clone();
    let after = spec.after.clone();
    state.plans[index].steps.remove(pos);
    insert_step(
        &mut state.plans[index].steps,
        step,
        before.as_deref(),
        after.as_deref(),
    );
    persist_steps(state, index, &plan_uid, false, None);
    let mut body = json!({"step": spec.id});
    if reopens {
        body["reopens"] = json!(true);
    }
    Ok(body)
}

pub fn drop_step(state: &mut State, id: &str) -> Result<Value, FlowError> {
    require_draft_edit(state)?;
    let index = state.draft_index().expect("draft");
    let plan_uid = state.plans[index].uid.clone();
    let Some(step) = state.plans[index]
        .steps
        .iter()
        .find(|s| s.id == id)
        .cloned()
    else {
        return Err(unknown(id));
    };
    if state
        .committed()
        .and_then(|p| p.steps.iter().find(|s| s.id == id))
        .is_some_and(|s| s.state == StepState::Succeeded)
    {
        return Err(FlowError::refusal(
            "completed_work",
            format!("{id} succeeded and cannot be dropped"),
            format!("keep {id} in the plan"),
        ));
    }
    let refs: Vec<String> = state.plans[index]
        .steps
        .iter()
        .filter(|s| s.id != id && references(s, id, step.produces.as_deref()))
        .map(|s| s.id.clone())
        .collect();
    if !refs.is_empty() {
        return Err(FlowError::refusal(
            "step_referenced",
            format!("{id} is still referenced by {}", refs.join(", ")),
            format!(
                "remove {id} from the needs or conditions of {}",
                refs.join(", ")
            ),
        ));
    }
    let uid = step.uid.clone();
    state.plans[index].steps.retain(|s| s.id != id);
    renumber(&mut state.plans[index].steps);
    state.writes.push(Write::DeleteStep { uid });
    persist_steps(state, index, &plan_uid, false, None);
    let ids: Vec<_> = state.plans[index]
        .steps
        .iter()
        .map(|s| s.id.clone())
        .collect();
    Ok(json!({"steps": ids}))
}

pub fn remove(state: &mut State, force: bool) -> Result<Value, FlowError> {
    if state.plans.is_empty() {
        return Err(no_plan(&state.plan_name));
    }
    if !force {
        if let Some(open) = state.attempts.iter().find(|attempt| attempt.is_open()) {
            return Err(FlowError::refusal(
                "lease_open",
                format!("{} is leased to {}", open.step_id, open.agent),
                format!("report {}, or pass --force", open.step_id),
            ));
        }
    }
    let versions = state.plans.len();
    let attempts = state.attempts.len();
    let name = state.plan_name.clone();
    let uids: Vec<String> = state.plans.iter().map(|plan| plan.uid.clone()).collect();
    for uid in uids {
        state.writes.push(Write::DeletePlan { uid });
    }
    state.plans.clear();
    state.attempts.clear();
    Ok(json!({
        "removed": name,
        "versions": versions,
        "attempts": attempts,
    }))
}

pub fn prune(state: &mut State) -> Result<Value, FlowError> {
    if state.plans.is_empty() {
        return Err(no_plan(&state.plan_name));
    }
    if state
        .plans
        .iter()
        .any(|plan| plan.kind != PlanKind::Template)
    {
        return Err(FlowError::refusal(
            "not_a_template",
            format!("{} is an execution", state.plan_name),
            "name the template",
        ));
    }
    let template = state.plan_name.clone();
    let mut names: Vec<String> = state
        .store_plans
        .iter()
        .filter(|plan| {
            plan.kind == PlanKind::Execution && plan.template.as_deref() == Some(template.as_str())
        })
        .map(|plan| plan.name.clone())
        .collect();
    names.sort();
    names.dedup();
    let mut pruned = Vec::new();
    for name in names {
        let versions: Vec<&Plan> = state
            .store_plans
            .iter()
            .filter(|plan| plan.name == name)
            .collect();
        if versions.iter().any(|plan| plan.is_draft()) {
            continue;
        }
        let Some(head) = versions
            .iter()
            .copied()
            .filter(|plan| plan.version.is_some())
            .max_by_key(|plan| plan.version.unwrap_or(0))
        else {
            continue;
        };
        if !head.steps.iter().all(|step| step.state.is_terminal()) {
            continue;
        }
        let step_uids: std::collections::HashSet<&str> = versions
            .iter()
            .flat_map(|plan| plan.steps.iter().map(|step| step.uid.as_str()))
            .collect();
        if state
            .store_attempts
            .iter()
            .any(|attempt| attempt.is_open() && step_uids.contains(attempt.step_uid.as_str()))
        {
            continue;
        }
        for plan in &versions {
            state.writes.push(Write::DeletePlan {
                uid: plan.uid.clone(),
            });
        }
        pruned.push(name);
    }
    Ok(json!({"pruned": pruned}))
}

pub fn abort(state: &mut State) -> Result<Value, FlowError> {
    let Some(index) = state.draft_index() else {
        return Ok(json!({"aborted": false}));
    };
    let uid = state.plans[index].uid.clone();
    state.plans.remove(index);
    state.writes.push(Write::DeletePlan { uid });
    Ok(json!({"aborted": true}))
}

pub fn revise(state: &mut State, now: DateTime<Utc>) -> Result<Value, FlowError> {
    if state.draft().is_some() {
        return Err(draft_open());
    }
    let Some(committed) = state.committed().cloned() else {
        return Err(no_plan(&state.plan_name));
    };
    let version = committed.version.unwrap_or(1);
    let draft = Plan {
        uid: fresh("plan"),
        name: committed.name.clone(),
        kind: committed.kind,
        inputs: committed.inputs.clone(),
        template: committed.template.clone(),
        template_version: committed.template_version,
        version: None,
        based_on: Some(version),
        goal: committed.goal.clone(),
        retries: committed.retries,
        created_at: stamp(now),
        steps: committed
            .steps
            .iter()
            .enumerate()
            .map(|(i, step)| Step {
                uid: fresh("step"),
                state: StepState::Pending,
                position: i as i64,
                ..step.clone()
            })
            .collect(),
        supersedes: None,
    };
    let body = json!({"draft": true, "basedOn": version});
    state.writes.push(Write::CreatePlan(draft.clone()));
    let plan_uid = draft.uid.clone();
    state.plans.push(draft);
    let index = state.draft_index().expect("draft");
    persist_steps(state, index, &plan_uid, true, None);
    Ok(body)
}

pub fn commit(state: &mut State, dry_run: bool, now: DateTime<Utc>) -> Result<Value, FlowError> {
    if dry_run {
        let mut ghost = state.clone();
        ghost.writes.clear();
        let mut body = commit_in(&mut ghost, now)?;
        body["dryRun"] = json!(true);
        return Ok(body);
    }
    commit_in(state, now)
}

fn commit_in(state: &mut State, now: DateTime<Utc>) -> Result<Value, FlowError> {
    let Some(draft) = state.draft().cloned() else {
        return Err(if state.committed().is_some() {
            FlowError::refusal("no_draft", "no draft is open", "call revise")
        } else {
            no_plan(&state.plan_name)
        });
    };
    validate::validate(&draft)?;
    let prev = state.committed().cloned();
    if let Some(prev) = &prev {
        for old in &prev.steps {
            if !draft.steps.iter().any(|s| s.id == old.id)
                && revision::absent(old) == Absent::Refuse
            {
                return Err(FlowError::refusal(
                    "completed_work",
                    format!("{} succeeded and cannot be dropped", old.id),
                    format!("keep {} in the plan", old.id),
                ));
            }
        }
    }
    let mut kept = Vec::new();
    let mut reopened = Vec::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut new_steps = Vec::new();
    let mut moves: Vec<(String, String)> = Vec::new();
    let mut cancels: Vec<String> = Vec::new();
    if let Some(prev) = &prev {
        for old in &prev.steps {
            if !draft.steps.iter().any(|s| s.id == old.id) {
                removed.push(old.id.clone());
                if old.state == StepState::Running {
                    cancels.push(old.uid.clone());
                }
            }
        }
    }
    for (i, draft_step) in draft.steps.iter().enumerate() {
        let old = prev
            .as_ref()
            .and_then(|p| p.steps.iter().find(|s| s.id == draft_step.id));
        let carry = revision::carry(old, draft_step);
        let mut step = draft_step.clone();
        step.uid = fresh("step");
        step.position = i as i64;
        match carry {
            Carry::Keep => {
                kept.push(step.id.clone());
                step.state = StepState::Succeeded;
                if let Some(old) = old {
                    moves.push((old.uid.clone(), step.uid.clone()));
                }
            }
            Carry::Reopen { cancel_lease } => {
                reopened.push(step.id.clone());
                step.state = StepState::Pending;
                if cancel_lease {
                    if let Some(old) = old {
                        cancels.push(old.uid.clone());
                    }
                } else if old.is_some_and(|s| s.state == StepState::Running) {
                    cancels.push(old.unwrap().uid.clone());
                }
            }
            Carry::Add => {
                added.push(step.id.clone());
                step.state = StepState::Pending;
            }
        }
        // Any open attempt on a step we did not keep must close.
        if !matches!(carry, Carry::Keep) {
            if let Some(old) = old {
                if !cancels.contains(&old.uid) {
                    cancels.push(old.uid.clone());
                }
            }
        }
        new_steps.push(step);
    }
    let attempts = state.attempts.clone();
    admission::recompute(&mut new_steps, &attempts_for(&attempts, &moves));
    let frontier = admission::frontier(&new_steps, &attempts_for(&attempts, &moves));
    let version = prev.as_ref().and_then(|p| p.version).unwrap_or(0) + 1;
    let new_plan = Plan {
        uid: fresh("plan"),
        name: draft.name.clone(),
        kind: draft.kind,
        inputs: draft.inputs.clone(),
        template: draft.template.clone(),
        template_version: draft.template_version,
        version: Some(version),
        based_on: prev.as_ref().and_then(|p| p.version),
        goal: draft.goal.clone(),
        retries: draft.retries,
        created_at: draft.created_at.clone(),
        steps: new_steps,
        supersedes: prev.as_ref().map(|p| p.uid.clone()),
    };
    state.writes.push(Write::CreatePlan(new_plan.clone()));
    for step in &new_plan.steps {
        state.writes.push(Write::UpsertStep {
            plan_uid: new_plan.uid.clone(),
            step: step.clone(),
            is_new: true,
            sync_needs: true,
        });
    }
    if let Some(old_uid) = &new_plan.supersedes {
        state.writes.push(Write::Supersedes {
            new_uid: new_plan.uid.clone(),
            old_uid: old_uid.clone(),
        });
    }
    for (old_uid, new_uid) in &moves {
        let moved: Vec<usize> = state
            .attempts
            .iter()
            .enumerate()
            .filter(|(_, a)| a.step_uid == *old_uid)
            .map(|(i, _)| i)
            .collect();
        for i in moved {
            state.attempts[i].step_uid = new_uid.clone();
            state.writes.push(Write::MoveAttempt {
                attempt_uid: state.attempts[i].uid.clone(),
                new_step_uid: new_uid.clone(),
            });
        }
    }
    close_cancelled(state, &cancels, now);
    state.writes.push(Write::DeletePlan { uid: draft.uid });
    if let Some(index) = state.draft_index() {
        state.plans.remove(index);
    }
    state.plans.push(new_plan);
    Ok(json!({
        "version": version,
        "kept": kept,
        "reopened": reopened,
        "added": added,
        "removed": removed,
        "frontier": frontier,
    }))
}

pub fn show(state: &State) -> Result<Value, FlowError> {
    let plan = view(state)?;
    Ok(show_body(plan))
}

pub fn status(state: &State) -> Result<Value, FlowError> {
    let plan = view(state)?;
    let mut steps = Vec::new();
    for step in &plan.steps {
        let disposition = explain::disposition(&plan.steps, &state.attempts, step);
        let mut obj = json!({
            "id": step.id,
            "disposition": disposition,
        });
        if let Some(actor) = &step.actor {
            obj["actor"] = json!(actor);
        }
        if step.state == StepState::Succeeded {
            if let Some(result) =
                admission::succeeded_attempt(&state.attempts, step).and_then(|a| a.result.clone())
            {
                obj["result"] = json!(result);
            }
        }
        steps.push(obj);
    }
    let mut body = json!({"steps": steps});
    if plan.is_draft() {
        body["draft"] = json!(true);
    } else if let Some(version) = plan.version {
        body["version"] = json!(version);
    }
    Ok(body)
}

pub fn explain_step(state: &State, id: &str) -> Result<Value, FlowError> {
    let plan = view(state)?;
    let Some(step) = plan.steps.iter().find(|s| s.id == id) else {
        return Err(unknown(id));
    };
    let mut body = explain::explain(&plan.steps, &state.attempts, step, plan.retries);
    if plan.is_draft() {
        body["draft"] = json!(true);
    }
    Ok(body)
}

pub fn log(state: &State, step: Option<&str>) -> Result<Value, FlowError> {
    if state.plans.is_empty() {
        return Err(no_plan(&state.plan_name));
    }
    if let Some(id) = step {
        let known = state
            .plans
            .iter()
            .any(|p| p.steps.iter().any(|s| s.id == id));
        if !known {
            return Err(unknown(id));
        }
    }
    let mut attempts: Vec<&Attempt> = state
        .attempts
        .iter()
        .filter(|a| step.is_none_or(|id| a.step_id == id))
        .collect();
    attempts.sort_by(|a, b| b.started_at.cmp(&a.started_at).then(b.id.cmp(&a.id)));
    attempts.truncate(50);
    Ok(json!({
        "attempts": attempts.iter().map(|a| a.to_log()).collect::<Vec<_>>()
    }))
}

pub fn execute(
    state: &mut State,
    name: &str,
    inputs: Vec<(String, String)>,
    now: DateTime<Utc>,
) -> Result<Value, FlowError> {
    if name == "skill" || !is_slug(name) {
        return Err(bad_slug(name));
    }
    if state.draft().is_some() {
        return Err(draft_open());
    }
    let Some(template) = state.committed().cloned() else {
        return Err(no_plan(&state.plan_name));
    };
    if template.kind != PlanKind::Template {
        return Err(FlowError::refusal(
            "not_a_template",
            format!("{} is an execution", template.name),
            "name the template",
        ));
    }
    if state.names.iter().any(|existing| existing == name) {
        return Err(FlowError::refusal(
            "name_taken",
            format!("a plan named {name} already exists"),
            "choose a name that is not already in the store",
        ));
    }
    let mut seen = Vec::new();
    for (key, value) in &inputs {
        if key.is_empty() {
            return Err(empty("input key"));
        }
        if value.is_empty() {
            return Err(empty("input value"));
        }
        if seen.contains(key) {
            return Err(FlowError::refusal(
                "duplicate_value",
                format!("input '{key}' is repeated"),
                "pass each input once",
            ));
        }
        seen.push(key.clone());
        if template
            .steps
            .iter()
            .any(|step| step.produces.as_deref() == Some(key.as_str()))
        {
            return Err(FlowError::refusal(
                "input_conflicts",
                format!("input '{key}' is already a produce"),
                "pick an input key that is not a produce",
            ));
        }
    }
    let steps: Vec<Step> = template
        .steps
        .iter()
        .enumerate()
        .map(|(i, step)| Step {
            uid: fresh("step"),
            state: StepState::Pending,
            position: i as i64,
            ..step.clone()
        })
        .collect();
    let plan = Plan {
        uid: fresh("plan"),
        name: name.to_string(),
        kind: PlanKind::Execution,
        inputs: inputs.clone(),
        template: Some(template.name.clone()),
        template_version: template.version,
        version: Some(1),
        based_on: None,
        goal: template.goal.clone(),
        retries: template.retries,
        created_at: stamp(now),
        steps,
        supersedes: None,
    };
    state.writes.push(Write::CreatePlan(plan.clone()));
    for step in &plan.steps {
        state.writes.push(Write::UpsertStep {
            plan_uid: plan.uid.clone(),
            step: step.clone(),
            is_new: true,
            sync_needs: true,
        });
    }
    let body = json!({
        "execution": plan.name,
        "template": template.name,
        "version": 1,
        "inputs": input_object(&plan.inputs),
    });
    state.names.push(name.to_string());
    state.plans.push(plan);
    Ok(body)
}

pub fn next_instruction(
    state: &mut State,
    agent: &str,
    lease: Duration,
    now: DateTime<Utc>,
) -> Result<Value, FlowError> {
    if agent.is_empty() || !is_slug(agent) {
        return Err(bad_slug(agent));
    }
    check_lease(lease)?;
    if state.draft().is_some() {
        return Err(draft_open());
    }
    let Some(index) = state.committed_index() else {
        return Err(no_plan(&state.plan_name));
    };
    require_run(&state.plans[index])?;
    expire(state, index, now);
    recompute_committed(state, index);
    let plan_uid = state.plans[index].uid.clone();
    let retries = state.plans[index].retries;

    if let Some(open) = open_lease(state, index, agent) {
        let until = stamp(add_duration(now, lease));
        let attempt_uid = open.uid.clone();
        let step_uid = open.step_uid.clone();
        if let Some(attempt) = state.attempts.iter_mut().find(|a| a.uid == attempt_uid) {
            attempt.lease_until = Some(until);
        }
        let attempt = state
            .attempts
            .iter()
            .find(|a| a.uid == attempt_uid)
            .unwrap()
            .clone();
        state.writes.push(Write::UpdateAttempt(attempt.clone()));
        let step = state.plans[index]
            .steps
            .iter()
            .find(|s| s.uid == step_uid)
            .unwrap()
            .clone();
        let bound = state.plans[index].inputs.clone();
        return Ok(run_body(
            &state.plans[index].steps,
            &state.attempts,
            &step,
            &attempt,
            true,
            retries,
            &bound,
        ));
    }

    let claim_id = state.plans[index]
        .steps
        .iter()
        .find(|s| admission::claimable(&state.plans[index].steps, &state.attempts, s, agent))
        .map(|s| s.id.clone());
    if let Some(id) = claim_id {
        return Ok(claim(state, index, &plan_uid, &id, agent, lease, now));
    }

    if let Some(wait) = waiting_step(state, index, agent) {
        let because = branch_because(&state.plans[index].steps, &state.attempts, &wait.id);
        let actor = if wait.state == StepState::Running {
            open_on_step(state, &wait.uid)
                .map(|a| a.agent.clone())
                .or(wait.actor.clone())
                .unwrap_or_else(|| "other".into())
        } else {
            wait.actor.clone().unwrap_or_else(|| "other".into())
        };
        return Ok(json!({
            "action": "waiting",
            "for": actor,
            "step": wait.id,
            "because": because,
        }));
    }

    if let Some(failed) = state.plans[index]
        .steps
        .iter()
        .find(|s| s.state == StepState::Failed)
        .cloned()
    {
        let n = admission::failed_count(&state.attempts, &failed.uid);
        return Ok(json!({
            "action": "replan",
            "step": failed.id,
            "because": format!("failed {n} times"),
        }));
    }

    if let Some(blocked) = state.plans[index]
        .steps
        .iter()
        .find(|s| s.state == StepState::Blocked)
        .cloned()
    {
        let because = state
            .attempts
            .iter()
            .rev()
            .find(|a| a.step_uid == blocked.uid && a.outcome == Some(Outcome::Blocked))
            .and_then(|a| a.because.clone())
            .unwrap_or_default();
        return Ok(json!({
            "action": "blocked",
            "step": blocked.id,
            "because": because,
        }));
    }

    let done = state.plans[index]
        .steps
        .iter()
        .all(|s| s.state.is_terminal());
    if !done {
        return Ok(json!({
            "action": "replan",
            "because": "no step can become ready",
        }));
    }
    let version = state.plans[index].version.unwrap_or(1);
    Ok(json!({"action": "done", "version": version}))
}

pub fn report(state: &mut State, report: Report, now: DateTime<Utc>) -> Result<Value, FlowError> {
    if report.agent.is_empty() || !is_slug(&report.agent) {
        return Err(bad_slug(&report.agent));
    }
    validate_report_shape(&report)?;
    if state.draft().is_some() {
        return Err(draft_open());
    }
    let Some(index) = state.committed_index() else {
        return Err(no_plan(&state.plan_name));
    };
    require_run(&state.plans[index])?;
    let Some(step) = state.plans[index]
        .steps
        .iter()
        .find(|s| s.id == report.step)
        .cloned()
    else {
        return Err(unknown(&report.step));
    };
    check_outcome_against_step(&step, &report)?;
    expire(state, index, now);
    let open = open_lease(state, index, &report.agent).cloned();
    match &open {
        Some(lease) if lease.step_id == report.step => {
            apply_outcome(state, index, lease, &step, &report, now)
        }
        Some(lease) => Err(FlowError::refusal(
            "not_leased",
            format!("{} is not the step {} holds", report.step, report.agent),
            format!(
                "report {}, which is leased until {}",
                lease.step_id,
                lease.lease_until.as_deref().unwrap_or("")
            ),
        )),
        None => {
            if let Some(previous) = state.attempts.iter().rev().find(|a| {
                a.step_id == report.step && a.agent == report.agent && a.outcome.is_some()
            }) {
                if same_report(previous, &report) {
                    return Ok(report_body(previous));
                }
                return Err(FlowError::conflict(
                    format!(
                        "{} attempt {} was already closed with {}",
                        report.step,
                        previous.id,
                        previous.outcome.map(Outcome::as_str).unwrap_or("open")
                    ),
                    "call next",
                    false,
                ));
            }
            Err(FlowError::refusal(
                "not_leased",
                format!("{} is not leased to {}", report.step, report.agent),
                "call next",
            ))
        }
    }
}

fn apply_outcome(
    state: &mut State,
    index: usize,
    lease: &Attempt,
    step: &Step,
    report: &Report,
    now: DateTime<Utc>,
) -> Result<Value, FlowError> {
    let retries = state.plans[index].retries;
    let mut attempt = lease.clone();
    attempt.outcome = Some(report.outcome);
    attempt.result = report.result.clone();
    attempt.note = report.note.clone();
    attempt.because = report.because.clone();
    attempt.artifact = report.artifact.clone();
    attempt.artifact_hash = report.artifact_hash.clone();
    attempt.finished_at = Some(stamp(now));
    let failed_after = admission::failed_count(&state.attempts, &step.uid)
        + usize::from(report.outcome == Outcome::Failed);
    let new_state = match report.outcome {
        Outcome::Succeeded => StepState::Succeeded,
        Outcome::Blocked => StepState::Blocked,
        Outcome::Released | Outcome::Expired => StepState::Pending,
        Outcome::Failed if failed_after >= retries as usize => StepState::Failed,
        Outcome::Failed => StepState::Pending,
    };
    if let Some(slot) = state.attempts.iter_mut().find(|a| a.uid == attempt.uid) {
        *slot = attempt.clone();
    }
    state.writes.push(Write::UpdateAttempt(attempt.clone()));
    let before: Vec<StepState> = state.plans[index].steps.iter().map(|s| s.state).collect();
    if let Some(slot) = state.plans[index]
        .steps
        .iter_mut()
        .find(|s| s.uid == step.uid)
    {
        slot.state = new_state;
    }
    let attempts = state.attempts.clone();
    admission::recompute(&mut state.plans[index].steps, &attempts);
    let plan_uid = state.plans[index].uid.clone();
    for (i, step) in state.plans[index].steps.iter().enumerate() {
        if step.state != before[i] {
            state.writes.push(Write::UpsertStep {
                plan_uid: plan_uid.clone(),
                step: step.clone(),
                is_new: false,
                sync_needs: false,
            });
        }
    }
    Ok(report_body(&attempt))
}

fn claim(
    state: &mut State,
    index: usize,
    plan_uid: &str,
    step_id: &str,
    agent: &str,
    lease: Duration,
    now: DateTime<Utc>,
) -> Value {
    let step = state.plans[index]
        .steps
        .iter()
        .find(|s| s.id == step_id)
        .unwrap()
        .clone();
    let attempt = Attempt {
        uid: fresh("attempt"),
        id: next_attempt_id(&state.attempts, state.attempt_seq),
        step_uid: step.uid.clone(),
        step_id: step.id.clone(),
        agent: agent.to_string(),
        outcome: None,
        result: None,
        note: None,
        because: None,
        artifact: None,
        artifact_hash: None,
        lease_until: Some(stamp(add_duration(now, lease))),
        started_at: stamp(now),
        finished_at: None,
    };
    if let Some(slot) = state.plans[index]
        .steps
        .iter_mut()
        .find(|s| s.id == step_id)
    {
        slot.state = StepState::Running;
    }
    let running = state.plans[index]
        .steps
        .iter()
        .find(|s| s.id == step_id)
        .unwrap()
        .clone();
    state.writes.push(Write::UpsertStep {
        plan_uid: plan_uid.to_string(),
        step: running.clone(),
        is_new: false,
        sync_needs: false,
    });
    state.writes.push(Write::CreateAttempt(attempt.clone()));
    state.attempts.push(attempt.clone());
    let retries = state.plans[index].retries;
    let bound = state.plans[index].inputs.clone();
    run_body(
        &state.plans[index].steps,
        &state.attempts,
        &running,
        &attempt,
        false,
        retries,
        &bound,
    )
}

fn expire(state: &mut State, index: usize, now: DateTime<Utc>) {
    let step_uids: Vec<String> = state.plans[index]
        .steps
        .iter()
        .map(|s| s.uid.clone())
        .collect();
    let stale: Vec<String> = state
        .attempts
        .iter()
        .filter(|a| {
            a.is_open()
                && step_uids.iter().any(|u| u == &a.step_uid)
                && a.lease_until
                    .as_deref()
                    .and_then(parse_stamp)
                    .is_some_and(|t| t <= now)
        })
        .map(|a| a.uid.clone())
        .collect();
    for uid in stale {
        let step_uid = state
            .attempts
            .iter()
            .find(|a| a.uid == uid)
            .unwrap()
            .step_uid
            .clone();
        if let Some(attempt) = state.attempts.iter_mut().find(|a| a.uid == uid) {
            attempt.outcome = Some(Outcome::Expired);
            attempt.finished_at = Some(stamp(now));
        }
        let attempt = state
            .attempts
            .iter()
            .find(|a| a.uid == uid)
            .unwrap()
            .clone();
        state.writes.push(Write::UpdateAttempt(attempt));
        if let Some(step) = state.plans[index]
            .steps
            .iter_mut()
            .find(|s| s.uid == step_uid)
        {
            if step.state == StepState::Running {
                step.state = StepState::Pending;
            }
        }
        if let Some(step) = state.plans[index]
            .steps
            .iter()
            .find(|s| s.uid == step_uid)
            .cloned()
        {
            state.writes.push(Write::UpsertStep {
                plan_uid: state.plans[index].uid.clone(),
                step,
                is_new: false,
                sync_needs: false,
            });
        }
    }
}

fn recompute_committed(state: &mut State, index: usize) {
    let before: Vec<StepState> = state.plans[index].steps.iter().map(|s| s.state).collect();
    let attempts = state.attempts.clone();
    admission::recompute(&mut state.plans[index].steps, &attempts);
    let plan_uid = state.plans[index].uid.clone();
    for (i, step) in state.plans[index].steps.iter().enumerate() {
        if step.state != before[i] {
            state.writes.push(Write::UpsertStep {
                plan_uid: plan_uid.clone(),
                step: step.clone(),
                is_new: false,
                sync_needs: false,
            });
        }
    }
}

fn close_cancelled(state: &mut State, step_uids: &[String], now: DateTime<Utc>) {
    let now = stamp(now);
    let open: Vec<usize> = state
        .attempts
        .iter()
        .enumerate()
        .filter(|(_, a)| a.is_open() && step_uids.iter().any(|u| u == &a.step_uid))
        .map(|(i, _)| i)
        .collect();
    for i in open {
        state.attempts[i].outcome = Some(Outcome::Released);
        state.attempts[i].finished_at = Some(now.clone());
        state.attempts[i].because = Some("cancelled by revision".into());
        state
            .writes
            .push(Write::UpdateAttempt(state.attempts[i].clone()));
    }
}

fn attempts_for(attempts: &[Attempt], moves: &[(String, String)]) -> Vec<Attempt> {
    attempts
        .iter()
        .cloned()
        .map(|mut attempt| {
            if let Some((_, new_uid)) = moves.iter().find(|(old, _)| old == &attempt.step_uid) {
                attempt.step_uid = new_uid.clone();
            }
            attempt
        })
        .collect()
}

fn open_lease<'a>(state: &'a State, index: usize, agent: &str) -> Option<&'a Attempt> {
    let uids: Vec<&str> = state.plans[index]
        .steps
        .iter()
        .map(|s| s.uid.as_str())
        .collect();
    state
        .attempts
        .iter()
        .find(|a| a.is_open() && a.agent == agent && uids.contains(&a.step_uid.as_str()))
}

fn open_on_step<'a>(state: &'a State, step_uid: &str) -> Option<&'a Attempt> {
    state
        .attempts
        .iter()
        .rev()
        .find(|a| a.is_open() && a.step_uid == step_uid)
}

fn waiting_step(state: &State, index: usize, agent: &str) -> Option<Step> {
    state.plans[index]
        .steps
        .iter()
        .find(|s| {
            let ready = admission::is_ready(&state.plans[index].steps, &state.attempts, s);
            if s.state == StepState::Running {
                return open_on_step(state, &s.uid).is_some_and(|a| a.agent != agent);
            }
            ready && s.actor.as_ref().is_some_and(|actor| actor != agent)
        })
        .cloned()
}

fn run_body(
    steps: &[Step],
    attempts: &[Attempt],
    step: &Step,
    attempt: &Attempt,
    renewed: bool,
    _retries: u32,
    bound: &[(String, String)],
) -> Value {
    let mut provided = input_object(bound);
    for (key, value) in inputs(steps, attempts, step) {
        provided.insert(key, value);
    }
    let mut body = json!({
        "id": step.id,
        "attempt": attempt.id,
        "title": step.title,
        "doneWhen": step.done_when,
        "needs": step.needs,
        "inputs": provided,
        "leaseUntil": attempt.lease_until,
    });
    if let Some(produces) = &step.produces {
        body["produces"] = json!(produces);
    }
    if !step.results.is_empty() {
        body["results"] = json!(step.results);
    }
    if !step.when.is_empty() {
        body["when"] = json!(step.when.iter().map(Pair::encode).collect::<Vec<_>>());
    }
    if !step.when_any.is_empty() {
        body["whenAny"] = json!(step
            .when_any
            .iter()
            .map(|alt| alt.iter().map(Pair::encode).collect::<Vec<_>>().join(","))
            .collect::<Vec<_>>());
    }
    let failed = admission::failed_count(attempts, &step.uid);
    if failed > 0 {
        body["retry"] = json!(failed);
    }
    let mut out = json!({"action": "run", "step": body});
    if renewed {
        out["renewed"] = json!(true);
    }
    out
}

fn inputs(steps: &[Step], attempts: &[Attempt], step: &Step) -> Map<String, Value> {
    let mut names = Vec::new();
    let mut queue = vec![step.id.clone()];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = queue.first().cloned() {
        queue.remove(0);
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(current) = steps.iter().find(|s| s.id == id) else {
            continue;
        };
        for need in &current.needs {
            if let Some(dep) = steps.iter().find(|s| s.id == *need) {
                if current.id == step.id {
                    if let Some(produce) = &dep.produces {
                        if !names.contains(produce) {
                            names.push(produce.clone());
                        }
                    }
                }
                queue.push(need.clone());
            }
        }
        let pairs: Vec<Pair> = if !current.when.is_empty() {
            current.when.clone()
        } else {
            current
                .when_any
                .iter()
                .filter(|alt| matches!(admission::eval_and(steps, attempts, alt), Cond::Match))
                .flatten()
                .cloned()
                .collect()
        };
        // Direct conditions are always named. Ancestor conditions are named
        // only once they have matched, so the instruction carries the note
        // that opened the branch.
        let include =
            current.id == step.id || !current.when.is_empty() || !current.when_any.is_empty();
        if include {
            for pair in &pairs {
                if !names.contains(&pair.produce) {
                    names.push(pair.produce.clone());
                }
            }
        }
    }
    let mut map = Map::new();
    for name in names {
        let Some(prod) = admission::producer(steps, &name) else {
            continue;
        };
        if prod.state != StepState::Succeeded {
            continue;
        }
        if let Some(note) =
            admission::succeeded_attempt(attempts, prod).and_then(|a| a.note.clone())
        {
            map.insert(name, json!(note));
        }
    }
    map
}

fn branch_because(steps: &[Step], attempts: &[Attempt], start: &str) -> String {
    let mut queue = vec![start.to_string()];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = queue.first().cloned() {
        queue.remove(0);
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(step) = steps.iter().find(|s| s.id == id) else {
            continue;
        };
        if let Some(note) = matched_note(steps, attempts, step) {
            return note;
        }
        for need in &step.needs {
            queue.push(need.clone());
        }
    }
    String::new()
}

fn matched_note(steps: &[Step], attempts: &[Attempt], step: &Step) -> Option<String> {
    let mut alts: Vec<Vec<Pair>> = Vec::new();
    if !step.when.is_empty() {
        alts.push(step.when.clone());
    } else {
        alts.extend(step.when_any.clone());
    }
    for alt in alts {
        if matches!(admission::eval_and(steps, attempts, &alt), Cond::Match) {
            for pair in &alt {
                let prod = admission::producer(steps, &pair.produce)?;
                if let Some(note) =
                    admission::succeeded_attempt(attempts, prod).and_then(|a| a.note.clone())
                {
                    return Some(note);
                }
            }
        }
    }
    None
}

fn report_body(attempt: &Attempt) -> Value {
    let mut body = json!({
        "step": attempt.step_id,
        "attempt": attempt.id,
        "outcome": attempt.outcome.map(Outcome::as_str).unwrap_or("open"),
    });
    if let Some(result) = &attempt.result {
        body["result"] = json!(result);
    }
    body
}

fn same_report(attempt: &Attempt, report: &Report) -> bool {
    attempt.outcome == Some(report.outcome)
        && attempt.result == report.result
        && attempt.note == report.note
        && attempt.because == report.because
        && attempt.artifact == report.artifact
        && attempt.artifact_hash == report.artifact_hash
}

fn validate_report_shape(report: &Report) -> Result<(), FlowError> {
    match report.outcome {
        Outcome::Succeeded => {
            if report.note.as_deref().unwrap_or("").trim().is_empty() {
                return Err(FlowError::refusal(
                    "missing_note",
                    "succeeded requires --note",
                    "pass --note TEXT",
                ));
            }
            if report.because.is_some() {
                return Err(unexpected("--because"));
            }
        }
        Outcome::Failed | Outcome::Blocked => {
            if report.because.as_deref().unwrap_or("").trim().is_empty() {
                return Err(FlowError::refusal(
                    "missing_because",
                    format!("{} requires --because", report.outcome.as_str()),
                    "pass --because TEXT",
                ));
            }
            if report.note.is_some() || report.result.is_some() {
                return Err(unexpected("a field this outcome does not take"));
            }
        }
        Outcome::Released => {
            if report.note.is_some() || report.because.is_some() || report.result.is_some() {
                return Err(unexpected("a field released does not take"));
            }
        }
        Outcome::Expired => {
            return Err(FlowError::usage(
                "expired is not a reportable outcome",
                "pass succeeded, failed, blocked, or released",
            ));
        }
    }
    Ok(())
}

fn check_outcome_against_step(step: &Step, report: &Report) -> Result<(), FlowError> {
    if report.outcome != Outcome::Succeeded {
        return Ok(());
    }
    if step.results.is_empty() {
        if report.result.is_some() {
            return Err(unexpected("--result"));
        }
        return Ok(());
    }
    let Some(result) = &report.result else {
        return Err(FlowError::refusal(
            "missing_result",
            format!("{} declares results and the report has none", step.id),
            format!("pass --result {}", step.results.join("|")),
        ));
    };
    if !step.results.iter().any(|r| r == result) {
        return Err(FlowError::refusal(
            "bad_result",
            format!("{result} is not a result of {}", step.id),
            format!("pass --result {}", step.results.join("|")),
        ));
    }
    Ok(())
}

pub fn hash_artifact(path: &str) -> Result<(String, String), FlowError> {
    let bytes = std::fs::read(path).map_err(|_| {
        FlowError::refusal(
            "artifact_unreadable",
            format!("cannot read artifact {path}"),
            "pass a readable --artifact path",
        )
    })?;
    let hash = Sha256::digest(&bytes);
    let hex = hash.iter().map(|b| format!("{b:02x}")).collect::<String>();
    Ok((path.to_string(), hex))
}

fn view(state: &State) -> Result<&Plan, FlowError> {
    if let Some(draft) = state.draft() {
        return Ok(draft);
    }
    state.committed().ok_or_else(|| no_plan(&state.plan_name))
}

fn show_body(plan: &Plan) -> Value {
    let mut body = json!({
        "plan": plan.name,
        "kind": plan.kind.as_str(),
        "draft": plan.is_draft(),
        "basedOn": plan.based_on,
        "goal": plan.goal,
        "retries": plan.retries,
        "steps": plan.steps.iter().map(Step::to_show).collect::<Vec<_>>(),
    });
    if plan.kind == PlanKind::Execution {
        body["template"] = json!(plan.template);
        body["inputs"] = json!(input_object(&plan.inputs));
    }
    body
}

fn input_object(inputs: &[(String, String)]) -> Map<String, Value> {
    inputs
        .iter()
        .map(|(key, value)| (key.clone(), json!(value)))
        .collect()
}

fn require_run(plan: &Plan) -> Result<(), FlowError> {
    if plan.kind == PlanKind::Template {
        Err(FlowError::refusal(
            "not_a_run",
            format!("{} is a template", plan.name),
            format!("relate-flow {} execute NAME", plan.name),
        ))
    } else {
        Ok(())
    }
}

fn require_draft_edit(state: &State) -> Result<(), FlowError> {
    if state.draft().is_some() {
        return Ok(());
    }
    if state.committed().is_some() {
        return Err(FlowError::refusal(
            "no_draft",
            "no draft is open",
            "call revise",
        ));
    }
    Err(no_plan(&state.plan_name))
}

fn persist_steps(
    state: &mut State,
    index: usize,
    plan_uid: &str,
    all_new: bool,
    new_uid: Option<&str>,
) {
    let steps = state.plans[index].steps.clone();
    for step in steps {
        let is_new = all_new || new_uid == Some(step.uid.as_str());
        state.writes.push(Write::UpsertStep {
            plan_uid: plan_uid.to_string(),
            step,
            is_new,
            sync_needs: true,
        });
    }
}

fn insert_step(steps: &mut Vec<Step>, step: Step, before: Option<&str>, after: Option<&str>) {
    let index = if let Some(id) = before {
        steps.iter().position(|s| s.id == id).unwrap_or(steps.len())
    } else if let Some(id) = after {
        steps
            .iter()
            .position(|s| s.id == id)
            .map(|i| i + 1)
            .unwrap_or(steps.len())
    } else {
        steps.len()
    };
    steps.insert(index, step);
    renumber(steps);
}

fn renumber(steps: &mut [Step]) {
    for (i, step) in steps.iter_mut().enumerate() {
        step.position = i as i64;
    }
}

fn add_body(plan: &Plan, id: &str) -> Value {
    json!({
        "draft": true,
        "step": id,
        "steps": plan.steps.iter().map(|s| s.id.clone()).collect::<Vec<_>>(),
    })
}

fn same_step(step: &Step, spec: &AddStep) -> bool {
    step.title == spec.title
        && step.done_when == spec.done_when
        && step.produces == spec.produces
        && step.results == spec.results
        && step.needs == spec.needs
        && step.when == spec.when
        && step.when_any == spec.when_any
        && step.actor == spec.actor
}

fn place_matches(steps: &[Step], id: &str, before: Option<&str>, after: Option<&str>) -> bool {
    let Some(index) = steps.iter().position(|s| s.id == id) else {
        return false;
    };
    if before.is_none() && after.is_none() {
        return true;
    }
    if let Some(before) = before {
        return steps.get(index + 1).is_some_and(|s| s.id == before);
    }
    if let Some(after) = after {
        return index > 0 && steps[index - 1].id == after;
    }
    true
}

fn references(step: &Step, id: &str, produce: Option<&str>) -> bool {
    step.needs.iter().any(|n| n == id)
        || produce.is_some_and(|produce| {
            step.when.iter().any(|p| p.produce == produce)
                || step.when_any.iter().flatten().any(|p| p.produce == produce)
        })
}

fn ensure_known_place(
    steps: &[Step],
    before: Option<&str>,
    after: Option<&str>,
) -> Result<(), FlowError> {
    for id in [before, after].into_iter().flatten() {
        if !steps.iter().any(|s| s.id == id) {
            return Err(unknown(id));
        }
    }
    Ok(())
}

fn check_place(before: Option<&str>, after: Option<&str>) -> Result<(), FlowError> {
    if before.is_some() && after.is_some() {
        return Err(FlowError::usage(
            "--before and --after cannot be combined",
            "pass only one of --before or --after",
        ));
    }
    Ok(())
}

fn check_condition_flags(when: bool, when_any: bool) -> Result<(), FlowError> {
    if when && when_any {
        return Err(FlowError::usage(
            "--when and --when-any cannot be combined",
            "pass only one of --when or --when-any",
        ));
    }
    Ok(())
}

fn validate_identity(
    id: &str,
    produces: Option<&str>,
    actor: Option<&str>,
    results: &[String],
    needs: &[String],
) -> Result<(), FlowError> {
    if !is_slug(id) {
        return Err(bad_slug(id));
    }
    if let Some(produce) = produces {
        if !is_slug(produce) {
            return Err(bad_slug(produce));
        }
    }
    if let Some(actor) = actor {
        if !is_slug(actor) {
            return Err(bad_slug(actor));
        }
    }
    for token in results.iter().chain(needs.iter()) {
        if !is_slug(token) {
            return Err(bad_slug(token));
        }
    }
    Ok(())
}

fn reject_dups(values: &[String], what: &str) -> Result<(), FlowError> {
    let mut seen = Vec::new();
    for value in values {
        if seen.contains(&value.as_str()) {
            return Err(FlowError::refusal(
                "duplicate_value",
                format!("duplicate {what} '{value}'"),
                format!("pass each {what} once"),
            ));
        }
        seen.push(value.as_str());
    }
    Ok(())
}

fn check_lease(lease: Duration) -> Result<(), FlowError> {
    if lease > MAX_LEASE {
        return Err(FlowError::refusal(
            "lease_too_long",
            "a lease cannot be longer than 24 hours",
            "pass --lease of 24h or less",
        ));
    }
    Ok(())
}

fn next_attempt_id(attempts: &[Attempt], floor: u64) -> String {
    let max = attempts
        .iter()
        .filter_map(|a| a.id.strip_prefix('a')?.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        .max(floor);
    format!("a{}", max + 1)
}

fn fresh(prefix: &str) -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}-{t}-{n}")
}

fn empty(field: &str) -> FlowError {
    FlowError::refusal(
        "empty_field",
        format!("{field} must not be empty"),
        format!("pass a non-empty {field}"),
    )
}

fn bad_slug(text: &str) -> FlowError {
    FlowError::refusal(
        "bad_slug",
        format!("'{text}' is not a slug"),
        "use a lowercase letter, then lowercase letters, digits, and hyphens",
    )
}

fn unknown(id: &str) -> FlowError {
    FlowError::refusal(
        "unknown_step",
        format!("there is no step {id}"),
        "call show and use a step id from the plan",
    )
}

fn results_need_produce(id: &str) -> FlowError {
    FlowError::refusal(
        "results_need_produce",
        format!("{id} has results and no produce name"),
        format!("pass --produces NAME for {id}, or drop --results"),
    )
}

fn unexpected(what: &str) -> FlowError {
    FlowError::refusal(
        "unexpected_field",
        format!("{what} is not allowed for this outcome"),
        "drop the extra flag",
    )
}

fn draft_open() -> FlowError {
    FlowError::refusal(
        "draft_open",
        "a draft is open",
        "commit the draft, or abort it",
    )
}

fn no_plan(name: &str) -> FlowError {
    FlowError::refusal(
        "no_plan",
        format!("there is no plan named {name}"),
        "call goal --text TEXT",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_attempt_ids_increase() {
        let attempt = Attempt {
            uid: "u".into(),
            id: "a3".into(),
            step_uid: "s".into(),
            step_id: "parse".into(),
            agent: "invoices".into(),
            outcome: None,
            result: None,
            note: None,
            because: None,
            artifact: None,
            artifact_hash: None,
            lease_until: None,
            started_at: "t".into(),
            finished_at: None,
        };
        assert_eq!(next_attempt_id(&[attempt], 0), "a4");
        assert_eq!(next_attempt_id(&[], 0), "a1");
        assert_eq!(next_attempt_id(&[], 9), "a10");
    }

    #[test]
    fn lease_cap() {
        assert!(check_lease(DEFAULT_LEASE).is_ok());
        assert!(check_lease(MAX_LEASE).is_ok());
        assert_eq!(
            check_lease(MAX_LEASE + Duration::from_secs(1))
                .unwrap_err()
                .code,
            "lease_too_long"
        );
    }
}
