# Final pay — implementation design (not yet implemented)

The individual payment ledger in PAYROLL-PAYMENTS.md is the first foundation. The final-pay worksheet, independent approval and final settlement described below are pending. The installed annual-tax advice does not yet apply to unpaid final compensation.

## Verified source baseline — 24 October 2026

DOLE's [January2026 reminder](https://dole.gov.ph/news/final-pay-coe-must-be-released-on-time-dole/) reiterates Labor Advisory06-2020: final pay within30days of separation unless a more favorable agreement/policy applies; a certificate of employment within3days of request. Clearance does not reset this separation-based due date. The deadline is a review/overdue signal, not a reason to refuse a late actual payment record.

The [DOLE2024 handbook](https://nwpc.dole.gov.ph/wp-content/uploads/2024/11/Workers-Statutory-Monetary-Benefits-Handbook-2024-Edition.pdf), printedpp28–29,40–43,45–50, describes SIL conversion, proportionate13th-month, cause-specific separation amounts and retirement. Eligible half-month separation has a one-month floor; redundancy/labor-saving causes use a month's pay per credited service year. A six-month fraction counts as a year. The ordinary statutory retirement minimum uses22.5daily units per credited year, with eligibility/exceptions requiring actual service/duties and company review. The account role is never a legal classification.

Do not infer that all company leave is cashable, all separation is tax-exempt or every retirement arrangement qualifies for tax exemption. Check [BIR RR29-2025](https://bir-cdn.bir.gov.ph/BIR/pdf/RR%20No.%2029-2025%20digest%20FINAL.pdf) for the current private vacation-conversion de-minimis limit and [RMC5-2025](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%205-2025.pdf) for retirement-plan tax conditions. Recheck the exact applicable text before implementing each treatment.

## Required settlement contract

- Prepare reviewed last salary/cutoff and proportionate13th-month slips through the existing engines. Final pay references unpaid issued slips; it must not project earnings or generate another13th payout for the same entitlement.
- Snapshot current employee/service facts, immutable selected slips, their individual receipts, previously paid compensation, benefits, settled annual adjustments, documented prior-employer balances and evidenced openings. Revisions/current memberships and company payroll lock7 guard save, submit, independent Administrator approval and settlement.
- Preview annual tax over received compensation plus selected unpaid compensation and reviewed additional taxable amounts at the planned cash date. Credit original withholding and earlier settled adjustments once. Keep the annual90k benefit exclusion shared. Classifications, extra contributions and lawful deductions need explicit evidence/review; no automatic absence/debt deductions.
- Review leave conversion, separation/retirement eligibility and pay bases, deposits/refunds and documented deductions separately. Avoid duplicate source periods, vacation conversions, previously paid benefits and previously settled adjustments. Store HR evidence separately from own aggregate statements.
- Record final settlement atomically: consume only selected outstanding individual payslips, issue/record the additional final amounts and the signed annual adjustment once, and retain all earlier published slips/advice. A normal later batch payment must skip those receipts. A concurrent payment or tax close requires a fresh review. New annual sources must recognize settled final amounts and credits without adding them twice.
- Keep any deficiency beyond collectible last-pay cash distinct as employer handling; do not create an automatic employee debt. No bank transfer, official2316 certificate, filing or remittance is implied.
- Do not require account deactivation, employee archiving, completed clearance or an active exit case before paying owed compensation. Finalizing employment/access remains a separate protected exit workflow. Archived employees still have retained payroll history.

Next: implement and verify the prospective final-pay engine and source/settlement guards, then native worksheet/statement and role-scoped exports. This design is historical implementation context, not a claim of delivered functionality.
