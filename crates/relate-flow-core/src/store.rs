//! Load and save the plan graph. One transaction per command; each [`Write`]
//! touches only the nodes that command changed.

use std::collections::BTreeMap;
use std::sync::Arc;

use relate::{Connection, Value};

use crate::condition::parse_pair;
use crate::error::FlowError;
use crate::model::{Attempt, Outcome, Plan, PlanKind, Step, StepState, Write};

#[derive(Debug, Clone)]
pub struct State {
    pub plans: Vec<Plan>,
    pub attempts: Vec<Attempt>,
    pub writes: Vec<Write>,
    /// Name passed to this command. Empty until [`State::focus`].
    pub plan_name: String,
    /// Highest attempt number in the whole store, so ids stay unique across plans.
    pub attempt_seq: u64,
    /// Every plan name in the store, recorded before [`State::focus`] drops the others.
    pub names: Vec<String>,
    /// Every plan in the store, recorded before [`State::focus`] drops the others.
    pub store_plans: Vec<Plan>,
    /// Every attempt in the store, recorded before [`State::focus`] drops the others.
    pub store_attempts: Vec<Attempt>,
}

impl State {
    pub fn draft(&self) -> Option<&Plan> {
        self.plans.iter().find(|p| p.is_draft())
    }

    pub fn draft_mut(&mut self) -> Option<&mut Plan> {
        self.plans.iter_mut().find(|p| p.is_draft())
    }

    pub fn committed(&self) -> Option<&Plan> {
        self.plans
            .iter()
            .filter(|p| p.version.is_some())
            .max_by_key(|p| p.version.unwrap_or(0))
    }

    pub fn committed_index(&self) -> Option<usize> {
        self.plans
            .iter()
            .enumerate()
            .filter(|(_, p)| p.version.is_some())
            .max_by_key(|(_, p)| p.version.unwrap_or(0))
            .map(|(i, _)| i)
    }

    pub fn draft_index(&self) -> Option<usize> {
        self.plans.iter().position(|p| p.is_draft())
    }

    /// Keep the lineage named `name` and the attempts on its steps.
    pub fn focus(&mut self, name: &str) {
        self.plan_name = name.to_string();
        self.names = self
            .plans
            .iter()
            .map(|plan| plan.name.clone())
            .filter(|name| !name.is_empty())
            .collect();
        self.store_plans = self.plans.clone();
        self.store_attempts = self.attempts.clone();
        self.attempt_seq = self
            .attempts
            .iter()
            .filter_map(|attempt| {
                attempt
                    .id
                    .strip_prefix('a')
                    .and_then(|n| n.parse::<u64>().ok())
            })
            .max()
            .unwrap_or(0);
        self.plans.retain(|plan| plan.name == name);
        let steps: std::collections::HashSet<&str> = self
            .plans
            .iter()
            .flat_map(|plan| plan.steps.iter().map(|step| step.uid.as_str()))
            .collect();
        self.attempts
            .retain(|attempt| steps.contains(attempt.step_uid.as_str()));
    }
}

