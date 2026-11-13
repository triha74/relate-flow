//! Relate Flow admission library. Every rule in the product design is applied
//! here. The CLI is a stateless caller.

mod admission;
mod clock;
mod condition;
mod error;
mod explain;
mod model;
mod ops;
mod revision;
mod slug;
mod store;
mod validate;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use relate::{Database, Error as RelateError};
use serde_json::Value;

pub use clock::{Clock, FixedClock, ManualClock, SystemClock};
pub use condition::{parse_alternative, parse_pair};
pub use error::{FlowError, ERROR_CODES};
pub use model::{Outcome, Pair, StepState};
pub use ops::{hash_artifact, AddStep, Report, SetStep, DEFAULT_LEASE, MAX_LEASE};

/// One embedded store. Cheap to clone; connections are opened per command.
#[derive(Clone)]
pub struct Flow {
    db: Arc<Database>,
    clock: Arc<dyn Clock>,
    plan: String,
}

/// One named workflow in a store, for `relate-flow --help`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanInfo {
    pub name: String,
    pub goal: String,
    pub draft: bool,
    pub version: Option<u64>,
    /// `template` or `execution`.
    pub kind: String,
    /// Template name when this plan is an execution.
    pub template: Option<String>,
}

impl Flow {
    pub fn open(uri: &str) -> Result<Self, FlowError> {
        Ok(Self {
            db: Arc::new(open_database(uri)?),
            clock: Arc::new(SystemClock),
            plan: String::new(),
        })
    }

    /// Select the named workflow. Commands after this call read and write that lineage only.
    pub fn on(&self, plan: &str) -> Self {
        Self {
            db: Arc::clone(&self.db),
            clock: Arc::clone(&self.clock),
            plan: plan.to_string(),
        }
    }

    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Every named plan in the store, in name order.
    pub fn catalog(&self) -> Result<Vec<PlanInfo>, FlowError> {
        self.read(|state| Ok(catalog_of(state)))
    }

    pub fn goal(&self, text: Option<String>, retries: Option<u32>) -> Result<Value, FlowError> {
        let now = self.now();
        self.transact(move |state| ops::goal(state, now, text, retries))
    }

    pub fn add(&self, spec: AddStep) -> Result<Value, FlowError> {
        self.transact(move |state| ops::add(state, spec))
    }

    pub fn set(&self, spec: SetStep) -> Result<Value, FlowError> {
        self.transact(move |state| ops::set_step(state, spec))
    }

    pub fn drop(&self, id: String) -> Result<Value, FlowError> {
        self.transact(move |state| ops::drop_step(state, &id))
    }

    pub fn execute(&self, name: String, inputs: Vec<(String, String)>) -> Result<Value, FlowError> {
        let now = self.now();
        self.transact(move |state| ops::execute(state, &name, inputs, now))
    }

    pub fn commit(&self, dry_run: bool) -> Result<Value, FlowError> {
        let now = self.now();
        self.transact(move |state| ops::commit(state, dry_run, now))
    }

    pub fn revise(&self) -> Result<Value, FlowError> {
        let now = self.now();
        self.transact(move |state| ops::revise(state, now))
    }

    pub fn abort(&self) -> Result<Value, FlowError> {
        self.transact(ops::abort)
    }

    pub fn remove(&self, force: bool) -> Result<Value, FlowError> {
        self.transact(move |state| ops::remove(state, force))
    }

    pub fn prune(&self) -> Result<Value, FlowError> {
        self.transact(ops::prune)
    }

    pub fn show(&self) -> Result<Value, FlowError> {
        self.transact(|state| ops::show(state))
    }

    pub fn status(&self) -> Result<Value, FlowError> {
        self.transact(|state| ops::status(state))
    }

    pub fn explain(&self, id: String) -> Result<Value, FlowError> {
        self.transact(move |state| ops::explain_step(state, &id))
    }

    pub fn log(&self, step: Option<String>) -> Result<Value, FlowError> {
        self.transact(move |state| ops::log(state, step.as_deref()))
    }

    pub fn next(&self, agent: String, lease: Duration) -> Result<Value, FlowError> {
        let now = self.now();
        self.transact(move |state| ops::next_instruction(state, &agent, lease, now))
    }

