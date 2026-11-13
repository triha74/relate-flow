#[path = "common/mod.rs"]
mod common;

use common::Store;

#[test]
fn revise_opens_a_draft_from_the_committed_plan() {
    let store = Store::new();
    store.run(&["goal", "--text", "Pay it"]);
    store.run(&[
        "add",
        "parse",
        "--title",
        "Parse",
        "--done-when",
        "recorded",
    ]);
    store.run(&["commit"]);
    let draft = store.run(&["revise"]);
    insta::assert_json_snapshot!(draft);
    assert_eq!(store.run(&["show"])["draft"], true);
    assert_eq!(store.run(&["show"])["basedOn"], 1);
}

#[test]
fn revise_with_a_draft_already_open_is_refused() {
    let store = Store::new();
    store.run(&["goal", "--text", "g"]);
    let err = store.fail(&["revise"], 4);
    assert_eq!(err["error"]["code"], "draft_open");
}
