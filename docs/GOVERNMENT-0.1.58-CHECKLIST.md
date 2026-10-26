# What to check in 0.1.58

Open **Payroll → Government → BIR → Review BIR file identities** as HR/Admin.

1. The current prototype company BIR registration is incomplete. Expect a clear
   setup message until its TIN/branch is entered under Government → Registration
   details. This update does not invent or overwrite those fields.
2. With company/employee BIR registrations configured, select the file's year
   (2025 onward) and a local standalone Schedule 1 or 2 `.dat` file. Its employer
   header/year must match, and its structure/control checks must be consistent.
3. Check the three outcomes: a unique current TIN/branch pair shows its employee
   code/name; unmatched rows select nobody; ambiguous pairs select nobody and
   show the candidate count. Similar names must not choose a match.
4. Compare original file line numbers and source hash. Matched rows show record
   counts from the annual ledger, including actual receipt-source counts. These
   counts are not a comparison of tax amounts or proof of required declarations.
5. Export a fresh CSV/Excel review to new filenames. It should contain the hash,
   year/snapshot, line/status, current matched code/name and counts. Raw tax IDs,
   file-declared names and compensation are omitted. Changing the source requires
   rechecking; an existing destination must survive.
6. Check light/dark/compact layout, Back to reconciliation and Manager denial.
   Existing BIR preflight and Annual recorded sources remain available.

The file stays local. Payroll, registrations and the source are unchanged.
Historical identity/agency assignment, 2316, expected withholding, MWE eligibility
and official BIR validation/filing remain separate and unverified.

Codex verified native logic, real local HTTP/file guards, hosted protected roster
reads and the missing-company-setup guard. Full matching with configured hosted
company data, signed-in button/file-dialog actions and Office UI still need a
manual/live check. Fictional positive cases were independently verified.
