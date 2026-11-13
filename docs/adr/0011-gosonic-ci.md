# ADR-0011: CI is gosonic

Status: Accepted

## Context

Relate runs its portable stages from `.sonic/ci.yml` and uses GitHub Actions only to build a pinned gosonic and launch one stage per job. Relate Flow should work the same way, including a lane that runs the example plans.

## Decision

`.sonic/ci.yml` is the pipeline. Stages are `lint`, `build`, `test`, `coverage`, and `examples`. `build` is `cargo build --workspace --release` with `./target/sonic` bind-mounted at the cargo target directory, so the Linux binary is `target/sonic/release/relate-flow` and a host `cargo build --release` in `target/release` is left in place. The examples stage is a matrix of `invoice`, `incident`, and `onboarding`. Suites are `smoke-test` (lint, build, test), `full-test` (those plus coverage and examples), and `all-examples`. The image is `localhost/relate-flow-sonic-rust:1.88`, Ubuntu 24.04, Rust 1.88, rustfmt, clippy, and cargo-llvm-cov. The workflow checks out `triha74/gosonic` at `36b8e47f575c2436515e3ba5f4819771131e83ff`, builds it with `CGO_ENABLED=0`, and runs one stage per job. `scripts/sonic-cache-sync.sh` seeds and saves the cargo volumes around `actions/cache`. The cache key includes the stage name.

## Alternatives considered

- Hand-written `cargo` steps in the workflow.
- Relate's mold, nextest, and Docker CLI lanes.

## Consequences

The commands live in one file. Docs-only path filters and the draft-versus-ready split are omitted. `make sonic-smoke`, `make sonic-full`, and `make sonic-examples` call `gosonic ci run`. `make clean` removes `target` and the `gosonic-cache-cargo-home` and `gosonic-cache-cargo-target` volumes.
