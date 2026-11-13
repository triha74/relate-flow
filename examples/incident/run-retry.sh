#!/usr/bin/env bash
# expect: done
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="Checkout is down."
rf next --agent oncall
rf report triage --agent oncall --outcome succeeded --result sev3 --note "A job failed."
rf next --agent oncall
rf report mitigate --agent oncall --outcome failed --because "rollback refused"
rf next --agent oncall
rf report mitigate --agent oncall --outcome succeeded --result resolved --note "Retried and it stuck."
rf next --agent oncall
rf report close-ticket --agent oncall --outcome succeeded --note "Closed."
rf next --agent oncall
