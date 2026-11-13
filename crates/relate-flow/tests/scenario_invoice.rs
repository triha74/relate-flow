#[path = "common/mod.rs"]
mod common;

use common::Store;
use serde_json::Value;

const INVOICE: &str =
    "Northwind, INV-1042, 100 widgets at $100, no discount stated, billed $10,000.";
const SHORT: &str =
    "NW-2024 gives 10% off this line. Agreed total $9,000, billed $10,000, shortfall $1,000.";

fn plan(store: &Store) {
    store.run(&["goal", "--text", "Pay INV-1042, or hand it to a person"]);
    store.run(&[
        "add",
        "parse",
        "--title",
        "Parse INV-1042",
        "--done-when",
        "Vendor, lines, stated discounts, and the billed total are recorded",
        "--produces",
        "invoice",
        "--actor",
        "invoices",
    ]);
    store.run(&[
        "add",
        "verify-vendor",
        "--title",
        "Verify Northwind is approved to bill",
        "--done-when",
        "Northwind is approved, or the reason it is not is recorded",
        "--needs",
        "parse",
        "--produces",
        "vendor",
        "--results",
        "approved",
        "--results",
        "rejected",
        "--actor",
        "invoices",
    ]);
    store.run(&[
        "add",
        "verify-pricing",
        "--title",
        "Match each line to NW-2024",
        "--done-when",
        "Every line matches an agreement price, or each miss is recorded",
        "--needs",
        "verify-vendor",
        "--when",
        "vendor=approved",
        "--produces",
        "pricing",
        "--results",
        "aligned",
        "--results",
        "misaligned",
        "--actor",
        "invoices",
    ]);
    store.run(&[
        "add",
        "verify-discounts",
        "--title",
        "Check the volume discount on INV-1042",
        "--done-when",
        "The 10% discount is confirmed, or the shortfall is recorded",
        "--needs",
        "parse",
        "--needs",
        "verify-pricing",
        "--when",
        "pricing=aligned",
        "--produces",
        "discounts",
        "--results",
        "correct",
        "--results",
        "incorrect",
        "--actor",
        "invoices",
    ]);
    store.run(&[
        "add",
        "escalate",
        "--title",
        "Open a human review for INV-1042",
        "--done-when",
        "A review is open and holds the invoice, NW-2024, and the mismatch",
        "--needs",
        "parse",
        "--when-any",
        "vendor=rejected",
        "--when-any",
        "pricing=misaligned",
        "--when-any",
        "discounts=incorrect",
        "--produces",
        "review",
        "--actor",
        "invoices",
    ]);
    store.run(&[
        "add",
        "human-review",
        "--title",
        "Decide whether INV-1042 may be paid",
        "--done-when",
        "A person approves a payment amount or rejects the invoice",
        "--needs",
        "escalate",
        "--produces",
        "decision",
        "--results",
        "approved",
        "--results",
        "rejected",
        "--actor",
        "human",
    ]);
    store.run(&[
        "add",
        "pay",
        "--title",
        "Pay the agreed amount for INV-1042",
        "--done-when",
        "Payment is submitted for the amount in the inputs and the reference is recorded",
        "--needs",
        "parse",
        "--when-any",
        "discounts=correct",
        "--when-any",
        "decision=approved",
        "--produces",
        "payment",
        "--actor",
        "invoices",
    ]);
    store.run(&[
        "add",
        "close-rejected",
        "--title",
        "Close INV-1042 unpaid",
        "--done-when",
        "The invoice is closed with the rejection reason",
        "--needs",
        "human-review",
        "--when",
        "decision=rejected",
        "--produces",
        "closure",
        "--actor",
        "invoices",
    ]);
    store.run(&["commit"]);
    store.execute();
}

fn succeed(store: &Store, step: &str, agent: &str, result: Option<&str>, note: &str) {
    let mut args = vec![
        "report",
        step,
        "--agent",
        agent,
        "--outcome",
        "succeeded",
        "--note",
        note,
    ];
    if let Some(result) = result {
        args.extend(["--result", result]);
    }
    store.run(&args);
}

fn take(store: &Store, agent: &str, step: &str) -> Value {
    let body = store.run(&["next", "--agent", agent]);
    assert_eq!(body["action"], "run", "{body}");
    assert_eq!(body["step"]["id"], step);
    body
}

