use proptest::prelude::*;
use relate_flow_core::{AddStep, Flow, Outcome, Report, DEFAULT_LEASE};

fn drive(flow: &Flow) -> Flow {
    flow.execute("run".into(), Vec::new()).unwrap();
    flow.on("run")
}

fn step(id: &str, needs: Vec<String>) -> AddStep {
    AddStep {
        id: id.into(),
        title: id.into(),
        done_when: "recorded".into(),
        produces: None,
        results: vec![],
        needs,
        when: vec![],
        when_any: vec![],
        actor: None,
        before: None,
        after: None,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn succeeding_ready_steps_finishes_an_acyclic_plan(edges in prop::collection::vec(any::<bool>(), 15)) {
        let flow = Flow::open(":memory:").unwrap().on("plan");
        flow.goal(Some("finish".into()), None).unwrap();
        let ids = ["a", "b", "c", "d", "e"];
        for (i, id) in ids.iter().enumerate() {
            let mut needs = Vec::new();
            for (j, earlier) in ids.iter().take(i).enumerate() {
                if edges[i * 3 + j % 3] {
                    needs.push((*earlier).to_string());
                }
            }
            flow.add(step(id, needs)).unwrap();
        }
        flow.commit(false).unwrap();
        let flow = drive(&flow);
        for _ in 0..ids.len() {
            let next = flow.next("worker".into(), DEFAULT_LEASE).unwrap();
            assert_eq!(next["action"], "run");
            let id = next["step"]["id"].as_str().unwrap().to_string();
            flow.report(Report {
                step: id,
                agent: "worker".into(),
                outcome: Outcome::Succeeded,
                result: None,
                note: Some("done".into()),
                because: None,
                artifact: None,
                artifact_hash: None,
            }).unwrap();
        }
        let done = flow.next("worker".into(), DEFAULT_LEASE).unwrap();
        assert_eq!(done["action"], "done");
        let status = flow.status().unwrap();
        for step in status["steps"].as_array().unwrap() {
            assert_eq!(step["disposition"], "succeeded");
        }
    }
}
