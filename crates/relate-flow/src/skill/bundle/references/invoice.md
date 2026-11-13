# Invoice

The invoice shape is a template. One invoice is one execution, named for that invoice, with the invoice text bound by `--input`. The agent `invoices` drives the execution. A person acts as `human`.

INV-1042 is from Northwind under agreement NW-2024: widgets at $100, with 10%
off a widget line over $5,000. The invoice is 100 widgets. The agreed total
is $9,000. This copy bills $10,000.

```text
parse → verify-vendor → verify-pricing → verify-discounts → pay
                      ↘ escalate → human-review → pay
                                              ↘ close-rejected
```

`verify-vendor` produces `vendor` (`approved` or `rejected`). `verify-pricing`
runs when `vendor=approved` and produces `pricing` (`aligned` or
`misaligned`). `verify-discounts` needs `parse` and `verify-pricing`, runs
when `pricing=aligned`, and produces `discounts` (`correct` or `incorrect`).
`escalate` needs only `parse` and is admitted by `vendor=rejected`,
`pricing=misaligned`, or `discounts=incorrect`. `human-review` needs
`escalate`, produces `decision` (`approved` or `rejected`), and its actor is
`human`. `pay` needs only `parse` and is admitted by `discounts=correct` or
`decision=approved`. `close-rejected` needs `human-review` and runs when
`decision=rejected`. Every other step's actor is `invoices`.

## The discount is missing

`next` leases `parse`. Report the vendor, the lines, and the billed total.
Report `verify-vendor` succeeded with `approved`, and `verify-pricing`
succeeded with `aligned`. `next` then leases `verify-discounts`. Its `inputs`
are the invoice note and the pricing note.

A report against `pay` at that moment is refused. The leased step is
`verify-discounts`, and the fix names it. Report `incorrect` with the
shortfall in the note. `next` leases `escalate`. `pay` stays pending, because
`decision` is unresolved.

`next`, still as `invoices`, then returns `waiting` for `human` on
`human-review`. The `because` is the shortfall note. Stop.

The person runs `next --agent human`, is leased `human-review`, and the
inputs carry the shortfall. They approve $9,000 or reject the invoice. An
approval leases `pay` to `invoices` and skips `close-rejected`. A rejection
skips `pay` and leases `close-rejected`.

## The discount is already correct

The run is the same through `verify-pricing`. Report `verify-discounts` with
`correct`. `escalate` is skipped, because every token it lists is recorded
and none match. `human-review` and `close-rejected` are skipped because they
need `escalate`. `next` leases `pay`. After the payment, `next` returns `done`.

## A missing agreement

If you cannot find NW-2024, report `verify-pricing` as `blocked` with a
because. Nothing else is ready. `next` returns `blocked`. `revise` and then
`commit`, with no edits, reopens the blocked step and keeps the steps that
succeeded.

## A rejected vendor

Report `verify-vendor` with `rejected`. `next` leases `escalate`. Pricing and
discounts are skipped. An approval still admits `pay`, because `pay` does not
need those skipped steps. A rejection admits `close-rejected`.
