#![allow(dead_code)]

use std::cell::RefCell;

use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

pub struct Store {
    dir: TempDir,
    /// Template name until `execute`, then the execution name.
    name: RefCell<String>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            dir: TempDir::new().unwrap(),
            name: RefCell::new("plan".into()),
        }
    }

    /// Copy the committed template into an execution named `run` and aim later commands at it.
    pub fn execute(&self) {
        let template = self.name.borrow().clone();
        let mut cmd = bin();
        cmd.args([&template, "execute", "run", "--input", "case=the case"])
            .arg("-d")
            .arg(self.path());
        cmd.assert().success();
        *self.name.borrow_mut() = "run".into();
    }

    pub fn path(&self) -> &str {
        self.dir.path().to_str().unwrap()
    }

    pub fn cmd(&self, args: &[&str]) -> Command {
        let mut cmd = bin();
        if args.first().copied() != Some("skill") {
            cmd.arg(self.name.borrow().as_str());
        }
        cmd.args(args).arg("-d").arg(self.path());
        cmd
    }

    pub fn run(&self, args: &[&str]) -> Value {
        parse(&self.cmd(args).assert().success().get_output().stdout)
    }

    pub fn fail(&self, args: &[&str], code: i32) -> Value {
        parse(
            &self
                .cmd(args)
                .assert()
                .failure()
                .code(code)
                .get_output()
                .stdout,
        )
    }
}

pub fn bin() -> Command {
    Command::cargo_bin("relate-flow").unwrap()
}

pub fn parse(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes)
        .unwrap_or_else(|err| panic!("{err}: {}", String::from_utf8_lossy(bytes)))
}

pub fn redact(value: &Value) -> Value {
    let mut copy = value.clone();
    walk(&mut copy);
    copy
}

fn walk(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if matches!(key.as_str(), "leaseUntil" | "startedAt" | "finishedAt") {
                    *child = json!("[time]");
                } else {
                    walk(child);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(walk),
        _ => {}
    }
}

pub fn goal(store: &Store, text: &str) {
    store.run(&["goal", "--text", text]);
}

pub fn add_plain(store: &Store, id: &str) {
    store.run(&[
        "add",
        id,
        "--title",
        id,
        "--done-when",
        &format!("{id} is recorded"),
    ]);
}
