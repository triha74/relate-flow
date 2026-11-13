//! Every command and every library error code is named by a test.

use relate_flow_core::ERROR_CODES;
use std::fs;
use std::path::Path;

const VERBS: &[&str] = &[
    "goal", "add", "set", "drop", "commit", "revise", "abort", "execute", "remove", "prune",
    "show", "next", "report", "status", "explain", "log", "skill",
];

#[test]
fn every_verb_and_error_code_has_a_test() {
    let mut sources = String::new();
    for root in [
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../relate-flow-core/tests"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../relate-flow-core/src"),
    ] {
        walk(&root, &mut sources);
    }
    for verb in VERBS {
        assert!(
            sources.contains(&format!("\"{verb}\"")) || sources.contains(verb),
            "no test mentions {verb}"
        );
    }
    for code in ERROR_CODES {
        assert!(
            sources.contains(&format!("\"{code}\"")),
            "no test mentions {code}"
        );
    }
}

fn walk(dir: &Path, out: &mut String) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push_str(&fs::read_to_string(&path).unwrap());
        }
    }
}
