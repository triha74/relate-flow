#[path = "common/mod.rs"]
mod common;

use common::{bin, Store};

fn prepared() -> Store {
    let store = Store::new();
    store.run(&["goal", "--text", "Restore the service, or escalate it"]);
    store.run(&[
        "add",
        "triage",
        "--title",
        "Classify the incident",
        "--done-when",
        "Severity is recorded",
        "--produces",
        "severity",
        "--results",
        "sev1",
        "--actor",
        "oncall",
    ]);
    store.run(&["commit"]);
    store
}

#[test]
fn execute_binds_the_case_and_next_on_the_template_is_refused() {
    let store = prepared();
    let body = store.run(&["execute", "inc-1042", "--input", "case=checkout down"]);
    assert_eq!(body["execution"], "inc-1042");
    assert_eq!(body["template"], "plan");
    assert_eq!(body["version"], 1);
    assert_eq!(body["inputs"]["case"], "checkout down");
    assert_eq!(
        store.fail(&["next", "--agent", "oncall"], 4)["error"]["code"],
        "not_a_run"
    );
    let next = bin()
        .args(["inc-1042", "next", "--agent", "oncall", "-d", store.path()])
        .assert()
        .success();
    let next = common::parse(&next.get_output().stdout);
    assert_eq!(next["step"]["inputs"]["case"], "checkout down");
}

#[test]
fn a_bad_input_a_taken_name_and_an_execution_are_refused() {
    let store = prepared();
    assert_eq!(
        store.fail(&["execute", "bad", "--input", "severity=sev1"], 4)["error"]["code"],
        "input_conflicts"
    );
    assert_eq!(
        store.fail(&["execute", "bad", "--input", "case"], 2)["error"]["code"],
        "usage"
    );
    store.run(&["execute", "inc-1042"]);
    assert_eq!(
        store.fail(&["execute", "inc-1042"], 4)["error"]["code"],
        "name_taken"
    );
    let err = bin()
        .args(["inc-1042", "execute", "other", "-d", store.path()])
        .assert()
        .failure()
        .code(4);
    let err = common::parse(&err.get_output().stdout);
    assert_eq!(err["error"]["code"], "not_a_template");
}
