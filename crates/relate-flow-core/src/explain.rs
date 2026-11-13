use crate::admission::{self, Cond, Resolved, SkipReason};
use crate::model::{Attempt, Outcome, Step, StepState};
use serde_json::{json, Value};

pub fn explain(steps: &[Step], attempts: &[Attempt], step: &Step, retries: u32) -> Value {
    let disposition = disposition(steps, attempts, step);
    let why = why(steps, attempts, step, retries, disposition);
    let missing_needs: Vec<&str> = step
        .needs
        .iter()
        .filter(|need| {
            steps
                .iter()
                .find(|s| s.id == **need)
                .map(|s| s.state != StepState::Succeeded)
                .unwrap_or(true)
        })
        .map(String::as_str)
        .collect();
    let mut missing_tokens = Vec::new();
    for pair in step.when.iter().chain(step.when_any.iter().flatten()) {
        if matches!(
            admission::resolve(steps, attempts, &pair.produce, &pair.token),
            Resolved::Unresolved
        ) && !missing_tokens.contains(&pair.produce)
        {
            missing_tokens.push(pair.produce.clone());
        }
    }
    let last = attempts
        .iter()
        .rev()
        .find(|a| a.step_uid == step.uid)
        .map(Attempt::to_log);
    json!({
        "step": step.id,
        "disposition": disposition,
        "why": why,
        "needs": step.needs,
        "when": step.when.iter().map(|p| p.encode()).collect::<Vec<_>>(),
        "whenAny": step.when_any.iter().map(|alt| {
            alt.iter().map(|p| p.encode()).collect::<Vec<_>>().join(",")
        }).collect::<Vec<_>>(),
        "missing": {
            "needs": missing_needs,
            "tokens": missing_tokens,
        },
        "lastAttempt": last,
    })
}

pub fn disposition(steps: &[Step], attempts: &[Attempt], step: &Step) -> &'static str {
    if admission::is_ready(steps, attempts, step) {
        "ready"
    } else {
        step.state.as_str()
    }
}

pub fn why(
    steps: &[Step],
    attempts: &[Attempt],
    step: &Step,
    retries: u32,
    disposition: &str,
) -> String {
    let failed = admission::failed_count(attempts, &step.uid);
    if disposition == "ready" {
        if failed > 0 {
            return format!("ready, failed {failed} of {retries}");
        }
        return "ready".to_string();
    }
    match step.state {
        StepState::Running => {
            let lease = attempts
                .iter()
                .rev()
                .find(|a| a.step_uid == step.uid && a.is_open());
            match lease {
                Some(a) => format!(
                    "leased to {} until {}",
                    a.agent,
                    a.lease_until.as_deref().unwrap_or("")
                ),
                None => "running".to_string(),
            }
        }
        StepState::Succeeded => "succeeded".to_string(),
        StepState::Skipped => match admission::skip_reason(steps, attempts, step) {
            Some(SkipReason::Need(id)) => format!("skipped because {id} was skipped"),
            Some(SkipReason::Condition { produce, token }) => {
                if token == "skipped" {
                    if let Some(prod) = admission::producer(steps, &produce) {
                        return format!("skipped because {} was skipped", prod.id);
                    }
                }
                format!("skipped because {produce} is {token}")
            }
            None => "skipped".to_string(),
        },
        StepState::Blocked => {
            format!(
                "blocked: {}",
                latest_text(attempts, &step.uid, Outcome::Blocked)
            )
        }
        StepState::Failed => format!(
            "failed {failed} of {retries}: {}",
            latest_text(attempts, &step.uid, Outcome::Failed)
        ),
        StepState::Pending => match admission::eval_condition(steps, attempts, step) {
            Cond::Unresolved { produce }
                if step.needs.iter().all(|need| {
                    steps
                        .iter()
                        .find(|s| s.id == *need)
                        .map(|s| s.state == StepState::Succeeded)
                        .unwrap_or(false)
                }) =>
            {
                format!("waiting on {produce}")
            }
            _ => {
                if let Some(need) = step.needs.iter().find(|need| {
                    steps
                        .iter()
                        .find(|s| s.id == **need)
                        .map(|s| s.state != StepState::Succeeded)
                        .unwrap_or(true)
                }) {
                    format!("waiting on {need}")
                } else if let Cond::Unresolved { produce } =
                    admission::eval_condition(steps, attempts, step)
                {
                    format!("waiting on {produce}")
                } else {
                    "pending".to_string()
                }
            }
        },
    }
}

