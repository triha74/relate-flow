#[path = "common/mod.rs"]
mod common;

use common::Store;

#[test]
fn drop_removes_a_step_and_refuses_a_reference() {
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
    let referenced = store.fail(&["drop", "parse"], 4);
    assert_eq!(referenced["error"]["code"], "step_referenced");
    let dropped = store.run(&["drop", "child"]);
    insta::assert_json_snapshot!(dropped);
}
