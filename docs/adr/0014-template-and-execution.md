# ADR-0014: A template and its executions

Status: Accepted

## Context

Reporting wrote step state onto the named plan. A second incident on that name saw steps that had already succeeded. One-off work and a repeated workflow were the same object.

## Decision

`goal` and `commit` write a template. `execute NAME` copies the committed template into a new plan, stores `--input KEY=VALUE` pairs on that plan, and records the template name and version. `next` and `report` refuse a template with `not_a_run`. The bound pairs are merged into `inputs` on every `run`. An input key that matches a produce is `input_conflicts`. A name already in the store is `name_taken`. `revise` on an execution changes that run and copies the template link and inputs forward. A later template commit does not change executions already copied. `execute` does not use `SUPERSEDES`.

## Alternatives considered

- Keep one plan per incident and rewrite the template text for each case. The first step then has nothing to read unless the case is baked into `title` and `doneWhen`, and the template cannot be run twice.
- A `--run` flag on every command. The store already isolates a lineage by name, so the execution is a plan name.

## Consequences

Designing one task is commit, then one `execute`, then `next`. A repeated workflow is many executions of one template. Plans loaded without `kind` count as executions, so a store written before this change still accepts `next`.
