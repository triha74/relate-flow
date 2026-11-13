#!/usr/bin/env bash
# expect: done
# The discount is missing and the person rejects the invoice.
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="Northwind, INV-1042, billed 10000."
note="Northwind, INV-1042, 100 widgets at \$100, no discount stated, billed \$10,000."
short="NW-2024 gives 10% off this line. Agreed total \$9,000, billed \$10,000, shortfall \$1,000."
rf next --agent invoices
rf report parse --agent invoices --outcome succeeded --note "$note"
rf next --agent invoices
rf report verify-vendor --agent invoices --outcome succeeded --result approved --note "Northwind is approved."
rf next --agent invoices
rf report verify-pricing --agent invoices --outcome succeeded --result aligned --note "100 widgets at \$100 matches NW-2024."
rf next --agent invoices
rf report verify-discounts --agent invoices --outcome succeeded --result incorrect --note "$short"
rf next --agent invoices
rf report escalate --agent invoices --outcome succeeded --note "Review REV-21 is open."
rf next --agent human
rf report human-review --agent human --outcome succeeded --result rejected --note "Do not pay INV-1042."
rf next --agent invoices
rf report close-rejected --agent invoices --outcome succeeded --note "Closed unpaid."
rf next --agent invoices
