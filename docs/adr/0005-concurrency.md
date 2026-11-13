# ADR-0005: Concurrency

Status: Accepted

## Context

Relate allows one writable owner of a directory, and in-process transactions conflict when they write the same node. Two agents will call `next` at the same time.

## Decision

Each command is one transaction. Open retries `Error::Locked` for about two seconds, then exits 5 with `store_busy` and `retryable: true`. `Error::Conflict` at commit exits 5 with `conflict`, `retryable: true`, and fix "call next again". A claim writes the step it leases and the new attempt. It does not rewrite `NEEDS`, and attempt ids are not a counter on the plan node. A closed attempt reported again with a different outcome is `conflict` and is not retryable.

## Alternatives considered

- A process-wide mutex around every command.
- A hot `nextAttempt` property on the plan.

## Consequences

Claims of different steps can both commit. Claims of the same step produce one winner. The caller retries `next`. A second process waits or gets `store_busy`.
