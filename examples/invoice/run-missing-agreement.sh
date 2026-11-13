#!/usr/bin/env bash
# expect: done
# NW-2024 is missing, so pricing is blocked until a revision reopens it.
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="Northwind, INV-1042, billed 10000."
rf next --agent invoices
rf report parse --agent invoices --outcome succeeded --note "Northwind, INV-1042, billed \$10,000."
rf next --agent invoices
rf report verify-vendor --agent invoices --outcome succeeded --result approved --note "Northwind is approved."
rf next --agent invoices
rf report verify-pricing --agent invoices --outcome blocked --because "NW-2024 is not on file"
rf next --agent invoices
rf revise
rf set verify-pricing --done-when "Every line matches NW-2024, which is now on file"
rf commit
rf next --agent invoices
rf report verify-pricing --agent invoices --outcome succeeded --result aligned --note "100 widgets at \$100 matches NW-2024."
rf next --agent invoices
rf report verify-discounts --agent invoices --outcome succeeded --result correct --note "The 10% discount is on the invoice."
rf next --agent invoices
rf report pay --agent invoices --outcome succeeded --note "Paid \$9,000 as PAY-2."
rf next --agent invoices
