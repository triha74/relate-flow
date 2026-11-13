# Errors, dispositions, and why

Every error is one JSON object:

```json
{
  "error": {
    "code": "not_leased",
    "message": "pay is not the step invoices holds",
    "fix": "report verify-discounts, which is leased until 2026-11-10T22:15:00Z",
    "retryable": false
  }
}
```

Do what `fix` says. When `errors` is present, the first entry is the same
problem as `fix`, and the rest are the other problems `commit` found. A
client that only reads `fix` repairs one problem per attempt.

| Exit | Codes you will see |
| --- | --- |
| 2 | `usage` |
| 4 | `foreign_store`, `draft_open`, `no_draft`, `no_plan`, `needs_revise`, `bad_slug`, `empty_field`, `step_differs`, `unknown_step`, `step_referenced`, `completed_work`, `results_need_produce`, `duplicate_value`, `invalid_plan`, `not_leased`, `missing_note`, `missing_because`, `missing_result`, `bad_result`, `unexpected_field`, `artifact_unreadable`, `lease_too_long` |
| 5 | `store_busy` (retryable), `conflict` (retryable when you should call `next` again, not retryable when this attempt was closed with a different outcome) |
| 1 | `store` |

`explain STEP` states `why` from the disposition:

| Situation | `why` |
| --- | --- |
| ready | `ready` |
| running | `leased to <agent> until <leaseUntil>` |
| succeeded | `succeeded` |
| skipped because a need was skipped | `skipped because <step> was skipped` |
| skipped because a condition failed | `skipped because <produce> is <token>` |
| pending on a need | `waiting on <step>` |
| pending on an unresolved token | `waiting on <produce>` |
| blocked | `blocked: <because>` |
| failed, budget used | `failed <n> of <retries>: <because>` |
| pending again after a failure | `ready, failed <n> of <retries>` |

`status` lists every step in document order with its disposition, and its
actor and result when it has them. `log` returns attempts, newest first, up
to 50. `log STEP` limits that to one step. On a terminal, `status` and `log`
print a table. Piped, they print JSON. `--json` forces JSON.

Stored states are `pending`, `running`, `succeeded`, `failed`, `blocked`, and
`skipped`. Ready is computed, not stored. You never write a state.