pub fn load(conn: &mut Connection) -> Result<State, FlowError> {
    let nodes = count(conn, "MATCH (n) RETURN count(n) AS c")?;
    let plans = count(conn, "MATCH (p:Plan) RETURN count(p) AS c")?;
    if nodes > 0 && plans == 0 {
        return Err(FlowError::refusal(
            "foreign_store",
            "the store already holds a graph and it is not a relate-flow plan",
            "open an empty directory, or a store relate-flow created",
        ));
    }
    let mut state = State {
        plans: Vec::new(),
        attempts: Vec::new(),
        writes: Vec::new(),
        plan_name: String::new(),
        attempt_seq: 0,
        names: Vec::new(),
        store_plans: Vec::new(),
        store_attempts: Vec::new(),
    };
    let plan_rows = query(
        conn,
        "MATCH (p:Plan) OPTIONAL MATCH (p)-[:SUPERSEDES]->(prev:Plan) \
         RETURN p.uid, p.goal, p.retries, p.version, p.basedOn, p.createdAt, prev.uid, p.name, \
                p.kind, p.inputs, p.template, p.templateVersion",
        BTreeMap::new(),
    )?;
    for row in &plan_rows.rows {
        state.plans.push(Plan {
            uid: req_string(&row[0], "plan.uid")?,
            goal: req_string(&row[1], "plan.goal")?,
            retries: req_int(&row[2], "plan.retries")? as u32,
            version: opt_int(&row[3]).map(|n| n as u64),
            based_on: opt_int(&row[4]).map(|n| n as u64),
            created_at: req_string(&row[5], "plan.createdAt")?,
            steps: Vec::new(),
            supersedes: opt_string(&row[6]),
            name: opt_string(&row[7]).unwrap_or_default(),
            kind: PlanKind::parse(opt_string(&row[8]).as_deref()),
            inputs: decode_inputs(&row[9]),
            template: opt_string(&row[10]),
            template_version: opt_int(&row[11]).map(|n| n as u64),
        });
    }
    let step_rows = query(
        conn,
        "MATCH (p:Plan)-[:HAS_STEP]->(s:Step) \
         RETURN p.uid, s.uid, s.id, s.title, s.doneWhen, s.produces, s.results, \
                s.needs, s.when, s.whenAny, s.actor, s.state, s.position \
         ORDER BY s.position",
        BTreeMap::new(),
    )?;
    for row in &step_rows.rows {
        let plan_uid = req_string(&row[0], "plan.uid")?;
        let step = Step {
            uid: req_string(&row[1], "step.uid")?,
            id: req_string(&row[2], "step.id")?,
            title: req_string(&row[3], "step.title")?,
            done_when: req_string(&row[4], "step.doneWhen")?,
            produces: opt_string(&row[5]),
            results: string_list(&row[6]),
            needs: string_list(&row[7]),
            when: decode_pairs(&string_list(&row[8]))?,
            when_any: decode_alts(&string_list(&row[9]))?,
            actor: opt_string(&row[10]),
            state: StepState::parse(opt_string(&row[11]).as_deref().unwrap_or("pending")),
            position: opt_int(&row[12]).unwrap_or(0),
        };
        if let Some(plan) = state.plans.iter_mut().find(|p| p.uid == plan_uid) {
            plan.steps.push(step);
        }
    }
    for plan in &mut state.plans {
        plan.steps.sort_by_key(|s| s.position);
    }
    let attempt_rows = query(
        conn,
        "MATCH (a:Attempt)-[:OF]->(s:Step) \
         RETURN a.uid, a.id, s.uid, a.stepId, a.agent, a.outcome, a.result, a.note, \
                a.because, a.artifact, a.artifactHash, a.leaseUntil, a.startedAt, a.finishedAt",
        BTreeMap::new(),
    )?;
    for row in &attempt_rows.rows {
        let outcome = opt_string(&row[5]).and_then(|s| Outcome::parse(&s));
        state.attempts.push(Attempt {
            uid: req_string(&row[0], "attempt.uid")?,
            id: req_string(&row[1], "attempt.id")?,
            step_uid: req_string(&row[2], "step.uid")?,
            step_id: req_string(&row[3], "attempt.stepId")?,
            agent: req_string(&row[4], "attempt.agent")?,
            outcome,
            result: opt_string(&row[6]),
            note: opt_string(&row[7]),
            because: opt_string(&row[8]),
            artifact: opt_string(&row[9]),
            artifact_hash: opt_string(&row[10]),
            lease_until: opt_string(&row[11]),
            started_at: req_string(&row[12], "attempt.startedAt")?,
            finished_at: opt_string(&row[13]),
        });
    }
    Ok(state)
}

