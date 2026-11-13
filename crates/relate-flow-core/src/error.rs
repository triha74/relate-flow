use serde_json::{json, Value};

/// Every refusal the library can return. The CLI prints `to_json()` and exits
/// with [`FlowError::exit_code`].
#[derive(Debug, Clone)]
pub struct FlowError {
    pub code: &'static str,
    pub message: String,
    pub fix: String,
    pub retryable: bool,
    pub errors: Vec<Problem>,
    pub exit: u8,
}

#[derive(Debug, Clone)]
pub struct Problem {
    pub code: &'static str,
    pub message: String,
    pub fix: String,
    pub step: Option<String>,
}

impl FlowError {
    pub fn usage(message: impl Into<String>, fix: impl Into<String>) -> Self {
        Self::new("usage", message, fix, false, 2)
    }

    pub fn refusal(code: &'static str, message: impl Into<String>, fix: impl Into<String>) -> Self {
        Self::new(code, message, fix, false, 4)
    }

    pub fn conflict(message: impl Into<String>, fix: impl Into<String>, retryable: bool) -> Self {
        Self::new("conflict", message, fix, retryable, 5)
    }

    pub fn store(message: impl Into<String>) -> Self {
        Self::new(
            "store",
            message,
            "check the database path and retry",
            false,
            1,
        )
    }

    pub fn busy() -> Self {
        Self::new(
            "store_busy",
            "the store is locked by another process",
            "retry the command",
            true,
            5,
        )
    }

    pub fn with_problems(mut self, problems: Vec<Problem>) -> Self {
        if let Some(first) = problems.first() {
            self.fix = first.fix.clone();
            self.message = first.message.clone();
            self.code = first.code;
        }
        self.errors = problems;
        self
    }

    fn new(
        code: &'static str,
        message: impl Into<String>,
        fix: impl Into<String>,
        retryable: bool,
        exit: u8,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            fix: fix.into(),
            retryable,
            errors: Vec::new(),
            exit,
        }
    }

    pub fn exit_code(&self) -> i32 {
        i32::from(self.exit)
    }

    pub fn to_json(&self) -> Value {
        let mut body = json!({
            "code": self.code,
            "message": self.message,
            "fix": self.fix,
            "retryable": self.retryable,
        });
        if !self.errors.is_empty() {
            body["errors"] = Value::Array(
                self.errors
                    .iter()
                    .map(|p| {
                        let mut entry = json!({
                            "code": p.code,
                            "message": p.message,
                            "fix": p.fix,
                        });
                        if let Some(step) = &p.step {
                            entry["step"] = json!(step);
                        }
                        entry
                    })
                    .collect(),
            );
        }
        json!({ "error": body })
    }
}

impl std::fmt::Display for FlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for FlowError {}

/// Codes a client or the command-matrix test can rely on.
pub const ERROR_CODES: &[&str] = &[
    "usage",
    "store",
    "store_busy",
    "conflict",
    "foreign_store",
    "draft_open",
    "no_draft",
    "no_plan",
    "needs_revise",
    "bad_slug",
    "empty_field",
    "step_differs",
    "unknown_step",
    "step_referenced",
    "completed_work",
    "results_need_produce",
    "duplicate_value",
    "invalid_plan",
    "not_a_run",
    "not_a_template",
    "name_taken",
    "input_conflicts",
    "not_leased",
    "missing_note",
    "missing_because",
    "missing_result",
    "bad_result",
    "unexpected_field",
    "artifact_unreadable",
    "lease_too_long",
    "lease_open",
];
