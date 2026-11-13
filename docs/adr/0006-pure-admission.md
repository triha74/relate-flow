# ADR-0006: Pure in-memory admission

Status: Accepted

## Context

Dispositions, skip reasons, and the frontier are functions of the plan and the attempts. Mixing them into Cypher would make the rules untestable and easy to drift.

## Decision

`admission` never touches the database. Stored states are pending, running, succeeded, failed, blocked, and skipped. Ready is computed. After a report, and at the start of `next`, the committed steps are recomputed and only the steps whose state changed are written.

## Alternatives considered

- Compute readiness in Cypher on every read.
- Store `ready` as a state.

## Consequences

The admission table in `DESIGN.md` is unit-tested directly. A stored skip with no remaining reason returns to pending, so tests must give a real skip reason.
