# From a task to a finished case

You hand an agent an invoice and one sentence: pay it, or hand it to a person. The relate-flow skill is how that agent turns the sentence into a plan, copies the plan into a case, and then does exactly one step at a time until the case is finished.

Relate Flow keeps the plan and admits each report. The agent does the reading, the checking, and the payment.

The prompts below are from one run of that case. Lease times and attempt ids belong to the run.

## The skill arrives first

```bash
relate-flow skill install
```

Install writes `SKILL.md` and four references into each agent it detects. For Cursor that directory is `~/.cursor/skills/relate-flow/`. The front matter tells the agent when to load it: turn a task into a plan, then drive the plan with `next` and `report`.

Before the agent adds a step, the skill sends it to [designing a workflow](../crates/relate-flow/src/skill/bundle/references/designing-a-workflow.md). The method there is the whole of the design phase.

1. State the outcome, including the other way the work can end.
2. Name who may perform each step.
3. Turn every fact that chooses a branch into a produce name and a closed set of tokens.
4. Write every branch now. A recorded token selects the branch later. The agent does that selecting by reporting a token, and the plan already contains the step that token admits.
5. Say, in `doneWhen`, what a later step needs to find in the note. Flow copies that note into the inputs of the steps that need it.

The [invoice reference](../crates/relate-flow/src/skill/bundle/references/invoice.md) is the pattern for a human decision that has to stay reachable after an earlier branch is skipped. The agent uses that pattern. It copies the method, and it writes this invoice's facts into the steps.

## The task, before any command

> Pay this invoice, or hand it to a person. Northwind bills under agreement NW-2024: widgets at $100, with 10% off a widget line over $5,000. INV-1042 is 100 widgets and bills $10,000. The agreed total is $9,000.

The agent writes five decisions down before it calls `goal`.

The outcome is two endings. The invoice is paid, or a person rejects it and it is closed unpaid.

Two actors do the work. `invoices` parses, checks, escalates, pays, and closes. `human` is the only actor who may approve or reject a mismatched invoice.

Four facts choose the path, and each one is a closed vocabulary:

| Produce | Tokens | Chosen by |
| --- | --- | --- |
| `vendor` | `approved`, `rejected` | Is Northwind allowed to bill? |
| `pricing` | `aligned`, `misaligned` | Does each line match NW-2024? |
| `discounts` | `correct`, `incorrect` | Was the 10% applied? |
| `decision` | `approved`, `rejected` | May this invoice be paid? |

`pay` needs only the parse. It becomes ready when `discounts=correct` or `decision=approved`. A rejected vendor skips pricing and discounts, and an approval can still admit payment. `close-rejected` runs when the person rejects.

The invoice body is an input on the case, so a second execution can carry a different bill. The step titles below name INV-1042, matching the shipped example in `examples/invoice/plan.sh`.

## The agent writes the template

Every command names the plan. `goal` creates the store at `./.relate-flow` and an empty draft. The default failure budget is 2.

```bash
relate-flow invoice goal --text "Pay INV-1042, or hand it to a person"

relate-flow invoice add parse --title "Parse INV-1042" \
  --done-when "Vendor, lines, stated discounts, and the billed total are recorded" \
  --produces invoice --actor invoices

relate-flow invoice add verify-vendor --title "Verify Northwind is approved to bill" \
  --done-when "Northwind is approved, or the reason it is not is recorded" \
  --needs parse --produces vendor --results approved --results rejected --actor invoices

relate-flow invoice add verify-pricing --title "Match each line to NW-2024" \
  --done-when "Every line matches an agreement price, or each miss is recorded" \
  --needs verify-vendor --when vendor=approved --produces pricing \
  --results aligned --results misaligned --actor invoices

relate-flow invoice add verify-discounts --title "Check the volume discount on INV-1042" \
  --done-when "The 10% discount is confirmed, or the shortfall is recorded" \
  --needs parse --needs verify-pricing --when pricing=aligned --produces discounts \
  --results correct --results incorrect --actor invoices

relate-flow invoice add escalate --title "Open a human review for INV-1042" \
  --done-when "A review is open and holds the invoice, NW-2024, and the mismatch" \
  --needs parse --when-any vendor=rejected --when-any pricing=misaligned \
  --when-any discounts=incorrect --produces review --actor invoices

relate-flow invoice add human-review --title "Decide whether INV-1042 may be paid" \
  --done-when "A person approves a payment amount or rejects the invoice" \
  --needs escalate --produces decision --results approved --results rejected --actor human

relate-flow invoice add pay --title "Pay the agreed amount for INV-1042" \
  --done-when "Payment is submitted for the amount in the inputs and the reference is recorded" \
  --needs parse --when-any discounts=correct --when-any decision=approved \
  --produces payment --actor invoices

relate-flow invoice add close-rejected --title "Close INV-1042 unpaid" \
  --done-when "The invoice is closed with the rejection reason" \
  --needs human-review --when decision=rejected --produces closure --actor invoices
```

