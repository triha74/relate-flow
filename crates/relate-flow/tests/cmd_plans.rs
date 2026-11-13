#[path = "common/mod.rs"]
mod common;

use common::{bin, Store};

#[test]
fn help_lists_the_plans_in_the_store() {
    let store = Store::new();
    store.run(&["goal", "--text", "Pay it"]);
    let mut beta = bin();
    beta.args(["beta", "goal", "--text", "Other work", "-d", store.path()])
        .assert()
        .success();
    let mut help = bin();
    let out = help
        .args(["--help", "-d", store.path()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("plan"), "{text}");
    assert!(text.contains("beta"), "{text}");
    assert!(text.contains("Pay it"), "{text}");
    assert!(text.contains("Other work"), "{text}");
    assert!(text.contains("draft"), "{text}");
}

#[test]
fn the_database_flag_selects_the_store_help_lists() {
    let store = Store::new();
    bin()
        .args([
            "-d",
            store.path(),
            "alpha",
            "goal",
            "--text",
            "From the flag",
        ])
        .assert()
        .success();
    let out = bin()
        .args(["-d", store.path()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("--database"), "{text}");
    assert!(text.contains(store.path()), "{text}");
    assert!(text.contains("alpha"), "{text}");
    assert!(text.contains("From the flag"), "{text}");
}
