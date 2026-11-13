#!/usr/bin/env bash
# Build the invoice plan from DESIGN.md. Sourced by each run script.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/../bin.sh"
example_store
PLAN="$(basename "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)")"

rf goal --text "Pay INV-1042, or hand it to a person"
rf add parse --title "Parse INV-1042" \
  --done-when "Vendor, lines, stated discounts, and the billed total are recorded" \
  --produces invoice --actor invoices
rf add verify-vendor --title "Verify Northwind is approved to bill" \
  --done-when "Northwind is approved, or the reason it is not is recorded" \
  --needs parse --produces vendor --results approved --results rejected --actor invoices
rf add verify-pricing --title "Match each line to NW-2024" \
  --done-when "Every line matches an agreement price, or each miss is recorded" \
  --needs verify-vendor --when vendor=approved --produces pricing \
  --results aligned --results misaligned --actor invoices
rf add verify-discounts --title "Check the volume discount on INV-1042" \
  --done-when "The 10% discount is confirmed, or the shortfall is recorded" \
  --needs parse --needs verify-pricing --when pricing=aligned --produces discounts \
  --results correct --results incorrect --actor invoices
rf add escalate --title "Open a human review for INV-1042" \
  --done-when "A review is open and holds the invoice, NW-2024, and the mismatch" \
  --needs parse --when-any vendor=rejected --when-any pricing=misaligned \
  --when-any discounts=incorrect --produces review --actor invoices
rf add human-review --title "Decide whether INV-1042 may be paid" \
  --done-when "A person approves a payment amount or rejects the invoice" \
  --needs escalate --produces decision --results approved --results rejected --actor human
rf add pay --title "Pay the agreed amount for INV-1042" \
  --done-when "Payment is submitted for the amount in the inputs and the reference is recorded" \
  --needs parse --when-any discounts=correct --when-any decision=approved \
  --produces payment --actor invoices
rf add close-rejected --title "Close INV-1042 unpaid" \
  --done-when "The invoice is closed with the rejection reason" \
  --needs human-review --when decision=rejected --produces closure --actor invoices
rf commit
