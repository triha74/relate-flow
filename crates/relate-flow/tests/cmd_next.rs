#[path = "common/mod.rs"]
mod common;

use common::{bin, redact, Store};

#[test]
fn next_leases_the_ready_step() {
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
    store.run(&["commit"]);
    store.execute();
    let missing = store.fail(&["next"], 2);
    assert_eq!(missing["error"]["code"], "usage");
    let leased = store.run(&["next", "--agent", "invoices", "--lease", "10m"]);
    insta::assert_json_snapshot!(redact(&leased));
    let renewed = store.run(&["next", "--agent", "invoices"]);
    assert_eq!(renewed["renewed"], true);
}

#[test]
fn next_text_prints_the_report_commands_and_a_refusal_stays_json() {
    let store = Store::new();
    store.run(&["goal", "--text", "g"]);
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
        "--results",
        "sev2",
        "--actor",
        "oncall",
    ]);
    store.run(&["commit"]);
    store.execute();
    let missing = store.fail(&["next", "--output", "text"], 2);
    assert_eq!(missing["error"]["code"], "usage");
    let output = store
        .cmd(&["next", "--agent", "oncall", "--output", "text"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains(
        "relate-flow run report triage --agent oncall --outcome succeeded --result <sev1|sev2> --note \"...\""
    ));
    assert!(text.contains(
        "relate-flow run report triage --agent oncall --outcome failed --because \"...\""
    ));
    assert!(text.contains("case: the case"));
}

#[test]
fn a_lease_over_24_hours_is_refused() {
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
    let err = store.fail(&["next", "--agent", "invoices", "--lease", "25h"], 4);
    assert_eq!(err["error"]["code"], "lease_too_long");
}

#[test]
fn the_agent_can_come_from_the_environment() {
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
    let out = store
        .cmd(&["next"])
        .env("RELATE_FLOW_AGENT", "invoices")
        .assert()
        .success();
    let body = common::parse(&out.get_output().stdout);
    assert_eq!(body["step"]["id"], "parse");
}

#[test]
fn a_bad_lease_duration_is_usage() {
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
    assert_eq!(
        store.fail(&["next", "--agent", "invoices", "--lease", "soon"], 2)["error"]["code"],
        "usage"
    );
}

#[test]
fn a_bad_clock_override_is_usage() {
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
    let out = store
        .cmd(&["next", "--agent", "invoices"])
        .env("RELATE_FLOW_NOW", "yesterday")
        .assert()
        .failure()
        .code(2);
    assert_eq!(
        common::parse(&out.get_output().stdout)["error"]["code"],
        "usage"
    );
}

#[test]
fn relate_flow_now_sets_the_lease_clock() {
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
    let out = store
        .cmd(&["next", "--agent", "invoices"])
        .env("RELATE_FLOW_NOW", "2026-11-12T12:00:00Z")
        .assert()
        .success();
    let body = common::parse(&out.get_output().stdout);
    assert_eq!(body["step"]["leaseUntil"], "2026-11-12T12:15:00Z");
    let _ = bin();
}
