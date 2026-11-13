# ADR-0007: Injectable clock

Status: Accepted

## Context

Leases expire. Tests cannot wait 15 minutes, and the library should not read the process environment.

## Decision

Callers pass a `Clock`. Production uses `SystemClock`. Tests use `ManualClock` or `FixedClock`. The CLI honors `RELATE_FLOW_NOW` (RFC3339) by installing a `FixedClock`. That variable is test-only. Stamps are RFC3339 UTC with second precision.

## Alternatives considered

- Sleep in the tests.
- Have the library read `RELATE_FLOW_NOW` itself.

## Consequences

Lease expiry is deterministic. A library caller that wants the real clock uses `SystemClock`, which is the default from `Flow::open`.
