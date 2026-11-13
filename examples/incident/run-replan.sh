#!/usr/bin/env bash
# expect: done
set -euo pipefail
source "$(dirname "$0")/plan.sh"
begin_run case --input case="A nightly job failed."
rf next --agent oncall
rf report triage --agent oncall --outcome succeeded --result sev3 --note "A job failed."
rf next --agent oncall
rf report mitigate --agent oncall --outcome failed --because "first"
rf next --agent oncall
rf report mitigate --agent oncall --outcome failed --because "second"
rf next --agent oncall
rf revise
rf set mitigate --title "Mitigate with the vendor on the call"
rf commit
rf next --agent oncall
rf report mitigate --agent oncall --outcome succeeded --result resolved --note "Vendor walked us through it."
rf next --agent oncall
rf report close-ticket --agent oncall --outcome succeeded --note "Closed."
rf next --agent oncall
