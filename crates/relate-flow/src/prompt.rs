//! Plain-text prompt for `next --output text`.
//!
//! The JSON instruction is unchanged. This renders a successful one.

use serde_json::Value;

pub fn render(plan: &str, agent: &str, value: &Value) -> String {
    match value.get("action").and_then(Value::as_str).unwrap_or("") {
        "run" => render_run(plan, agent, value),
        "waiting" => line(
            &format!(
                "Waiting for {} to report {}.",
                text(value, "for"),
                text(value, "step")
            ),
            &text(value, "because"),
        ),
        "blocked" => line(
            &format!("Blocked on {}.", text(value, "step")),
            &text(value, "because"),
        ),
        "replan" => {
            let head = match value.get("step").and_then(Value::as_str) {
                Some(step) if !step.is_empty() => format!("Replan {step}."),
                _ => "Replan.".to_string(),
            };
            line(&head, &text(value, "because"))
        }
        "done" => format!("Done. version {}\n", text(value, "version")),
        other => format!("{other}\n"),
    }
}

fn render_run(plan: &str, agent: &str, value: &Value) -> String {
    let step = value.get("step").cloned().unwrap_or(Value::Null);
    let id = text(&step, "id");
    let mut out = format!(
        "Run {id} (attempt {}). Lease until {}.\n",
        text(&step, "attempt"),
        text(&step, "leaseUntil")
    );
    if value.get("renewed") == Some(&Value::Bool(true)) {
        out.push_str("Renewed.\n");
    }
    if let Some(retry) = step.get("retry").and_then(Value::as_i64) {
        out.push_str(&format!("Retry {retry}.\n"));
    }
    out.push('\n');
    out.push_str(&text(&step, "title"));
    out.push('\n');
    out.push_str("Done when: ");
    out.push_str(&text(&step, "doneWhen"));
    out.push('\n');
    if let Some(inputs) = step.get("inputs").and_then(Value::as_object) {
        if !inputs.is_empty() {
            out.push('\n');
            out.push_str("Inputs:\n");
            for (key, item) in inputs {
                if let Some(note) = item.as_str() {
                    out.push_str("  ");
                    out.push_str(key);
                    out.push_str(": ");
                    out.push_str(note);
                    out.push('\n');
                }
            }
        }
    }
    out.push('\n');
    out.push_str("Then run:\n");
    let result = step
        .get("results")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .map(|items| {
            let tokens: Vec<&str> = items.iter().filter_map(Value::as_str).collect();
            format!(" --result <{}>", tokens.join("|"))
        })
        .unwrap_or_default();
    out.push_str(&format!(
        "  relate-flow {plan} report {id} --agent {agent} --outcome succeeded{result} --note \"...\"\n"
    ));
    out.push_str(&format!(
        "  relate-flow {plan} report {id} --agent {agent} --outcome failed --because \"...\"\n"
    ));
    out
}

fn line(head: &str, because: &str) -> String {
    if because.is_empty() {
        format!("{head}\n")
    } else {
        format!("{head} {because}\n")
    }
}

fn text(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_run_with_results_prints_both_reports() {
        let text = render(
            "case",
            "oncall",
            &json!({
                "action": "run",
                "step": {
                    "id": "triage",
                    "attempt": "a1",
                    "title": "Classify the incident",
                    "doneWhen": "Severity is recorded",
                    "inputs": {"case": "Checkout is down."},
                    "leaseUntil": "2026-11-13T09:21:18Z",
                    "results": ["sev1", "sev2", "sev3"]
                }
            }),
        );
        assert_eq!(
            text,
            "\
Run triage (attempt a1). Lease until 2026-11-13T09:21:18Z.

Classify the incident
Done when: Severity is recorded

Inputs:
  case: Checkout is down.

Then run:
  relate-flow case report triage --agent oncall --outcome succeeded --result <sev1|sev2|sev3> --note \"...\"
  relate-flow case report triage --agent oncall --outcome failed --because \"...\"
"
        );
    }

    #[test]
    fn a_run_without_results_omits_result() {
        let text = render(
            "case",
            "oncall",
            &json!({
                "action": "run",
                "step": {
                    "id": "close-ticket",
                    "attempt": "a3",
                    "title": "Close the ticket",
                    "doneWhen": "The ticket is closed",
                    "inputs": {},
                    "leaseUntil": "2026-11-13T09:21:18Z"
                }
            }),
        );
        assert!(text.contains(
            "relate-flow case report close-ticket --agent oncall --outcome succeeded --note \"...\""
        ));
        assert!(!text.contains("--result"));
        assert!(!text.contains("Inputs:"));
    }

    #[test]
    fn a_renewed_retry_names_both() {
        let text = render(
            "case",
            "oncall",
            &json!({
                "action": "run",
                "renewed": true,
                "step": {
                    "id": "mitigate",
                    "attempt": "a2",
                    "title": "Mitigate",
                    "doneWhen": "recorded",
                    "leaseUntil": "2026-11-13T09:21:18Z",
                    "retry": 1
                }
            }),
        );
        assert!(text.contains("Renewed.\nRetry 1.\n"));
    }

    #[test]
    fn other_actions_are_one_line() {
        assert_eq!(
            render(
                "case",
                "oncall",
                &json!({"action": "waiting", "for": "ic", "step": "page-ic", "because": "Customers can see the notice."})
            ),
            "Waiting for ic to report page-ic. Customers can see the notice.\n"
        );
        assert_eq!(
            render(
                "case",
                "oncall",
                &json!({"action": "blocked", "step": "verify-pricing", "because": "NW-2024 is not in the store."})
            ),
            "Blocked on verify-pricing. NW-2024 is not in the store.\n"
        );
        assert_eq!(
            render(
                "case",
                "oncall",
                &json!({"action": "replan", "step": "mitigate", "because": "failed 2 times"})
            ),
            "Replan mitigate. failed 2 times\n"
        );
        assert_eq!(
            render(
                "case",
                "oncall",
                &json!({"action": "replan", "because": "no step can become ready"})
            ),
            "Replan. no step can become ready\n"
        );
        assert_eq!(
            render(
                "case",
                "oncall",
                &json!({"action": "waiting", "for": "ic", "step": "page-ic", "because": ""})
            ),
            "Waiting for ic to report page-ic.\n"
        );
        assert_eq!(
            render("case", "oncall", &json!({"action": "done", "version": 1})),
            "Done. version 1\n"
        );
    }
}
