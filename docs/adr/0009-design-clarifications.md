# ADR-0009: Design clarifications

Status: Accepted

## Context

`DESIGN.md` specifies the product and leaves a few representations open: attempt ids, lease syntax, and the shape of `explain`, `log`, and the status table.

## Decision

Attempt ids are `aN`, one greater than the highest existing id. They are global in the store, not a counter on the plan. `--lease` is a humantime duration, capped at 24 hours, default 15 minutes. `explain` returns `step`, `disposition`, `why`, `needs`, `when`, `whenAny`, `missing.needs`, `missing.tokens`, and `lastAttempt`. `log` returns `attempts`, newest first, capped at 50. The status and log tables are plain aligned columns: id, disposition, actor, result; and attempt, step, outcome, time.

## Alternatives considered

- Per-step attempt numbers that restart at `a1`.
- A custom duration grammar.

## Consequences

Two concurrent claims may briefly compute the same attempt id. The concurrency test accepts that. The `why` strings match the table in `DESIGN.md`.
