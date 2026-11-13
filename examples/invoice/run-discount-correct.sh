#!/usr/bin/env bash
# expect: done
# The invoice already has the 10% discount, so review is skipped.
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="Northwind, INV-1042, billed 10000."
rf next --agent invoices
rf report parse --agent invoices --outcome succeeded --note "Northwind, INV-1042, billed \$9,000 after the 10% discount."
rf next --agent invoices
rf report verify-vendor --agent invoices --outcome succeeded --result approved --note "Northwind is approved."
rf next --agent invoices
rf report verify-pricing --agent invoices --outcome succeeded --result aligned --note "100 widgets at \$100 matches NW-2024."
rf next --agent invoices
rf report verify-discounts --agent invoices --outcome succeeded --result correct --note "The 10% discount is on the invoice."
rf next --agent invoices
rf report pay --agent invoices --outcome succeeded --note "Paid \$9,000 as PAY-1."
rf next --agent invoices
