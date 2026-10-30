# Local declared compensation arithmetic

This adds a separate read-only review inside the existing BIR file check. It
compares compensation subtotals already declared in a standalone Schedule1/2
file. It never computes expected tax, income classifications, wage eligibility,
PERA, December withholding/refunds or HRIS payroll agreement. Official module
acceptance remains unverified. No government software is executed.

References reviewed27October2026:

- [BIR published electronic layout, RMC25-2024 AnnexA](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2025-2024%20Annex%20A.pdf),
  pp6–9, including the distinct final Schedule2 net-statutory-basic column.
- [BIR printed compensation schedules](https://bir-cdn.bir.gov.ph/local/pdf/1604C%20Alphalist%20Format%20-%20final.pdf),
  pp1–2, which explicitly show subtotal relationships.

The inert supplied7.4 readable methods are corroborating reference evidence,
not the executed validator or a reason to discard centavos. Several methods
truncate to whole pesos. This review retains exact cents and does not claim
that its findings reproduce official validator acceptance or rejection.

## Supported equations

All column numbers below are **one-based electronic columns**, not printed-form
box numbers. Each row is checked independently, so opposite errors for two
people cannot cancel in an aggregate control. No source amounts are changed.

| Schedule | Declared target | Declared terms |
| --- | --- | --- |
| D1 |13|19 +23|
| D1 |23|20 +21 +22|
| D1 |26|32 +36|
| D1 |36|33 +34 +35|
| D1 |38|23 +36|
| D2 |13|23 +26|
| D2 |23|14 through22|
| D2 |26|24 +25|
| D2 |29|42 +45|
| D2 |42|59 +34 through41|
| D2 |45|43 +44|
| D2 |47|26 +45|

Schedule2 uses **actual net statutory basic in column59** in its present-exempt
sum. Daily/monthly/yearly amounts and the days/year factor are excluded from
that equation. This checks a declaration sum, not a statutory wage, employment
basis, deduction entitlement or MWE eligibility.

Schedule1 exempt-component equations are excluded: its electronic layout has a
separate exempt-basic field absent from the printed subtotal breakdown. The
combined-compensation column is also excluded: its published gross-style naming
conflicts with readable module methods using a taxable subtotal. No basis is
invented or silently relabelled. Tax due/credits/withholding/refund formulas and
eligibility need separate verification; they remain outside this increment.

## Whole-file, permission and export contract

Arithmetic runs only after the whole file has zero existing structure/control
errors. Missing/invalid amounts never become zero or an incomplete subtotal.
Unsupported mixed/encoded/sealed files retain whole-file refusal. A report with
errors, or an older serialized report without this review, shows Not checked.

Keep arithmetic comparisons/differences separate from established structure/error
counts and identity/declaration readers. The native header clearly reports
arithmetic differences even when exact file controls match. Every supported
detail contributes to counts; retain at most500 arithmetic diagnostics and show
the omitted count explicitly. A capped list is never a passing result.

The existing minimal CSV/Excel findings add an arithmetic summary and generic
line/field/equation diagnostics. Never export raw employee names, TIN/branch,
compensation amounts, source rows or HR evidence. Preserve the existing findings
columns/scope and the separate literal declaration worksheet. A declaration
reader is still a literal review, even when this arithmetic finds a discrepancy.

Reuse the current protected Payroll preparation adapter, fresh protected registration/access
checks, source-hash binding, source reread and final access check before new-file
IO. No source is uploaded, no export overwrites a file, and no registration,
payroll, receipt or Auth fact is changed.
