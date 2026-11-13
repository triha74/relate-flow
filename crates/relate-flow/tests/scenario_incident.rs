#[path = "common/mod.rs"]
mod common;

use common::Store;

fn plan(store: &Store) {
    store.run(&["goal", "--text", "Restore the service, or escalate it"]);
    store.run(&[
        "add",
        "triage",
        "--title",
        "Classify the incident",
        "--done-when",
        "Severity is recorded",
        "--produces",
        "severity",
        "--results",
        "sev1",
        "--results",
        "sev2",
        "--results",
        "sev3",
        "--actor",
        "oncall",
    ]);
    store.run(&[
        "add",
        "page-ic",
        "--title",
        "Page the incident commander",
        "--done-when",
        "The incident commander is paging",
        "--needs",
        "triage",
        "--when",
        "severity=sev1",
        "--produces",
        "page",
        "--actor",
        "ic",
    ]);
    store.run(&[
        "add",
        "mitigate",
        "--title",
        "Mitigate the impact",
        "--done-when",
        "The impact is resolved, or it is recorded as unresolved",
        "--needs",
        "triage",
        "--produces",
        "mitigation",
        "--results",
        "resolved",
        "--results",
        "unresolved",
        "--actor",
        "oncall",
    ]);
    store.run(&[
        "add",
        "status-page",
        "--title",
        "Publish a status notice",
        "--done-when",
        "Customers can see the notice",
        "--needs",
        "triage",
        "--when-any",
        "severity=sev1",
        "--when-any",
        "severity=sev2",
        "--produces",
        "notice",
        "--actor",
        "comms",
    ]);
    store.run(&[
        "add",
        "vendor-escalation",
        "--title",
        "Escalate to the vendor",
        "--done-when",
        "The vendor is paged",
        "--needs",
        "mitigate",
        "--when",
        "mitigation=unresolved",
        "--produces",
        "vendor",
        "--results",
        "paged",
        "--actor",
        "oncall",
    ]);
    store.run(&[
        "add",
        "postmortem",
        "--title",
        "Write the postmortem",
        "--done-when",
        "The postmortem is filed",
        "--needs",
        "mitigate",
        "--when-any",
        "severity=sev1",
        "--when-any",
        "severity=sev2",
        "--actor",
        "oncall",
    ]);
    store.run(&[
        "add",
        "close-ticket",
        "--title",
        "Close the ticket",
        "--done-when",
        "The ticket is closed",
        "--when-any",
        "mitigation=resolved",
        "--when-any",
        "vendor=paged",
        "--actor",
        "oncall",
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
fn sev1_resolved_pages_and_closes() {
    let store = Store::new();
    plan(&store);
    take(&store, "oncall", "triage");
    succeed(
        &store,
        "triage",
        "oncall",
        Some("sev1"),
        "Checkout is down.",
    );
    take(&store, "ic", "page-ic");
    succeed(&store, "page-ic", "ic", None, "IC is on the bridge.");
    take(&store, "oncall", "mitigate");
    succeed(
        &store,
        "mitigate",
        "oncall",
        Some("resolved"),
        "Rolled back the deploy.",
    );
    take(&store, "comms", "status-page");
    succeed(&store, "status-page", "comms", None, "Status page updated.");
    take(&store, "oncall", "postmortem");
    succeed(
        &store,
        "postmortem",
        "oncall",
        None,
        "Postmortem PM-14 filed.",
    );
    take(&store, "oncall", "close-ticket");
    succeed(&store, "close-ticket", "oncall", None, "Ticket closed.");
    assert_eq!(store.run(&["next", "--agent", "oncall"])["action"], "done");
    assert_eq!(disposition(&store, "vendor-escalation"), "skipped");
}

#[test]
fn sev2_unresolved_escalates_to_the_vendor() {
    let store = Store::new();
    plan(&store);
    take(&store, "oncall", "triage");
    succeed(&store, "triage", "oncall", Some("sev2"), "Search is slow.");
    assert_eq!(disposition(&store, "page-ic"), "skipped");
    take(&store, "oncall", "mitigate");
    succeed(
        &store,
        "mitigate",
        "oncall",
        Some("unresolved"),
        "Still degraded.",
    );
    take(&store, "comms", "status-page");
    succeed(&store, "status-page", "comms", None, "Notice posted.");
    take(&store, "oncall", "vendor-escalation");
    succeed(
        &store,
        "vendor-escalation",
        "oncall",
        Some("paged"),
        "Vendor paged.",
    );
    take(&store, "oncall", "postmortem");
    succeed(&store, "postmortem", "oncall", None, "Postmortem filed.");
    take(&store, "oncall", "close-ticket");
    succeed(
        &store,
        "close-ticket",
        "oncall",
        None,
        "Closed after the vendor page.",
    );
    assert_eq!(store.run(&["next", "--agent", "oncall"])["action"], "done");
}

#[test]
fn sev3_skips_paging_status_and_postmortem() {
    let store = Store::new();
    plan(&store);
    take(&store, "oncall", "triage");
    succeed(
        &store,
        "triage",
        "oncall",
        Some("sev3"),
        "One chart is stale.",
    );
    take(&store, "oncall", "mitigate");
    succeed(
        &store,
        "mitigate",
        "oncall",
        Some("resolved"),
        "Refreshed the chart.",
    );
    take(&store, "oncall", "close-ticket");
    succeed(&store, "close-ticket", "oncall", None, "Closed.");
    assert_eq!(store.run(&["next", "--agent", "oncall"])["action"], "done");
    assert_eq!(disposition(&store, "page-ic"), "skipped");
    assert_eq!(disposition(&store, "status-page"), "skipped");
    assert_eq!(disposition(&store, "postmortem"), "skipped");
}

#[test]
fn a_failed_mitigation_is_retried_once() {
    let store = Store::new();
    plan(&store);
    take(&store, "oncall", "triage");
    succeed(&store, "triage", "oncall", Some("sev3"), "A job failed.");
    take(&store, "oncall", "mitigate");
    store.run(&[
        "report",
        "mitigate",
        "--agent",
        "oncall",
        "--outcome",
        "failed",
        "--because",
        "rollback refused",
    ]);
    let retry = store.run(&["next", "--agent", "oncall"]);
    assert_eq!(retry["step"]["id"], "mitigate");
    assert_eq!(retry["step"]["retry"], 1);
    succeed(
        &store,
        "mitigate",
        "oncall",
        Some("resolved"),
        "Retried and it stuck.",
    );
    take(&store, "oncall", "close-ticket");
    succeed(&store, "close-ticket", "oncall", None, "Closed.");
    assert_eq!(store.run(&["next", "--agent", "oncall"])["action"], "done");
}

#[test]
fn two_failures_ask_for_a_replan() {
    let store = Store::new();
    plan(&store);
    take(&store, "oncall", "triage");
    succeed(&store, "triage", "oncall", Some("sev3"), "A job failed.");
    take(&store, "oncall", "mitigate");
    store.run(&[
        "report",
        "mitigate",
        "--agent",
        "oncall",
        "--outcome",
        "failed",
        "--because",
        "first",
    ]);
    take(&store, "oncall", "mitigate");
    store.run(&[
        "report",
        "mitigate",
        "--agent",
        "oncall",
        "--outcome",
        "failed",
        "--because",
        "second",
    ]);
    let replan = store.run(&["next", "--agent", "oncall"]);
    assert_eq!(replan["action"], "replan");
    assert_eq!(replan["step"], "mitigate");
    store.run(&["revise"]);
    store.run(&[
        "set",
        "mitigate",
        "--title",
        "Mitigate with the vendor on the call",
    ]);
    store.run(&["commit"]);
    take(&store, "oncall", "mitigate");
    succeed(
        &store,
        "mitigate",
        "oncall",
        Some("resolved"),
        "Vendor walked us through it.",
    );
    take(&store, "oncall", "close-ticket");
    succeed(&store, "close-ticket", "oncall", None, "Closed.");
    assert_eq!(store.run(&["next", "--agent", "oncall"])["action"], "done");
}
