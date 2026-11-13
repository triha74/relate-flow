#[path = "common/mod.rs"]
mod common;

use std::fs;

use common::bin;

#[test]
fn install_list_check_and_remove_round_trip() {
    let home = tempfile::tempdir().unwrap();
    let installed = skill(&home, &["skill", "install", "--agent", "cursor"]);
    insta::assert_json_snapshot!(installed);
    let skill_md = home.path().join(".cursor/skills/relate-flow/SKILL.md");
    assert!(skill_md.is_file());
    assert!(fs::read_to_string(&skill_md)
        .unwrap()
        .contains("version: 0.1.0"));
    assert!(home
        .path()
        .join(".cursor/skills/relate-flow/.relate-flow-installed")
        .is_file());

    let listed = skill(&home, &["skill", "list", "--agent", "cursor"]);
    assert_eq!(listed["agents"][0]["state"], "installed");
    let checked = skill(&home, &["skill", "check", "--agent", "cursor"]);
    assert_eq!(checked["drift"], false);

    fs::write(&skill_md, "---\nname: relate-flow\nversion: 9\n---\n").unwrap();
    let drift = skill_fail(&home, &["skill", "check", "--agent", "cursor"], 4);
    assert_eq!(drift["error"]["code"], "skill_drift");

    skill(&home, &["skill", "install", "--agent", "cursor"]);
    skill(&home, &["skill", "remove", "--agent", "cursor"]);
    assert!(!skill_md.exists());
}

#[test]
fn a_foreign_directory_is_refused_until_force() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".cursor/skills/relate-flow");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), "---\nname: other\n---\n").unwrap();
    let refused = skill_fail(&home, &["skill", "install", "--agent", "cursor"], 4);
    assert_eq!(refused["error"]["code"], "foreign_skill");
    skill(&home, &["skill", "install", "--agent", "cursor", "--force"]);
    assert!(dir.join(".relate-flow-installed").is_file());
    fs::write(dir.join("SKILL.md"), "---\nname: other\n---\n").unwrap();
    fs::remove_file(dir.join(".relate-flow-installed")).unwrap();
    let still = skill_fail(&home, &["skill", "remove", "--agent", "cursor"], 4);
    assert_eq!(still["error"]["code"], "foreign_skill");
}

#[test]
fn unknown_and_missing_agents_are_refused() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(
        skill_fail(&home, &["skill", "install", "--agent", "nope"], 4)["error"]["code"],
        "unknown_agent"
    );
    assert_eq!(
        skill_fail(&home, &["skill", "list"], 4)["error"]["code"],
        "no_agent"
    );
}

fn skill(home: &tempfile::TempDir, args: &[&str]) -> serde_json::Value {
    let out = bin().args(args).env("HOME", home.path()).assert().success();
    common::parse(&out.get_output().stdout)
}

fn skill_fail(home: &tempfile::TempDir, args: &[&str], code: i32) -> serde_json::Value {
    let out = bin()
        .args(args)
        .env("HOME", home.path())
        .assert()
        .failure()
        .code(code);
    common::parse(&out.get_output().stdout)
}
