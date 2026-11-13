use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use relate_flow_core::{
    parse_alternative, parse_pair, AddStep, FixedClock, Flow, FlowError, Outcome, Report, SetStep,
    DEFAULT_LEASE,
};
use serde_json::{json, Value};

use crate::output::{self, emit};
use crate::skill;

#[allow(dead_code)]
pub const VERBS: &[&str] = &[
    "goal", "add", "set", "drop", "commit", "revise", "abort", "execute", "remove", "prune",
    "show", "next", "report", "status", "explain", "log", "skill",
];

#[derive(Parser)]
#[command(
    name = "relate-flow",
    version,
    about = "Named plans in one store. Each call is the next step of one plan",
    override_usage = "relate-flow [OPTIONS] PLAN <COMMAND>"
)]
struct Cli {
    /// Relate store. RELATE_FLOW_DATABASE is the same value. The flag wins.
    #[arg(
        short = 'd',
        long = "database",
        global = true,
        env = "RELATE_FLOW_DATABASE",
        default_value = "./.relate-flow"
    )]
    database: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum ShowFormat {
    Json,
    Text,
}

#[derive(Subcommand)]
enum Command {
    /// Create the store and a draft, or update the open draft's goal.
    Goal {
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        retries: Option<u32>,
    },
    /// Append a step to the open draft.
    Add {
        id: String,
        #[arg(long)]
        title: String,
        #[arg(long = "done-when")]
        done_when: String,
        #[arg(long)]
        produces: Option<String>,
        #[arg(long = "needs")]
        needs: Vec<String>,
        #[arg(long = "results")]
        results: Vec<String>,
        #[arg(long = "when", conflicts_with = "when_any")]
        when: Vec<String>,
        #[arg(long = "when-any", conflicts_with = "when")]
        when_any: Vec<String>,
        #[arg(long)]
        actor: Option<String>,
        #[arg(long, conflicts_with = "after")]
        before: Option<String>,
        #[arg(long, conflicts_with = "before")]
        after: Option<String>,
    },
    /// Change the flags given for one step.
    Set {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long = "done-when")]
        done_when: Option<String>,
        #[arg(long, conflicts_with = "no_produces")]
        produces: Option<String>,
        #[arg(long = "no-produces", conflicts_with = "produces")]
        no_produces: bool,
        #[arg(long = "needs", conflicts_with = "no_needs")]
        needs: Vec<String>,
        #[arg(long = "no-needs", conflicts_with = "needs")]
        no_needs: bool,
        #[arg(long = "results", conflicts_with = "no_results")]
        results: Vec<String>,
        #[arg(long = "no-results", conflicts_with = "results")]
        no_results: bool,
        #[arg(long = "when", conflicts_with_all = ["when_any", "no_when"])]
        when: Vec<String>,
        #[arg(long = "when-any", conflicts_with_all = ["when", "no_when"])]
        when_any: Vec<String>,
        #[arg(long = "no-when", conflicts_with_all = ["when", "when_any"])]
        no_when: bool,
        #[arg(long, conflicts_with = "no_actor")]
        actor: Option<String>,
        #[arg(long = "no-actor", conflicts_with = "actor")]
        no_actor: bool,
        #[arg(long, conflicts_with = "after")]
        before: Option<String>,
        #[arg(long, conflicts_with = "before")]
        after: Option<String>,
    },
    /// Remove a step from the open draft.
    Drop { id: String },
    /// Check the draft and write the next version.
    Commit {
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
    /// Copy the committed template into a named execution.
    Execute {
        name: String,
        /// `KEY=VALUE` bound onto every step. Repeat the flag for each pair.
        #[arg(long = "input")]
        input: Vec<String>,
    },
    /// Copy the committed plan into a new draft.
    Revise,
    /// Delete the open draft.
    Abort,
    /// Delete this plan, its versions, and its attempts.
    Remove {
        /// Delete the plan even when a step is leased.
        #[arg(long)]
        force: bool,
    },
    /// Delete finished executions of this template.
    Prune,
    /// Print the open draft, or the committed plan.
    Show {
        /// `json` is one object. `text` draws the flow.
        #[arg(long, value_enum, default_value = "json")]
        output: ShowFormat,
    },
    /// Lease the next step for this agent, or say why there isn't one.
    Next {
        #[arg(long, env = "RELATE_FLOW_AGENT")]
        agent: Option<String>,
        #[arg(long)]
        lease: Option<String>,
        /// `json` is one object. `text` prints the instruction as a prompt.
        #[arg(long, value_enum, default_value = "json")]
        output: ShowFormat,
    },
    /// Admit the outcome of the step this agent holds.
    Report {
        step: String,
        #[arg(long, env = "RELATE_FLOW_AGENT")]
        agent: Option<String>,
        #[arg(long)]
        outcome: String,
        #[arg(long)]
        result: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        because: Option<String>,
        #[arg(long)]
        artifact: Option<String>,
    },
    /// List every step and its disposition.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Say why one step is in its current disposition.
    Explain { step: String },
    /// List attempts, newest first.
    Log {
        step: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Install or inspect the agent skill.
    Skill {
        #[command(subcommand)]
        action: SkillCommand,
    },
}

#[derive(Subcommand)]
enum SkillCommand {
    Install {
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        force: bool,
    },
    Remove {
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        force: bool,
    },
    List {
        #[arg(long)]
        agent: Option<String>,
    },
    Check {
        #[arg(long)]
        agent: Option<String>,
    },
}

pub fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match split_args(&args) {
        Invocation::Top { version, database } => {
            if version {
                print!("{}", Cli::command().render_long_version());
                return 0;
            }
            print_top_help(&database)
        }
        Invocation::Run {
            plan,
            rest,
            database,
        } => match parse_command(&rest, database.as_deref()) {
            Ok(cli) => dispatch(&plan, cli),
            Err(code) => code,
        },
    }
}

