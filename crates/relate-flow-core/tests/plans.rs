use relate_flow_core::Flow;

#[test]
fn two_plans_in_one_store_stay_separate() {
    let flow = Flow::open(":memory:").unwrap();
    flow.on("alpha").goal(Some("Alpha".into()), None).unwrap();
    flow.on("beta").goal(Some("Beta".into()), None).unwrap();
    let alpha = flow.on("alpha").show().unwrap();
    let beta = flow.on("beta").show().unwrap();
    assert_eq!(alpha["plan"], "alpha");
    assert_eq!(alpha["goal"], "Alpha");
    assert_eq!(beta["plan"], "beta");
    assert_eq!(beta["goal"], "Beta");
    let names: Vec<_> = flow
        .catalog()
        .unwrap()
        .into_iter()
        .map(|plan| plan.name)
        .collect();
    assert_eq!(names, ["alpha", "beta"]);
}

#[test]
fn a_plan_name_must_be_a_slug() {
    let flow = Flow::open(":memory:").unwrap();
    let err = flow.on("Invoice").goal(Some("g".into()), None).unwrap_err();
    assert_eq!(err.code, "bad_slug");
}
