---
name: relate-flow
description: Relate Flow keeps one plan for a piece of work and, on each call, tells the agent the single next step. Use this skill to turn a task into a plan, then to drive that plan with next and report.
version: {{VERSION}}
---

# Relate Flow

Relate Flow is not a runtime. It does not run a step, call a tool, or decide
whether the work was done well. It decides which step may be performed, by
whom, and what a report is allowed to say. The binary is `relate-flow`. One
invocation opens the store, performs one admission, prints one JSON object,
and exits.

You do the work. You write the plan before you do any step. A recorded result
selects the branch. You do not.

## Design the plan from the task

Read [designing a workflow](references/designing-a-workflow.md) before you add
a step. The short form:

1. Pick a plan name, a slug, and state the outcome in one goal, including the
   other way the work can end. "Pay INV-1042, or hand it to a person."
   The command is `relate-flow invoice goal --text "..."`. `relate-flow --help`
   lists the plans in the store. The default store is `./.relate-flow`. `-d`
   and `RELATE_FLOW_DATABASE` select another. `:memory:` is for tests.
2. Name the actors. A step only one of them may perform gets `--actor`. A
   step anyone may perform omits it.
3. Split the work into steps. `doneWhen` is shown to the agent who runs the
   step. Flow never judges that text. The note on the report is the evidence.
4. A fact that chooses a branch becomes a produce name and result tokens
   (`approved`/`rejected`, `sev1`/`sev2`/`sev3`). Write every branch into the
   plan now. Do not invent the next step on the day a result comes back.
5. `--needs` is the strong edge. The step waits until each need has succeeded,
   and it is skipped when a need is skipped. A step that must still run after
   some other branch was skipped does not need the steps that branch skips.
   Put that condition in `--when` or `--when-any`. The invoice `pay` needs
   only `parse`, and is admitted by `discounts=correct` or `decision=approved`,
   so a rejected vendor can still be paid after a person approves.
6. `--when` requires every pair. `--when-any` admits the step when any one
   alternative matches. One flag can AND pairs with commas:
   `--when-any discounts=correct,vendor=approved`. The two flags never share
   a step. A producer may be added later. `commit` is what refuses a token
   or a produce name the plan does not have.
7. Document order is add order, and it is the order `next` claims when more
   than one step is ready. `--before` and `--after` place a step elsewhere.
8. `show` is how you read back what you built. `show --output text` draws the
   flow. `commit` checks the whole plan.
   A refusal names the fix. Repair one problem and call again.

The [invoice](references/invoice.md), incident, and onboarding walkthroughs in
[scenarios](references/scenarios.md) are patterns for that method, not scripts
to copy.

## Drive the plan

Commit the template, then copy it into a case. `next` and `report` name that case, not the template. `next` on a template is refused. The fix is `execute`.

```bash
relate-flow invoice commit
relate-flow invoice execute inv-1042 --input case="Northwind, INV-1042, billed 10000."
relate-flow inv-1042 next --agent invoices
relate-flow inv-1042 next --agent invoices --output text
```

`--output text` prints that instruction as a prompt, including the `succeeded` and `failed` report commands. JSON stays the default. A refusal stays JSON.

`--input KEY=VALUE` repeats. Those values are in `inputs` on every `run`, including the first step. A key that is already a produce is refused. `revise` on the case changes that case. The template stays reusable.

`remove` deletes one plan. An open lease is refused unless `--force`. `prune` on a template deletes executions whose every step succeeded or was skipped. A running case stays.

`next` and `report` require `--agent` (or `RELATE_FLOW_AGENT`). The flag wins.

Switch on `action`. `run`, `waiting`, `blocked`, `replan`, and `done` all
exit 0. They are answers.

