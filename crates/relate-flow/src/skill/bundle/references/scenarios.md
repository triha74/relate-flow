# Two more shapes

These are patterns. Build the template that fits the task, `execute` a case with the facts in `--input`, then drive that case. Do not copy a step you do not need.

## Incident response

Actors are `oncall`, `comms`, and `ic`. The goal is to restore the service,
or to escalate it.

`triage` (actor `oncall`) produces `severity` with results `sev1`, `sev2`,
and `sev3`. `page-ic` (actor `ic`) runs when `severity=sev1`. `mitigate`
(actor `oncall`) produces `mitigation` with results `resolved` and
`unresolved`. `status-page` (actor `comms`) runs when severity is `sev1` or
`sev2`, so a sev1 incident has the incident commander, the on-call, and
communications leased at the same time. An agent whose step is not the ready
one is told `waiting`.

`vendor-escalation` runs when `mitigation=unresolved`. `postmortem` runs when
severity is `sev1` or `sev2`, after mitigation has succeeded. `close-ticket`
is admitted by `mitigation=resolved` or by the vendor escalation's produce,
and it does not need the steps a sev3 incident skips.

A failed `mitigate` under the budget comes back as `run` with `"retry": 1`.
Two failures, with the default budget of 2, leave the step failed and `next`
returns `replan` with because `failed 2 times`. `revise` then `commit`
reopens it.

## Customer onboarding

The goal is to open the account, or to decline the application. Actors are
`intake`, `kyc`, `screening`, and `compliance`.

`collect-docs` (actor `intake`) produces `application`. `verify-identity`
(actor `kyc`) and `sanctions-screen` (actor `screening`) both need only that
step, so the two checks are claimed by two agents at once. Identity produces
`identity` (`verified` or `mismatch`). Sanctions produces `sanctions`
(`clear` or `hit`).

`risk-score` needs both checks and runs when `identity=verified` and
`sanctions=clear`. It produces `risk` (`low` or `high`). `compliance-review`
(actor `compliance`) is admitted by `identity=mismatch`, `sanctions=hit`, or
`risk=high`. One recorded match is enough, so a mismatch admits the review
while sanctions is still open. `open-account` is admitted by `risk=low` or
`ruling=approved`. `decline` runs when `ruling=declined`. Neither account
step needs the check that a mismatch skips, so an approval after a mismatch
can still open the account.

An unreadable passport is `blocked` on `verify-identity`, not `failed`.
`next` returns `blocked`. `revise` and `commit` reopen that step and keep
`collect-docs`. Changing `doneWhen` on a step that already succeeded reopens
it. `set` tells you, with `"reopens": true`, before you commit.
