# Desktop0.1.62 — what to check after delivery

**Installed and privately published.** This builder starts in0.1.62;
earlier0.1.61does not contain it. See
[verification and limits](BIR-0.1.62-VERIFICATION.md).

1. Sign in as HR/Administrator and open **Payroll → Government → Reconciliation**,
   select **BIR**, then **Build BIR1604C draft**. Managers/Employees must not receive
   the builder or its templates through their account scope.
2. Select Schedule1or Schedule2and a supported year. Save a blank CSV or Excel
   template. It must contain headers only, without sample people or assumed pay.
   Keep identifier/amount/date cells as reviewed text in Excel; do not use formulas.
3. Choose a completed declaration template and preview it. Explicit zero is
   required where applicable; blank amounts, unsupported names/codes, duplicates,
   invalid dates, excess rows and inconsistent supported subtotals must fail.
4. Incomplete company BIR registration must block preparation without changing
   company facts. With complete reviewed registration, the preview shows the
   selected schedule/year, count, published filename and expandable exact totals.
5. Change the source or company registration after preview. Saving must require
   another preview/review. Switching year/schedule clears the prior preview and
   review checkbox. A failed request must not leave an old success displayed.
6. Review the declarations explicitly, then save the DAT draft in a new folder.
   Its fixed filename follows the registered employer/year. Existing files must
   remain untouched; no source, payroll, receipt or registration is rewritten.
7. Check normal/compact and light/dark views, especially the declared control
   labels/amounts, longer MWE text, review checkbox and save button.

The DAT contains declared tax IDs/pay. It is a local draft awaiting official BIR
validation, not a filed/accepted return or an automatically reconciled payroll
declaration. No subscription, public portal, hardware or external delivery is
needed for this local assembly workflow. Actual agency acceptance remains separate.
