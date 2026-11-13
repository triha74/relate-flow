use relate_flow_core::{AddStep, Flow, Outcome, Report, DEFAULT_LEASE};

fn drive(flow: &Flow) -> Flow {
    flow.execute("run".into(), Vec::new()).unwrap();
    flow.on("run")
}

fn add(id: &str, title: &str, done: &str) -> AddStep {
    AddStep {
        id: id.into(),
        title: title.into(),
        done_when: done.into(),
        produces: None,
        results: vec![],
        needs: vec![],
        when: vec![],
        when_any: vec![],
        actor: Some("invoices".into()),
        before: None,
        after: None,
    }
}

#[test]
fn goal_add_commit_next_report_round_trip() {
    let flow = Flow::open(":memory:").unwrap().on("plan");
    let goal = flow.goal(Some("Pay the invoice".into()), None).unwrap();
    assert_eq!(goal["retries"], 2);
    let mut spec = add("parse", "Parse", "recorded");
    spec.produces = Some("invoice".into());
    flow.add(spec).unwrap();
    let committed = flow.commit(false).unwrap();
    assert_eq!(committed["version"], 1);
    assert_eq!(committed["frontier"][0], "parse");
    let flow = drive(&flow);
    let next = flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(next["action"], "run");
    assert_eq!(next["step"]["id"], "parse");
    assert_eq!(next["step"]["attempt"], "a1");
    let again = flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(again["renewed"], true);
    assert_eq!(again["step"]["attempt"], "a1");
    let reported = flow
        .report(Report {
            step: "parse".into(),
            agent: "invoices".into(),
            outcome: Outcome::Succeeded,
            result: None,
            note: Some("Northwind".into()),
            because: None,
            artifact: None,
            artifact_hash: None,
        })
        .unwrap();
    assert_eq!(reported["outcome"], "succeeded");
    let done = flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(done["action"], "done");
    let shown = flow.show().unwrap();
    assert_eq!(shown["draft"], false);
    assert_eq!(shown["steps"][0]["id"], "parse");
}
