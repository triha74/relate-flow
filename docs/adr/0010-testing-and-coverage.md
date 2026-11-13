# ADR-0010: Testing strategy and coverage gate

Status: Accepted

## Context

The product is a set of refusals and state transitions. A green build that misses a verb or an error code would ship a contract the tests do not hold.

## Decision

Library tests cover the rules, including proptest over acyclic plans and a concurrency test for two claims. CLI tests are one file per verb, with insta snapshots and timestamps redacted. A matrix test fails when a verb or an `ERROR_CODES` entry is not named by a test. Scenario tests drive the binary through invoice, incident, onboarding, and lifecycle. `cargo llvm-cov --workspace --fail-under-lines 95` is the gate. Example shell scripts are not a cargo test. The gosonic `examples` stage runs them.

## Alternatives considered

- Snapshot the whole store after every command.
- Count example scripts as `cargo test`.

## Consequences

A new error code or verb has to be mentioned by a test or the matrix fails. Coverage below 95% fails `make coverage` and the coverage stage.
