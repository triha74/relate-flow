use relate_flow_core::{AddStep, Flow, Outcome, Report, DEFAULT_LEASE};

fn step(id: &str) -> AddStep {
    AddStep {
        id: id.into(),
        title: id.into(),
        done_when: format!("{id} is recorded"),
        produces: None,
        results: vec![],
        needs: vec![],
        when: vec![],
        when_any: vec![],
        actor: Some("oncall".into()),
        before: None,
        after: None,
    }
}

fn template() -> Flow {
    let flow = Flow::open(":memory:").unwrap().on("incident");
    flow.goal(Some("Restore the service, or escalate it".into()), None)
        .unwrap();
    flow.add(step("triage")).unwrap();
    flow.commit(false).unwrap();
    flow
}

fn succeed(flow: &Flow) {
    let next = flow.next("oncall".into(), DEFAULT_LEASE).unwrap();
    flow.report(Report {
        step: next["step"]["id"].as_str().unwrap().to_string(),
        agent: "oncall".into(),
        outcome: Outcome::Succeeded,
        result: None,
        note: Some("recorded".into()),
        because: None,
        artifact: None,
        artifact_hash: None,
    })
    .unwrap();
}

#[test]
fn remove_frees_the_name_and_leaves_the_other_plan() {
    let flow = template();
    flow.execute("inc-1042".into(), vec![("case".into(), "down".into())])
        .unwrap();
    let removed = flow.on("inc-1042").remove(false).unwrap();
    assert_eq!(removed["removed"], "inc-1042");
    assert_eq!(removed["versions"], 1);
    assert_eq!(removed["attempts"], 0);
    flow.execute("inc-1042".into(), vec![("case".into(), "again".into())])
        .unwrap();
    let gone = flow.remove(false).unwrap();
    assert_eq!(gone["removed"], "incident");
    assert_eq!(flow.on("inc-1042").show().unwrap()["kind"], "execution");
    assert_eq!(flow.show().unwrap_err().code, "no_plan");
}

#[test]
fn an_open_lease_refuses_remove_until_forced() {
    let flow = template();
    flow.execute("inc-1042".into(), Vec::new()).unwrap();
    let run = flow.on("inc-1042");
    run.next("oncall".into(), DEFAULT_LEASE).unwrap();
    let err = run.remove(false).unwrap_err();
    assert_eq!(err.code, "lease_open");
    assert!(err.fix.contains("--force"));
    let removed = run.remove(true).unwrap();
    assert_eq!(removed["removed"], "inc-1042");
    assert_eq!(removed["attempts"], 1);
    assert_eq!(run.show().unwrap_err().code, "no_plan");
}

#[test]
fn prune_drops_a_finished_execution_and_keeps_a_live_one() {
    let flow = template();
    flow.execute("inc-1042".into(), Vec::new()).unwrap();
    flow.execute("inc-1043".into(), Vec::new()).unwrap();
    succeed(&flow.on("inc-1042"));
    flow.on("inc-1043")
        .next("oncall".into(), DEFAULT_LEASE)
        .unwrap();
    let pruned = flow.prune().unwrap();
    assert_eq!(pruned["pruned"], serde_json::json!(["inc-1042"]));
    assert_eq!(flow.show().unwrap()["kind"], "template");
    assert_eq!(
        flow.on("inc-1043").status().unwrap()["steps"][0]["disposition"],
        "running"
    );
    assert_eq!(flow.on("inc-1042").show().unwrap_err().code, "no_plan");
    assert_eq!(
        flow.on("inc-1043").prune().unwrap_err().code,
        "not_a_template"
    );
}

#[test]
fn prune_keeps_an_execution_with_an_open_draft() {
    let flow = template();
    flow.execute("inc-1042".into(), Vec::new()).unwrap();
    let run = flow.on("inc-1042");
    succeed(&run);
    run.revise().unwrap();
    assert_eq!(flow.prune().unwrap()["pruned"], serde_json::json!([]));
    assert_eq!(run.show().unwrap()["draft"], true);
}