`--needs` waits until that step has succeeded, and skips this step when that step is skipped. `--when` requires every pair. `--when-any` admits the step when any one alternative matches. `escalate` therefore opens on a rejected vendor, a misaligned price, or a wrong discount. Add order is the order `next` claims when two steps are both ready, so `parse` is added first.

The agent reads the draft back before it commits.

```bash
relate-flow invoice show --output text
```

```text
invoice
Pay INV-1042, or hand it to a person

┌───────────┐
│ parse     │
│ invoices  │
│ → invoice │
└─────┴─────┘
      │
      └───────┬─────────────────────┐
              ▼ needs               │
┌──────────────────────────┐        │
│ verify-vendor            │        │
│ invoices                 │        │
│ → vendor approved|rejec… │        │
└─────────────┴────────────┘        │
              │                     │
              ├─────────────────────┤
              ▼ vendor=approved     │
┌──────────────────────────┐        │
│ verify-pricing           │        │
│ invoices                 │        │
│ → pricing aligned|misal… │        │
└─────────────┴────────────┘        │
              │                     │
              ├─────────────────────┤
              ▼ needs               │
┌──────────────────────────┐        │
│ verify-discounts         │        │
│ invoices                 │        │
│ → discounts correct|inc… │        │
└─────────────┴────────────┘        │
              │                     │
      ┌───────┴─────────────────────┤
      ▼ needs                       │
┌──────────┐                        │
│ escalate │                        │
│ invoices │                        │
│ → review │                        │
└─────┴────┘                        │
      │                             │
      └───────┐                     │
              ▼ needs               │
┌──────────────────────────┐        │
│ human-review             │        │
│ human                    │        │
│ → decision approved|rej… │        │
└─────────────┴────────────┘        │
              │                     │
      ┌───────┴───────────┬─────────┘
      ▼ needs             ▼ decision=rejected
┌───────────┐    ┌────────────────┐
│ pay       │    │ close-rejected │
│ invoices  │    │ invoices       │
│ → payment │    │ → closure      │
└───────────┘    └────────────────┘
```

`commit` checks that every token names a produce the plan has. A refusal is one JSON object, exit 4, and `fix` names the repair. The agent changes that one thing and calls `commit` again.

```bash
relate-flow invoice commit
```

```json
{"added":["parse","verify-vendor","verify-pricing","verify-discounts","escalate","human-review","pay","close-rejected"],"frontier":["parse"],"kept":[],"removed":[],"reopened":[],"version":1}
```

`frontier` is the first step with nothing left to wait for. Here that is `parse`, which is the step the agent meant to run first. `commit` writes version 1. The first lease comes from `next`.

The committed plan is a template. `next` on it is refused, and the fix is the command that copies it:

```json
{"error":{"code":"not_a_run","fix":"relate-flow invoice execute NAME","message":"invoice is a template","retryable":false}}
```

## One invoice becomes a case

```bash
relate-flow invoice execute inv-1042 \
  --input case="Northwind, INV-1042, billed 10000."
```

```json
{"execution":"inv-1042","inputs":{"case":"Northwind, INV-1042, billed 10000."},"template":"invoice","version":1}
```

`inv-1042` is a copy of template version 1. The template stays reusable. A later `revise` on `inv-1042` changes the case. From here, `next` and `report` name `inv-1042`.

## The agent does one step

```bash
relate-flow inv-1042 next --agent invoices --output text
```

```text
Run parse (attempt a1). Lease until 2026-11-13T13:57:33Z.

Parse INV-1042
Done when: Vendor, lines, stated discounts, and the billed total are recorded

Inputs:
  case: Northwind, INV-1042, billed 10000.

Then run:
  relate-flow inv-1042 report parse --agent invoices --outcome succeeded --note "..."
  relate-flow inv-1042 report parse --agent invoices --outcome failed --because "..."
```

`--output text` is a prompt for this step. JSON stays the default, and a refusal stays JSON. The agent holds the lease until the printed time. It parses the invoice, and it reports the step it holds. The note is the evidence the later steps will read.

```bash
relate-flow inv-1042 report parse --agent invoices --outcome succeeded \
  --note "Northwind, INV-1042, 100 widgets at $100, no discount stated, billed $10,000."
```

`report` records the result. The agent calls `next` again to be leased the following step.

Vendor is approved, and the line price matches NW-2024. Each of those is the same loop: `next`, do that step, `report` with the token the plan declared. A report against `pay` while `verify-discounts` is leased comes back `not_leased`, exit 4, and `fix` names `verify-discounts`.

The discount check is where this invoice leaves the happy path. Its prompt already carries the notes that opened the branch:

