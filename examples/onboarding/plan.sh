#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/../bin.sh"
example_store
PLAN="$(basename "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)")"

rf goal --text "Open the account, or decline the application"
rf add collect-docs --title "Collect the application" \
  --done-when "The application documents are on file" \
  --produces application --actor intake
rf add verify-identity --title "Verify identity" \
  --done-when "Identity is verified, or the mismatch is recorded" \
  --needs collect-docs --produces identity --results verified --results mismatch --actor kyc
rf add sanctions-screen --title "Screen sanctions" \
  --done-when "The screening result is recorded" \
  --needs collect-docs --produces sanctions --results clear --results hit --actor screening
rf add risk-score --title "Score the risk" \
  --done-when "Risk is low or high" \
  --needs verify-identity --needs sanctions-screen \
  --when identity=verified --when sanctions=clear \
  --produces risk --results low --results high --actor kyc
rf add compliance-review --title "Compliance review" \
  --done-when "Compliance approves or declines" \
  --needs collect-docs \
  --when-any identity=mismatch --when-any sanctions=hit --when-any risk=high \
  --produces ruling --results approved --results declined --actor compliance
rf add open-account --title "Open the account" \
  --done-when "The account number is recorded" \
  --needs collect-docs --when-any risk=low --when-any ruling=approved \
  --produces account --actor intake
rf add decline --title "Decline the application" \
  --done-when "The applicant is told and the file is closed" \
  --needs collect-docs --when ruling=declined --produces closure --actor intake
rf commit
