use relate_flow_core::{AddStep, Flow, DEFAULT_LEASE};

fn step(id: &str, produces: Option<&str>) -> AddStep {
    AddStep {
        id: id.into(),
        title: id.into(),
        done_when: format!("{id} is recorded"),
        produces: produces.map(str::to_string),
        results: produces.map(|_| vec!["sev1".into()]).unwrap_or_default(),
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
    flow.add(step("triage", Some("severity"))).unwrap();
    flow.add(step("mitigate", None)).unwrap();
    flow.commit(false).unwrap();
    flow
}

#[test]
fn two_executions_stay_separate_and_next_on_a_template_is_refused() {
    let flow = template();
    assert_eq!(
        flow.next("oncall".into(), DEFAULT_LEASE).unwrap_err().code,
        "not_a_run"
    );
    flow.execute(
        "inc-1042".into(),
        vec![("case".into(), "checkout down".into())],
    )
    .unwrap();
    flow.execute("inc-1043".into(), vec![("case".into(), "disk full".into())])
        .unwrap();
    let first = flow
        .on("inc-1042")
        .next("oncall".into(), DEFAULT_LEASE)
        .unwrap();
    let second = flow
        .on("inc-1043")
        .next("oncall".into(), DEFAULT_LEASE)
        .unwrap();
    assert_eq!(first["step"]["inputs"]["case"], "checkout down");
    assert_eq!(second["step"]["inputs"]["case"], "disk full");
    assert_eq!(flow.status().unwrap()["steps"][0]["disposition"], "ready");
}

#[test]
fn input_keys_names_and_executions_are_refused() {
    let flow = template();
    assert_eq!(
        flow.execute("bad".into(), vec![("severity".into(), "sev1".into())])
            .unwrap_err()
            .code,
        "input_conflicts"
    );
    flow.execute("inc-1042".into(), Vec::new()).unwrap();
    assert_eq!(
        flow.execute("inc-1042".into(), Vec::new())
            .unwrap_err()
            .code,
        "name_taken"
    );
    assert_eq!(
        flow.on("inc-1042")
            .execute("other".into(), Vec::new())
            .unwrap_err()
            .code,
        "not_a_template"
    );
}

#[test]
fn a_revision_and_a_later_template_commit_stay_on_their_own_plan() {
    let flow = template();
    flow.execute("inc-1042".into(), vec![("case".into(), "checkout".into())])
        .unwrap();
    let run = flow.on("inc-1042");
    run.next("oncall".into(), DEFAULT_LEASE).unwrap();
    run.revise().unwrap();
    let committed = run.commit(false).unwrap();
    assert_eq!(committed["version"], 2);
    assert_eq!(flow.show().unwrap()["kind"], "template");
    assert_eq!(flow.show().unwrap()["draft"], false);
    flow.revise().unwrap();
    flow.add(step("postmortem", None)).unwrap();
    flow.commit(false).unwrap();
    let shown = run.show().unwrap();
    let ids: Vec<_> = shown["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["id"].as_str().unwrap())
        .collect();
    assert!(!ids.contains(&"postmortem"));
    assert_eq!(shown["template"], "incident");
    assert_eq!(shown["inputs"]["case"], "checkout");
    assert_eq!(shown["kind"], "execution");
}
