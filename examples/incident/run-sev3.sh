#!/usr/bin/env bash
# expect: done
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="A nightly job failed."
rf next --agent oncall
rf report triage --agent oncall --outcome succeeded --result sev3 --note "One chart is stale."
rf next --agent oncall
rf report mitigate --agent oncall --outcome succeeded --result resolved --note "Refreshed the chart."
rf next --agent oncall
rf report close-ticket --agent oncall --outcome succeeded --note "Closed."
rf next --agent oncall
