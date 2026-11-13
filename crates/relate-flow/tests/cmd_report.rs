#[path = "common/mod.rs"]
mod common;

use common::{redact, Store};

fn ready(store: &Store) {
    store.run(&["goal", "--text", "g"]);
    store.run(&[
        "add",
        "parse",
        "--title",
        "Parse",
        "--done-when",
        "recorded",
        "--produces",
        "invoice",
        "--results",
        "ok",
        "--results",
        "bad",
    ]);
    store.run(&["commit"]);
    store.execute();
    store.run(&["next", "--agent", "invoices"]);
}

#[test]
fn report_admits_a_success() {
    let store = Store::new();
    ready(&store);
    let body = store.run(&[
        "report",
        "parse",
        "--agent",
        "invoices",
        "--outcome",
        "succeeded",
        "--result",
        "ok",
        "--note",
        "recorded",
    ]);
    insta::assert_json_snapshot!(redact(&body));
    let again = store.run(&[
        "report",
        "parse",
        "--agent",
        "invoices",
        "--outcome",
        "succeeded",
        "--result",
        "ok",
        "--note",
        "recorded",
    ]);
    assert_eq!(again["attempt"], "a1");
}

#[test]
fn an_unknown_outcome_is_usage() {
    let store = Store::new();
    ready(&store);
    let err = store.fail(
        &[
            "report",
            "parse",
            "--agent",
            "invoices",
            "--outcome",
            "maybe",
        ],
        2,
    );
    assert_eq!(err["error"]["code"], "usage");
}

#[test]
fn report_without_an_agent_is_usage() {
    let store = Store::new();
    ready(&store);
    let err = store.fail(
        &[
            "report",
            "parse",
            "--outcome",
            "succeeded",
            "--note",
            "n",
            "--result",
            "ok",
        ],
        2,
    );
    assert_eq!(err["error"]["code"], "usage");
}

#[test]
fn a_missing_note_is_refused() {
    let store = Store::new();
    ready(&store);
    let err = store.fail(
        &[
            "report",
            "parse",
            "--agent",
            "invoices",
            "--outcome",
            "succeeded",
            "--result",
            "ok",
        ],
        4,
    );
    assert_eq!(err["error"]["code"], "missing_note");
}
