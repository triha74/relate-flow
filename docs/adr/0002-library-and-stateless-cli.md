# ADR-0002: Library plus stateless CLI as two crates

Status: Accepted

## Context

Every admission rule has to be testable without a terminal, and the process that an agent runs has to do exactly one thing and exit.

## Decision

`relate-flow-core` owns drafts, validation, admission, leases, reports, and revisions. `relate-flow` parses flags, calls the library, prints one JSON object, and exits. The binary holds no transition logic. The skill installer is in the binary because it never opens a store.

## Alternatives considered

- One crate, with the rules behind the CLI only.
- A long-running service that keeps the plan in memory.

## Consequences

Library tests drive `Flow` directly. CLI tests drive the binary. A rule change is a library change, and the CLI stays a parser.
