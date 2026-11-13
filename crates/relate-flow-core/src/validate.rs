use crate::error::{FlowError, Problem};
use crate::model::{Pair, Plan, Step};

pub fn validate(plan: &Plan) -> Result<(), FlowError> {
    let mut problems = Vec::new();
    if plan.goal.trim().is_empty() {
        problems.push(problem(
            "invalid_plan",
            "the plan has no goal",
            "call goal --text TEXT",
            None,
        ));
    }
    if plan.steps.is_empty() {
        problems.push(problem(
            "invalid_plan",
            "the plan has no steps",
            "call add before commit",
            None,
        ));
    }
    let mut seen_produce: Vec<&str> = Vec::new();
    for step in &plan.steps {
        if let Some(produce) = &step.produces {
            if seen_produce.contains(&produce.as_str()) {
                problems.push(problem(
                    "duplicate_value",
                    format!("produce name '{produce}' is used by more than one step"),
                    format!("give {produce} to only one step"),
                    Some(step.id.clone()),
                ));
            }
            seen_produce.push(produce);
        }
        if !step.results.is_empty() && step.produces.is_none() {
            problems.push(problem(
                "results_need_produce",
                format!("{} has results but does not produce a name", step.id),
                format!("set {} --produces NAME, or pass --no-results", step.id),
                Some(step.id.clone()),
            ));
        }
        for need in &step.needs {
            if !plan.steps.iter().any(|s| s.id == *need) {
                problems.push(problem(
                    "invalid_plan",
                    format!("{} needs {need}, which is not a step", step.id),
                    format!("add {need}, or set {} --needs without it", step.id),
                    Some(step.id.clone()),
                ));
            }
        }
        if !step.when.is_empty() && !step.when_any.is_empty() {
            problems.push(problem(
                "invalid_plan",
                format!("{} has both when and whenAny", step.id),
                format!("set {} with only one of --when or --when-any", step.id),
                Some(step.id.clone()),
            ));
        }
        for pair in step.when.iter().chain(step.when_any.iter().flatten()) {
            check_pair(&mut problems, plan, step, pair);
        }
    }
    if let Some(path) = cycle(plan) {
        let at = path.last().cloned().unwrap_or_default();
        problems.push(problem(
            "invalid_plan",
            format!("needs and conditions cycle through {}", path.join(" -> ")),
            format!("remove a needs or condition edge so {at} is not on a cycle"),
            Some(at),
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(FlowError::refusal("invalid_plan", "", "").with_problems(problems))
    }
}

fn check_pair(problems: &mut Vec<Problem>, plan: &Plan, step: &Step, pair: &Pair) {
    let Some(producer) = plan
        .steps
        .iter()
        .find(|s| s.produces.as_deref() == Some(pair.produce.as_str()))
    else {
        problems.push(problem(
            "invalid_plan",
            format!(
                "{} conditions on {}, which no step produces",
                step.id, pair.produce
            ),
            format!(
                "add a step that --produces {}, or drop the condition",
                pair.produce
            ),
            Some(step.id.clone()),
        ));
        return;
    };
    if !producer.results.iter().any(|r| r == &pair.token) {
        problems.push(problem(
            "invalid_plan",
            format!(
                "{} expects {}={}, which is not a result of {}",
                step.id, pair.produce, pair.token, producer.id
            ),
            format!(
                "set {} --results {}, or change the condition",
                producer.id, pair.token
            ),
            Some(step.id.clone()),
        ));
    }
}

fn problem(
    code: &'static str,
    message: impl Into<String>,
    fix: impl Into<String>,
    step: Option<String>,
) -> Problem {
    Problem {
        code,
        message: message.into(),
        fix: fix.into(),
        step,
    }
}

fn cycle(plan: &Plan) -> Option<Vec<String>> {
    let ids: Vec<&str> = plan.steps.iter().map(|s| s.id.as_str()).collect();
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); ids.len()];
    for (i, step) in plan.steps.iter().enumerate() {
        for dep in deps(plan, step) {
            if let Some(j) = ids.iter().position(|id| *id == dep) {
                edges[i].push(j);
            }
        }
    }
    let mut color = vec![0u8; ids.len()];
    let mut stack = Vec::new();
    for i in 0..ids.len() {
        if let Some(path) = dfs(i, &edges, &mut color, &mut stack, &ids) {
            return Some(path);
        }
    }
    None
}

fn deps<'a>(plan: &'a Plan, step: &'a Step) -> Vec<&'a str> {
    let mut out: Vec<&str> = step.needs.iter().map(String::as_str).collect();
    for pair in step.when.iter().chain(step.when_any.iter().flatten()) {
        if let Some(prod) = plan
            .steps
            .iter()
            .find(|s| s.produces.as_deref() == Some(pair.produce.as_str()))
        {
            if !out.contains(&prod.id.as_str()) {
                out.push(prod.id.as_str());
            }
        }
    }
    out
}

