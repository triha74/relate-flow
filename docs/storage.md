# Storage

One directory is one Relate database and holds many named plans. A name is one lineage: the committed head and, when a revision is open, its draft. `:memory:` is accepted by the library for tests. A second process that opens the directory gets `Error::Locked`.

If the database has nodes and none of them are `:Plan`, the command refuses with `foreign_store`. An empty store is a new plan.

## Labels

`Plan` holds `uid`, `name`, `kind` (`template` or `execution`), `inputs` (a list of `key=value` strings), `template` and `templateVersion` (set on an execution), `goal`, `retries`, `version` (null on a draft), `basedOn`, and `createdAt`. A plan loaded without `kind` counts as an execution. Attempt ids (`a1`, `a2`, …) are unique across every plan in the store. `execute` copies steps onto a new plan and does not link `SUPERSEDES`.

`Step` holds `uid`, `id`, `title`, `doneWhen`, `produces`, `results`, `needs`, `when`, `whenAny`, `actor`, `state`, and `position`. Conditions are stored as strings (`produce=token`). One `--when-any` alternative is a comma-joined string. Absent strings are null. Lists are Relate lists.

`Attempt` holds `uid`, `id` (`a1`, `a2`, …), `stepId` (the slug, unchanged when the attempt moves), `agent`, `outcome`, `result`, `note`, `because`, `artifact`, `artifactHash`, `leaseUntil`, `startedAt`, and `finishedAt`.

## Relationships

| Relationship | From | To |
| --- | --- | --- |
| `HAS_STEP` | Plan | Step |
| `SUPERSEDES` | newer Plan | older Plan |
| `NEEDS` | Step | Step it needs |
| `OF` | Attempt | Step |

`NEEDS` is rewritten only when the command changes the step's needs. A claim updates the step's `state` and does not touch `NEEDS`, so two claims of different steps do not write the same relationships.

A kept success moves `OF` to the new step uid. The attempt's `stepId` slug stays. Attempts on a step that was not kept are closed `released` with because "cancelled by revision" when a lease was still open. Their `OF` stays on the old step, which remains in the superseded plan.

## Writes

Each command builds a `Write` list and runs it inside the open transaction: create or update a plan, upsert or delete a step, create or update or move an attempt, link `SUPERSEDES`, delete a draft. Deleting a plan deletes its steps and the attempts on those steps. `remove` deletes every version of one name. `prune` deletes finished executions of a template. Neither command deletes the directory. The load path reads scalar columns. It does not round-trip Relate node values.

Uids are `prefix-{nanos}-{counter}` so a retried command does not collide with a row the previous attempt already wrote.
