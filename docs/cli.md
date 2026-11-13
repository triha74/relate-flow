# relate-flow

```text
relate-flow [OPTIONS]
relate-flow [OPTIONS] PLAN COMMAND
relate-flow skill COMMAND
```

One invocation opens the store, performs one admission, prints one compact JSON object on stdout, and exits. `--help` and `--version` print plain text and exit 0.

`PLAN` is a slug: a lowercase letter, then lowercase letters, digits, and hyphens. `skill` is reserved and is not a plan name. Step ids, produce names, result tokens, actors, and agent names are slugs. A condition is `produce=token`.

A committed plan built with `goal` and `commit` is a template. `execute` copies it into a named execution. `next` and `report` run on the execution.

## Options

| Flag | Environment | Default | Meaning |
| --- | --- | --- | --- |
| `-d`, `--database PATH` | `RELATE_FLOW_DATABASE` | `./.relate-flow` | Relate store. The flag wins. It may come before the plan or after the command. |
| `-h`, `--help` | | | Plain-text help. With no command, also lists the plans in the store. |
| `-V`, `--version` | | | Plain-text version. |

`relate-flow` and `relate-flow -d PATH` list the plans in that directory. A missing directory prints `(none)` and does not create the store. Each line is the name, then one of:

| State | Meaning |
| --- | --- |
| `template draft` | A template with no committed version. |
| `template vN` | A committed template. |
| `template vN, draft` | A committed template with a revision open. |
| `execution of TEMPLATE vN` | A run copied from that template. |
| `execution vN, draft` | That run has a revision open. |

## Environment

| Variable | Read by | Meaning |
| --- | --- | --- |
| `RELATE_FLOW_DATABASE` | every command | Store path. `--database` wins. |
| `RELATE_FLOW_AGENT` | `next`, `report` | Caller. `--agent` wins. |
| `RELATE_FLOW_NOW` | the CLI only | RFC3339 clock for tests. The library does not read it. |

## Exit status

| Exit | When |
| --- | --- |
| 0 | The command was admitted, including `waiting`, `blocked`, `replan`, and `done`. `--help` and `--version`. |
| 1 | The store could not be opened or written, or a skill directory could not be written. |
| 2 | The flags are wrong. The body is still JSON, with `code` `usage`. |
| 4 | The plan refused the command. `fix` names the next command. |
| 5 | The store is busy, or two writes raced. |

An error is one object:

```json
{"error":{"code":"not_leased","message":"…","fix":"…","retryable":false}}
```

`commit` with several problems puts the first `fix` in `fix` and lists every problem in `errors`. Each entry has `code`, `message`, `fix`, and `step` when the problem is about one step. `retryable` is true when the caller should call `next` again.

## Commands

### goal

```text
relate-flow PLAN goal [--text TEXT] [--retries N]
```

Creates a template draft, or updates the open draft. The first goal needs `--text`. `--retries` is the failure budget. The default is 2. A later goal on the same draft updates the text, the retries, or both.

Stdout: `{"goal","retries"}`.

| Code | Exit | When |
| --- | --- | --- |
| `usage` | 2 | Neither `--text` nor `--retries` was passed. |
| `empty_field` | 4 | The text is blank. |
| `needs_revise` | 4 | A plan is committed and no draft is open. |
| `no_plan` | 4 | The name has no plan. |

### add

```text
relate-flow PLAN add ID --title TEXT --done-when TEXT
    [--produces NAME] [--needs ID]... [--results TOKEN]...
    [--when PRODUCE=TOKEN]... [--when-any PRODUCE=TOKEN[,PRODUCE=TOKEN]]...
    [--actor NAME] [--before ID | --after ID]
```

Appends a step to the open draft. `--needs`, `--results`, `--when`, and `--when-any` repeat. `--when` is AND. Each `--when-any` is one alternative, and alternatives are OR. Repeating the same add is a no-op.

Stdout: `{"step","steps"}`.

