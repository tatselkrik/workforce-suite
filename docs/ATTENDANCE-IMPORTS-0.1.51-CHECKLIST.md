# Desktop0.1.51 — attendance correction imports

Open **Reports > Attendance import** as an Administrator or HR Generalist.

- Save a CSV or Excel template. Add reviewed employee codes, ISO work dates,
  check-in/checkout timestamps with explicit offsets, a correction reason and an
  optional HR reference. Excel dates can be date-only cells; clocks stay Text.
- Open the file. Unknown/archived employees, duplicate dates, overlapping clocks
  and invalid source rows should show errors. Save the row review as CSV/Excel.
- Enter the private import note, then preview all rows. Preview creates nothing.
  A changed attendance/break/employee source requires a fresh preview.
- Submit a small reviewed batch. Its rows should appear as **Pending** in import
  history and in normal attendance-correction review. Attendance stays unchanged.
- Have a different authorized reviewer approve a request. Its clocks and normal
  correction history should change once, and import history should show Approved.
  The importer cannot approve their own submission.
- The original importer can withdraw a pending row with a reason. Its history
  stays visible; an approved row cannot be withdrawn. The reason is shared with
  the employee/reviewer, while HR references remain private.
- If confirmation is interrupted, reload history or retry the exact submission.
  Keep its original identity and payload instead of creating a replacement batch.
- Managers cannot open this import tab. Employees retain their own ordinary
  portal requests and cannot read private import ledgers.

This release supplies the attendance part of roadmap6. Leave-request batch imports
remain next. No automatic approval, employee impersonation, raw-file upload,
payroll rewrite, paid service or biometric connection is introduced.

Local candidate evidence:534default Rust checks (seven explicit opt-ins ignored),
82attendance-import PostgreSQL assertions,6,287existing migration/workflow/
preservation assertions and fresh67migration replay pass. Exact hosted, installed
and private-channel delivery evidence is recorded separately when completed.
Fictional native layout capture does not claim real file-dialog/Office interaction.
