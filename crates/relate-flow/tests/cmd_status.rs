#[path = "common/mod.rs"]
mod common;

use common::Store;

#[test]
fn status_is_json_when_stdout_is_not_a_terminal() {
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
    let status = store.run(&["status"]);
    insta::assert_json_snapshot!(status);
    let forced = store.run(&["status", "--json"]);
    assert_eq!(forced["draft"], true);
    assert_eq!(forced["steps"][0]["disposition"], "ready");
}
