#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/../bin.sh"
example_store
PLAN="$(basename "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)")"

rf goal --text "Restore the service, or escalate it"
rf add triage --title "Classify the incident" \
  --done-when "Severity is recorded" --produces severity \
  --results sev1 --results sev2 --results sev3 --actor oncall
rf add page-ic --title "Page the incident commander" \
  --done-when "The incident commander is paging" \
  --needs triage --when severity=sev1 --produces page --actor ic
rf add mitigate --title "Mitigate the impact" \
  --done-when "The impact is resolved, or it is recorded as unresolved" \
  --needs triage --produces mitigation --results resolved --results unresolved --actor oncall
rf add status-page --title "Publish a status notice" \
  --done-when "Customers can see the notice" \
  --needs triage --when-any severity=sev1 --when-any severity=sev2 \
  --produces notice --actor comms
rf add vendor-escalation --title "Escalate to the vendor" \
  --done-when "The vendor is paged" \
  --needs mitigate --when mitigation=unresolved --produces vendor --results paged --actor oncall
rf add postmortem --title "Write the postmortem" \
  --done-when "The postmortem is filed" \
  --needs mitigate --when-any severity=sev1 --when-any severity=sev2 --actor oncall
rf add close-ticket --title "Close the ticket" \
  --done-when "The ticket is closed" \
  --when-any mitigation=resolved --when-any vendor=paged --actor oncall
rf commit