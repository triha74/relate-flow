# Designing a workflow

You are given a challenge, not a plan. You write the plan from your
understanding of the task, then you commit it, then you do the first step
`next` leases you.

## What you are deciding

- The outcome, including the way the work can end other than success.
- Who may perform each step. Omit `--actor` only when any caller may.
- Which facts choose a branch. Each fact is a produce name and a closed set
  of result tokens. The tokens are the only vocabulary the branch has.
- Which steps must have succeeded (`--needs`) and which steps are admitted
  by a recorded token (`--when`, `--when-any`).
- What a later agent needs in order to do the step. That becomes `doneWhen`
  for the producer and the note you will report. Flow copies that note into
  `inputs`. It does not copy the rest of the plan.

## Needs and conditions

`--needs` is the strong edge.

- The step cannot run until every need has succeeded.
- If a need is skipped, this step is skipped too.
- A blocked or failed need leaves this step pending. It does not skip it.

So a step that must still be reachable after some other branch was skipped
must not need a step that branch skips. The invoice plan pays either because
the discount was correct or because a person approved a payment. `pay` needs
only `parse`. A rejected vendor skips pricing and discounts, and an approval
can still admit `pay`, because `pay` does not need those skipped steps.

`--when` is an AND of `produce=token` pairs. `--when-any` is a list of
alternatives, and each flag is one alternative. The step becomes ready when
one alternative matches, even if the others are still unresolved. It is
skipped only once every alternative is resolved and none match.

A condition may name a produce that you have not added yet. `commit` is the
check that the produce exists and the token is one of that step's results.

## Order, actors, and failure

When two steps are ready, `next` leases the earlier one in document order.
Add the step you want claimed first before the step that can wait. Two
actors can hold two steps at once. An agent is told `waiting` when the ready
or running step belongs to someone else.

`--retries` is the number of failed attempts a step may record. The default
is 2. A failure under the budget returns the step to pending and the next
`run` carries `"retry": N`. A failure at the budget leaves the step failed,
and `next` returns `replan`. `blocked` is not a failure and is not retried.
`revise` then `commit`, with no edits, reopens a blocked or failed step and
keeps every success whose `doneWhen` is unchanged. Changing `doneWhen` on a
success reopens that step. `set` says so with `"reopens": true` before you
commit.

## A shape worth reaching for

Use the invoice when a later human decision must stay reachable after an
earlier branch is skipped. Use the incident when one token has more than two
values and two actors work at the same time. Use onboarding when two checks
can run together and a missing document blocks the plan until `revise`.

Write the goal, add one step per command, `show`, then `commit`. That plan is a template. `execute NAME --input KEY=VALUE` copies it into a case, and `next` runs on the case. Keep the case out of the template text so the same template can run again. The frontier
of a new plan is the first step that has no unmet need and no unmatched
condition. If that is not the step you meant to run first, the edges are wrong.
Fix them before anyone reports.