fn dfs(
    i: usize,
    edges: &[Vec<usize>],
    color: &mut [u8],
    stack: &mut Vec<usize>,
    ids: &[&str],
) -> Option<Vec<String>> {
    if color[i] == 1 {
        let start = stack.iter().position(|s| *s == i).unwrap_or(0);
        let mut path: Vec<String> = stack[start..].iter().map(|s| ids[*s].to_string()).collect();
        path.push(ids[i].to_string());
        return Some(path);
    }
    if color[i] == 2 {
        return None;
    }
    color[i] = 1;
    stack.push(i);
    for &j in &edges[i] {
        if let Some(path) = dfs(j, edges, color, stack, ids) {
            return Some(path);
        }
    }
    stack.pop();
    color[i] = 2;
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::StepState;

    fn bare(id: &str) -> Step {
        Step {
            uid: id.into(),
            id: id.into(),
            title: id.into(),
            done_when: "done".into(),
            produces: None,
            results: vec![],
            needs: vec![],
            when: vec![],
            when_any: vec![],
            actor: None,
            state: StepState::Pending,
            position: 0,
        }
    }

    fn plan(steps: Vec<Step>) -> Plan {
        Plan {
            uid: "p".into(),
            name: "p".into(),
            kind: crate::model::PlanKind::Template,
            inputs: Vec::new(),
            template: None,
            template_version: None,
            version: None,
            based_on: None,
            goal: "g".into(),
            retries: 2,
            created_at: "t".into(),
            steps,
            supersedes: None,
        }
    }

    #[test]
    fn empty_plan_reports_every_problem() {
        let err = validate(&plan(vec![])).unwrap_err();
        assert!(!err.errors.is_empty());
        assert_eq!(err.errors[0].code, "invalid_plan");
    }

    #[test]
    fn missing_need_unknown_token_and_cycle() {
        let mut a = bare("a");
        a.needs = vec!["missing".into()];
        a.when = vec![Pair {
            produce: "nope".into(),
            token: "x".into(),
        }];
        let mut b = bare("b");
        b.needs = vec!["a".into()];
        b.produces = Some("vendor".into());
        b.results = vec!["approved".into()];
        let mut c = bare("c");
        c.needs = vec!["b".into()];
        a.needs.push("c".into());
        let err = validate(&plan(vec![a, b, c])).unwrap_err();
        let codes: Vec<_> = err.errors.iter().map(|p| p.code).collect();
        assert!(codes.contains(&"invalid_plan"));
        assert!(err.errors.iter().any(|p| p.message.contains("cycle")));
    }

    #[test]
    fn goal_produce_condition_and_both_flags() {
        let mut empty = plan(vec![bare("a")]);
        empty.goal = "  ".into();
        assert!(validate(&empty).unwrap_err().message.contains("goal"));

        let mut a = bare("a");
        a.produces = Some("invoice".into());
        let mut b = bare("b");
        b.produces = Some("invoice".into());
        assert!(validate(&plan(vec![a, b]))
            .unwrap_err()
            .errors
            .iter()
            .any(|p| p.code == "duplicate_value"));

        let mut producer = bare("vendor-step");
        producer.produces = Some("vendor".into());
        producer.results = vec!["approved".into()];
        let mut child = bare("child");
        child.when = vec![Pair {
            produce: "vendor".into(),
            token: "nope".into(),
        }];
        child.when_any = vec![vec![Pair {
            produce: "vendor".into(),
            token: "approved".into(),
        }]];
        let err = validate(&plan(vec![producer, child])).unwrap_err();
        assert!(err.errors.iter().any(|p| p.message.contains("both when")));
        assert!(err
            .errors
            .iter()
            .any(|p| p.message.contains("not a result")));

        let mut results = bare("r");
        results.results = vec!["ok".into()];
        assert!(validate(&plan(vec![results]))
            .unwrap_err()
            .errors
            .iter()
            .any(|p| p.code == "results_need_produce"));
    }

    #[test]
    fn accepts_the_invoice_shape() {
        let mut parse = bare("parse");
        parse.produces = Some("invoice".into());
        let mut vendor = bare("verify-vendor");
        vendor.needs = vec!["parse".into()];
        vendor.produces = Some("vendor".into());
        vendor.results = vec!["approved".into(), "rejected".into()];
        let mut pay = bare("pay");
        pay.needs = vec!["parse".into()];
        pay.when_any = vec![
            vec![Pair {
                produce: "vendor".into(),
                token: "approved".into(),
            }],
            vec![Pair {
                produce: "vendor".into(),
                token: "rejected".into(),
            }],
        ];
        assert!(validate(&plan(vec![parse, vendor, pay])).is_ok());
    }
}
