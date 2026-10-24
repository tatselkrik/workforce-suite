# Linked payroll amendments and delayed final pay

Source 0.1.41 adds **Payroll → Amendments** and extends **Final pay** to a prior-year separation with a reviewed 2026 payment date. Hosted deployment, release and installation are established separately by versioned evidence. This module records reviewed obligations and payments; it makes no transfer or filing.

## Original records and supplemental corrections

Select an actually paid monthly, cutoff or13th-month payslip, or a settled final statement. Its original figures and receipt remain immutable. HR/Admin create a linked supplemental worksheet, review the corrected difference, save and submit it. An Administrator distinct from the creator, most recent preparer and submitter issues its immutable statement. One approved supplemental payout per employee may await payment; settle or withdraw earlier pending tax advice/final pay first.

Enter **additional amounts owed**, rather than replacement totals. Additional basic is part of regular taxable earnings, and only salary/cutoff originals accept a basic correction. Qualifying 13th-month/other benefits share the annual PHP 90,000 exclusion. Known issued benefits and evidenced outside benefits set protected floors; later annual/13th worksheets retain them without inventing duplicate income. Additional earned basic increases the original period's 13th-month basis once, without adding overlapping service dates. An additional 13th-month payment counts as an earlier payout in the same entitlement review; evidence must explain the corrected entitlement or more favorable policy.

SSS, PhilHealth, Pag-IBIG and withholding inputs are **reviewed differences**. Reconcile the original contribution month and supplementary/cumulative withholding; do not enter a second full contribution charge. Employer-side remittance corrections remain separate reviewed reconciliation. Evidence must support any exempt compensation, reimbursement, deposit return or lawful deduction. The app does not infer exemptions or authority from labels, roles or minimum-wage flags.

A negative employee contribution/withholding amount represents a same-receipt-year refund of a retained charge. It cannot exceed the original plus earlier approved linked charges. Withholding refunds also cannot exceed the employee's remaining recorded company tax credits after settled annual/final adjustments. This prevents refunding the nominal original charge again after an annual tax refund. Cross-year tax/contribution refunds require a separate year-specific process.

Only an Administrator records actual supplemental cash on its approved payment date, after approval and no later than today, including a reviewed zero cash result. That receipt belongs to the supplement; it never marks the original or a coworker paid again. Withdraw an **unpaid** supplement to reschedule, retaining its issued statement and terminal status. Paid amendments and receipts cannot be rewritten or reversed. Current membership, payroll locking, source freshness, independent approval and actor-bound retry identities apply throughout. An audit failure rolls the entire write back.

## Overpayment discrepancies

Choose **Overpayment review only** for a negative discrepancy. Independent approval records an immutable reviewed amount, without creating a collectible receivable, deducting wages, reducing taxable income/contributions or changing13th-month entitlement. No payment can be recorded against it. Actual recovery, litigation, amended returns and prior-year liability restatements are separate processes requiring their own resolved legal and financial basis. A review is not employee consent or an instruction to recover money.

## Payment and service in different years

The reviewed final-pay pack accepts cash dates from 6 January through 31 December 2026, with an actual 2025 or 2026 separation. Service, credited years, entitlements and release deadline use the real employment dates. If employment ended before2026, the tax source has **no current-year service coverage**; it must not demand invented zero-income service days.

HR must explicitly review actual/constructive receipt evidence, earlier-year certificates/withholding and other employment before issuing delayed final pay. Income already made available or received in an earlier year must not be moved into2026 or credited again. Ordinary delayed unpaid amounts are reviewed in the supported receipt year. Current-year certified successive employment may be included when this employer's service ended before that year. Concurrent employment, special awards/backwages and unusual tax regimes require separate review. The annual tax workflow can retain the actual earlier service end for a later receipt-year confirmation. No2027 rate pack is implied.

The default final-pay deadline remains30days from separation, or an earlier more favorable company deadline. A delayed payment is flagged rather than blocked. Clearance, archiving and account access remain independent from owed pay. Existing same-year v1 final statements retain their original slips and settlement path.

## Privacy, statements and reporting

Private worksheets, source evidence, payment references and audit records are HR/Admin-only. Other staff receive only their own issued aggregate amendment, original link and aggregate paid/withdrawn status. Ordinary Employee accounts still use the separate portal and cannot sign into the desktop. UTF-8CSV and printablePDF omit private evidence and references. Export reloads current scope and rechecks access before creating a local file.

The new report-v2 endpoint adds linked amendments while retaining the earlier report endpoint for older installations. Supplemental cash appears once, separately from original source receipts and combined final cash. Signed statutory adjustments and review-only discrepancies remain distinct. Withdrawn statements retain issue history and create no cash/outstanding amount. An overpayment review is never added to payroll cash or collectible balances. Reports retain their366-day/5000-row bounds and fail rather than truncate totals. Older clients keep the legacy worksheet shape and source reports. Consolidated reports affected by amendments, and cross-year annual-tax reports, require an app update instead of returning incomplete figures.

## Sources checked 24 October 2026

- [BIR RR11-2018](https://bir-cdn.bir.gov.ph/local/pdf/RR%20No.%2011-2018.pdf), annualization and2023-onwards schedule, printedpp33–35: received current-year regular/supplementary compensation, net previous withholding and annual adjustments.
- [BIR RR29-2025](https://bir-cdn.bir.gov.ph/BIR/pdf/RR%20No.%2029-2025.pdf): effective dated de-minimis provisions. Final leave/retirement/separation classifications retain the sources and bounds in [PAYROLL-FINAL-PAY.md](PAYROLL-FINAL-PAY.md).
- [DOLE final-pay reminder,21January2026](https://dole.gov.ph/final-pay-coe-must-be-released-on-time-dole/):30days from separation unless a more favorable policy applies.
- [Supreme Court, G.R.167679,22July2015](https://elibrary.judiciary.gov.ph/thebookshelf/showdocs/23/60762), with [decision text](https://lawphil.net/judjuris/juri2015/jul2015/gr_167679_2015.html): compensation actually/constructively paid and the conditions for constructive receipt. Applying the receipt-year separation in this prototype is a bounded implementation interpretation, subject to explicit evidence review, rather than an automatic allocation of special backwages.

The current Supabase changelog and function/RLS documentation were checked. New public tables use explicit grants and current protected membership policies; exposed RPC wrappers remain invokers and protected writers stay in the private schema.
