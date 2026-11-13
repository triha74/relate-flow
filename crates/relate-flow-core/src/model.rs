use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Running,
    Succeeded,
    Failed,
    Blocked,
    Skipped,
}

impl StepState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Skipped => "skipped",
        }
    }

    pub fn parse(text: &str) -> Self {
        match text {
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "blocked" => Self::Blocked,
            "skipped" => Self::Skipped,
            _ => Self::Pending,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Skipped)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Succeeded,
    Failed,
    Blocked,
    Released,
    Expired,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Released => "released",
            Self::Expired => "expired",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "blocked" => Self::Blocked,
            "released" => Self::Released,
            "expired" => Self::Expired,
            _ => return None,
        })
    }
}

/// One `produce=token` pair. A `when` is a list of these (AND). A `whenAny`
/// alternative is also a list of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub produce: String,
    pub token: String,
}

impl Pair {
    pub fn encode(&self) -> String {
        format!("{}={}", self.produce, self.token)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub uid: String,
    pub id: String,
    pub title: String,
    pub done_when: String,
    pub produces: Option<String>,
    pub results: Vec<String>,
    pub needs: Vec<String>,
    pub when: Vec<Pair>,
    pub when_any: Vec<Vec<Pair>>,
    pub actor: Option<String>,
    pub state: StepState,
    pub position: i64,
}

impl Step {
    pub fn to_show(&self) -> Value {
        let mut obj = json!({
            "id": self.id,
            "title": self.title,
            "doneWhen": self.done_when,
        });
        if let Some(p) = &self.produces {
            obj["produces"] = json!(p);
        }
        if !self.results.is_empty() {
            obj["results"] = json!(self.results);
        }
        if !self.needs.is_empty() {
            obj["needs"] = json!(self.needs);
        }
        if !self.when.is_empty() {
            obj["when"] = json!(self.when.iter().map(Pair::encode).collect::<Vec<_>>());
        }
        if !self.when_any.is_empty() {
            obj["whenAny"] = json!(self
                .when_any
                .iter()
                .map(|alt| alt.iter().map(Pair::encode).collect::<Vec<_>>().join(","))
                .collect::<Vec<_>>());
        }
        if let Some(a) = &self.actor {
            obj["actor"] = json!(a);
        }
        obj
    }
}

/// A template is the reusable workflow. An execution is one run copied from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanKind {
    Template,
    Execution,
}

impl PlanKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Template => "template",
            Self::Execution => "execution",
        }
    }

    /// A plan written before kinds existed is an execution, so `next` still runs.
    pub fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("template") => Self::Template,
            _ => Self::Execution,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub uid: String,
    /// The workflow name. One store holds many names. A name is one lineage:
    /// its committed head and, when a revision is open, its draft.
    pub name: String,
    pub kind: PlanKind,
    /// Case values bound by `execute`, in the order they were given.
    pub inputs: Vec<(String, String)>,
    /// Template this execution was copied from.
    pub template: Option<String>,
    pub template_version: Option<u64>,
    pub version: Option<u64>,
    pub based_on: Option<u64>,
    pub goal: String,
    pub retries: u32,
    pub created_at: String,
    pub steps: Vec<Step>,
    /// Uid of the plan this one supersedes.
    pub supersedes: Option<String>,
}

impl Plan {
    pub fn is_draft(&self) -> bool {
        self.version.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    pub uid: String,
    pub id: String,
    pub step_uid: String,
    pub step_id: String,
    pub agent: String,
    pub outcome: Option<Outcome>,
    pub result: Option<String>,
    pub note: Option<String>,
    pub because: Option<String>,
    pub artifact: Option<String>,
    pub artifact_hash: Option<String>,
    pub lease_until: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

impl Attempt {
    pub fn is_open(&self) -> bool {
        self.outcome.is_none()
    }

    pub fn to_log(&self) -> Value {
        let mut obj = json!({
            "id": self.id,
            "step": self.step_id,
            "agent": self.agent,
            "startedAt": self.started_at,
        });
        if let Some(o) = self.outcome {
            obj["outcome"] = json!(o.as_str());
        }
        if let Some(r) = &self.result {
            obj["result"] = json!(r);
        }
        if let Some(n) = &self.note {
            obj["note"] = json!(n);
        }
        if let Some(b) = &self.because {
            obj["because"] = json!(b);
        }
        if let Some(a) = &self.artifact {
            obj["artifact"] = json!(a);
        }
        if let Some(h) = &self.artifact_hash {
            obj["artifactHash"] = json!(h);
        }
        if let Some(l) = &self.lease_until {
            obj["leaseUntil"] = json!(l);
        }
        if let Some(f) = &self.finished_at {
            obj["finishedAt"] = json!(f);
        }
        obj
    }
}

#[derive(Debug, Clone)]
pub enum Write {
    CreatePlan(Plan),
    UpdatePlan(Plan),
    DeletePlan {
        uid: String,
    },
    Supersedes {
        new_uid: String,
        old_uid: String,
    },
    UpsertStep {
        plan_uid: String,
        step: Step,
        is_new: bool,
        /// Rewrite `NEEDS` edges. State-only updates leave them alone so two
        /// claims on different steps do not touch the same relationships.
        sync_needs: bool,
    },
    DeleteStep {
        uid: String,
    },
    CreateAttempt(Attempt),
    UpdateAttempt(Attempt),
    MoveAttempt {
        attempt_uid: String,
        new_step_uid: String,
    },
}
