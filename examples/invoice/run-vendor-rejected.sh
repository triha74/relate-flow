#!/usr/bin/env bash
# expect: done
# Northwind is not approved. A person still approves payment, and pay runs.
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="Northwind, INV-1042, billed 10000."
rf next --agent invoices
rf report parse --agent invoices --outcome succeeded --note "Northwind, INV-1042, billed \$10,000."
rf next --agent invoices
rf report verify-vendor --agent invoices --outcome succeeded --result rejected --note "Northwind is not an approved vendor."
rf next --agent invoices
rf report escalate --agent invoices --outcome succeeded --note "Review REV-22 is open."
rf next --agent human
rf report human-review --agent human --outcome succeeded --result approved --note "Pay \$9,000 anyway."
rf next --agent invoices
rf report pay --agent invoices --outcome succeeded --note "Paid \$9,000 as PAY-3."
rf next --agent invoices
