# ADR-0012: The agent skill ships inside the binary

Status: Accepted

## Context

An agent driving a plan needs the design method and the drive loop next to the binary it is calling. An agent changing this repository needs a different contract.

## Decision

The skill is markdown compiled into the binary with `include_str!`. `relate-flow skill install|remove|list|check` writes it. The directory name is `relate-flow`. The ownership marker is `.relate-flow-installed`. The catalog is Relate's eleven agents. Install creates a missing directory, updates one it owns, and adopts an unmarked directory whose `SKILL.md` says `name: relate-flow`. Anything else is refused unless `--force`. Remove deletes only a directory carrying the marker. `check` exits 4 with `skill_drift` when a copy does not match this binary. The skill teaches the agent to design a workflow from the task, then drive it. `AGENTS.md` is the contract for changing the repo. `DESIGN.md` has a Skill section so the design stays the source of truth.

## Alternatives considered

- A skill checked out from the repo at install time.
- One document that both drives plans and edits the source.

## Consequences

`skill` is a verb `DESIGN.md` did not originally list. It never opens a store, and it still prints one JSON object. A CLI or admission change updates the bundle in the same change.
