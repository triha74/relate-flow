# ADR-0004: Plan graph schema

Status: Accepted

## Context

The plan has to survive the process, and a revision has to keep attempts that already succeeded without rewriting history.

## Decision

Labels are `Plan`, `Step`, and `Attempt`. Relationships are `HAS_STEP`, `SUPERSEDES`, `NEEDS`, and `OF`. A draft plan has a null `version`. On commit, a kept success moves `OF` to the new step uid. The attempt's `stepId` slug does not change. A store that has nodes and no `:Plan` is `foreign_store`.

## Alternatives considered

- Store the plan as one JSON property.
- Copy attempts onto the new step as new rows.

## Consequences

History stays on the attempt that recorded it. Superseded plans remain in the store. `docs/storage.md` is the field list.
