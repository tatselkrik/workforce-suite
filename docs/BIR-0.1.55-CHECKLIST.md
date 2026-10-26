# Desktop 0.1.55 — BIR file checks to try

Installed/private delivery is verified in [the evidence note](BIR-0.1.55-VERIFICATION.md). Use a
fictional, unvalidated plaintext1604C Schedule 1 file; never upload real employee
files to another service for these checks.

1. Open **Payroll → Government → Reconciliation**, choose **BIR**, then
   **Check BIR 1604C file**. This control is available only to HR/Admin.
2. Choose an unvalidated `.dat` source with one H1604C header, 49-column D1
   details and 36-column C1 control. Read the source hash, year, detail count and
   findings. Missing current company BIR registration must remain unverified;
   a different registered employer must be flagged.
3. Expand **Exact Schedule 1 control totals**. Each published monetary field,
   including PERA, shows the exact detail sum and declared control. Blank or
   malformed amounts show an unavailable total, rather than zero. A one-cent
   discrepancy remains visible.
4. Save findings as **CSV** and **Excel** to new files. They contain the source
   hash, year, line/field and generic diagnostics. Employee names, identifiers
   and compensation values must not appear in the diagnostic export.
5. Change a separate source copy after checking it, then try Save findings.
   Saving must stop until you recheck. Existing exports must never be overwritten.
6. An unsupported Schedule 2, sealed/validated file, non-ASCII encoding or source
   above 2 MiB must be refused explicitly. The checker supports at most 500 details.
7. Manager/ordinary Employee roles must not gain this company-wide file view.

These checks never import employees, edit registration or payroll, calculate
tax, certify2316/substituted-filing eligibility or submit a return. **Structure
consistent does not mean officially validated.** Official module behavior,
including the published PERA control/reference-method conflict, remains separate.
