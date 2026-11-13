# Architecture

Relate Flow is two crates.

`relate-flow-core` owns every rule. `relate-flow` is a stateless binary: clap, one JSON line, an exit code. The skill installer lives in the binary because it never touches a store.

```text
agent or harness
    |  argv, env
    v
relate-flow
    |  Flow::{goal,add,set,drop,commit,execute,revise,abort,remove,prune,show,status,explain,log,next,report}
    v
relate-flow-core
    |  one transaction: load, rule, admission, surgical writes, commit
    v
relate::Database
```

## One transaction

`Flow::transact` connects, begins, loads the plan graph into memory, runs the command, applies a list of `Write` values, and commits. A refusal rolls back. `Error::Conflict` at commit becomes exit 5, `conflict`, retryable, fix "call next again". `Error::Locked` on open is retried for about two seconds, then `store_busy`.

Admission is a pure function of steps and attempts. It never sees a connection, so the disposition rules are tested without a database.

## What is stored and what is computed

Stored step states are `pending`, `running`, `succeeded`, `failed`, `blocked`, and `skipped`. `ready` is computed. After every report and at the start of `next`, the committed steps are recomputed and any state that changed is written back. A stored `skipped` with no remaining reason returns to `pending`.

`when` is AND. It does not skip until every pair is resolved. `whenAny` admits the step when one alternative matches, and skips it only when every alternative is resolved and none match. A skipped need skips its dependents. Only `succeeded` satisfies `needs`.

## Template and execution

A committed plan written by `goal` and `commit` is a template. `execute` copies its steps onto a new named plan, stores the bound inputs, and records the template name and version. `next` and `report` refuse a template with `not_a_run`. A revision of an execution copies `kind`, `inputs`, and the template link onto the new version. A later template commit does not rewrite executions already copied.

## next

After expiry and recompute:

1. This agent's open lease is renewed. A second step is not handed out.
2. The earliest claimable ready step is leased. Document order is the claim order. The actor, when set, must match.
3. A ready or running step for someone else is `waiting`. `because` walks `needs` until a condition matches, then returns that producer's note.
4. A failed step whose failure count has reached the budget is `replan`.
5. A blocked step is `blocked`.
6. Anything still not succeeded or skipped is `replan` ("no step can become ready").
7. Otherwise `done`.

An expired lease closes the attempt as `expired` and returns the step to `pending`. `expired` and `released` do not count toward the failure budget. The default budget is 2: the first failure returns the step to `pending` with `retry: 1`, and the second leaves it `failed`.

Attempt ids are `aN`, one greater than the highest id already in the store. They are not a counter on the plan node, so two claims do not have to write a shared plan field. Two claims of the same step still write that step, and Relate rejects the second commit with `Conflict`. The caller runs `next` again and receives the other ready step, or `waiting`.

## Revisions

`commit` of a draft that follows a committed plan classifies each step:

| Previous | Draft | Carry |
| --- | --- | --- |
| succeeded, same `doneWhen` | present | keep, and move `OF` onto the new step |
| succeeded, different `doneWhen` | present | reopen |
| anything else | present | reopen, and cancel a live lease as `released` |
| succeeded | absent | refuse `completed_work` |
| anything else | absent | remove |

`inputs` on a `run` instruction are produce-keyed notes. Direct needs contribute their produce. The step's own conditions contribute theirs. A matching ancestor condition contributes the note that opened the branch, which is why a human review sees the shortfall.

## Clock

The library takes a `Clock`. Production uses `SystemClock`. Tests use `ManualClock` or `FixedClock`. The CLI reads `RELATE_FLOW_NOW` and installs a `FixedClock`. The library does not read the environment.

## Skill

The markdown bundle is compiled in with `include_str!`. `skill install` writes it under the agent's skills directory and leaves `.relate-flow-installed`. The catalog is the same eleven agents Relate installs for. See ADR 0012.
