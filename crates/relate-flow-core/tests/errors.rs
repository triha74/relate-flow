//! One assertion per error code the library can return.

use std::time::Duration;

use relate::Database;
use relate_flow_core::{AddStep, Flow, FlowError, Outcome, Report, DEFAULT_LEASE};

fn flow() -> Flow {
    Flow::open(":memory:").unwrap().on("plan")
}

fn drive(flow: &Flow) -> Flow {
    flow.execute("run".into(), Vec::new()).unwrap();
    flow.on("run")
}

fn bare(id: &str) -> AddStep {
    AddStep {
        id: id.into(),
        title: id.into(),
        done_when: format!("{id} is recorded"),
        produces: None,
        results: vec![],
        needs: vec![],
        when: vec![],
        when_any: vec![],
        actor: None,
        before: None,
        after: None,
    }
}

fn must_err(result: Result<relate_flow_core::Flow, FlowError>) -> FlowError {
    match result {
        Ok(_) => panic!("expected the open to fail"),
        Err(err) => err,
    }
}

fn code(err: FlowError) -> String {
    assert!(!err.fix.is_empty(), "{}", err.message);
    err.code.to_string()
}

#[test]
fn usage_when_goal_has_no_flags_on_an_open_draft() {
    let flow = flow();
    flow.goal(Some("g".into()), None).unwrap();
    assert_eq!(code(flow.goal(None, None).unwrap_err()), "usage");
}

#[test]
fn store_refuses_a_path_that_is_a_file() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let err = must_err(Flow::open(file.path().to_str().unwrap()));
    assert_eq!(err.code, "store");
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn store_busy_when_another_process_holds_the_writer_lock() {
    let dir = tempfile::tempdir().unwrap();
    let _held = Flow::open(dir.path().to_str().unwrap()).unwrap();
    let err = must_err(Flow::open(dir.path().to_str().unwrap()));
    assert_eq!(err.code, "store_busy");
    assert!(err.retryable);
    assert_eq!(err.exit_code(), 5);
}

#[test]
fn foreign_store_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path().to_str().unwrap()).unwrap();
    let mut conn = db.connect().unwrap();
    conn.query("CREATE (:Other {name: 'kept'})").unwrap();
    drop(conn);
    drop(db);
    let flow = Flow::open(dir.path().to_str().unwrap()).unwrap().on("plan");
    assert_eq!(code(flow.show().unwrap_err()), "foreign_store");
}

#[test]
fn draft_and_plan_presence_codes() {
    let flow = flow();
    assert_eq!(code(flow.show().unwrap_err()), "no_plan");
    assert_eq!(code(flow.add(bare("parse")).unwrap_err()), "no_plan");
    flow.goal(Some("g".into()), None).unwrap();
    assert_eq!(
        code(flow.next("invoices".into(), DEFAULT_LEASE).unwrap_err()),
        "draft_open"
    );
    flow.add(bare("parse")).unwrap();
    flow.commit(false).unwrap();
    assert_eq!(
        code(flow.goal(Some("other".into()), None).unwrap_err()),
        "needs_revise"
    );
    assert_eq!(code(flow.add(bare("other")).unwrap_err()), "no_draft");
    assert_eq!(code(flow.commit(false).unwrap_err()), "no_draft");
}

#[test]
fn slug_empty_duplicate_and_unknown() {
    let flow = flow();
    flow.goal(Some("g".into()), None).unwrap();
    let mut bad = bare("No");
    bad.id = "No".into();
    assert_eq!(code(flow.add(bad).unwrap_err()), "bad_slug");
    let mut empty = bare("parse");
    empty.title = "  ".into();
    assert_eq!(code(flow.add(empty).unwrap_err()), "empty_field");
    flow.add(bare("parse")).unwrap();
    let mut again = bare("parse");
    again.title = "different".into();
    assert_eq!(code(flow.add(again).unwrap_err()), "step_differs");
    assert_eq!(
        code(flow.drop("missing".into()).unwrap_err()),
        "unknown_step"
    );
    let mut child = bare("child");
    child.needs = vec!["parse".into()];
    flow.add(child).unwrap();
    assert_eq!(
        code(flow.drop("parse".into()).unwrap_err()),
        "step_referenced"
    );
}

#[test]
fn results_produces_and_invalid_plan() {
    let flow = flow();
    flow.goal(Some("g".into()), None).unwrap();
    let mut bad = bare("parse");
    bad.results = vec!["approved".into()];
    assert_eq!(code(flow.add(bad).unwrap_err()), "results_need_produce");
    let mut a = bare("a");
    a.produces = Some("invoice".into());
    flow.add(a).unwrap();
    let mut b = bare("b");
    b.produces = Some("invoice".into());
    assert_eq!(code(flow.add(b).unwrap_err()), "duplicate_value");
    let mut c = bare("c");
    c.needs = vec!["missing".into()];
    flow.add(c).unwrap();
    let err = flow.commit(false).unwrap_err();
    assert_eq!(err.code, "invalid_plan");
    assert!(!err.errors.is_empty());
    assert_eq!(err.fix, err.errors[0].fix);
}

