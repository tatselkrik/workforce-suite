# Desktop0.1.61 - declared compensation sums

1. Sign in as HR/Admin. Open **Payroll → Government → Reconciliation**, select
   **BIR**, then **Check BIR1604C file**. Ordinary Employee/Manager accounts must
   not gain this company-wide review.
2. Choose a fictional unvalidated standalone Schedule1/2 `.dat` source. The
   usual structure/control counts remain visible. The separate **Declared
   compensation arithmetic** section shows five supported sums per Schedule1
   row, or seven per Schedule2 row.
3. A one-cent subtotal discrepancy must remain visible even when all file
   controls match. Opposite errors for two people must appear separately. Check
   physical line, field and related column numbers; no source amount is adjusted.
4. A malformed/missing amount or control mismatch must show arithmetic **Not
   checked**, without a partial result. Schedule2 wage/eligibility review remains
   visible even when its declared sums match.
5. Save findings to new CSV and Excel files. They contain separate structure/
   arithmetic summaries and generic line/field/equation diagnostics. Employee
   names, tax identifiers and compensation values must stay out of these minimal
   findings. The separate literal-declaration worksheet retains its own scope.
6. Source changes require rechecking; existing export files must survive. A role
   revoked before export must no longer save a fresh company report.

Supported equations and exclusions are documented in the
[arithmetic contract](BIR-DECLARATION-ARITHMETIC.md). Daily/monthly/yearly MWE
figures and the days/year factor never replace actual net statutory basic.
Ambiguous combined-compensation, Schedule1 exempt-component, tax/PERA/refund
relationships and official acceptance remain outside this review. It does not
calculate tax, certify2316s, import records or file a return.

Verification, exact frozen package, installation and private-channel evidence
are recorded separately in [the release evidence](BIR-0.1.61-VERIFICATION.md).
The local portal remains on its separate verified0.1.7 lifecycle.
