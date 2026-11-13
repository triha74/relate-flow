use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use relate_flow_core::{AddStep, Flow, ManualClock, Outcome, Report, SetStep, DEFAULT_LEASE};

fn flow_at(time: &str) -> (Flow, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(
        DateTime::parse_from_rfc3339(time)
            .unwrap()
            .with_timezone(&Utc),
    ));
    let flow = Flow::open(":memory:")
        .unwrap()
        .on("plan")
        .with_clock(Arc::clone(&clock) as Arc<dyn relate_flow_core::Clock>);
    (flow, clock)
}

fn bare(id: &str) -> AddStep {
    AddStep {
        id: id.into(),
        title: id.into(),
        done_when: format!("{id} is done"),
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

fn drive(flow: &Flow) -> Flow {
    flow.execute("run".into(), Vec::new()).unwrap();
    flow.on("run")
}

fn report(flow: &Flow, step: &str, outcome: Outcome, note: Option<&str>, because: Option<&str>) {
    flow.report(Report {
        step: step.into(),
        agent: "worker".into(),
        outcome,
        result: None,
        note: note.map(str::to_string),
        because: because.map(str::to_string),
        artifact: None,
        artifact_hash: None,
    })
    .unwrap_or_else(|err| panic!("{err} {}", err.fix));
}

#[test]
fn an_expired_lease_returns_the_step_to_pending() {
    let (flow, clock) = flow_at("2026-11-12T12:00:00Z");
    flow.goal(Some("one step".into()), None).unwrap();
    flow.add(bare("only")).unwrap();
    flow.commit(false).unwrap();
    let flow = drive(&flow);
    let first = flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(first["step"]["attempt"], "a1");
    assert_eq!(first["step"]["leaseUntil"], "2026-11-12T12:15:00Z");
    let renewed = flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(renewed["renewed"], true);
    assert_eq!(renewed["step"]["attempt"], "a1");
    clock.advance(Duration::from_secs(16 * 60));
    let again = flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(again["step"]["id"], "only");
    assert_eq!(again["step"]["attempt"], "a2");
    let log = flow.log(None).unwrap();
    let expired = log["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "a1")
        .unwrap();
    assert_eq!(expired["outcome"], "expired");
}

#[test]
fn released_does_not_spend_the_failure_budget() {
    let flow = Flow::open(":memory:").unwrap().on("plan");
    flow.goal(Some("g".into()), Some(2)).unwrap();
    flow.add(bare("only")).unwrap();
    flow.commit(false).unwrap();
    let flow = drive(&flow);
    flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    report(&flow, "only", Outcome::Released, None, None);
    let again = flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(again["action"], "run");
    assert!(again["step"].get("retry").is_none());
    report(&flow, "only", Outcome::Failed, None, Some("disk full"));
    let retry = flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(retry["step"]["retry"], 1);
    report(&flow, "only", Outcome::Failed, None, Some("still full"));
    let replan = flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(replan["action"], "replan");
    assert!(replan["because"].as_str().unwrap().contains("failed"));
}

#[test]
fn blocked_stops_the_plan_until_a_revision() {
    let flow = Flow::open(":memory:").unwrap().on("plan");
    flow.goal(Some("g".into()), None).unwrap();
    flow.add(bare("only")).unwrap();
    flow.commit(false).unwrap();
    let flow = drive(&flow);
    flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    report(
        &flow,
        "only",
        Outcome::Blocked,
        None,
        Some("the agreement is missing"),
    );
    let blocked = flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(blocked["action"], "blocked");
    assert_eq!(blocked["because"], "the agreement is missing");
    flow.revise().unwrap();
    flow.set(SetStep {
        id: "only".into(),
        title: Some("Only, with the agreement".into()),
        done_when: Some("the agreement is attached and the step is done".into()),
        ..SetStep::default()
    })
    .unwrap();
    let committed = flow.commit(false).unwrap();
    assert!(committed["reopened"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "only"));
    let run = flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(run["action"], "run");
    report(
        &flow,
        "only",
        Outcome::Succeeded,
        Some("agreement attached"),
        None,
    );
    assert_eq!(
        flow.next("worker".into(), DEFAULT_LEASE).unwrap()["action"],
        "done"
    );
}

#[test]
fn revision_keeps_succeeded_work_and_a_dry_run_writes_nothing() {
    let flow = Flow::open(":memory:").unwrap().on("plan");
    flow.goal(Some("g".into()), None).unwrap();
    flow.add(bare("keep")).unwrap();
    flow.add(bare("change")).unwrap();
    flow.commit(false).unwrap();
    let flow = drive(&flow);
    flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    report(&flow, "keep", Outcome::Succeeded, Some("kept"), None);
    flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    report(&flow, "change", Outcome::Succeeded, Some("old done"), None);
    flow.revise().unwrap();
    flow.set(SetStep {
        id: "change".into(),
        done_when: Some("a different completion".into()),
        ..SetStep::default()
    })
    .unwrap();
    let extra = bare("extra");
    flow.add(extra).unwrap();
    let preview = flow.commit(true).unwrap();
    assert_eq!(preview["dryRun"], true);
    assert!(preview["kept"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "keep"));
    assert!(preview["reopened"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "change"));
    assert!(preview["added"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "extra"));
    let show = flow.show().unwrap();
    assert_eq!(show["draft"], true);
    let committed = flow.commit(false).unwrap();
    assert_eq!(committed["version"], 2);
    let status = flow.status().unwrap();
    let keep = status["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "keep")
        .unwrap();
    assert_eq!(keep["disposition"], "succeeded");
    flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    report(&flow, "change", Outcome::Succeeded, Some("redone"), None);
    let log = flow.log(Some("keep".into())).unwrap();
    assert_eq!(log["attempts"][0]["note"], "kept");
    assert_eq!(flow.explain("extra".into()).unwrap()["step"], "extra");
}

#[test]
fn artifact_hash_is_recorded_and_abort_drops_the_draft() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("proof.txt");
    std::fs::write(&path, b"paid").unwrap();
    let flow = Flow::open(":memory:").unwrap().on("plan");
    flow.goal(Some("g".into()), None).unwrap();
    flow.add(bare("only")).unwrap();
    flow.abort().unwrap();
    assert_eq!(flow.show().unwrap_err().code, "no_plan");
    flow.goal(Some("g".into()), None).unwrap();
    flow.add(bare("only")).unwrap();
    flow.commit(false).unwrap();
    let flow = drive(&flow);
    flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    let body = flow
        .report(Report {
            step: "only".into(),
            agent: "worker".into(),
            outcome: Outcome::Succeeded,
            result: None,
            note: Some("done".into()),
            because: None,
            artifact: Some(path.display().to_string()),
            artifact_hash: None,
        })
        .unwrap();
    assert_eq!(body["outcome"], "succeeded");
    let log = flow.log(None).unwrap();
    let hash = log["attempts"][0]["artifactHash"].as_str().unwrap();
    assert_eq!(hash.len(), 64);
}

#[test]
fn set_moves_a_step_and_drop_removes_an_unreferenced_one() {
    let flow = Flow::open(":memory:").unwrap().on("plan");
    flow.goal(Some("g".into()), None).unwrap();
    flow.add(bare("a")).unwrap();
    flow.add(bare("b")).unwrap();
    flow.add(bare("c")).unwrap();
    flow.set(SetStep {
        id: "c".into(),
        before: Some("b".into()),
        ..SetStep::default()
    })
    .unwrap();
    let show = flow.show().unwrap();
    let ids: Vec<&str> = show["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["a", "c", "b"]);
    flow.drop("b".into()).unwrap();
    let show = flow.show().unwrap();
    assert_eq!(show["steps"].as_array().unwrap().len(), 2);
}
