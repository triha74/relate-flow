#[path = "common/mod.rs"]
mod common;

use common::Store;

#[test]
fn commit_writes_a_version_and_dry_run_leaves_the_draft() {
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
    let preview = store.run(&["commit", "--dry-run"]);
    assert_eq!(preview["dryRun"], true);
    assert_eq!(store.run(&["show"])["draft"], true);
    let committed = store.run(&["commit"]);
    insta::assert_json_snapshot!(committed);
    assert_eq!(store.run(&["show"])["draft"], false);
}

#[test]
fn commit_lists_every_problem() {
    let store = Store::new();
    store.run(&["goal", "--text", "g"]);
    store.run(&[
        "add",
        "child",
        "--title",
        "Child",
        "--done-when",
        "recorded",
        "--needs",
        "missing",
    ]);
    let err = store.fail(&["commit"], 4);
    assert_eq!(err["error"]["code"], "invalid_plan");
    assert!(!err["error"]["errors"].as_array().unwrap().is_empty());
}
