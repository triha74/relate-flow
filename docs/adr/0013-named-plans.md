# ADR-0013: Many named plans in one store

Status: Accepted

## Context

The first design kept one plan lineage in a store. A directory then meant one piece of work, and the command line was `relate-flow COMMAND`. Working on invoice, incident, and onboarding at the same time meant three directories.

## Decision

A store holds many plans. The plan `name` is a slug stored on the `Plan` node and shared by that lineage's draft and committed versions. The command is `relate-flow PLAN COMMAND`. `relate-flow --help` lists the names in the store, with the goal and whether the plan is a draft, a version, or both. `skill` is not a plan name. A command loads the whole graph, then keeps only the named lineage and the attempts on its steps. Attempt ids stay unique across the store.

## Alternatives considered

- One directory per plan, which is what the first design did.
- A `use` command that selects a plan for later calls. The CLI is stateless, so the name has to travel with every invocation.

## Consequences

`goal` creates the name when it is new. A second name in the same store does not see the first name's steps. Help does not create a store that does not exist yet. Plans written before `name` existed have an empty name and do not appear in the list.