| `action` | What you do |
| --- | --- |
| `run` | You hold the lease. Perform that step and only that step. `report` it. |
| `waiting` | Work is ready or running, and none of it is yours. Stop until the named actor reports. |
| `blocked` | A step is blocked and nothing else can run. `revise`, then `commit`, reopens it and keeps work that succeeded. |
| `replan` | A step used its failure budget, or nothing left can become ready. Change the plan with `revise`. |
| `done` | Every step succeeded or was skipped. |

Then call `next` again. `report` does not lease the following step.

A `run` carries the step, its `doneWhen`, the case inputs and the notes it needs under `inputs`,
and `leaseUntil`. When the attempt is a retry it also carries `"retry": N`,
the number of failed attempts already recorded. Calling `next` again while
you hold the lease renews it and returns the same step with `"renewed": true`.

```bash
relate-flow report verify-discounts --outcome succeeded --result incorrect \
  --note "Agreed total $9,000, billed $10,000, shortfall $1,000."
```

| Outcome | Requires | Effect |
| --- | --- | --- |
| `succeeded` | `--note`. `--result` when the step declares results, and it must be one of those tokens. | The step succeeds. Dispositions are recomputed. |
| `failed` | `--because` | Under the budget the step returns to pending. At the budget it stays failed and `next` returns `replan`. |
| `blocked` | `--because` | The step is blocked. It is not retried. |
| `released` | Nothing | The lease closes. The step returns to pending. The failure count is unchanged. |

Report the step you hold. A report on any other step is refused, and the fix
names the step you do hold. `--artifact PATH` records the path and the
SHA-256. Flow does not copy the file. An unreadable path writes nothing.

The default lease is 15 minutes. `--lease` overrides it, up to 24 hours.
There is no heartbeat. An expired lease is closed with outcome `expired`
when `next` or `report` next runs. `expired` does not count as a failure.

## Building commands

`next` and `report` are refused while a draft is open. The fix is `commit`
or `abort`. `status`, `show`, `explain`, and `log` still work.

```bash
relate-flow invoice goal --text "Pay the invoice, or hand it to a person"
relate-flow invoice add parse --title "Parse the invoice" \
  --done-when "Vendor, lines, stated discounts, and the billed total are recorded" \
  --produces invoice --actor invoices
relate-flow invoice commit
relate-flow invoice execute inv-1042 --input case="Northwind, INV-1042, billed 10000."
```

`goal` creates the store and an empty draft. `--retries N` sets the failure
budget and defaults to 2. A later `goal` on the same draft updates the text,
the retries, or both. Called against a committed plan, it is refused. The
fix is `revise`.

`revise` copies the committed plan into a new draft. `abort` deletes the
draft. Aborting when no draft exists exits 0. `set` changes only the flags
it is given. A repeated list flag replaces that whole list. `--no-actor`,
`--no-produces`, `--no-needs`, `--no-results`, and `--no-when` clear those
fields. `set` of `--done-when` on a step that succeeded includes
`"reopens": true`. `drop` is refused when another step still references the
id, and when that id succeeded on the committed plan.

`commit` prints `version`, `kept`, `reopened`, `added`, `removed`, and
`frontier`. It does not lease a step. `commit --dry-run` prints the same
object with `"dryRun": true` and leaves the draft.

## Exit codes

Stdout is one JSON object on every path, including a mistake. Read `fix` and
do that. A plan with several problems keeps that shape, puts the first
problem's fix in `fix`, and lists every problem in `errors`.

| Exit | Meaning |
| --- | --- |
| 0 | An instruction or a successful write, including `done`, `waiting`, `blocked`, and `replan`. |
| 2 | Usage. Unknown verb, missing flag, `--when` together with `--when-any`, `--before` together with `--after`. |
| 4 | Refusal. Invalid plan, illegal outcome, missing note or result, report on a step you do not hold. |
| 5 | Conflict. The lease is held by someone else, the store is busy, or this attempt was already closed with a different outcome. |
| 1 | The store could not be opened or written. |

`retryable` is true when you should call `next` again. The codes and the
`why` strings are in [errors](references/errors.md).

Do not write Cypher to move a step. Cypher is not the agent API.