| Code | Exit | When |
| --- | --- | --- |
| `usage` | 2 | A required flag is missing, or `--when` is combined with `--when-any`, or `--before` with `--after`. |
| `no_plan` | 4 | The name has no plan. |
| `no_draft` | 4 | A plan is committed and no draft is open. |
| `bad_slug` | 4 | An id, produce, result, actor, or need is not a slug. |
| `empty_field` | 4 | The title or done-when is blank. |
| `step_differs` | 4 | The id exists with different flags. |
| `duplicate_value` | 4 | A produce name, result, or need is repeated. |
| `results_need_produce` | 4 | `--results` was passed without `--produces`. |
| `unknown_step` | 4 | `--before` or `--after` names a step that is not in the draft. |

### set

```text
relate-flow PLAN set ID
    [--title TEXT] [--done-when TEXT]
    [--produces NAME | --no-produces]
    [--needs ID]... [--no-needs]
    [--results TOKEN]... [--no-results]
    [--when PRODUCE=TOKEN]... [--when-any PRODUCE=TOKEN]... [--no-when]
    [--actor NAME | --no-actor]
    [--before ID | --after ID]
```

Changes only the flags given. A repeated list flag replaces that whole list. `--done-when` on a committed step that succeeded includes `"reopens": true`. The step reopens at `commit`.

Stdout: `{"step"}`, plus `"reopens": true` when the change will reopen a success.

| Code | Exit | When |
| --- | --- | --- |
| `usage` | 2 | A `--no-*` flag is combined with the flag it clears, or `--when` with `--when-any`. |
| `no_draft` | 4 | No draft is open. |
| `unknown_step` | 4 | The id is not in the draft. |
| `bad_slug` | 4 | A new id, produce, result, or actor is not a slug. |
| `duplicate_value` | 4 | A produce name is already used. |
| `results_need_produce` | 4 | Results were set on a step with no produce. |

### drop

```text
relate-flow PLAN drop ID
```

Removes a step from the open draft.

Stdout: `{"steps"}`.

| Code | Exit | When |
| --- | --- | --- |
| `no_draft` | 4 | No draft is open. |
| `unknown_step` | 4 | The id is not in the draft. |
| `step_referenced` | 4 | Another step still names it in a need or a condition. |
| `completed_work` | 4 | The step succeeded on the committed plan. |

### commit

```text
relate-flow PLAN commit [--dry-run]
```

Checks the draft and writes the next version. `--dry-run` prints the same object with `"dryRun": true` and writes nothing. The first commit of a name makes a template.

Stdout: `{"version","kept","reopened","added","removed","frontier"}`.

| Code | Exit | When |
| --- | --- | --- |
| `no_plan` | 4 | The name has no plan. |
| `no_draft` | 4 | A plan is committed and no draft is open. |
| `invalid_plan` | 4 | The draft has one or more problems. All of them are in `errors`. |
| `completed_work` | 4 | A succeeded step was dropped. |

### execute

```text
relate-flow PLAN execute NAME [--input KEY=VALUE]...
```

Copies the committed template `PLAN` into a new plan `NAME`. Repeat `--input` once per pair. Those pairs are stored on the execution and appear in `inputs` on every later `run`, including the first step. The template's steps stay pending. A later template commit is what the next `execute` copies.

Stdout: `{"execution","template","version","inputs"}`.

| Code | Exit | When |
| --- | --- | --- |
| `usage` | 2 | `--input` has no `=`, or the key or the value is empty. |
| `no_plan` | 4 | The name has no committed plan. |
| `draft_open` | 4 | The template has an open draft. |
| `not_a_template` | 4 | `PLAN` is an execution. `fix` says to name the template. |
| `bad_slug` | 4 | `NAME` is not a slug, or it is `skill`. |
| `name_taken` | 4 | A plan with that name already exists. |
| `input_conflicts` | 4 | An input key equals a step's `produces`. |
| `duplicate_value` | 4 | The same input key was passed twice. |

### remove

```text
relate-flow PLAN remove [--force]
```

Deletes every version of `PLAN`: the committed head, superseded versions, and an open draft, plus their steps and attempts. Removing a template leaves its executions. Removing an execution leaves the template, and that name can be executed again. An open lease is refused unless `--force`.

Stdout: `{"removed","versions","attempts"}`.

