#[path = "common/mod.rs"]
mod common;

use common::Store;

#[test]
fn abort_deletes_the_draft() {
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
    let aborted = store.run(&["abort"]);
    insta::assert_json_snapshot!(aborted);
    assert_eq!(store.fail(&["show"], 4)["error"]["code"], "no_plan");
}
