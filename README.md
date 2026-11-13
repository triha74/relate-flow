# Relate Flow

Relate Flow embeds [Relate](https://github.com/relatedb/relate) and keeps many named plans in one store. Each call names a plan and tells an agent the single next instruction. The agent does the work and reports. Flow admits the report or refuses it.

The binary is `relate-flow`. It is not a runtime: it does not run steps or judge a `doneWhen`.

## Quick start

```bash
cargo build -p relate-flow
export PATH="$PWD/target/debug:$PATH"

relate-flow invoice goal --text "Pay the invoice, or hand it to a person"
relate-flow invoice add parse --title "Parse the invoice" \
  --done-when "Vendor, lines, and the billed total are recorded" \
  --produces invoice --actor invoices
relate-flow invoice commit
relate-flow invoice execute inv-1042 --input case="Northwind, INV-1042"
relate-flow inv-1042 next --agent invoices
```

The store defaults to `./.relate-flow`. `-d` and `RELATE_FLOW_DATABASE` select another one. `--agent` falls back to `RELATE_FLOW_AGENT`.

Every result is one compact JSON object on stdout. Exit 0 is an instruction or a successful write. Exit 2 is usage. Exit 4 is a refusal. Exit 5 is a conflict or a busy store. Exit 1 is a store or skill filesystem failure. `relate-flow --help` and `--version` print plain text and exit 0.

## Examples

Three plans ship as shell scripts. Each script ends with `next` returning `done`.

```bash
cargo build --release -p relate-flow
bash examples/run-scenario.sh invoice
bash examples/run-scenario.sh incident
bash examples/run-scenario.sh onboarding
```

The scripts call `target/release/relate-flow`. Set `RELATE_FLOW_BIN` to use a different binary. `gosonic ci run build` writes a Linux binary to `target/sonic/release/relate-flow`.

## Skill

`relate-flow skill install` writes the embedded skill into the agents it detects. The skill teaches an agent to design a plan from the task in front of it, then drive that plan. It does not describe how to change this repository. That contract is `AGENTS.md`.

## Docs

- [DESIGN.md](DESIGN.md) is the product contract.
- [docs/cli.md](docs/cli.md) is the command reference.
- [docs/architecture.md](docs/architecture.md) is the crate and admission layout.
- [docs/storage.md](docs/storage.md) is the Relate graph.
- [docs/scenarios/](docs/scenarios/) walks the three example plans.
- [docs/adr/](docs/adr/) records the decisions.

## Develop

```bash
make test
make lint
make coverage
```

CI is gosonic. Stages live in `.sonic/ci.yml`. `make sonic-smoke`, `make sonic-full`, and `make sonic-examples` run them locally when Docker and gosonic are installed.
