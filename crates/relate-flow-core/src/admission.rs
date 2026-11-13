//! Pure disposition rules. Nothing here touches the database.

use crate::model::{Attempt, Outcome, Pair, Step, StepState};

/// Owned variant used when the caller needs to keep the actual token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    Unresolved,
    Matched,
    Unmatched {
        actual: Option<String>,
        producer_skipped: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cond {
    Absent,
    Match,
    NoMatch { produce: String, token: String },
    Unresolved { produce: String },
}

pub fn producer<'a>(steps: &'a [Step], produce: &str) -> Option<&'a Step> {
    steps
        .iter()
        .find(|s| s.produces.as_deref() == Some(produce))
}

pub fn succeeded_attempt<'a>(attempts: &'a [Attempt], step: &Step) -> Option<&'a Attempt> {
    attempts
        .iter()
        .rev()
        .find(|a| a.step_uid == step.uid && a.outcome == Some(Outcome::Succeeded))
}

pub fn resolve(steps: &[Step], attempts: &[Attempt], produce: &str, token: &str) -> Resolved {
    let Some(step) = producer(steps, produce) else {
        return Resolved::Unresolved;
    };
    match step.state {
        StepState::Succeeded => {
            let actual = succeeded_attempt(attempts, step).and_then(|a| a.result.clone());
            if actual.as_deref() == Some(token) {
                Resolved::Matched
            } else {
                Resolved::Unmatched {
                    actual,
                    producer_skipped: false,
                }
            }
        }
        StepState::Skipped => Resolved::Unmatched {
            actual: None,
            producer_skipped: true,
        },
        _ => Resolved::Unresolved,
    }
}

pub fn eval_and(steps: &[Step], attempts: &[Attempt], pairs: &[Pair]) -> Cond {
    if pairs.is_empty() {
        return Cond::Absent;
    }
    let mut miss: Option<(String, String)> = None;
    let mut unresolved: Option<String> = None;
    for pair in pairs {
        match resolve(steps, attempts, &pair.produce, &pair.token) {
            Resolved::Matched => {}
            Resolved::Unmatched { actual, .. } => {
                if miss.is_none() {
                    let shown = actual.unwrap_or_else(|| "skipped".to_string());
                    miss = Some((pair.produce.clone(), shown));
                }
            }
            Resolved::Unresolved => {
                if unresolved.is_none() {
                    unresolved = Some(pair.produce.clone());
                }
            }
        }
    }
    // A miss counts only once every pair is resolved. Until then the step
    // can still be admitted.
    if let Some(produce) = unresolved {
        return Cond::Unresolved { produce };
    }
    if let Some((produce, token)) = miss {
        return Cond::NoMatch { produce, token };
    }
    Cond::Match
}

