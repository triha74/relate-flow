#!/usr/bin/env bash
# expect: done
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="Checkout is down."
rf next --agent oncall
rf report triage --agent oncall --outcome succeeded --result sev2 --note "Search is slow."
rf next --agent oncall
rf report mitigate --agent oncall --outcome succeeded --result unresolved --note "Still degraded."
rf next --agent comms
rf report status-page --agent comms --outcome succeeded --note "Notice posted."
rf next --agent oncall
rf report vendor-escalation --agent oncall --outcome succeeded --result paged --note "Vendor paged."
rf next --agent oncall
rf report postmortem --agent oncall --outcome succeeded --note "Postmortem filed."
rf next --agent oncall
rf report close-ticket --agent oncall --outcome succeeded --note "Closed after the vendor page."
rf next --agent oncall