    pub fn report(&self, mut report: Report) -> Result<Value, FlowError> {
        if let Some(path) = report.artifact.clone() {
            let (path, hash) = hash_artifact(&path)?;
            report.artifact = Some(path);
            report.artifact_hash = Some(hash);
        }
        let now = self.now();
        self.transact(move |state| ops::report(state, report, now))
    }

    fn read<T>(
        &self,
        f: impl FnOnce(&store::State) -> Result<T, FlowError>,
    ) -> Result<T, FlowError> {
        let mut conn = self.db.connect().map_err(store_err)?;
        conn.begin().map_err(store_err)?;
        let state = match store::load(&mut conn) {
            Ok(state) => state,
            Err(err) => {
                conn.rollback().ok();
                return Err(err);
            }
        };
        let result = f(&state);
        conn.rollback().ok();
        result
    }

    fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    fn transact<F>(&self, f: F) -> Result<Value, FlowError>
    where
        F: FnOnce(&mut store::State) -> Result<Value, FlowError>,
    {
        if !crate::slug::is_slug(&self.plan) {
            return Err(FlowError::refusal(
                "bad_slug",
                format!("'{}' is not a slug", self.plan),
                "name the plan with a lowercase letter, then lowercase letters, digits, and hyphens",
            ));
        }
        let mut conn = self.db.connect().map_err(store_err)?;
        conn.begin().map_err(store_err)?;
        let mut state = match store::load(&mut conn) {
            Ok(state) => state,
            Err(err) => {
                conn.rollback().ok();
                return Err(err);
            }
        };
        state.focus(&self.plan);
        match f(&mut state) {
            Ok(value) => match store::apply(&mut conn, &state.writes) {
                Ok(()) => match conn.commit() {
                    Ok(_) => Ok(value),
                    Err(err) if is_conflict(&err) => Err(FlowError::conflict(
                        "the store changed while this command ran",
                        "call next again",
                        true,
                    )),
                    Err(err) => Err(store_err(err)),
                },
                Err(err) => {
                    conn.rollback().ok();
                    Err(err)
                }
            },
            Err(err) => {
                conn.rollback().ok();
                Err(err)
            }
        }
    }
}

fn catalog_of(state: &store::State) -> Vec<PlanInfo> {
    let mut names: Vec<&str> = state
        .plans
        .iter()
        .map(|plan| plan.name.as_str())
        .filter(|name| !name.is_empty())
        .collect();
    names.sort_unstable();
    names.dedup();
    names
        .into_iter()
        .map(|name| {
            let lineage: Vec<&crate::model::Plan> = state
                .plans
                .iter()
                .filter(|plan| plan.name == name)
                .collect();
            let draft = lineage.iter().any(|plan| plan.is_draft());
            let head = lineage
                .iter()
                .filter(|plan| plan.version.is_some())
                .max_by_key(|plan| plan.version.unwrap_or(0));
            let shown = lineage
                .iter()
                .copied()
                .find(|plan| plan.is_draft())
                .or(head.copied());
            PlanInfo {
                name: name.to_string(),
                goal: shown.map(|plan| plan.goal.clone()).unwrap_or_default(),
                draft,
                version: head.and_then(|plan| plan.version),
                kind: shown
                    .map(|plan| plan.kind.as_str().to_string())
                    .unwrap_or_else(|| "template".into()),
                template: shown.and_then(|plan| plan.template.clone()),
            }
        })
        .collect()
}

fn open_database(uri: &str) -> Result<Database, FlowError> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match Database::open(uri) {
            Ok(db) => return Ok(db),
            Err(RelateError::Locked(_)) => {
                if Instant::now() >= deadline {
                    return Err(FlowError::busy());
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => return Err(store_err(err)),
        }
    }
}

fn is_conflict(err: &RelateError) -> bool {
    matches!(err, RelateError::Conflict)
}

fn store_err(err: RelateError) -> FlowError {
    if matches!(err, RelateError::Locked(_)) {
        FlowError::busy()
    } else {
        FlowError::store(err.to_string())
    }
}