enum Invocation {
    Top {
        version: bool,
        database: String,
    },
    Run {
        plan: String,
        rest: Vec<String>,
        database: Option<String>,
    },
}

fn split_args(args: &[String]) -> Invocation {
    let mut database = None;
    let mut help = false;
    let mut version = false;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" {
            index += 1;
            break;
        }
        if arg == "-h" || arg == "--help" {
            help = true;
            index += 1;
            continue;
        }
        if arg == "-V" || arg == "--version" {
            version = true;
            index += 1;
            continue;
        }
        if arg == "-d" || arg == "--database" {
            if let Some(path) = args.get(index + 1) {
                database = Some(path.clone());
                index += 2;
                continue;
            }
        }
        if let Some(path) = arg.strip_prefix("--database=") {
            database = Some(path.to_string());
            index += 1;
            continue;
        }
        if arg.starts_with('-') {
            break;
        }
        break;
    }
    if index >= args.len() {
        return Invocation::Top {
            version,
            database: database.unwrap_or_else(default_database),
        };
    }
    if args[index] == "skill" {
        return Invocation::Run {
            plan: String::new(),
            rest: args[index..].to_vec(),
            database,
        };
    }
    let plan = args[index].clone();
    let rest = args[index + 1..].to_vec();
    if help || version {
        return Invocation::Top {
            version,
            database: database.clone().unwrap_or_else(default_database),
        };
    }
    Invocation::Run {
        plan,
        rest,
        database,
    }
}

fn default_database() -> String {
    std::env::var("RELATE_FLOW_DATABASE").unwrap_or_else(|_| "./.relate-flow".to_string())
}

fn parse_command(rest: &[String], database: Option<&str>) -> Result<Cli, i32> {
    let mut clap_args = vec!["relate-flow".to_string()];
    clap_args.extend(rest.iter().cloned());
    let has_database = rest
        .iter()
        .any(|arg| arg == "-d" || arg == "--database" || arg.starts_with("--database="));
    if let Some(path) = database {
        if !has_database {
            clap_args.push("-d".into());
            clap_args.push(path.to_string());
        }
    }
    match Cli::try_parse_from(clap_args) {
        Ok(cli) => Ok(cli),
        Err(err) => Err(match err.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                let _ = err.print();
                0
            }
            _ => {
                let message = err.to_string().replace('\n', " ");
                emit(&usage(&message, "fix the flags and run the command again"));
                2
            }
        }),
    }
}

