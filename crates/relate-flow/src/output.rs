use std::io::{self, IsTerminal};

use serde_json::Value;

#[cfg(test)]
static FORCE_TABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn emit(value: &Value) {
    println!("{}", serde_json::to_string(value).expect("json"));
}

pub fn json_mode(force: bool) -> bool {
    #[cfg(test)]
    if FORCE_TABLE.load(std::sync::atomic::Ordering::Relaxed) && !force {
        return false;
    }
    force || !io::stdout().is_terminal()
}

pub fn status_table(value: &Value) -> String {
    let mut out = String::new();
    if value.get("draft") == Some(&Value::Bool(true)) {
        out.push_str("draft\n");
    } else if let Some(version) = value.get("version") {
        out.push_str(&format!("version {version}\n"));
    }
    out.push_str(&format!(
        "{:<20} {:<12} {:<12} {}\n",
        "ID", "DISPOSITION", "ACTOR", "RESULT"
    ));
    for step in value["steps"].as_array().into_iter().flatten() {
        let id = step["id"].as_str().unwrap_or("");
        let disposition = step["disposition"].as_str().unwrap_or("");
        let actor = step["actor"].as_str().unwrap_or("");
        let result = step["result"].as_str().unwrap_or("");
        out.push_str(&format!(
            "{id:<20} {disposition:<12} {actor:<12} {result}\n"
        ));
    }
    out
}

pub fn log_table(value: &Value) -> String {
    let mut out = format!(
        "{:<8} {:<20} {:<12} {}\n",
        "ATTEMPT", "STEP", "OUTCOME", "AT"
    );
    for attempt in value["attempts"].as_array().into_iter().flatten() {
        let id = attempt["id"].as_str().unwrap_or("");
        let step = attempt["step"].as_str().unwrap_or("");
        let outcome = attempt["outcome"].as_str().unwrap_or("open");
        let at = attempt["finishedAt"]
            .as_str()
            .or(attempt["startedAt"].as_str())
            .unwrap_or("");
        out.push_str(&format!("{id:<8} {step:<20} {outcome:<12} {at}\n"));
    }
    out
}

pub fn print_status(value: &Value, as_json: bool) {
    if json_mode(as_json) {
        emit(value);
    } else {
        print!("{}", status_table(value));
    }
}

pub fn print_log(value: &Value, as_json: bool) {
    if json_mode(as_json) {
        emit(value);
    } else {
        print!("{}", log_table(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn status_table_lists_disposition_actor_and_result() {
        let text = status_table(&json!({
            "draft": true,
            "steps": [{
                "id": "parse",
                "disposition": "ready",
                "actor": "invoices",
                "result": "approved"
            }]
        }));
        assert!(text.starts_with("draft\n"));
        assert!(text.contains("ID"));
        assert!(text.contains("parse"));
        assert!(text.contains("ready"));
        assert!(text.contains("invoices"));
        assert!(text.contains("approved"));
    }

    #[test]
    fn log_table_lists_attempts_newest_shape() {
        let text = log_table(&json!({
            "attempts": [{
                "id": "a2",
                "step": "parse",
                "outcome": "failed",
                "startedAt": "2026-11-10T22:00:00Z"
            }]
        }));
        assert!(text.contains("ATTEMPT"));
        assert!(text.contains("a2"));
        assert!(text.contains("failed"));
        assert!(text.contains("2026-11-10T22:00:00Z"));
    }

    #[test]
    fn a_committed_status_table_prints_the_version() {
        let text = status_table(&json!({"version": 3, "steps": []}));
        assert!(text.starts_with("version 3\n"));
    }

    #[test]
    fn print_helpers_can_emit_the_table() {
        FORCE_TABLE.store(true, std::sync::atomic::Ordering::Relaxed);
        print_status(&json!({"version": 1, "steps": []}), false);
        print_log(&json!({"attempts": []}), false);
        FORCE_TABLE.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}