pub fn apply(conn: &mut Connection, writes: &[Write]) -> Result<(), FlowError> {
    for write in writes {
        match write {
            Write::CreatePlan(plan) => {
                query(
                    conn,
                    "CREATE (:Plan {uid: $uid, name: $name, kind: $kind, inputs: $inputs, \
                     template: $template, templateVersion: $templateVersion, goal: $goal, \
                     retries: $retries, version: $version, basedOn: $basedOn, createdAt: $createdAt})",
                    plan_params(plan),
                )?;
            }
            Write::UpdatePlan(plan) => {
                query(
                    conn,
                    "MATCH (p:Plan {uid: $uid}) \
                     SET p.name = $name, p.kind = $kind, p.inputs = $inputs, \
                         p.template = $template, p.templateVersion = $templateVersion, \
                         p.goal = $goal, p.retries = $retries, p.version = $version, \
                         p.basedOn = $basedOn, p.createdAt = $createdAt",
                    plan_params(plan),
                )?;
            }
            Write::DeletePlan { uid } => {
                query(
                    conn,
                    "MATCH (p:Plan {uid: $uid})-[:HAS_STEP]->(s:Step)<-[:OF]-(a:Attempt) DETACH DELETE a",
                    one("uid", v_str(uid)),
                )?;
                query(
                    conn,
                    "MATCH (p:Plan {uid: $uid})-[:HAS_STEP]->(s:Step) DETACH DELETE s",
                    one("uid", v_str(uid)),
                )?;
                query(
                    conn,
                    "MATCH (p:Plan {uid: $uid}) DETACH DELETE p",
                    one("uid", v_str(uid)),
                )?;
            }
            Write::Supersedes { new_uid, old_uid } => {
                query(
                    conn,
                    "MATCH (n:Plan {uid: $new}), (o:Plan {uid: $old}) CREATE (n)-[:SUPERSEDES]->(o)",
                    params(&[("new", v_str(new_uid)), ("old", v_str(old_uid))]),
                )?;
            }
            Write::UpsertStep {
                plan_uid,
                step,
                is_new,
                sync_needs,
            } => {
                let mut p = step_params(step);
                p.insert("plan".into(), v_str(plan_uid));
                if *is_new {
                    query(
                        conn,
                        "MATCH (p:Plan {uid: $plan}) \
                         CREATE (s:Step {uid: $uid, id: $id, title: $title, doneWhen: $doneWhen, \
                           produces: $produces, results: $results, needs: $needs, when: $when, \
                           whenAny: $whenAny, actor: $actor, state: $state, position: $position}) \
                         CREATE (p)-[:HAS_STEP]->(s)",
                        p,
                    )?;
                } else {
                    query(
                        conn,
                        "MATCH (s:Step {uid: $uid}) \
                         SET s.id = $id, s.title = $title, s.doneWhen = $doneWhen, \
                             s.produces = $produces, s.results = $results, s.needs = $needs, \
                             s.when = $when, s.whenAny = $whenAny, s.actor = $actor, \
                             s.state = $state, s.position = $position",
                        p,
                    )?;
                }
                if *sync_needs {
                    query(
                        conn,
                        "MATCH (s:Step {uid: $uid})-[r:NEEDS]->() DELETE r",
                        one("uid", v_str(&step.uid)),
                    )?;
                    for need in &step.needs {
                        query(
                            conn,
                            "MATCH (s:Step {uid: $uid})<-[:HAS_STEP]-(p:Plan)-[:HAS_STEP]->(n:Step {id: $need}) \
                             CREATE (s)-[:NEEDS]->(n)",
                            params(&[("uid", v_str(&step.uid)), ("need", v_str(need))]),
                        )?;
                    }
                }
            }
            Write::DeleteStep { uid } => {
                query(
                    conn,
                    "MATCH (s:Step {uid: $uid}) DETACH DELETE s",
                    one("uid", v_str(uid)),
                )?;
            }
            Write::CreateAttempt(attempt) => {
                let mut p = attempt_params(attempt);
                p.insert("step".into(), v_str(&attempt.step_uid));
                query(
                    conn,
                    "MATCH (s:Step {uid: $step}) \
                     CREATE (a:Attempt {uid: $uid, id: $id, stepId: $stepId, agent: $agent, \
                       outcome: $outcome, result: $result, note: $note, because: $because, \
                       artifact: $artifact, artifactHash: $artifactHash, leaseUntil: $leaseUntil, \
                       startedAt: $startedAt, finishedAt: $finishedAt}) \
                     CREATE (a)-[:OF]->(s)",
                    p,
                )?;
            }
            Write::UpdateAttempt(attempt) => {
                query(
                    conn,
                    "MATCH (a:Attempt {uid: $uid}) \
                     SET a.id = $id, a.stepId = $stepId, a.agent = $agent, a.outcome = $outcome, \
                         a.result = $result, a.note = $note, a.because = $because, \
                         a.artifact = $artifact, a.artifactHash = $artifactHash, \
                         a.leaseUntil = $leaseUntil, a.startedAt = $startedAt, \
                         a.finishedAt = $finishedAt",
                    attempt_params(attempt),
                )?;
            }
            Write::MoveAttempt {
                attempt_uid,
                new_step_uid,
            } => {
                query(
                    conn,
                    "MATCH (a:Attempt {uid: $uid})-[r:OF]->() DELETE r",
                    one("uid", v_str(attempt_uid)),
                )?;
                query(
                    conn,
                    "MATCH (a:Attempt {uid: $uid}), (s:Step {uid: $step}) CREATE (a)-[:OF]->(s)",
                    params(&[("uid", v_str(attempt_uid)), ("step", v_str(new_step_uid))]),
                )?;
            }
        }
    }
    Ok(())
}

fn plan_params(plan: &Plan) -> BTreeMap<String, Value> {
    params(&[
        ("uid", v_str(&plan.uid)),
        ("name", v_str(&plan.name)),
        ("kind", v_str(plan.kind.as_str())),
        ("inputs", encode_inputs(&plan.inputs)),
        (
            "template",
            plan.template.as_deref().map(v_str).unwrap_or(Value::Null),
        ),
        (
            "templateVersion",
            plan.template_version
                .map(|v| Value::Int(v as i64))
                .unwrap_or(Value::Null),
        ),
        ("goal", v_str(&plan.goal)),
        ("retries", Value::Int(i64::from(plan.retries))),
        (
            "version",
            plan.version
                .map(|v| Value::Int(v as i64))
                .unwrap_or(Value::Null),
        ),
        (
            "basedOn",
            plan.based_on
                .map(|v| Value::Int(v as i64))
                .unwrap_or(Value::Null),
        ),
        ("createdAt", v_str(&plan.created_at)),
    ])
}

