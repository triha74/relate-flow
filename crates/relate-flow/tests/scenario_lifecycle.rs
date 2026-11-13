#[path = "common/mod.rs"]
mod common;

use common::Store;

#[test]
fn a_lease_expires_and_the_step_can_be_claimed_again() {
    let store = Store::new();
    store.run(&["goal", "--text", "one step"]);
    store.run(&["add", "only", "--title", "Only", "--done-when", "done"]);
    store.run(&["commit"]);
    store.execute();
    let first = store
        .cmd(&["next", "--agent", "worker"])
        .env("RELATE_FLOW_NOW", "2026-11-12T12:00:00Z")
        .assert()
        .success();
    let first = common::parse(&first.get_output().stdout);
    assert_eq!(first["step"]["attempt"], "a1");
    let second = store
        .cmd(&["next", "--agent", "worker"])
        .env("RELATE_FLOW_NOW", "2026-11-12T12:16:00Z")
        .assert()
        .success();
    let second = common::parse(&second.get_output().stdout);
    assert_eq!(second["step"]["attempt"], "a2");
    let log = store.run(&["log"]);
    let expired = log["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "a1")
        .unwrap();
    assert_eq!(expired["outcome"], "expired");
}

#[test]
fn status_after_each_transition_stays_consistent() {
    let store = Store::new();
    store.run(&["goal", "--text", "two steps"]);
    store.run(&[
        "add",
        "first",
        "--title",
        "First",
        "--done-when",
        "first is recorded",
        "--actor",
        "worker",
    ]);
    store.run(&[
        "add",
        "second",
        "--title",
        "Second",
        "--done-when",
        "second is recorded",
        "--needs",
        "first",
        "--actor",
        "worker",
    ]);
    let draft = store.run(&["status"]);
    assert_eq!(draft["draft"], true);
    assert_eq!(draft["steps"][1]["disposition"], "pending");
    store.run(&["commit"]);
    let ready = store.run(&["status"]);
    assert_eq!(ready["version"], 1);
    assert_eq!(ready["steps"][0]["disposition"], "ready");
    store.execute();
    store.run(&["next", "--agent", "worker"]);
    assert_eq!(store.run(&["status"])["steps"][0]["disposition"], "running");
    store.run(&[
        "report",
        "first",
        "--agent",
        "worker",
        "--outcome",
        "succeeded",
        "--note",
        "done",
    ]);
    let after = store.run(&["status"]);
    assert_eq!(after["steps"][0]["disposition"], "succeeded");
    assert_eq!(after["steps"][1]["disposition"], "ready");
    let why = store.run(&["explain", "second"]);
    assert_eq!(why["disposition"], "ready");
}