```text
Run verify-discounts (attempt a4). Lease until 2026-11-13T13:57:34Z.

Check the volume discount on INV-1042
Done when: The 10% discount is confirmed, or the shortfall is recorded

Inputs:
  case: Northwind, INV-1042, billed 10000.
  invoice: Northwind, INV-1042, 100 widgets at $100, no discount stated, billed $10,000.
  pricing: 100 widgets at $100 matches NW-2024.
  vendor: Northwind is approved.

Then run:
  relate-flow inv-1042 report verify-discounts --agent invoices --outcome succeeded --result <correct|incorrect> --note "..."
  relate-flow inv-1042 report verify-discounts --agent invoices --outcome failed --because "..."
```

Attempt `a4` is the fourth lease in the case. The result flag lists the only tokens `commit` will accept. The agent compares the billed $10,000 with the agreed $9,000 and reports `incorrect`:

```bash
relate-flow inv-1042 report verify-discounts --agent invoices --outcome succeeded \
  --result incorrect \
  --note "NW-2024 gives 10% off this line. Agreed total $9,000, billed $10,000, shortfall $1,000."
```

`next` then leases `escalate`. `pay` stays pending, because `decision` is still open. The agent opens review REV-19, reports that note, and calls `next` once more.

```text
Waiting for human to report human-review. NW-2024 gives 10% off this line. Agreed total $9,000, billed $10,000, shortfall $1,000.
```

`waiting` is an answer, exit 0. Work is ready, and it belongs to `human`. The invoices agent stops.

## A person takes the lease

The person, or an agent allowed to act as `human`, asks for their own step:

```bash
relate-flow inv-1042 next --agent human --output text
```

```text
Run human-review (attempt a6). Lease until 2026-11-13T13:57:34Z.

Decide whether INV-1042 may be paid
Done when: A person approves a payment amount or rejects the invoice

Inputs:
  case: Northwind, INV-1042, billed 10000.
  discounts: NW-2024 gives 10% off this line. Agreed total $9,000, billed $10,000, shortfall $1,000.
  review: Review REV-19 is open.

Then run:
  relate-flow inv-1042 report human-review --agent human --outcome succeeded --result <approved|rejected> --note "..."
  relate-flow inv-1042 report human-review --agent human --outcome failed --because "..."
```

The shortfall is in `discounts` because that token admitted `escalate`, and the review note is there because `human-review` needs `escalate`. The person approves a payment of $9,000:

```bash
relate-flow inv-1042 report human-review --agent human --outcome succeeded \
  --result approved \
  --note "Pay $9,000. Do not pay the billed $10,000."
```

`decision=approved` admits `pay` and skips `close-rejected`. The invoices agent calls `next` and receives the amount inside the inputs:

```text
Run pay (attempt a7). Lease until 2026-11-13T13:57:34Z.

Pay the agreed amount for INV-1042
Done when: Payment is submitted for the amount in the inputs and the reference is recorded

Inputs:
  case: Northwind, INV-1042, billed 10000.
  decision: Pay $9,000. Do not pay the billed $10,000.
  invoice: Northwind, INV-1042, 100 widgets at $100, no discount stated, billed $10,000.

Then run:
  relate-flow inv-1042 report pay --agent invoices --outcome succeeded --note "..."
  relate-flow inv-1042 report pay --agent invoices --outcome failed --because "..."
```

```bash
relate-flow inv-1042 report pay --agent invoices --outcome succeeded \
  --note "Paid $9,000 as PAY-8891."
relate-flow inv-1042 next --agent invoices --output text
```

```text
Done. version 1
```

Every step succeeded or was skipped. The template `invoice` is still version 1, ready for the next invoice.

## The other endings were written on the first day

The same template already contains the other ways this work ends. The agent picks one by reporting a token.

A correct discount skips `escalate`, `human-review`, and `close-rejected`, and the next lease is `pay`.

A rejected vendor skips pricing and discounts. `escalate` still runs, and an approval still admits `pay`, because `pay` needs only `parse`.

A missing agreement is reported as `blocked` on `verify-pricing`, with a `--because`. The following `next` returns `blocked`. `revise` and then `commit`, with no edits, reopens that step and keeps `parse` and `verify-vendor`.

A failure under the budget returns the step to pending, and the next prompt carries `Retry N`. At the budget, `next` returns `replan`, and the agent changes the plan with `revise`.

## What the agent repeats

The loop after `execute` is short.

| `next` says | The agent |
| --- | --- |
| `run` | Does that step. Reports `succeeded`, `failed`, or `blocked` with the flags the prompt printed. Calls `next`. |
| `waiting` | Stops. The named actor reports. |
| `blocked` or `replan` | Opens a draft with `revise`, repairs the plan, and `commit`s. |
| `done` | Stops. The case is finished. |

`relate-flow --help` lists the plans in the store. The command reference is [cli.md](cli.md). The same invoice, with the other four endings, is [scenarios/invoice.md](scenarios/invoice.md).