fn print_top_help(database: &str) -> i32 {
    let mut out = Vec::new();
    let _ = Cli::command().write_help(&mut out);
    print!("{}", String::from_utf8_lossy(&out));
    println!();
    println!("Name the plan, then the command. `skill` is not a plan name.");
    println!();
    println!("  relate-flow invoice goal --text \"Pay the invoice, or hand it to a person\"");
    println!("  relate-flow invoice execute inv-1042 --input case=\"Northwind, INV-1042\"");
    println!("  relate-flow inv-1042 next --agent invoices");
    println!();
    print_plans(database);
    0
}

fn print_plans(database: &str) {
    println!("Plans in {database}:");
    if database != ":memory:" && !std::path::Path::new(database).exists() {
        println!("  (none)  relate-flow NAME goal --text TEXT");
        return;
    }
    match open(database).and_then(|flow| flow.catalog()) {
        Ok(plans) if plans.is_empty() => {
            println!("  (none)  relate-flow NAME goal --text TEXT");
        }
        Ok(plans) => {
            for plan in plans {
                let state = match (plan.kind.as_str(), plan.version, plan.draft) {
                    ("execution", Some(version), true) => format!("execution v{version}, draft"),
                    ("execution", Some(version), false) => format!(
                        "execution of {} v{version}",
                        plan.template.as_deref().unwrap_or("template")
                    ),
                    ("execution", None, _) => "execution draft".to_string(),
                    (_, Some(version), true) => format!("template v{version}, draft"),
                    (_, Some(version), false) => format!("template v{version}"),
                    (_, None, _) => "template draft".to_string(),
                };
                let goal = clip_goal(&plan.goal);
                println!("  {:<16} {:<28} {goal}", plan.name, state);
            }
        }
        Err(err) => println!("  ({}) {}", err.code, err.message),
    }
}

fn parse_inputs(raw: &[String]) -> Result<Vec<(String, String)>, FlowError> {
    let mut inputs = Vec::new();
    for item in raw {
        let Some((key, value)) = item.split_once('=') else {
            return Err(FlowError::usage(
                format!("--input {item} needs KEY=VALUE"),
                "pass --input KEY=VALUE",
            ));
        };
        if key.is_empty() || value.is_empty() {
            return Err(FlowError::usage(
                "--input needs a key and a value",
                "pass --input KEY=VALUE",
            ));
        }
        inputs.push((key.to_string(), value.to_string()));
    }
    Ok(inputs)
}

