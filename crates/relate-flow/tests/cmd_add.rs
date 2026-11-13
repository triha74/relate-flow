#[path = "common/mod.rs"]
mod common;

use common::Store;

#[test]
fn add_appends_and_repeats_cleanly() {
    let store = Store::new();
    store.run(&["goal", "--text", "g"]);
    let added = store.run(&[
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
        "--results",
        "ok",
    ]);
    insta::assert_json_snapshot!(added);
    let again = store.run(&[
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
        "--results",
        "ok",
    ]);
    assert_eq!(again["step"], "parse");
}

#[test]
fn a_different_repeat_and_a_bad_condition_are_refused() {
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
    let differs = store.fail(
        &[
            "add",
            "parse",
            "--title",
            "Other",
            "--done-when",
            "recorded",
        ],
        4,
    );
    assert_eq!(differs["error"]["code"], "step_differs");
    let bad = store.fail(
        &[
            "add",
            "child",
            "--title",
            "Child",
            "--done-when",
            "recorded",
            "--when",
            "not a pair",
        ],
        2,
    );
    assert_eq!(bad["error"]["code"], "usage");
}

#[test]
fn before_and_after_together_are_usage() {
    let store = Store::new();
    store.run(&["goal", "--text", "g"]);
    let err = store.fail(
        &[
            "add",
            "parse",
            "--title",
            "Parse",
            "--done-when",
            "recorded",
            "--before",
            "x",
            "--after",
            "y",
        ],
        2,
    );
    assert_eq!(err["error"]["code"], "usage");
}
