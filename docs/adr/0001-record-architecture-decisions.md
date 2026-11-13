# ADR-0001: Record architecture decisions

Status: Accepted

## Context

Relate Flow is a new product. The design in `DESIGN.md` leaves room for how the binary, the store, and the tests are put together. Those choices will be hard to recover from the code alone.

## Decision

Architectural decisions live in `docs/adr/` as `ADR-NNNN`, with Status, Context, Decision, Alternatives considered, and Consequences. A behavior change updates `DESIGN.md` in the same change. An architectural change gets an ADR first. Numbers start at 0001 for this repository.

## Alternatives considered

- Leave the decisions in commit messages.
- Reuse Relate's ADR counter.

## Consequences

`AGENTS.md` tells a later change to add an ADR before it changes the architecture. This series is local to relate-flow.
