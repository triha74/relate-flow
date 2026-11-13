#[path = "common/mod.rs"]
mod common;

use common::{bin, Store};

fn prepared() -> Store {
    let store = Store::new();
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
        "--actor",
        "oncall",
    ]);
    store.run(&["commit"]);
    store
}

fn on(store: &Store, plan: &str, args: &[&str]) -> serde_json::Value {
    let mut cmd = bin();
    cmd.arg(plan).args(args).arg("-d").arg(store.path());
    let output = cmd.assert().success().get_output().stdout.clone();
    common::parse(&output)
}

#[test]
fn remove_refuses_an_open_lease_until_forced() {
    let store = prepared();
    store.run(&["execute", "inc-1042"]);
    on(&store, "inc-1042", &["next", "--agent", "oncall"]);
    let err = bin()
        .args(["inc-1042", "remove", "-d", store.path()])
        .assert()
        .failure()
        .code(4);
    let err = common::parse(&err.get_output().stdout);
    assert_eq!(err["error"]["code"], "lease_open");
    let removed = on(&store, "inc-1042", &["remove", "--force"]);
    assert_eq!(removed["removed"], "inc-1042");
    assert_eq!(removed["attempts"], 1);
}

#[test]
fn prune_drops_the_finished_execution() {
    let store = prepared();
    store.run(&["execute", "done-run"]);
    store.run(&["execute", "live-run"]);
    on(&store, "done-run", &["next", "--agent", "oncall"]);
    on(
        &store,
        "done-run",
        &[
            "report",
            "triage",
            "--agent",
            "oncall",
            "--outcome",
            "succeeded",
            "--result",
            "sev1",
            "--note",
            "recorded",
        ],
    );
    on(&store, "live-run", &["next", "--agent", "oncall"]);
    let pruned = store.run(&["prune"]);
    assert_eq!(pruned["pruned"], serde_json::json!(["done-run"]));
    let shown = store.run(&["show"]);
    assert_eq!(shown["kind"], "template");
}
