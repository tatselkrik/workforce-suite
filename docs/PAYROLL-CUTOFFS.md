# Semi-monthly payroll cutoffs

Payroll → Cutoffs provides two runs per month:1st–15th and16th–month-end.
Each draft has a planned pay date and independently approved employee payslips.
Monthly registers remain available for reconciliation. Server guards prohibit
issuing both a monthly register and cutoff pay for the same employee/month,
including requests from an older installed desktop.

## Calculation policy

This prototype allocates monthly basic and recurring components equally between
the halves. The first receives the whole-centavo floor; the second receives the
remainder. Daily-paid basic uses the reviewed actual dates. Premium hourly rates
continue to use the full salary reference and existing Philippine holiday,
overtime and night-differential rules. Missing attendance never creates an
automatic absence deduction. A mid-cutoff joiner needs explicit reviewed basic
proration; worksheet dates must fall inside the cutoff and follow the hire date.

An overtime authorization crossing the 15th/16th boundary cannot be used twice.
The second cutoff retains current monthly authorization facts and subtracts
overtime minutes already issued in the first. Extra minutes require an explicit
HR exception note and independent approval; refreshing sources clears review.

The rule pack deducts monthly SSS, PhilHealth and Pag-IBIG contributions on the
second cutoff. This is this prototype's deduction-timing policy, rather than a
claim that every company must follow that timing. The second uses issued first
gross/basic/allowance facts plus current reviewed amounts. PhilHealth still uses
the fixed monthly salary reference. Overrides need a documented review. Rice and
laundry exemptions share their monthly caps across both halves; recurring
allowances, deductions and advances are allocated once rather than charged twice.

Withholding uses the applicable **semi-monthly** table in
[BIR RR11-2018 AnnexE, effective2023 onwards](https://bir-cdn.bir.gov.ph/local/pdf/Annex%20E%20RR%2011-2018.pdf).
It is computed separately for each cutoff's taxable compensation and mandatory
employee deductions. Its rounding/thresholds can differ from one monthly
withholding calculation. See [annual tax review](PAYROLL-ANNUAL-TAX.md) for year-end annualization and prior-employer evidence. Contribution references remain in [payroll](PAYROLL.md).

An existing employee needs an issued first cutoff before the second can be
submitted. A person hired after the15th may have only the second. Refreshing a
source clears its review; a newly issued first slip invalidates an older second
draft until refreshed. The first issued slip stays unchanged. A13th-month basic
accrual memo reconciles the two halves' rounding; it is not an actual payout.

## Workflow and access

1. Payroll Officers and Administrators create the period/title/planned pay date and add employee worksheets.
2. Review actual days or use an attendance proposal limited to this cutoff.
   Review leave/holiday classifications, current OT approvals and documented
   exceptions, coverage, proration and deductions. Save the worksheet.
3. Confirm included/omitted employees and submit. Another Administrator approves.
   Approval rechecks current sources and publishes immutable redacted slips.
4. An Administrator may record an actual paid date/reference. This moves no money.

Draft title/pay-date edits, returns, cancellation and removal retain reasons and
audit history. Issued pay cannot be cancelled or overwritten. Negative net pay
must be corrected before approval. Planned dates are bounded from period end to
16days afterward; employers must still review the actual legal/contractual pay
schedule. Approval requires a completed cutoff and a supported2026 rule pack.

Payroll Officers and Administrators see drafts and registers. Other staff receive only their own issued
cutoff payslips; ordinary Employee desktop admission stays prohibited. Workforce Employee displays the signed-in person's issued cutoff payslips. Slips exclude raw source
bundles, HR reviews/coverage notes and component evidence references. Monthly and
cutoff records have separate tables and are compatible with older installations.

## Reports

Cutoff detail offers a register CSV and an issued monthly-cutoff reconciliation
CSV. The latter identifies the included halves and incomplete months; it does
not fill missing pay or produce a filed government return. Issued slips have CSV
and printable PDF exports with their exact period and planned pay date. The
older Reports annual export is explicitly limited to monthly registers.
