# Customer onboarding

Open the account, or decline the application. `plan.sh` commits the template. Each run script executes a case before `next`. `intake` collects documents. `kyc` and `screening` then run at the same time. `risk-score` runs only when identity is `verified` and sanctions are `clear`. Anything else goes to `compliance`. The account opens on `risk=low` or `ruling=approved`. A declined ruling closes the file.

| Script | What it shows |
| --- | --- |
| `run-clear.sh` | Verified, clear, low risk, account opened. Compliance is skipped. |
| `run-mismatch-approved.sh` | A name mismatch is approved and the account still opens. |
| `run-hit-declined.sh` | A sanctions hit is declined. |
| `run-blocked.sh` | An unreadable document blocks the plan until `revise` and `commit`. |
| `run-reopen-donewhen.sh` | Changing `doneWhen` on a succeeded step reopens it. |

```bash
cargo build --release -p relate-flow
bash examples/run-scenario.sh onboarding
```