fn step_params(step: &Step) -> BTreeMap<String, Value> {
    params(&[
        ("uid", v_str(&step.uid)),
        ("id", v_str(&step.id)),
        ("title", v_str(&step.title)),
        ("doneWhen", v_str(&step.done_when)),
        (
            "produces",
            step.produces.as_deref().map(v_str).unwrap_or(Value::Null),
        ),
        ("results", v_list(&step.results)),
        ("needs", v_list(&step.needs)),
        (
            "when",
            v_list(&step.when.iter().map(|p| p.encode()).collect::<Vec<_>>()),
        ),
        (
            "whenAny",
            v_list(
                &step
                    .when_any
                    .iter()
                    .map(|alt| alt.iter().map(|p| p.encode()).collect::<Vec<_>>().join(","))
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "actor",
            step.actor.as_deref().map(v_str).unwrap_or(Value::Null),
        ),
        ("state", v_str(step.state.as_str())),
        ("position", Value::Int(step.position)),
    ])
}

fn attempt_params(attempt: &Attempt) -> BTreeMap<String, Value> {
    let opt = |v: &Option<String>| v.as_deref().map(v_str).unwrap_or(Value::Null);
    params(&[
        ("uid", v_str(&attempt.uid)),
        ("id", v_str(&attempt.id)),
        ("stepId", v_str(&attempt.step_id)),
        ("agent", v_str(&attempt.agent)),
        (
            "outcome",
            attempt
                .outcome
                .map(|o| v_str(o.as_str()))
                .unwrap_or(Value::Null),
        ),
        ("result", opt(&attempt.result)),
        ("note", opt(&attempt.note)),
        ("because", opt(&attempt.because)),
        ("artifact", opt(&attempt.artifact)),
        ("artifactHash", opt(&attempt.artifact_hash)),
        ("leaseUntil", opt(&attempt.lease_until)),
        ("startedAt", v_str(&attempt.started_at)),
        ("finishedAt", opt(&attempt.finished_at)),
    ])
}

fn decode_pairs(raw: &[String]) -> Result<Vec<crate::model::Pair>, FlowError> {
    raw.iter()
        .map(|s| {
            parse_pair(s)
                .map_err(|_| FlowError::store(format!("stored condition '{s}' is unreadable")))
        })
        .collect()
}

fn decode_alts(raw: &[String]) -> Result<Vec<Vec<crate::model::Pair>>, FlowError> {
    raw.iter()
        .map(|s| {
            s.split(',')
                .map(|piece| {
                    parse_pair(piece).map_err(|_| {
                        FlowError::store(format!("stored condition '{piece}' is unreadable"))
                    })
                })
                .collect()
        })
        .collect()
}

fn count(conn: &mut Connection, cypher: &str) -> Result<i64, FlowError> {
    let result = query(conn, cypher, BTreeMap::new())?;
    Ok(result
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(opt_int)
        .unwrap_or(0))
}

fn query(
    conn: &mut Connection,
    cypher: &str,
    params: BTreeMap<String, Value>,
) -> Result<relate::QueryResult, FlowError> {
    conn.query_with_params(cypher, params)
        .map_err(|e| FlowError::store(e.to_string()))
}

fn params(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

fn one(key: &str, value: Value) -> BTreeMap<String, Value> {
    params(&[(key, value)])
}

fn v_str(text: &str) -> Value {
    Value::String(Arc::from(text))
}

fn encode_inputs(inputs: &[(String, String)]) -> Value {
    v_list(
        &inputs
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>(),
    )
}

fn decode_inputs(value: &Value) -> Vec<(String, String)> {
    string_list(value)
        .into_iter()
        .filter_map(|raw| {
            let (key, value) = raw.split_once('=')?;
            if key.is_empty() {
                None
            } else {
                Some((key.to_string(), value.to_string()))
            }
        })
        .collect()
}

fn v_list(items: &[String]) -> Value {
    Value::List(Arc::new(items.iter().map(|s| v_str(s)).collect()))
}

fn opt_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.to_string()),
        _ => None,
    }
}

fn req_string(value: &Value, what: &str) -> Result<String, FlowError> {
    opt_string(value).ok_or_else(|| FlowError::store(format!("missing {what}")))
}

fn opt_int(value: &Value) -> Option<i64> {
    match value {
        Value::Int(n) => Some(*n),
        _ => None,
    }
}

fn req_int(value: &Value, what: &str) -> Result<i64, FlowError> {
    opt_int(value).ok_or_else(|| FlowError::store(format!("missing {what}")))
}

fn string_list(value: &Value) -> Vec<String> {
    match value {
        Value::List(items) => items.iter().filter_map(opt_string).collect(),
        _ => Vec::new(),
    }
}
