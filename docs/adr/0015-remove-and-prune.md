# ADR-0015: Remove one plan, prune finished executions

Status: Accepted

## Context

`execute` copies a template into a named plan and never deletes it. Each revision leaves the previous version on `SUPERSEDES`. A store that handles many cases keeps every run and every old version. Deleting the directory is already the way to wipe a store, and a command that deletes every plan is one wrong `-d` away from emptying a shared store.

## Decision

`remove` deletes every version of one name, including its steps and the attempts on those steps. An open lease is `lease_open` unless `--force`. Removing a template does not remove its executions, and removing an execution does not remove the template.

`prune` on a template deletes executions copied from it whose committed head is finished: every step is succeeded or skipped, no attempt is open, and no draft is open. Anything else stays, including the template.

Deleting a plan deletes its `Attempt` nodes. `DETACH DELETE` on a step only drops the `OF` relationship.

## Alternatives considered

- A `wipe` verb for the whole directory. The directory is the database, and the CLI has no confirmation prompt.
- Reusing `drop`. `drop` removes one step from a draft.

## Consequences

A finished case can be deleted without touching the template. A name freed by `remove` can be executed again. Attempt ids already used are not reused, because the sequence is the highest id ever stored and a deleted id is simply gone.