| Code | Exit | When |
| --- | --- | --- |
| `no_plan` | 4 | The name has no plan. |
| `lease_open` | 4 | A step is leased. `fix` names the step and says to pass `--force`. |

### prune

```text
relate-flow TEMPLATE prune
```

Deletes executions copied from this template whose committed head is finished: every step succeeded or was skipped, no step is leased, and no draft is open. A pending, running, failed, or blocked execution stays. The template stays. `TEMPLATE` must be a template.

Stdout: `{"pruned":["NAME"]}`. An empty list means nothing was finished.

| Code | Exit | When |
| --- | --- | --- |
| `no_plan` | 4 | The name has no plan. |
| `not_a_template` | 4 | The name is an execution. |

### revise

```text
relate-flow PLAN revise
```

Copies the committed plan into a new draft. On an execution, the draft stays that execution and keeps its inputs and template link.

Stdout: `{"draft":true,"basedOn":N}`.

| Code | Exit | When |
| --- | --- | --- |
| `no_plan` | 4 | The name has no committed plan. |
| `draft_open` | 4 | A draft is already open. |

### abort

```text
relate-flow PLAN abort
```

Deletes the open draft. The committed plan is left as it is. Aborting when no draft exists exits 0.

Stdout: `{"aborted":true}` or `{"aborted":false}`.

### show

```text
relate-flow PLAN show [--output json|text]
```

Prints the open draft, or the committed plan. `json` is the default. `text` draws the steps as a flowchart and prints bound inputs under the goal. `show` opens a template and an execution.

JSON: `plan`, `kind` (`template` or `execution`), `draft`, `basedOn`, `goal`, `retries`, `steps`. An execution also has `template` and `inputs`.

### next

```text
relate-flow PLAN next --agent NAME [--lease DURATION] [--output json|text]
```

Leases the next step for this agent, or says why there is none. `--lease` is a humantime duration (`15m`, `1h`). The default is 15 minutes. More than 24 hours is refused. `next` on a template is refused.

`json` is the default. `--output text` prints a successful instruction as a prompt. A `run` ends with the two report commands: `succeeded` with `--note` (and `--result <tok1|tok2>` when the step declares results) and `failed` with `--because`. `waiting`, `blocked`, `replan`, and `done` are one line. A refusal stays JSON.

The decision order is: renew this agent's open lease, claim the earliest ready step this agent may perform, wait for someone else's ready or running step, replan a failed step, return a block, replan a plan that cannot move, then done.

| `action` | Fields |
| --- | --- |
| `run` | `step` holds `id`, `attempt`, `title`, `doneWhen`, `needs`, `inputs`, `leaseUntil`, and `produces`, `results`, `when`, `whenAny` when the step has them. `retry` is present when the failed-attempt count is greater than zero. `"renewed": true` at the top level means the same lease was extended. |
| `waiting` | `for` is the actor who must report `step`. `because` is the note that opened the branch. |
| `blocked` | `step` and `because`. |
| `replan` | `because`. `step` when one step failed its budget. |
| `done` | `version`. |

| Code | Exit | When |
| --- | --- | --- |
| `usage` | 2 | `--agent` is missing, or `--lease` is not a duration. |
| `bad_slug` | 4 | The agent is not a slug. |
| `lease_too_long` | 4 | The lease is longer than 24 hours. |
| `no_plan` | 4 | The name has no committed plan. |
| `draft_open` | 4 | A draft is open. |
| `not_a_run` | 4 | The name is a template. `fix` is `relate-flow PLAN execute NAME`. |
| `conflict` | 5 | Another claim wrote the same step. `retryable` is true. Call `next` again. |

### report

```text
relate-flow PLAN report STEP --agent NAME --outcome OUTCOME
    [--result TOKEN] [--note TEXT] [--because TEXT] [--artifact PATH]
```

Admits the outcome of the step this agent holds. `report` does not lease the next step. `report` on a template is refused.

| Outcome | Required | Refused |
| --- | --- | --- |
| `succeeded` | `--note`. `--result` when the step declares results, and it must be one of those tokens. | `--because` |
| `failed`, `blocked` | `--because` | `--note`, `--result` |
| `released` | nothing | `--note`, `--because`, `--result` |

