#[path = "common/mod.rs"]
mod common;

use common::Store;

fn plan(store: &Store) {
    store.run(&[
        "goal",
        "--text",
        "Open the account, or decline the application",
    ]);
    store.run(&[
        "add",
        "collect-docs",
        "--title",
        "Collect the application",
        "--done-when",
        "The application documents are on file",
        "--produces",
        "application",
        "--actor",
        "intake",
    ]);
    store.run(&[
        "add",
        "verify-identity",
        "--title",
        "Verify identity",
        "--done-when",
        "Identity is verified, or the mismatch is recorded",
        "--needs",
        "collect-docs",
        "--produces",
        "identity",
        "--results",
        "verified",
        "--results",
        "mismatch",
        "--actor",
        "kyc",
    ]);
    store.run(&[
        "add",
        "sanctions-screen",
        "--title",
        "Screen sanctions",
        "--done-when",
        "The screening result is recorded",
        "--needs",
        "collect-docs",
        "--produces",
        "sanctions",
        "--results",
        "clear",
        "--results",
        "hit",
        "--actor",
        "screening",
    ]);
    store.run(&[
        "add",
        "risk-score",
        "--title",
        "Score the risk",
        "--done-when",
        "Risk is low or high",
        "--needs",
        "verify-identity",
        "--needs",
        "sanctions-screen",
        "--when",
        "identity=verified",
        "--when",
        "sanctions=clear",
        "--produces",
        "risk",
        "--results",
        "low",
        "--results",
        "high",
        "--actor",
        "kyc",
    ]);
    store.run(&[
        "add",
        "compliance-review",
        "--title",
        "Compliance review",
        "--done-when",
        "Compliance approves or declines",
        "--needs",
        "collect-docs",
        "--when-any",
        "identity=mismatch",
        "--when-any",
        "sanctions=hit",
        "--when-any",
        "risk=high",
        "--produces",
        "ruling",
        "--results",
        "approved",
        "--results",
        "declined",
        "--actor",
        "compliance",
    ]);
    store.run(&[
        "add",
        "open-account",
        "--title",
        "Open the account",
        "--done-when",
        "The account number is recorded",
        "--needs",
        "collect-docs",
        "--when-any",
        "risk=low",
        "--when-any",
        "ruling=approved",
        "--produces",
        "account",
        "--actor",
        "intake",
    ]);
    store.run(&[
        "add",
        "decline",
        "--title",
        "Decline the application",
        "--done-when",
        "The applicant is told and the file is closed",
        "--needs",
        "collect-docs",
        "--when",
        "ruling=declined",
        "--produces",
        "closure",
        "--actor",
        "intake",
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

fn take(store: &Store, agent: &str, step: &str) {
    let body = store.run(&["next", "--agent", agent]);
    assert_eq!(body["step"]["id"], step, "{body}");
}

fn disposition(store: &Store, id: &str) -> String {
    store.run(&["status", "--json"])["steps"]
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
fn a_clear_application_opens_the_account() {
    let store = Store::new();
    plan(&store);
    take(&store, "intake", "collect-docs");
    succeed(&store, "collect-docs", "intake", None, "Application APP-1.");
    take(&store, "kyc", "verify-identity");
    succeed(
        &store,
        "verify-identity",
        "kyc",
        Some("verified"),
        "Identity matches.",
    );
    take(&store, "screening", "sanctions-screen");
    succeed(
        &store,
        "sanctions-screen",
        "screening",
        Some("clear"),
        "No hit.",
    );
    take(&store, "kyc", "risk-score");
    succeed(&store, "risk-score", "kyc", Some("low"), "Low risk.");
    take(&store, "intake", "open-account");
    succeed(
        &store,
        "open-account",
        "intake",
        None,
        "Account ACC-9 opened.",
    );
    assert_eq!(store.run(&["next", "--agent", "intake"])["action"], "done");
    assert_eq!(disposition(&store, "compliance-review"), "skipped");
    assert_eq!(disposition(&store, "decline"), "skipped");
}

#[test]
fn a_mismatch_can_still_be_approved() {
    let store = Store::new();
    plan(&store);
    take(&store, "intake", "collect-docs");
    succeed(&store, "collect-docs", "intake", None, "Application APP-2.");
    take(&store, "kyc", "verify-identity");
    succeed(
        &store,
        "verify-identity",
        "kyc",
        Some("mismatch"),
        "Name does not match the document.",
    );
    take(&store, "screening", "sanctions-screen");
    succeed(
        &store,
        "sanctions-screen",
        "screening",
        Some("clear"),
        "No hit.",
    );
    assert_eq!(disposition(&store, "risk-score"), "skipped");
    take(&store, "compliance", "compliance-review");
    succeed(
        &store,
        "compliance-review",
        "compliance",
        Some("approved"),
        "Approved with the corrected name.",
    );
    take(&store, "intake", "open-account");
    succeed(
        &store,
        "open-account",
        "intake",
        None,
        "Account ACC-10 opened.",
    );
    assert_eq!(store.run(&["next", "--agent", "intake"])["action"], "done");
    assert_eq!(disposition(&store, "decline"), "skipped");
}

#[test]
fn a_sanctions_hit_is_declined() {
    let store = Store::new();
    plan(&store);
    take(&store, "intake", "collect-docs");
    succeed(&store, "collect-docs", "intake", None, "Application APP-3.");
    take(&store, "kyc", "verify-identity");
    succeed(
        &store,
        "verify-identity",
        "kyc",
        Some("verified"),
        "Identity matches.",
    );
    take(&store, "screening", "sanctions-screen");
    succeed(
        &store,
        "sanctions-screen",
        "screening",
        Some("hit"),
        "Sanctions hit.",
    );
    take(&store, "compliance", "compliance-review");
    succeed(
        &store,
        "compliance-review",
        "compliance",
        Some("declined"),
        "Decline the application.",
    );
    take(&store, "intake", "decline");
    succeed(&store, "decline", "intake", None, "Applicant notified.");
    assert_eq!(store.run(&["next", "--agent", "intake"])["action"], "done");
    assert_eq!(disposition(&store, "open-account"), "skipped");
    assert_eq!(disposition(&store, "risk-score"), "skipped");
}

#[test]
fn a_blocked_identity_check_reopens_on_revision() {
    let store = Store::new();
    plan(&store);
    take(&store, "intake", "collect-docs");
    succeed(&store, "collect-docs", "intake", None, "Application APP-4.");
    take(&store, "kyc", "verify-identity");
    store.run(&[
        "report",
        "verify-identity",
        "--agent",
        "kyc",
        "--outcome",
        "blocked",
        "--because",
        "the document image is unreadable",
    ]);
    take(&store, "screening", "sanctions-screen");
    succeed(
        &store,
        "sanctions-screen",
        "screening",
        Some("clear"),
        "No hit.",
    );
    let blocked = store.run(&["next", "--agent", "kyc"]);
    assert_eq!(blocked["action"], "blocked");
    assert_eq!(blocked["because"], "the document image is unreadable");
    store.run(&["revise"]);
    let committed = store.run(&["commit"]);
    assert!(committed["kept"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "sanctions-screen"));
    take(&store, "kyc", "verify-identity");
    succeed(
        &store,
        "verify-identity",
        "kyc",
        Some("verified"),
        "A new image matches.",
    );
    take(&store, "kyc", "risk-score");
    succeed(&store, "risk-score", "kyc", Some("low"), "Low risk.");
    take(&store, "intake", "open-account");
    succeed(
        &store,
        "open-account",
        "intake",
        None,
        "Account ACC-11 opened.",
    );
    assert_eq!(store.run(&["next", "--agent", "intake"])["action"], "done");
}

#[test]
fn changing_done_when_on_a_success_reopens_it() {
    let store = Store::new();
    plan(&store);
    take(&store, "intake", "collect-docs");
    succeed(&store, "collect-docs", "intake", None, "Application APP-5.");
    store.run(&["revise"]);
    let updated = store.run(&[
        "set",
        "collect-docs",
        "--done-when",
        "The application and a proof of address are on file",
    ]);
    assert_eq!(updated["reopens"], true);
    let committed = store.run(&["commit"]);
    assert!(committed["reopened"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "collect-docs"));
    take(&store, "intake", "collect-docs");
    succeed(
        &store,
        "collect-docs",
        "intake",
        None,
        "Proof of address added.",
    );
    take(&store, "kyc", "verify-identity");
    succeed(
        &store,
        "verify-identity",
        "kyc",
        Some("verified"),
        "Identity matches.",
    );
    take(&store, "screening", "sanctions-screen");
    succeed(
        &store,
        "sanctions-screen",
        "screening",
        Some("clear"),
        "No hit.",
    );
    take(&store, "kyc", "risk-score");
    succeed(&store, "risk-score", "kyc", Some("low"), "Low risk.");
    take(&store, "intake", "open-account");
    succeed(
        &store,
        "open-account",
        "intake",
        None,
        "Account ACC-12 opened.",
    );
    assert_eq!(store.run(&["next", "--agent", "intake"])["action"], "done");
}
