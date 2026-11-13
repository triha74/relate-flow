#[path = "common/mod.rs"]
mod common;

use common::Store;

#[test]
fn explain_says_why_a_step_is_pending() {
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
    store.run(&[
        "add",
        "child",
        "--title",
        "Child",
        "--done-when",
        "recorded",
        "--needs",
        "parse",
    ]);
    let explained = store.run(&["explain", "child"]);
    insta::assert_json_snapshot!(explained);
    assert_eq!(
        store.fail(&["explain", "missing"], 4)["error"]["code"],
        "unknown_step"
    );
}
