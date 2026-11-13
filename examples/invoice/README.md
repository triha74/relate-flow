# Invoice

Pay INV-1042, or hand it to a person. The plan is the one in `DESIGN.md`. `plan.sh` commits the template. Each run script executes a case before `next`.

| Script | What it shows |
| --- | --- |
| `run-discount-missing.sh` | The shortfall is escalated, a person approves $9,000, and `pay` runs. |
| `run-discount-rejected.sh` | The same shortfall, and the person rejects the invoice. |
| `run-discount-correct.sh` | The discount is already on the invoice, so review is skipped. |
| `run-vendor-rejected.sh` | An unapproved vendor still reaches `pay` after a person approves. |
| `run-missing-agreement.sh` | Pricing is blocked until `revise` and `commit` reopen it. |

```bash
cargo build --release -p relate-flow
bash examples/run-scenario.sh invoice
```

Each script prints only the CLI's JSON. The last line is `{"action":"done","version":1}` or, after a revision, a later version.
