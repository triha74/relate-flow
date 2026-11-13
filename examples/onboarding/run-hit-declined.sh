#!/usr/bin/env bash
# expect: done
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="Application APP-5."
rf next --agent intake
rf report collect-docs --agent intake --outcome succeeded --note "Application APP-3."
rf next --agent kyc
rf report verify-identity --agent kyc --outcome succeeded --result verified --note "Identity matches."
rf next --agent screening
rf report sanctions-screen --agent screening --outcome succeeded --result hit --note "Sanctions hit."
rf next --agent compliance
rf report compliance-review --agent compliance --outcome succeeded --result declined --note "Decline the application."
rf next --agent intake
rf report decline --agent intake --outcome succeeded --note "Applicant notified."
rf next --agent intake