pub fn eval_condition(steps: &[Step], attempts: &[Attempt], step: &Step) -> Cond {
    if !step.when.is_empty() && !step.when_any.is_empty() {
        return Cond::NoMatch {
            produce: step.id.clone(),
            token: "invalid".to_string(),
        };
    }
    if !step.when.is_empty() {
        return eval_and(steps, attempts, &step.when);
    }
    if step.when_any.is_empty() {
        return Cond::Absent;
    }
    let mut any_unresolved: Option<String> = None;
    let mut first_miss: Option<(String, String)> = None;
    for alt in &step.when_any {
        match eval_and(steps, attempts, alt) {
            Cond::Match | Cond::Absent => return Cond::Match,
            Cond::Unresolved { produce } => {
                if any_unresolved.is_none() {
                    any_unresolved = Some(produce);
                }
            }
            Cond::NoMatch { produce, token } => {
                if first_miss.is_none() {
                    first_miss = Some((produce, token));
                }
            }
        }
    }
    if let Some(produce) = any_unresolved {
        // One recorded match already returned. Every remaining alternative
        // is either a miss or still open, so the step stays unresolved.
        return Cond::Unresolved { produce };
    }
    if let Some((produce, token)) = first_miss {
        return Cond::NoMatch { produce, token };
    }
    Cond::Unresolved {
        produce: step.id.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    Need(String),
    Condition { produce: String, token: String },
}

/// Recompute stored states. Succeeded, running, failed, and blocked stay.
/// Everything else becomes skipped or pending. Ready is not stored.
/// Repeating the pass lets a skip cascade along `needs`.
pub fn recompute(steps: &mut [Step], attempts: &[Attempt]) {
    let order = topo(steps);
    for _ in 0..steps.len().max(1) {
        let mut changed = false;
        for &index in &order {
            let next = next_state(steps, attempts, index);
            if steps[index].state != next {
                steps[index].state = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

fn next_state(steps: &[Step], attempts: &[Attempt], index: usize) -> StepState {
    match steps[index].state {
        StepState::Succeeded | StepState::Running | StepState::Failed | StepState::Blocked => {
            steps[index].state
        }
        _ => {
            if skip_reason(steps, attempts, &steps[index]).is_some() {
                StepState::Skipped
            } else {
                StepState::Pending
            }
        }
    }
}

pub fn skip_reason(steps: &[Step], attempts: &[Attempt], step: &Step) -> Option<SkipReason> {
    for need in &step.needs {
        if let Some(dep) = steps.iter().find(|s| s.id == *need) {
            if dep.state == StepState::Skipped {
                return Some(SkipReason::Need(need.clone()));
            }
        }
    }
    match eval_condition(steps, attempts, step) {
        Cond::NoMatch { produce, token } => Some(SkipReason::Condition { produce, token }),
        _ => None,
    }
}

pub fn is_ready(steps: &[Step], attempts: &[Attempt], step: &Step) -> bool {
    if step.state != StepState::Pending {
        return false;
    }
    if skip_reason(steps, attempts, step).is_some() {
        return false;
    }
    for need in &step.needs {
        match steps.iter().find(|s| s.id == *need) {
            Some(dep) if dep.state == StepState::Succeeded => {}
            _ => return false,
        }
    }
    matches!(
        eval_condition(steps, attempts, step),
        Cond::Absent | Cond::Match
    )
}

pub fn claimable(steps: &[Step], attempts: &[Attempt], step: &Step, agent: &str) -> bool {
    if !is_ready(steps, attempts, step) {
        return false;
    }
    match &step.actor {
        None => true,
        Some(actor) => actor == agent,
    }
}

pub fn frontier(steps: &[Step], attempts: &[Attempt]) -> Vec<String> {
    steps
        .iter()
        .filter(|s| is_ready(steps, attempts, s))
        .map(|s| s.id.clone())
        .collect()
}

pub fn failed_count(attempts: &[Attempt], step_uid: &str) -> usize {
    attempts
        .iter()
        .filter(|a| a.step_uid == step_uid && a.outcome == Some(Outcome::Failed))
        .count()
}

fn topo(steps: &[Step]) -> Vec<usize> {
    let n = steps.len();
    let mut indegree = vec![0usize; n];
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    let index_of = |id: &str| steps.iter().position(|s| s.id == id);
    for (i, step) in steps.iter().enumerate() {
        let mut deps = step.needs.clone();
        for pair in step.when.iter().chain(step.when_any.iter().flatten()) {
            if let Some(prod) = producer(steps, &pair.produce) {
                if !deps.contains(&prod.id) {
                    deps.push(prod.id.clone());
                }
            }
        }
        for dep in deps {
            if let Some(j) = index_of(&dep) {
                edges[j].push(i);
                indegree[i] += 1;
            }
        }
    }
    let mut queue: Vec<usize> = (0..n).filter(|i| indegree[*i] == 0).collect();
    queue.sort_unstable();
    let mut out = Vec::with_capacity(n);
    while let Some(i) = queue.first().copied() {
        queue.remove(0);
        out.push(i);
        for &child in &edges[i] {
            indegree[child] -= 1;
            if indegree[child] == 0 {
                queue.push(child);
                queue.sort_unstable();
            }
        }
    }
    // A cycle leaves nodes out. Append them so recompute still visits everyone.
    for i in 0..n {
        if !out.contains(&i) {
            out.push(i);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str, state: StepState) -> Step {
        Step {
            uid: format!("u-{id}"),
            id: id.to_string(),
            title: id.to_string(),
            done_when: "done".into(),
            produces: None,
            results: vec![],
            needs: vec![],
            when: vec![],
            when_any: vec![],
            actor: None,
            state,
            position: 0,
        }
    }

    fn succeed(step: &Step, result: &str) -> Attempt {
        Attempt {
            uid: format!("att-{}", step.id),
            id: "a1".into(),
            step_uid: step.uid.clone(),
            step_id: step.id.clone(),
            agent: "invoices".into(),
            outcome: Some(Outcome::Succeeded),
            result: Some(result.into()),
            note: Some(format!("note-{result}")),
            because: None,
            artifact: None,
            artifact_hash: None,
            lease_until: None,
            started_at: "t".into(),
            finished_at: Some("t".into()),
        }
    }

    #[test]
    fn when_any_one_match_is_enough() {
        let mut vendor = step("verify-vendor", StepState::Succeeded);
        vendor.produces = Some("vendor".into());
        vendor.results = vec!["approved".into(), "rejected".into()];
        let mut pricing = step("verify-pricing", StepState::Pending);
        pricing.produces = Some("pricing".into());
        let mut escalate = step("escalate", StepState::Pending);
        escalate.when_any = vec![
            vec![Pair {
                produce: "vendor".into(),
                token: "rejected".into(),
            }],
            vec![Pair {
                produce: "pricing".into(),
                token: "misaligned".into(),
            }],
        ];
        let steps = vec![vendor.clone(), pricing, escalate.clone()];
        let attempts = vec![succeed(&vendor, "rejected")];
        assert!(is_ready(&steps, &attempts, &steps[2]));
    }

    #[test]
    fn when_any_waits_while_any_alternative_is_open() {
        let mut vendor = step("verify-vendor", StepState::Pending);
        vendor.produces = Some("vendor".into());
        let mut escalate = step("escalate", StepState::Pending);
        escalate.when_any = vec![
            vec![Pair {
                produce: "vendor".into(),
                token: "rejected".into(),
            }],
            vec![Pair {
                produce: "pricing".into(),
                token: "misaligned".into(),
            }],
        ];
        let steps = vec![vendor, escalate.clone()];
        assert!(!is_ready(&steps, &[], &steps[1]));
        assert!(matches!(
            eval_condition(&steps, &[], &steps[1]),
            Cond::Unresolved { .. }
        ));
    }

    #[test]
    fn skipped_need_skips_the_dependent() {
        let mut vendor = step("verify-vendor", StepState::Succeeded);
        vendor.produces = Some("vendor".into());
        vendor.uid = "u-verify-vendor".into();
        let mut pricing = step("verify-pricing", StepState::Pending);
        pricing.when = vec![Pair {
            produce: "vendor".into(),
            token: "approved".into(),
        }];
        let mut discounts = step("verify-discounts", StepState::Pending);
        discounts.needs = vec!["verify-pricing".into()];
        let attempt = Attempt {
            uid: "att".into(),
            id: "a1".into(),
            step_uid: vendor.uid.clone(),
            step_id: vendor.id.clone(),
            agent: "invoices".into(),
            outcome: Some(Outcome::Succeeded),
            result: Some("rejected".into()),
            note: Some("no".into()),
            because: None,
            artifact: None,
            artifact_hash: None,
            lease_until: None,
            started_at: "t".into(),
            finished_at: Some("t".into()),
        };
        let mut steps = vec![vendor, pricing, discounts];
        recompute(&mut steps, &[attempt]);
        assert_eq!(steps[1].state, StepState::Skipped);
        assert_eq!(steps[2].state, StepState::Skipped);
    }

    #[test]
    fn skipped_producer_does_not_satisfy_needs() {
        let mut parse = step("parse", StepState::Skipped);
        parse.produces = Some("invoice".into());
        let mut pay = step("pay", StepState::Pending);
        pay.needs = vec!["parse".into()];
        let steps = vec![parse, pay];
        assert!(!is_ready(&steps, &[], &steps[1]));
    }

    #[test]
    fn failed_dependency_stays_pending() {
        let blocked = step("verify-pricing", StepState::Blocked);
        let mut next = step("verify-discounts", StepState::Pending);
        next.needs = vec!["verify-pricing".into()];
        let mut steps = vec![blocked, next];
        recompute(&mut steps, &[]);
        assert_eq!(steps[1].state, StepState::Pending);
        assert!(!is_ready(&steps, &[], &steps[1]));
    }

    #[test]
    fn condition_miss_skips_even_before_needs_finish() {
        let mut vendor = step("verify-vendor", StepState::Succeeded);
        vendor.produces = Some("vendor".into());
        let mut pricing = step("verify-pricing", StepState::Pending);
        pricing.needs = vec!["verify-vendor".into()];
        pricing.when = vec![Pair {
            produce: "vendor".into(),
            token: "approved".into(),
        }];
        let attempts = vec![succeed(&vendor, "rejected")];
        let mut steps = vec![vendor, pricing];
        recompute(&mut steps, &attempts);
        assert_eq!(steps[1].state, StepState::Skipped);
    }

    #[test]
    fn frontier_is_document_order() {
        let mut a = step("b-step", StepState::Pending);
        a.position = 1;
        let mut b = step("a-step", StepState::Pending);
        b.position = 0;
        let steps = vec![b, a];
        assert_eq!(
            frontier(&steps, &[]),
            vec!["a-step".to_string(), "b-step".to_string()]
        );
    }

    #[test]
    fn actor_gates_the_claim() {
        let mut human = step("human-review", StepState::Pending);
        human.actor = Some("human".into());
        let steps = vec![human];
        assert!(!claimable(&steps, &[], &steps[0], "invoices"));
        assert!(claimable(&steps, &[], &steps[0], "human"));
    }
}
