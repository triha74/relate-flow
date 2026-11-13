#!/usr/bin/env bash
# expect: done
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="Checkout is down."
rf next --agent oncall
rf report triage --agent oncall --outcome succeeded --result sev1 --note "Checkout is down."
rf next --agent ic
rf report page-ic --agent ic --outcome succeeded --note "IC is on the bridge."
rf next --agent oncall
rf report mitigate --agent oncall --outcome succeeded --result resolved --note "Rolled back the deploy."
rf next --agent comms
rf report status-page --agent comms --outcome succeeded --note "Status page updated."
rf next --agent oncall
rf report postmortem --agent oncall --outcome succeeded --note "Postmortem PM-14 filed."
rf next --agent oncall
rf report close-ticket --agent oncall --outcome succeeded --note "Ticket closed."
rf next --agent oncall