fn clip_goal(goal: &str) -> String {
    let mut chars = goal.chars();
    let head: String = chars.by_ref().take(72).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn dispatch(plan: &str, cli: Cli) -> i32 {
    let database = cli.database;
    match cli.command {
        Command::Goal { text, retries } => {
            finish(open_plan(plan, &database).and_then(|flow| flow.goal(text, retries)))
        }
        Command::Add {
            id,
            title,
            done_when,
            produces,
            needs,
            results,
            when,
            when_any,
            actor,
            before,
            after,
        } => {
            let spec = match add_spec(
                id, title, done_when, produces, needs, results, when, when_any, actor, before,
                after,
            ) {
                Ok(spec) => spec,
                Err(err) => return finish(Err(err)),
            };
            finish(open_plan(plan, &database).and_then(|flow| flow.add(spec)))
        }
        Command::Set {
            id,
            title,
            done_when,
            produces,
            no_produces,
            needs,
            no_needs,
            results,
            no_results,
            when,
            when_any,
            no_when,
            actor,
            no_actor,
            before,
            after,
        } => {
            let spec = match set_spec(
                id,
                title,
                done_when,
                produces,
                no_produces,
                needs,
                no_needs,
                results,
                no_results,
                when,
                when_any,
                no_when,
                actor,
                no_actor,
                before,
                after,
            ) {
                Ok(spec) => spec,
                Err(err) => return finish(Err(err)),
            };
            finish(open_plan(plan, &database).and_then(|flow| flow.set(spec)))
        }
        Command::Drop { id } => finish(open_plan(plan, &database).and_then(|flow| flow.drop(id))),
        Command::Commit { dry_run } => {
            finish(open_plan(plan, &database).and_then(|flow| flow.commit(dry_run)))
        }
        Command::Execute { name, input } => {
            let inputs = match parse_inputs(&input) {
                Ok(inputs) => inputs,
                Err(err) => return finish(Err(err)),
            };
            finish(open_plan(plan, &database).and_then(|flow| flow.execute(name, inputs)))
        }
        Command::Revise => finish(open_plan(plan, &database).and_then(|flow| flow.revise())),
        Command::Abort => finish(open_plan(plan, &database).and_then(|flow| flow.abort())),
        Command::Remove { force } => {
            finish(open_plan(plan, &database).and_then(|flow| flow.remove(force)))
        }
        Command::Prune => finish(open_plan(plan, &database).and_then(|flow| flow.prune())),
        Command::Show { output } => {
            let shown = open_plan(plan, &database).and_then(|flow| flow.show());
            if matches!(output, ShowFormat::Text) {
                if let Ok(value) = &shown {
                    print!("{}", crate::diagram::render(value));
                    return 0;
                }
            }
            finish(shown)
        }
        Command::Next {
            agent,
            lease,
            output,
        } => {
            let Some(agent) = agent.filter(|a| !a.is_empty()) else {
                return finish(Err(FlowError::usage(
                    "next requires --agent",
                    "pass --agent NAME or set RELATE_FLOW_AGENT",
                )));
            };
            let lease = match parse_lease(lease) {
                Ok(lease) => lease,
                Err(err) => return finish(Err(err)),
            };
            let next = open_plan(plan, &database).and_then(|flow| flow.next(agent.clone(), lease));
            if matches!(output, ShowFormat::Text) {
                if let Ok(value) = &next {
                    print!("{}", crate::prompt::render(plan, &agent, value));
                    return 0;
                }
            }
            finish(next)
        }
        Command::Report {
            step,
            agent,
            outcome,
            result,
            note,
            because,
            artifact,
        } => {
            let Some(agent) = agent.filter(|a| !a.is_empty()) else {
                return finish(Err(FlowError::usage(
                    "report requires --agent",
                    "pass --agent NAME or set RELATE_FLOW_AGENT",
                )));
            };
            let outcome = match parse_outcome(&outcome) {
                Ok(outcome) => outcome,
                Err(err) => return finish(Err(err)),
            };
            let report = Report {
                step,
                agent,
                outcome,
                result,
                note,
                because,
                artifact,
                artifact_hash: None,
            };
            finish(open_plan(plan, &database).and_then(|flow| flow.report(report)))
        }
        Command::Status { json } => {
            match open_plan(plan, &database).and_then(|flow| flow.status()) {
                Ok(value) => {
                    output::print_status(&value, json);
                    0
                }
                Err(err) => finish(Err(err)),
            }
        }
        Command::Explain { step } => {
            finish(open_plan(plan, &database).and_then(|flow| flow.explain(step)))
        }
        Command::Log { step, json } => {
            match open_plan(plan, &database).and_then(|flow| flow.log(step)) {
                Ok(value) => {
                    output::print_log(&value, json);
                    0
                }
                Err(err) => finish(Err(err)),
            }
        }
        Command::Skill { action } => match action {
            SkillCommand::Install { agent, force } => skill::install(agent.as_deref(), force),
            SkillCommand::Remove { agent, force } => skill::remove(agent.as_deref(), force),
            SkillCommand::List { agent } => skill::list(agent.as_deref()),
            SkillCommand::Check { agent } => skill::check(agent.as_deref()),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn add_spec(
    id: String,
    title: String,
    done_when: String,
    produces: Option<String>,
    needs: Vec<String>,
    results: Vec<String>,
    when: Vec<String>,
    when_any: Vec<String>,
    actor: Option<String>,
    before: Option<String>,
    after: Option<String>,
) -> Result<AddStep, FlowError> {
    Ok(AddStep {
        id,
        title,
        done_when,
        produces,
        needs,
        results,
        actor,
        before,
        after,
        when: parse_when(&when)?,
        when_any: parse_when_any(&when_any)?,
    })
}

#[allow(clippy::too_many_arguments)]
fn set_spec(
    id: String,
    title: Option<String>,
    done_when: Option<String>,
    produces: Option<String>,
    no_produces: bool,
    needs: Vec<String>,
    no_needs: bool,
    results: Vec<String>,
    no_results: bool,
    when: Vec<String>,
    when_any: Vec<String>,
    no_when: bool,
    actor: Option<String>,
    no_actor: bool,
    before: Option<String>,
    after: Option<String>,
) -> Result<SetStep, FlowError> {
    Ok(SetStep {
        id,
        title,
        done_when,
        produces,
        no_produces,
        needs: if needs.is_empty() && !no_needs {
            None
        } else {
            Some(needs)
        },
        no_needs,
        results: if results.is_empty() && !no_results {
            None
        } else {
            Some(results)
        },
        no_results,
        when: if when.is_empty() {
            None
        } else {
            Some(parse_when(&when)?)
        },
        when_any: if when_any.is_empty() {
            None
        } else {
            Some(parse_when_any(&when_any)?)
        },
        no_when,
        actor,
        no_actor,
        before,
        after,
    })
}

fn parse_when(flags: &[String]) -> Result<Vec<relate_flow_core::Pair>, FlowError> {
    flags.iter().map(|flag| parse_pair(flag)).collect()
}

fn parse_when_any(flags: &[String]) -> Result<Vec<Vec<relate_flow_core::Pair>>, FlowError> {
    flags.iter().map(|flag| parse_alternative(flag)).collect()
}

fn parse_outcome(text: &str) -> Result<Outcome, FlowError> {
    Outcome::parse(text)
        .filter(|o| !matches!(o, Outcome::Expired))
        .ok_or_else(|| {
            FlowError::usage(
                format!("unknown outcome '{text}'"),
                "pass succeeded, failed, blocked, or released",
            )
        })
}

fn parse_lease(text: Option<String>) -> Result<Duration, FlowError> {
    let Some(text) = text else {
        return Ok(DEFAULT_LEASE);
    };
    humantime::parse_duration(&text).map_err(|_| {
        FlowError::usage(
            format!("cannot parse lease duration '{text}'"),
            "pass a duration such as 15m or 1h, up to 24h",
        )
    })
}

fn open_plan(plan: &str, path: &str) -> Result<Flow, FlowError> {
    open(path).map(|flow| flow.on(plan))
}

fn open(path: &str) -> Result<Flow, FlowError> {
    let flow = Flow::open(path)?;
    if let Ok(raw) = std::env::var("RELATE_FLOW_NOW") {
        if !raw.is_empty() {
            let time = DateTime::parse_from_rfc3339(&raw).map_err(|_| {
                FlowError::usage(
                    format!("RELATE_FLOW_NOW '{raw}' is not RFC3339"),
                    "set RELATE_FLOW_NOW to an RFC3339 UTC timestamp",
                )
            })?;
            return Ok(flow.with_clock(Arc::new(FixedClock(time.with_timezone(&Utc)))));
        }
    }
    Ok(flow)
}

fn finish(result: Result<Value, FlowError>) -> i32 {
    match result {
        Ok(value) => {
            emit(&value);
            0
        }
        Err(err) => {
            emit(&err.to_json());
            err.exit_code()
        }
    }
}

fn usage(message: &str, fix: &str) -> Value {
    json!({"error": {"code": "usage", "message": message, "fix": fix, "retryable": false}})
}
