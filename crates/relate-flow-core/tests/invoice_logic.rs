use relate_flow_core::{parse_pair, AddStep, Flow, Outcome, Report, DEFAULT_LEASE};

fn spec(id: &str, title: &str, done: &str) -> AddStep {
    AddStep {
        id: id.into(),
        title: title.into(),
        done_when: done.into(),
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

fn succeed(flow: &Flow, step: &str, agent: &str, result: Option<&str>, note: &str) {
    flow.report(Report {
        step: step.into(),
        agent: agent.into(),
        outcome: Outcome::Succeeded,
        result: result.map(str::to_string),
        note: Some(note.into()),
        because: None,
        artifact: None,
        artifact_hash: None,
    })
    .unwrap_or_else(|err| panic!("{step}: {err} {}", err.fix));
}

fn build(flow: &Flow) {
    flow.goal(Some("Pay INV-1042, or hand it to a person".into()), None)
        .unwrap();
    let mut parse = spec(
        "parse",
        "Parse INV-1042",
        "Vendor, lines, stated discounts, and the billed total are recorded",
    );
    parse.produces = Some("invoice".into());
    parse.actor = Some("invoices".into());
    flow.add(parse).unwrap();

    let mut vendor = spec(
        "verify-vendor",
        "Verify Northwind is approved to bill",
        "Northwind is approved, or the reason it is not is recorded",
    );
    vendor.needs = vec!["parse".into()];
    vendor.produces = Some("vendor".into());
    vendor.results = vec!["approved".into(), "rejected".into()];
    vendor.actor = Some("invoices".into());
    flow.add(vendor).unwrap();

    let mut pricing = spec(
        "verify-pricing",
        "Match each line to NW-2024",
        "Every line matches an agreement price, or each miss is recorded",
    );
    pricing.needs = vec!["verify-vendor".into()];
    pricing.when = vec![parse_pair("vendor=approved").unwrap()];
    pricing.produces = Some("pricing".into());
    pricing.results = vec!["aligned".into(), "misaligned".into()];
    pricing.actor = Some("invoices".into());
    flow.add(pricing).unwrap();

    let mut discounts = spec(
        "verify-discounts",
        "Check the volume discount on INV-1042",
        "The 10% discount is confirmed, or the shortfall is recorded",
    );
    discounts.needs = vec!["parse".into(), "verify-pricing".into()];
    discounts.when = vec![parse_pair("pricing=aligned").unwrap()];
    discounts.produces = Some("discounts".into());
    discounts.results = vec!["correct".into(), "incorrect".into()];
    discounts.actor = Some("invoices".into());
    flow.add(discounts).unwrap();

    let mut escalate = spec(
        "escalate",
        "Open a human review for INV-1042",
        "A review is open and holds the invoice, NW-2024, and the mismatch",
    );
    escalate.needs = vec!["parse".into()];
    escalate.when_any = vec![
        vec![parse_pair("vendor=rejected").unwrap()],
        vec![parse_pair("pricing=misaligned").unwrap()],
        vec![parse_pair("discounts=incorrect").unwrap()],
    ];
    escalate.produces = Some("review".into());
    escalate.actor = Some("invoices".into());
    flow.add(escalate).unwrap();

    let mut human = spec(
        "human-review",
        "Decide whether INV-1042 may be paid",
        "A person approves a payment amount or rejects the invoice",
    );
    human.needs = vec!["escalate".into()];
    human.produces = Some("decision".into());
    human.results = vec!["approved".into(), "rejected".into()];
    human.actor = Some("human".into());
    flow.add(human).unwrap();

    let mut pay = spec(
        "pay",
        "Pay the agreed amount for INV-1042",
        "Payment is submitted for the amount in the inputs and the reference is recorded",
    );
    pay.needs = vec!["parse".into()];
    pay.when_any = vec![
        vec![parse_pair("discounts=correct").unwrap()],
        vec![parse_pair("decision=approved").unwrap()],
    ];
    pay.produces = Some("payment".into());
    pay.actor = Some("invoices".into());
    flow.add(pay).unwrap();

    let mut close = spec(
        "close-rejected",
        "Close INV-1042 unpaid",
        "The invoice is closed with the rejection reason",
    );
    close.needs = vec!["human-review".into()];
    close.when = vec![parse_pair("decision=rejected").unwrap()];
    close.produces = Some("closure".into());
    close.actor = Some("invoices".into());
    flow.add(close).unwrap();
    let committed = flow.commit(false).unwrap();
    assert_eq!(committed["frontier"][0], "parse");
}

const INVOICE: &str =
    "Northwind, INV-1042, 100 widgets at $100, no discount stated, billed $10,000.";
const SHORT: &str =
    "NW-2024 gives 10% off this line. Agreed total $9,000, billed $10,000, shortfall $1,000.";

#[test]
fn missing_discount_is_approved_and_paid() {
    let flow = Flow::open(":memory:").unwrap().on("plan");
    build(&flow);
    let flow = drive(&flow);
    assert_eq!(
        flow.next("invoices".into(), DEFAULT_LEASE).unwrap()["step"]["id"],
        "parse"
    );
    succeed(&flow, "parse", "invoices", None, INVOICE);
    assert_eq!(
        flow.next("invoices".into(), DEFAULT_LEASE).unwrap()["step"]["id"],
        "verify-vendor"
    );
    succeed(
        &flow,
        "verify-vendor",
        "invoices",
        Some("approved"),
        "Northwind is approved.",
    );
    let pricing = flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(pricing["step"]["id"], "verify-pricing");
    succeed(
        &flow,
        "verify-pricing",
        "invoices",
        Some("aligned"),
        "100 widgets at $100 matches NW-2024.",
    );
    let discounts = flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(discounts["step"]["id"], "verify-discounts");
    assert_eq!(discounts["step"]["inputs"]["invoice"], INVOICE);
    assert_eq!(
        discounts["step"]["inputs"]["pricing"],
        "100 widgets at $100 matches NW-2024."
    );
    let refused = flow
        .report(Report {
            step: "pay".into(),
            agent: "invoices".into(),
            outcome: Outcome::Succeeded,
            result: None,
            note: Some("nope".into()),
            because: None,
            artifact: None,
            artifact_hash: None,
        })
        .unwrap_err();
    assert_eq!(refused.code, "not_leased");
    assert!(refused.fix.contains("verify-discounts"));
    succeed(
        &flow,
        "verify-discounts",
        "invoices",
        Some("incorrect"),
        SHORT,
    );
    assert_eq!(
        flow.next("invoices".into(), DEFAULT_LEASE).unwrap()["step"]["id"],
        "escalate"
    );
    succeed(
        &flow,
        "escalate",
        "invoices",
        None,
        "Review REV-19 is open.",
    );
    let waiting = flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(waiting["action"], "waiting");
    assert_eq!(waiting["for"], "human");
    assert_eq!(waiting["step"], "human-review");
    assert_eq!(waiting["because"], SHORT);
    let human = flow.next("human".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(human["step"]["id"], "human-review");
    assert_eq!(human["step"]["inputs"]["discounts"], SHORT);
    assert_eq!(human["step"]["inputs"]["review"], "Review REV-19 is open.");
    succeed(
        &flow,
        "human-review",
        "human",
        Some("approved"),
        "Pay $9,000. Do not pay the billed $10,000.",
    );
    let pay = flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(pay["step"]["id"], "pay");
    succeed(&flow, "pay", "invoices", None, "Paid $9,000 as PAY-8891.");
    let done = flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(done["action"], "done");
    let status = flow.status().unwrap();
    let close = status["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "close-rejected")
        .unwrap();
    assert_eq!(close["disposition"], "skipped");
}

#[test]
fn correct_discount_skips_the_review() {
    let flow = Flow::open(":memory:").unwrap().on("plan");
    build(&flow);
    let flow = drive(&flow);
    succeed_next(&flow, "parse", None, INVOICE);
    succeed_next(&flow, "verify-vendor", Some("approved"), "approved");
    succeed_next(&flow, "verify-pricing", Some("aligned"), "aligned");
    succeed_next(
        &flow,
        "verify-discounts",
        Some("correct"),
        "The 10% discount is on the invoice.",
    );
    let pay = flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(pay["step"]["id"], "pay");
    succeed(&flow, "pay", "invoices", None, "Paid $9,000 as PAY-1.");
    assert_eq!(
        flow.next("invoices".into(), DEFAULT_LEASE).unwrap()["action"],
        "done"
    );
}

fn succeed_next(flow: &Flow, step: &str, result: Option<&str>, note: &str) {
    let leased = flow.next("invoices".into(), DEFAULT_LEASE).unwrap();
    assert_eq!(leased["step"]["id"], step, "{leased}");
    succeed(flow, step, "invoices", result, note);
}
