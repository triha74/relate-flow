#!/usr/bin/env bash
# expect: done
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="Application APP-5."
rf next --agent intake
rf report collect-docs --agent intake --outcome succeeded --note "Application APP-2."
rf next --agent kyc
rf report verify-identity --agent kyc --outcome succeeded --result mismatch --note "Name does not match the document."
rf next --agent screening
rf report sanctions-screen --agent screening --outcome succeeded --result clear --note "No hit."
rf next --agent compliance
rf report compliance-review --agent compliance --outcome succeeded --result approved --note "Approved with the corrected name."
rf next --agent intake
rf report open-account --agent intake --outcome succeeded --note "Account ACC-10 opened."
rf next --agent intake
