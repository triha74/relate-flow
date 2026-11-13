#!/usr/bin/env bash
# expect: done
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="Application APP-5."
rf next --agent intake
rf report collect-docs --agent intake --outcome succeeded --note "Application APP-4."
rf next --agent kyc
rf report verify-identity --agent kyc --outcome blocked --because "the document image is unreadable"
rf next --agent screening
rf report sanctions-screen --agent screening --outcome succeeded --result clear --note "No hit."
rf next --agent kyc
rf revise
rf commit
rf next --agent kyc
rf report verify-identity --agent kyc --outcome succeeded --result verified --note "A new image matches."
rf next --agent kyc
rf report risk-score --agent kyc --outcome succeeded --result low --note "Low risk."
rf next --agent intake
rf report open-account --agent intake --outcome succeeded --note "Account ACC-11 opened."
rf next --agent intake
