# Desktop0.1.54 — government Excel review checklist

Delivery/installation status and completed technical checks are recorded in the
matching verification report. This checklist describes the intended increment;
it does not by itself prove installation or agency acceptance.

1. Open **Payroll → Government → Reconciliation** as Administrator or HR
   Generalist. Choose an agency/month and Generate/refresh. Select **Excel
   (.xlsx)**, then **Export fresh review**, using a new filename.
2. Open the workbook in an already available spreadsheet viewer. Check the frozen
   header/filter, complete source rows, period/inclusion fields and summary.
   Identifiers and PHP amounts are literal text. Editing them does not recalculate
   totals or change HRIS. CSV remains selectable.
3. Export **agency preparation**. Check its selected agency identifiers, leading
   zeros, legal names/revisions and missing-registration flags. Confirm it remains
   clearly an internal review worksheet. Keep these files private to HR.
4. Review any unresolved source: unknown shares/coverage must remain marked,
   rather than becoming zero or a complete liability. BIR postdated receipts
   retain their date and are excluded from current received withholding.
5. For split government receipts, compare the selected-month allocation to the
   whole receipt. The worksheet uses only that month. Voided receipts stay visible
   with zero remitted contribution; original payroll deductions remain intact.
6. Try an existing filename: the app must preserve the existing file. Use `.csv`
   or `.xlsx`; legacy `.xls`, executable and other extensions are unsupported.
7. Manager/ordinary Employee roles must not receive government export controls or
   private registration/report data. Role changes are rechecked before file saving.

The separate SSS reference draft remains internal review. Official agency-file
validation and portal browser verification are separate pending work. This
increment does not install Office, send documents, file returns or transfer money.
