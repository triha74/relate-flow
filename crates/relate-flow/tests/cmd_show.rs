#[path = "common/mod.rs"]
mod common;

use common::Store;

#[test]
fn show_prints_the_draft() {
    let store = Store::new();
    store.run(&["goal", "--text", "Pay it", "--retries", "3"]);
    store.run(&[
        "add",
        "parse",
        "--title",
        "Parse",
        "--done-when",
        "recorded",
        "--produces",
        "invoice",
    ]);
    let show = store.run(&["show"]);
    insta::assert_json_snapshot!(show);
}

#[test]
fn show_text_draws_the_flow() {
    let store = Store::new();
    store.run(&["goal", "--text", "Pay it"]);
    store.run(&[
        "add",
        "parse",
        "--title",
        "Parse",
        "--done-when",
        "recorded",
        "--produces",
        "invoice",
        "--actor",
        "invoices",
    ]);
    store.run(&[
        "add",
        "check",
        "--title",
        "Check",
        "--done-when",
        "recorded",
        "--needs",
        "parse",
        "--when",
        "invoice=ok",
    ]);
    let out = store
        .cmd(&["show", "--output", "text"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("Pay it"));
    assert!(text.contains("parse"));
    assert!(text.contains("check"));
    assert!(text.contains("invoice=ok"));
    assert!(!text.trim_start().starts_with('{'));
}
