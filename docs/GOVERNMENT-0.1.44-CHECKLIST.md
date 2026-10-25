# Version 0.1.44 review checklist

This candidate adds agency preparation exports. Installation evidence is recorded
separately; use fictional employee details while testing.

1. As HR/Admin, open **Payroll → Government**, select an agency/month and generate
   the report. Choose **Export agency preparation CSV** and a new local filename.
2. Check the employer/employee legal fields and registration revisions. SSS must
   include SSS identifiers only; PhilHealth adds PIN/PEN and employee birth date/
   sex; Pag-IBIG uses MID/employer IDs; BIR uses TIN/branch/nationality. Another
   agency's identifiers and private notes/evidence must not appear.
3. Blank registered fields stay blank with missing-data flags. Unknown shares or
   totals stay UNRESOLVED. A source marked NO is excluded from the summary even
   though its original amount remains visible for review. Preserve leading zeros
   by importing identifier columns as text in a spreadsheet.
4. Compare the summary with fresh reconciliation: contributions follow earned
   months, BIR follows individual received dates, and remittances/voids remain
   distinct from wage receipts. Exporting must not change payroll or registration.
5. Export the older **fresh review CSV** too. It must remain free of registered
   identifiers. Manager/ordinary Employee access must not expose either HR export.
6. Inspect the new button and descriptive text in your usual window size/theme.

Preparation CSV is an internal worksheet. It is not an agency upload or official
return. Exact agency file controls/validator acceptance remain queued; the SSS
reference encoder is a tested foundation without a product download control.
