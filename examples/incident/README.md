# Incident response

Restore the service, or escalate it. Actors are `oncall`, `ic`, and `comms`. `plan.sh` commits the template. Each run script executes a case named `case` before `next`.

`triage` records `sev1`, `sev2`, or `sev3`. `page-ic` runs only for `sev1`. `status-page` runs for `sev1` or `sev2` while mitigation is in progress. `vendor-escalation` runs when mitigation is `unresolved`. `close-ticket` runs when mitigation is `resolved` or the vendor was `paged`, so a skipped vendor escalation does not block the close.

| Script | What it shows |
| --- | --- |
| `run-sev1-resolved.sh` | Page the commander, mitigate, post a notice, write the postmortem, close. |
| `run-sev2-unresolved.sh` | Skip the page, escalate to the vendor, then close. |
| `run-sev3.sh` | Skip paging, the status page, and the postmortem. |
| `run-retry.sh` | One failed mitigation comes back with `retry: 1`. |
| `run-replan.sh` | Two failures return `replan`. `revise` and `commit` continue the work. |

```bash
cargo build --release -p relate-flow
bash examples/run-scenario.sh incident
```
