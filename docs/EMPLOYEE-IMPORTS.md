# Employee CSV imports

Administrators and HR Generalists can open **Reports > Employee import**.
Managers and ordinary Employees cannot import people or read batch history.
Imports create active employee records and optional contact/emergency details.
They do not create sign-in accounts or assign account roles.

1. Save CSV template, replace the fictional sample with company records, and
   save as UTF-8 comma-separated CSV. XLSX, UTF-16 and semicolon files are not
   import formats in this version.
2. Choose CSV to preview. Review each row, its physical source line and
   expandable employment/contact details. Previewing changes nothing.
3. Fix every error in the source file and reopen it. Export errors CSV lists
   source line, employee code, name and errors; it excludes private contact
   cell values. This error file is not an import template.
4. Enter an import reason and choose Import all employees. The database saves
   the whole batch, contacts, source links and employment history atomically.
   One invalid/conflicting row leaves no partial import.
5. Import history retains the actor, role, time, reason and created records.
   Open employee from a batch to use the normal employee editor.

Required headers are `employee_code` and `full_name`. Header names are case
insensitive and can be reordered; use each once. Optional headers are:

| Employment | Contact/emergency |
|---|---|
| department, job_title, joined_on | phone, personal_email, address |
| annual_leave_days | emergency_name, emergency_relationship |
| employment_type, work_location | emergency_phone, emergency_alternate_phone |
| reports_to_code | |

Use ISO dates (`YYYY-MM-DD`) and whole annual leave days from 0 to 366.
Blank annual_leave_days uses the company default captured in the preview.
Employment type/location must match existing active option names; reporting
manager codes must identify existing active employees. Import new managers
first, then import their reports. A changed referenced manager/option requires
a new preview. Other blank optional fields create empty values; there is no
silent update of existing employees.

Codes are normalized to uppercase. Codes already present, including archived
records, and duplicates within the source file are errors. Update an existing
record in Employees. Unknown columns such as role/password are rejected.
Standard employee/contact constraints apply, including complete emergency
name/relationship/primary phone when any emergency field is provided.

The prototype accepts at most 500 employee rows, 2 MiB and 4 KiB per cell.
The normalized server request also has a 2 MiB budget; large Unicode batches
that exceed it are flagged during preview and must be split.
Quoted commas, escaped quotes, UTF-8 BOM and quoted multiline addresses are
supported. Malformed quoting/encoding/headers stop preview; valid CSV rows
with field errors remain visible for correction. Blank physical lines are
ignored; a quoted empty record is validated as a record.

If an import's refresh fails after saving, Refresh or retry the **same preview**.
Its stable batch ID and server hash of normalized rows/reason prevent duplicate
people/history. Different details under that ID are rejected. Reopening a
successfully imported file instead finds existing codes and blocks new creates.

RLS and protected membership checks enforce HR access. The server supplies the
company, active status, new IDs and actor, rechecks references, serializes the
company write and retains immutable HR-only batch/items. Client roles cannot
edit/delete import audit tables. Raw CSV files and raw source paths are not
stored in that audit; normal imported employee/contact data is stored in the
company database. Closing/discarding the preview clears its in-memory source
values. Error exports recheck HR access before writing and preserve existing
files, using the same safe CSV quoting/formula-prefix handling as reports.

Migration `20261022095123_employee_csv_imports.sql` is additive and does not
rewrite existing employees. Batch import reversal is not an automatic delete:
archive synthetic or departed employees through the existing history-preserving
workflow. Bulk updates, account provisioning, attendance/leave imports and
XLSX mapping remain future modules.
