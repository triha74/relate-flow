# ADR-0008: JSON envelope for every error path

Status: Accepted

## Context

An agent has to read every failure the same way, including a mistake in the flags.

## Decision

clap is parsed with `try_parse`. Usage errors become `{"error":{"code":"usage","message","fix","retryable":false}}` and exit 2. Refusals exit 4, conflicts and `store_busy` exit 5, and store or skill IO failures exit 1. `--help` and `--version` stay plain text and exit 0. A multi-problem commit puts the first fix in `fix` and the full list in `errors`.

## Alternatives considered

- Let clap print its own usage text to stderr.
- A second JSON mode behind a flag.

## Consequences

Agents parse stdout only. Scripts in `examples/` can treat every line as JSON. A human at a terminal still gets a table from `status` and `log`.
