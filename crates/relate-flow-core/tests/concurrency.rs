use std::sync::{Arc, Barrier};
use std::thread;

use relate_flow_core::{AddStep, Flow, DEFAULT_LEASE};

fn next_retry(flow: &Flow, agent: &str) -> serde_json::Value {
    let mut last = None;
    for _ in 0..6 {
        match flow.next(agent.to_string(), DEFAULT_LEASE) {
            Ok(value) => return value,
            Err(err) if err.code == "conflict" && err.retryable => last = Some(err),
            Err(err) => panic!("{err}"),
        }
    }
    panic!("still conflicting: {}", last.unwrap());
}

fn drive(flow: &Flow) -> Flow {
    flow.execute("run".into(), Vec::new()).unwrap();
    flow.on("run")
}

fn bare(id: &str) -> AddStep {
    AddStep {
        id: id.into(),
        title: id.into(),
        done_when: "recorded".into(),
        produces: None,
        results: vec![],
        needs: vec![],
        when: vec![],
        when_any: vec![],
        actor: None,
        before: None,
        after: None,
    }
}

#[test]
fn claims_on_different_steps_both_succeed() {
    let flow = Flow::open(":memory:").unwrap().on("plan");
    flow.goal(Some("two steps".into()), None).unwrap();
    flow.add(bare("alpha")).unwrap();
    flow.add(bare("beta")).unwrap();
    flow.commit(false).unwrap();
    let flow = drive(&flow);
    let barrier = Arc::new(Barrier::new(2));
    let left = {
        let flow = flow.clone();
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            next_retry(&flow, "one")
        })
    };
    let right = {
        let flow = flow.clone();
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            next_retry(&flow, "two")
        })
    };
    let left = left.join().unwrap();
    let right = right.join().unwrap();
    assert_eq!(left["action"], "run");
    assert_eq!(right["action"], "run");
    assert_ne!(left["step"]["id"], right["step"]["id"]);
}

#[test]
fn overlapping_claims_on_one_step_leave_a_single_lease() {
    for _ in 0..8 {
        let flow = Flow::open(":memory:").unwrap().on("plan");
        flow.goal(Some("one step".into()), None).unwrap();
        flow.add(bare("only")).unwrap();
        flow.commit(false).unwrap();
        let flow = drive(&flow);
        let barrier = Arc::new(Barrier::new(2));
        let spawn = |agent: &str| {
            let flow = flow.clone();
            let barrier = Arc::clone(&barrier);
            let agent = agent.to_string();
            thread::spawn(move || {
                barrier.wait();
                flow.next(agent, DEFAULT_LEASE)
            })
        };
        let left = spawn("one");
        let right = spawn("two");
        let left = left.join().unwrap();
        let right = right.join().unwrap();
        let runs = [&left, &right]
            .iter()
            .filter(|r| r.as_ref().ok().is_some_and(|v| v["action"] == "run"))
            .count();
        assert_eq!(runs, 1, "left={left:?} right={right:?}");
        let other_ok = [&left, &right].iter().any(|r| match r {
            Ok(value) => value["action"] == "waiting",
            Err(err) => err.code == "conflict" && err.retryable,
        });
        assert!(
            other_ok,
            "the loser must wait or hit a retryable conflict: {left:?} {right:?}"
        );
    }
}
