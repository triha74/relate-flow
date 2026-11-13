#[path = "common/mod.rs"]
mod common;

use common::{bin, Store};

#[test]
fn goal_creates_a_draft_and_updates_it() {
    let store = Store::new();
    let created = store.run(&["goal", "--text", "Pay the invoice"]);
    insta::assert_json_snapshot!(created);
    let same = store.run(&["goal", "--text", "Pay the invoice"]);
    assert_eq!(same["retries"], 2);
    let updated = store.run(&["goal", "--retries", "4"]);
    assert_eq!(updated["retries"], 4);
    assert_eq!(updated["goal"], "Pay the invoice");
}

#[test]
fn goal_without_text_on_an_empty_store_is_usage() {
    let store = Store::new();
    let err = store.fail(&["goal"], 2);
    assert_eq!(err["error"]["code"], "usage");
}

#[test]
fn a_blank_goal_is_refused() {
    let store = Store::new();
    let err = store.fail(&["goal", "--text", "   "], 4);
    assert_eq!(err["error"]["code"], "empty_field");
}

#[test]
fn unknown_flag_is_usage_json() {
    let out = bin().args(["nope"]).assert().failure().code(2);
    let err = common::parse(&out.get_output().stdout);
    assert_eq!(err["error"]["code"], "usage");
    assert_eq!(err["error"]["retryable"], false);
}

#[test]
fn help_and_version_are_plain_text() {
    bin().arg("--help").assert().success();
    bin().arg("--version").assert().success();
}