fn disposition(store: &Store, id: &str) -> String {
    let status = store.run(&["status", "--json"]);
    status["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
        .unwrap()["disposition"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn missing_discount_is_approved_and_paid() {
    let store = Store::new();
    plan(&store);
    take(&store, "invoices", "parse");
    succeed(&store, "parse", "invoices", None, INVOICE);
    take(&store, "invoices", "verify-vendor");
    succeed(
        &store,
        "verify-vendor",
        "invoices",
        Some("approved"),
        "Northwind is approved.",
    );
    take(&store, "invoices", "verify-pricing");
    succeed(
        &store,
        "verify-pricing",
        "invoices",
        Some("aligned"),
        "100 widgets at $100 matches NW-2024.",
    );
    let discounts = take(&store, "invoices", "verify-discounts");
    assert_eq!(discounts["step"]["inputs"]["invoice"], INVOICE);
    let refused = store.fail(
        &[
            "report",
            "pay",
            "--agent",
            "invoices",
            "--outcome",
            "succeeded",
            "--note",
            "nope",
        ],
        4,
    );
    assert_eq!(refused["error"]["code"], "not_leased");
    succeed(
        &store,
        "verify-discounts",
        "invoices",
        Some("incorrect"),
        SHORT,
    );
    take(&store, "invoices", "escalate");
    succeed(
        &store,
        "escalate",
        "invoices",
        None,
        "Review REV-19 is open.",
    );
    let waiting = store.run(&["next", "--agent", "invoices"]);
    assert_eq!(waiting["action"], "waiting");
    assert_eq!(waiting["for"], "human");
    assert_eq!(waiting["because"], SHORT);
    let human = take(&store, "human", "human-review");
    assert_eq!(human["step"]["inputs"]["discounts"], SHORT);
    assert_eq!(human["step"]["inputs"]["review"], "Review REV-19 is open.");
    succeed(
        &store,
        "human-review",
        "human",
        Some("approved"),
        "Pay $9,000. Do not pay the billed $10,000.",
    );
    take(&store, "invoices", "pay");
    succeed(&store, "pay", "invoices", None, "Paid $9,000 as PAY-8891.");
    assert_eq!(
        store.run(&["next", "--agent", "invoices"])["action"],
        "done"
    );
    assert_eq!(disposition(&store, "close-rejected"), "skipped");
}

#[test]
fn a_correct_discount_skips_review() {
    let store = Store::new();
    plan(&store);
    take(&store, "invoices", "parse");
    succeed(&store, "parse", "invoices", None, INVOICE);
    take(&store, "invoices", "verify-vendor");
    succeed(
        &store,
        "verify-vendor",
        "invoices",
        Some("approved"),
        "approved",
    );
    take(&store, "invoices", "verify-pricing");
    succeed(
        &store,
        "verify-pricing",
        "invoices",
        Some("aligned"),
        "aligned",
    );
    take(&store, "invoices", "verify-discounts");
    succeed(
        &store,
        "verify-discounts",
        "invoices",
        Some("correct"),
        "The 10% discount is on the invoice.",
    );
    take(&store, "invoices", "pay");
    succeed(&store, "pay", "invoices", None, "Paid $9,000 as PAY-1.");
    assert_eq!(
        store.run(&["next", "--agent", "invoices"])["action"],
        "done"
    );
    assert_eq!(disposition(&store, "escalate"), "skipped");
}

#[test]
fn a_rejected_invoice_is_closed_unpaid() {
    let store = Store::new();
    plan(&store);
    take(&store, "invoices", "parse");
    succeed(&store, "parse", "invoices", None, INVOICE);
    take(&store, "invoices", "verify-vendor");
    succeed(
        &store,
        "verify-vendor",
        "invoices",
        Some("rejected"),
        "Northwind is not an approved vendor.",
    );
    take(&store, "invoices", "escalate");
    succeed(
        &store,
        "escalate",
        "invoices",
        None,
        "Review REV-20 is open.",
    );
    take(&store, "human", "human-review");
    succeed(
        &store,
        "human-review",
        "human",
        Some("rejected"),
        "Do not pay INV-1042.",
    );
    take(&store, "invoices", "close-rejected");
    succeed(&store, "close-rejected", "invoices", None, "Closed unpaid.");
    assert_eq!(
        store.run(&["next", "--agent", "invoices"])["action"],
        "done"
    );
    assert_eq!(disposition(&store, "pay"), "skipped");
}

#[test]
fn a_missing_agreement_blocks_until_the_plan_is_revised() {
    let store = Store::new();
    plan(&store);
    take(&store, "invoices", "parse");
    succeed(&store, "parse", "invoices", None, INVOICE);
    take(&store, "invoices", "verify-vendor");
    succeed(
        &store,
        "verify-vendor",
        "invoices",
        Some("approved"),
        "approved",
    );
    take(&store, "invoices", "verify-pricing");
    store.run(&[
        "report",
        "verify-pricing",
        "--agent",
        "invoices",
        "--outcome",
        "blocked",
        "--because",
        "NW-2024 is not on file",
    ]);
    let blocked = store.run(&["next", "--agent", "invoices"]);
    assert_eq!(blocked["action"], "blocked");
    assert_eq!(blocked["step"], "verify-pricing");
    store.run(&["revise"]);
    store.run(&[
        "set",
        "verify-pricing",
        "--done-when",
        "Every line matches NW-2024, which is now on file",
    ]);
    let committed = store.run(&["commit"]);
    assert!(committed["reopened"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "verify-pricing"));
    take(&store, "invoices", "verify-pricing");
    succeed(
        &store,
        "verify-pricing",
        "invoices",
        Some("aligned"),
        "100 widgets at $100 matches NW-2024.",
    );
    take(&store, "invoices", "verify-discounts");
    succeed(
        &store,
        "verify-discounts",
        "invoices",
        Some("correct"),
        "The 10% discount is on the invoice.",
    );
    take(&store, "invoices", "pay");
    succeed(&store, "pay", "invoices", None, "Paid $9,000 as PAY-2.");
    assert_eq!(
        store.run(&["next", "--agent", "invoices"])["action"],
        "done"
    );
}
