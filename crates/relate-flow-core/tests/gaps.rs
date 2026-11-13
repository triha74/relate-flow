//! Branches the scenario suites do not pass through.

use relate_flow_core::{
    parse_pair, AddStep, Flow, FlowError, Outcome, Report, SetStep, DEFAULT_LEASE,
};

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

fn code(err: FlowError) -> String {
    err.to_string();
    err.code.to_string()
}

#[test]
fn draft_edits_cover_the_flag_combinations() {
    let flow = Flow::open(":memory:").unwrap().on("plan");
    assert_eq!(code(flow.commit(false).unwrap_err()), "no_plan");
    flow.goal(Some("g".into()), None).unwrap();
    assert_eq!(
        code(flow.goal(Some("  ".into()), None).unwrap_err()),
        "empty_field"
    );

    let mut bad = bare("parse");
    bad.results = vec!["ok".into()];
    assert_eq!(code(flow.add(bad).unwrap_err()), "results_need_produce");
    let mut slug = bare("parse");
    slug.produces = Some("No".into());
    assert_eq!(code(flow.add(slug).unwrap_err()), "bad_slug");
    let mut actor = bare("parse");
    actor.actor = Some("No".into());
    assert_eq!(code(flow.add(actor).unwrap_err()), "bad_slug");
    let mut dup = bare("parse");
    dup.needs = vec!["parse".into(), "parse".into()];
    assert_eq!(code(flow.add(dup).unwrap_err()), "duplicate_value");

    flow.add(bare("a")).unwrap();
    flow.add(bare("b")).unwrap();
    let mut placed = bare("c");
    placed.after = Some("missing".into());
    assert_eq!(code(flow.add(placed).unwrap_err()), "unknown_step");
    let mut placed = bare("c");
    placed.after = Some("a".into());
    flow.add(placed).unwrap();
    let show = flow.show().unwrap();
    let ids: Vec<_> = show["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["a", "c", "b"]);
    assert!(show["steps"][0].get("when").is_none());

    assert_eq!(
        code(
            flow.set(SetStep {
                id: "a".into(),
                title: Some("  ".into()),
                ..SetStep::default()
            })
            .unwrap_err()
        ),
        "empty_field"
    );
    flow.set(SetStep {
        id: "a".into(),
        produces: Some("invoice".into()),
        results: Some(vec!["ok".into(), "bad".into()]),
        needs: Some(vec![]),
        actor: Some("invoices".into()),
        ..SetStep::default()
    })
    .unwrap();
    flow.set(SetStep {
        id: "c".into(),
        when: Some(vec![parse_pair("invoice=ok").unwrap()]),
        needs: Some(vec!["a".into()]),
        ..SetStep::default()
    })
    .unwrap();
    let shown = flow.show().unwrap();
    let step = |id: &str| {
        shown["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(step("c")["when"][0], "invoice=ok");
    assert_eq!(step("a")["results"][0], "ok");
    flow.set(SetStep {
        id: "c".into(),
        when_any: Some(vec![vec![parse_pair("invoice=bad").unwrap()]]),
        ..SetStep::default()
    })
    .unwrap();
    flow.set(SetStep {
        id: "c".into(),
        no_when: true,
        no_needs: true,
        no_actor: true,
        no_results: true,
        no_produces: true,
        ..SetStep::default()
    })
    .unwrap();
    assert_eq!(
        code(
            flow.set(SetStep {
                id: "b".into(),
                produces: Some("invoice".into()),
                ..SetStep::default()
            })
            .unwrap_err()
        ),
        "duplicate_value"
    );
    assert_eq!(
        code(
            flow.set(SetStep {
                id: "b".into(),
                actor: Some("No".into()),
                ..SetStep::default()
            })
            .unwrap_err()
        ),
        "bad_slug"
    );
    assert_eq!(
        code(
            flow.set(SetStep {
                id: "nowhere".into(),
                before: Some("missing".into()),
                ..SetStep::default()
            })
            .unwrap_err()
        ),
        "unknown_step"
    );
    let same = flow
        .set(SetStep {
            id: "b".into(),
            title: Some("b".into()),
            ..SetStep::default()
        })
        .unwrap();
    assert_eq!(same["step"], "b");
}

#[test]
fn a_revision_cancels_an_open_lease_and_reports_cover_the_outcome_table() {
    let flow = Flow::open(":memory:").unwrap().on("plan");
    flow.goal(Some("g".into()), None).unwrap();
    let mut first = bare("first");
    first.produces = Some("invoice".into());
    first.results = vec!["ok".into()];
    flow.add(first).unwrap();
    flow.add(bare("second")).unwrap();
    flow.commit(false).unwrap();
    let flow = drive(&flow);
    flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    flow.revise().unwrap();
    flow.drop("second".into()).unwrap();
    let committed = flow.commit(false).unwrap();
    assert!(committed["removed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "second"));
    assert!(committed["reopened"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "first"));
    let log = flow.log(None).unwrap();
    assert_eq!(log["attempts"][0]["outcome"], "released");
    assert_eq!(log["attempts"][0]["because"], "cancelled by revision");

    flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(
        code(
            flow.report(Report {
                step: "first".into(),
                agent: "worker".into(),
                outcome: Outcome::Released,
                result: None,
                note: Some("no".into()),
                because: None,
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
                step: "first".into(),
                agent: "worker".into(),
                outcome: Outcome::Expired,
                result: None,
                note: None,
                because: None,
                artifact: None,
                artifact_hash: None,
            })
            .unwrap_err()
        ),
        "usage"
    );
    assert_eq!(
        code(
            flow.report(Report {
                step: "first".into(),
                agent: "worker".into(),
                outcome: Outcome::Failed,
                result: Some("ok".into()),
                note: None,
                because: Some("no".into()),
                artifact: None,
                artifact_hash: None,
            })
            .unwrap_err()
        ),
        "unexpected_field"
    );
    flow.report(Report {
        step: "first".into(),
        agent: "worker".into(),
        outcome: Outcome::Succeeded,
        result: Some("ok".into()),
        note: Some("done".into()),
        because: None,
        artifact: None,
        artifact_hash: None,
    })
    .unwrap();
    let logged = flow.log(Some("first".into())).unwrap();
    assert_eq!(logged["attempts"][0]["result"], "ok");
    assert_eq!(
        code(flow.next("No".into(), DEFAULT_LEASE).unwrap_err()),
        "bad_slug"
    );
    let why = flow.explain("first".into()).unwrap();
    assert_eq!(why["disposition"], "succeeded");
}

#[test]
fn a_result_on_a_step_without_results_is_refused() {
    let flow = Flow::open(":memory:").unwrap().on("plan");
    flow.goal(Some("g".into()), None).unwrap();
    flow.add(bare("only")).unwrap();
    flow.commit(false).unwrap();
    let flow = drive(&flow);
    flow.next("worker".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(
        code(
            flow.report(Report {
                step: "only".into(),
                agent: "worker".into(),
                outcome: Outcome::Succeeded,
                result: Some("ok".into()),
                note: Some("done".into()),
                because: None,
                artifact: None,
                artifact_hash: None,
            })
            .unwrap_err()
        ),
        "unexpected_field"
    );
}
