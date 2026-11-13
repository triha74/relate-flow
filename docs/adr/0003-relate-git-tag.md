# ADR-0003: Depend on Relate through a git tag

Status: Accepted

## Context

Relate Flow embeds Relate. A path dependency would follow a local checkout that is not the release the product is specified against.

## Decision

The workspace depends on `relate = { git = "https://github.com/relatedb/relate", tag = "v1.0.1" }`. The API used is `Database::open`, `connect`, `begin`, `commit`, `rollback`, and `query_with_params`.

## Alternatives considered

- A path dependency on a local checkout.
- A crates.io release. v1.0.1 is published as a git tag.

## Consequences

Builds need the git tag. Moving to a newer Relate is a deliberate version bump, not a side effect of a local edit.
