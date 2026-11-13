#[path = "common/mod.rs"]
mod common;

use common::Store;

#[test]
fn set_changes_the_flags_that_were_passed() {
    let store = Store::new();
    store.run(&["goal", "--text", "g"]);
    store.run(&[
        "add",
        "parse",
        "--title",
        "Parse",
        "--done-when",
        "recorded",
        "--actor",
        "invoices",
    ]);
    let updated = store.run(&["set", "parse", "--title", "Parse the invoice", "--no-actor"]);
    insta::assert_json_snapshot!(updated);
    let show = store.run(&["show"]);
    assert_eq!(show["steps"][0]["title"], "Parse the invoice");
    assert!(show["steps"][0].get("actor").is_none());
}

#[test]
fn set_replaces_needs_results_and_conditions() {
    let store = Store::new();
    store.run(&["goal", "--text", "g"]);
    store.run(&[
        "add",
        "parse",
        "--title",
        "Parse",
        "--done-when",
        "recorded",
        "--produces",
        "invoice",
        "--results",
        "ok",
    ]);
    store.run(&[
        "add",
        "child",
        "--title",
        "Child",
        "--done-when",
        "recorded",
    ]);
    store.run(&["set", "child", "--needs", "parse", "--when", "invoice=ok"]);
    store.run(&[
        "set",
        "child",
        "--when-any",
        "invoice=ok",
        "--results",
        "done",
        "--produces",
        "child-out",
    ]);
    let err = store.fail(&["set", "child", "--when", "not-a-pair"], 2);
    assert_eq!(err["error"]["code"], "usage");
    let status = store.run(&["status"]);
    assert_eq!(status["draft"], true);
    let empty = Store::new();
    assert_eq!(empty.fail(&["status"], 4)["error"]["code"], "no_plan");
    assert_eq!(empty.fail(&["log"], 4)["error"]["code"], "no_plan");
}

#[test]
fn clearing_and_setting_the_same_flag_is_usage() {
    let store = Store::new();
    store.run(&["goal", "--text", "g"]);
    store.run(&[
        "add",
        "parse",
        "--title",
        "Parse",
        "--done-when",
        "recorded",
    ]);
    let err = store.fail(&["set", "parse", "--actor", "invoices", "--no-actor"], 2);
    assert_eq!(err["error"]["code"], "usage");
}