fn latest_text(attempts: &[Attempt], step_uid: &str, outcome: Outcome) -> String {
    attempts
        .iter()
        .rev()
        .find(|a| a.step_uid == step_uid && a.outcome == Some(outcome))
        .and_then(|a| a.because.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pair;

    fn step(id: &str, state: StepState) -> Step {
        Step {
            uid: format!("u-{id}"),
            id: id.into(),
            title: id.into(),
            done_when: "d".into(),
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

    #[test]
    fn why_table() {
        let ready = step("parse", StepState::Pending);
        let steps = vec![ready];
        assert_eq!(why(&steps, &[], &steps[0], 2, "ready"), "ready");

        let mut running = step("parse", StepState::Running);
        running.uid = "u-parse".into();
        let attempt = Attempt {
            uid: "a".into(),
            id: "a1".into(),
            step_uid: running.uid.clone(),
            step_id: "parse".into(),
            agent: "invoices".into(),
            outcome: None,
            result: None,
            note: None,
            because: None,
            artifact: None,
            artifact_hash: None,
            lease_until: Some("2026-11-10T22:15:00Z".into()),
            started_at: "t".into(),
            finished_at: None,
        };
        let steps = vec![running];
        assert_eq!(
            why(&steps, &[attempt], &steps[0], 2, "running"),
            "leased to invoices until 2026-11-10T22:15:00Z"
        );

        let done = step("parse", StepState::Succeeded);
        assert_eq!(
            why(std::slice::from_ref(&done), &[], &done, 2, "succeeded"),
            "succeeded"
        );

        let mut skipped = step("pay", StepState::Skipped);
        skipped.needs = vec!["parse".into()];
        let parent = step("parse", StepState::Skipped);
        let steps = vec![parent, skipped];
        assert_eq!(
            why(&steps, &[], &steps[1], 2, "skipped"),
            "skipped because parse was skipped"
        );

        let mut vendor = step("verify-vendor", StepState::Succeeded);
        vendor.produces = Some("vendor".into());
        vendor.uid = "u-verify-vendor".into();
        let mut pricing = step("verify-pricing", StepState::Skipped);
        pricing.when = vec![Pair {
            produce: "vendor".into(),
            token: "approved".into(),
        }];
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
        let steps = vec![vendor, pricing];
        assert_eq!(
            why(&steps, &[attempt], &steps[1], 2, "skipped"),
            "skipped because vendor is rejected"
        );

        let mut child = step("verify-vendor", StepState::Pending);
        child.needs = vec!["parse".into()];
        let parent = step("parse", StepState::Pending);
        let steps = vec![parent, child];
        assert_eq!(
            why(&steps, &[], &steps[1], 2, "pending"),
            "waiting on parse"
        );

        let mut vendor = step("verify-vendor", StepState::Pending);
        vendor.produces = Some("vendor".into());
        let mut pricing = step("verify-pricing", StepState::Pending);
        pricing.when = vec![Pair {
            produce: "vendor".into(),
            token: "approved".into(),
        }];
        let steps = vec![vendor, pricing];
        assert_eq!(
            why(&steps, &[], &steps[1], 2, "pending"),
            "waiting on vendor"
        );

        let mut blocked = step("verify-pricing", StepState::Blocked);
        blocked.uid = "u-verify-pricing".into();
        let attempt = Attempt {
            uid: "b".into(),
            id: "a2".into(),
            step_uid: blocked.uid.clone(),
            step_id: blocked.id.clone(),
            agent: "invoices".into(),
            outcome: Some(Outcome::Blocked),
            result: None,
            note: None,
            because: Some("NW-2024 is not in the store.".into()),
            artifact: None,
            artifact_hash: None,
            lease_until: None,
            started_at: "t".into(),
            finished_at: Some("t".into()),
        };
        assert_eq!(
            why(
                std::slice::from_ref(&blocked),
                std::slice::from_ref(&attempt),
                &blocked,
                2,
                "blocked"
            ),
            "blocked: NW-2024 is not in the store."
        );

        let mut failed = step("parse", StepState::Failed);
        failed.uid = "u-parse".into();
        let attempt = Attempt {
            uid: "f".into(),
            id: "a2".into(),
            step_uid: failed.uid.clone(),
            step_id: "parse".into(),
            agent: "invoices".into(),
            outcome: Some(Outcome::Failed),
            result: None,
            note: None,
            because: Some("unreadable".into()),
            artifact: None,
            artifact_hash: None,
            lease_until: None,
            started_at: "t".into(),
            finished_at: Some("t".into()),
        };
        let earlier = Attempt {
            outcome: Some(Outcome::Failed),
            uid: "f0".into(),
            id: "a1".into(),
            ..attempt.clone()
        };
        assert_eq!(
            why(&[failed.clone()], &[earlier, attempt], &failed, 2, "failed"),
            "failed 2 of 2: unreadable"
        );

        let mut again = step("parse", StepState::Pending);
        again.uid = "u-parse".into();
        let attempt = Attempt {
            uid: "f".into(),
            id: "a1".into(),
            step_uid: again.uid.clone(),
            step_id: "parse".into(),
            agent: "invoices".into(),
            outcome: Some(Outcome::Failed),
            result: None,
            note: None,
            because: Some("unreadable".into()),
            artifact: None,
            artifact_hash: None,
            lease_until: None,
            started_at: "t".into(),
            finished_at: Some("t".into()),
        };
        assert_eq!(
            why(
                std::slice::from_ref(&again),
                std::slice::from_ref(&attempt),
                &again,
                2,
                "ready"
            ),
            "ready, failed 1 of 2"
        );
    }

    #[test]
    fn the_other_why_strings() {
        let running = step("parse", StepState::Running);
        assert_eq!(
            why(std::slice::from_ref(&running), &[], &running, 2, "running"),
            "running"
        );

        let mut vendor = step("check", StepState::Skipped);
        vendor.produces = Some("vendor".into());
        let mut child = step("pay", StepState::Skipped);
        child.when = vec![Pair {
            produce: "vendor".into(),
            token: "approved".into(),
        }];
        let text = why(&[vendor, child.clone()], &[], &child, 2, "skipped");
        assert!(text.contains("check was skipped"), "{text}");

        let parent = step("parse", StepState::Pending);
        let mut waiting = step("child", StepState::Pending);
        waiting.needs = vec!["parse".into()];
        assert_eq!(
            why(&[parent, waiting.clone()], &[], &waiting, 2, "pending"),
            "waiting on parse"
        );

        let mut both = step("bad", StepState::Pending);
        both.when = vec![Pair {
            produce: "vendor".into(),
            token: "approved".into(),
        }];
        both.when_any = vec![vec![]];
        let _ = why(std::slice::from_ref(&both), &[], &both, 2, "pending");

        let body = explain(std::slice::from_ref(&waiting), &[], &waiting, 2);
        assert_eq!(body["missing"]["needs"][0], "parse");
        assert!(body["when"].as_array().unwrap().is_empty());
    }
}
