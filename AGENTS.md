# Agents changing this repository

`DESIGN.md` is the product contract. A behavior change updates it in the same change. An architectural decision gets an ADR in `docs/adr/` first.

The library in `crates/relate-flow-core` owns every rule: drafts, validation, admission, leases, reports, and revisions. The CLI in `crates/relate-flow` parses flags, prints one JSON object, and exits. It holds no transition logic. A store holds many named plans. A committed plan is a template until `execute` copies it into a run. `next` and `report` run on the execution. The command is `relate-flow PLAN COMMAND`.

Do not execute a step, judge a `doneWhen`, or add a workflow language, a service, or a Cypher surface for agents.

A new verb, flag, error code, or disposition needs a CLI test and a row the command matrix can see. A new workflow shape needs a scenario test. Error codes live in `ERROR_CODES`.

CI commands live in `.sonic/ci.yml`. The GitHub workflow only builds gosonic and runs one stage per job. `make lint`, `make test`, and `make coverage` are the local equivalents. Coverage stays at or above 95% lines.

The skill bundle under `crates/relate-flow/src/skill/bundle/` describes the product the binary ships. A CLI or admission change updates the bundle in the same change. `AGENTS.md` is this contract. The skill is for an agent driving a plan, not for an agent changing the repo.

Relate is the git dependency `tag = "v1.0.1"`. Do not replace it with a path dependency.