#[test]
fn report_refusals() {
    let flow = flow();
    flow.goal(Some("g".into()), None).unwrap();
    let mut parse = bare("parse");
    parse.produces = Some("invoice".into());
    parse.results = vec!["ok".into(), "bad".into()];
    parse.actor = Some("invoices".into());
    flow.add(parse).unwrap();
    flow.commit(false).unwrap();
    let flow = drive(&flow);
    let not_leased = flow
        .report(Report {
            step: "parse".into(),
            agent: "invoices".into(),
            outcome: Outcome::Succeeded,
            result: Some("ok".into()),
            note: Some("n".into()),
            because: None,
            artifact: None,
            artifact_hash: None,
        })
        .unwrap_err();
    assert_eq!(not_leased.code, "not_leased");
    flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(
        code(
            flow.report(Report {
                step: "parse".into(),
                agent: "invoices".into(),
                outcome: Outcome::Succeeded,
                result: Some("ok".into()),
                note: None,
                because: None,
                artifact: None,
                artifact_hash: None,
            })
            .unwrap_err()
        ),
        "missing_note"
    );
    assert_eq!(
        code(
            flow.report(Report {
                step: "parse".into(),
                agent: "invoices".into(),
                outcome: Outcome::Failed,
                result: None,
                note: None,
                because: None,
                artifact: None,
                artifact_hash: None,
            })
            .unwrap_err()
        ),
        "missing_because"
    );
    assert_eq!(
        code(
            flow.report(Report {
                step: "parse".into(),
                agent: "invoices".into(),
                outcome: Outcome::Succeeded,
                result: None,
                note: Some("n".into()),
                because: None,
                artifact: None,
                artifact_hash: None,
            })
            .unwrap_err()
        ),
        "missing_result"
    );
    assert_eq!(
        code(
            flow.report(Report {
                step: "parse".into(),
                agent: "invoices".into(),
                outcome: Outcome::Succeeded,
                result: Some("nope".into()),
                note: Some("n".into()),
                because: None,
                artifact: None,
                artifact_hash: None,
            })
            .unwrap_err()
        ),
        "bad_result"
    );
    assert_eq!(
        code(
            flow.report(Report {
                step: "parse".into(),
                agent: "invoices".into(),
                outcome: Outcome::Succeeded,
                result: Some("ok".into()),
                note: Some("n".into()),
                because: Some("x".into()),
                artifact: None,
                artifact_hash: None,
            })
            .unwrap_err()
        ),
        "unexpected_field"
    );
    assert_eq!(
        code(
            flow.report(Report {
                step: "parse".into(),
                agent: "invoices".into(),
                outcome: Outcome::Succeeded,
                result: Some("ok".into()),
                note: Some("n".into()),
                because: None,
                artifact: Some("/no/such/artifact".into()),
                artifact_hash: None,
            })
            .unwrap_err()
        ),
        "artifact_unreadable"
    );
    assert_eq!(
        code(
            flow.next("invoices".into(), Duration::from_secs(25 * 60 * 60))
                .unwrap_err()
        ),
        "lease_too_long"
    );
}

#[test]
fn a_different_outcome_on_a_closed_attempt_conflicts() {
    let flow = flow();
    flow.goal(Some("g".into()), None).unwrap();
    flow.add(bare("parse")).unwrap();
    flow.commit(false).unwrap();
    let flow = drive(&flow);
    flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    flow.report(Report {
        step: "parse".into(),
        agent: "invoices".into(),
        outcome: Outcome::Succeeded,
        result: None,
        note: Some("n".into()),
        because: None,
        artifact: None,
        artifact_hash: None,
    })
    .unwrap();
    let err = flow
        .report(Report {
            step: "parse".into(),
            agent: "invoices".into(),
            outcome: Outcome::Failed,
            result: None,
            note: None,
            because: Some("no".into()),
            artifact: None,
            artifact_hash: None,
        })
        .unwrap_err();
    assert_eq!(err.code, "conflict");
    assert!(!err.retryable);
    assert_eq!(err.exit_code(), 5);
    let same = flow
        .report(Report {
            step: "parse".into(),
            agent: "invoices".into(),
            outcome: Outcome::Succeeded,
            result: None,
            note: Some("n".into()),
            because: None,
            artifact: None,
            artifact_hash: None,
        })
        .unwrap();
    assert_eq!(same["outcome"], "succeeded");
}

#[test]
fn completed_work_cannot_be_dropped() {
    let flow = flow();
    flow.goal(Some("g".into()), None).unwrap();
    flow.add(bare("parse")).unwrap();
    flow.commit(false).unwrap();
    let flow = drive(&flow);
    flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    flow.report(Report {
        step: "parse".into(),
        agent: "invoices".into(),
        outcome: Outcome::Succeeded,
        result: None,
        note: Some("n".into()),
        because: None,
        artifact: None,
        artifact_hash: None,
    })
    .unwrap();
    flow.revise().unwrap();
    assert_eq!(
        code(flow.drop("parse".into()).unwrap_err()),
        "completed_work"
    );
}
