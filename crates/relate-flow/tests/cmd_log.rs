#[path = "common/mod.rs"]
mod common;

use common::{redact, Store};

#[test]
fn log_lists_attempts_newest_first() {
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
    store.run(&["commit"]);
    store.execute();
    store.run(&["next", "--agent", "invoices"]);
    store.run(&[
        "report",
        "parse",
        "--agent",
        "invoices",
        "--outcome",
        "succeeded",
        "--note",
        "done",
    ]);
    let log = store.run(&["log", "--json"]);
    insta::assert_json_snapshot!(redact(&log));
    let one = store.run(&["log", "parse"]);
    assert_eq!(one["attempts"][0]["step"], "parse");
    assert_eq!(
        store.fail(&["log", "missing"], 4)["error"]["code"],
        "unknown_step"
    );
}