`expired` is not reportable. A readable `--artifact` is stored as its path and its SHA-256. The file is not copied. An unreadable path writes nothing. Repeating the same outcome, result, note, because, and artifact returns the original body and exits 0.

Stdout: `{"step","attempt","outcome"}`, plus the fields that were recorded.

| Code | Exit | When |
| --- | --- | --- |
| `usage` | 2 | `--agent` or `--outcome` is missing. |
| `bad_slug` | 4 | The agent is not a slug. |
| `no_plan` | 4 | The name has no committed plan. |
| `draft_open` | 4 | A draft is open. |
| `not_a_run` | 4 | The name is a template. |
| `unknown_step` | 4 | `STEP` is not in the plan. |
| `not_leased` | 4 | This agent holds a different step, or holds nothing. `fix` names the held step. |
| `missing_note` | 4 | A success has no `--note`. |
| `missing_because` | 4 | A failure or a block has no `--because`. |
| `missing_result` | 4 | A success on a step with results has no `--result`. |
| `bad_result` | 4 | `--result` is not one of the step's tokens. |
| `unexpected_field` | 4 | A flag was passed that this outcome does not take. |
| `artifact_unreadable` | 4 | `--artifact` cannot be read. |
| `conflict` | 5 | This attempt was already closed with a different outcome. `retryable` is false. |

### status

```text
relate-flow PLAN status [--json]
```

Lists every step in document order. A terminal prints a table with columns `ID`, `DISPOSITION`, `ACTOR`, and `RESULT`. Any other stdout, or `--json`, prints JSON: `draft` or `version`, and `steps` of `{id, disposition, actor, result}`. `status` reads the stored state. It does not expire leases. A pending step with no unmet need has disposition `ready`.

`status` opens a template and an execution. A missing plan is `no_plan` (exit 4).

### explain

```text
relate-flow PLAN explain STEP
```

Says why one step is in its current disposition. `explain` opens a template and an execution.

Stdout: `step`, `disposition`, `why`, `needs`, `when`, `whenAny`, `missing`, `lastAttempt`.

| Disposition | `why` |
| --- | --- |
| ready | `ready` |
| running | `leased to AGENT until LEASE` |
| succeeded | `succeeded` |
| skipped, because a need was skipped | `skipped because STEP was skipped` |
| skipped, because a condition failed | `skipped because PRODUCE is TOKEN` |
| pending on a need | `waiting on STEP` |
| pending on an unresolved token | `waiting on PRODUCE` |
| blocked | `blocked: BECAUSE` |
| failed, budget used | `failed N of RETRIES: BECAUSE` |
| pending again after a failure | `ready, failed N of RETRIES` |

`unknown_step` (exit 4) when `STEP` is not in the plan. `no_plan` when the name has no plan.

### log

```text
relate-flow PLAN log [STEP] [--json]
```

Lists attempts, newest first, capped at 50. `STEP` limits the list to one step. A terminal prints columns `ATTEMPT`, `STEP`, `OUTCOME`, and `AT`. An open attempt prints `open`. `--json` and a non-terminal print `{"attempts":[…]}`. Each attempt has `id`, `step`, `agent`, `outcome`, `result`, `note`, `because`, `artifact`, `artifactHash`, `leaseUntil`, `startedAt`, and `finishedAt`.

`log` opens a template and an execution. `unknown_step` (exit 4) when `STEP` is not in the plan.

### skill

`skill` does not take a plan name and does not open a store. With no `--agent`, the command applies to every detected agent.

```text
relate-flow skill install [--agent NAME] [--force]
relate-flow skill remove  [--agent NAME] [--force]
relate-flow skill list    [--agent NAME]
relate-flow skill check   [--agent NAME]
```

Success: `{"skill":"relate-flow","version":"…","agents":[…]}`.

| Code | Exit | When |
| --- | --- | --- |
| `unknown_agent` | 4 | `--agent` is not a known agent. |
| `no_agent` | 4 | No agent was detected. |
| `foreign_skill` | 4 | The directory was not installed by this command. `--force` replaces it. |
| `skill_drift` | 4 | `check` found an installed copy that differs from this binary. |
| `skill_io` | 1 | A skill directory could not be written or read. |
