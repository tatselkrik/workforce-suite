# Native Excel exchange

Delivered0.1.50 adds local `.xlsx` support using pinned free Rust libraries. It
requires no Microsoft Office installation, subscription or external conversion
service. CSV remains supported. No schema change is needed beyond installed
schema66, and existing protected transactions remain authoritative.

## Supported screens

- Reports: CSV or Excel for attendance, leave, employee summaries, headcount and
  trends, including saved/custom column order and the same scoped rows/metadata.
  Dates, integer counts/minutes and exact half-day quantities have native cell
  types; identifiers and timezone-bearing timestamp strings remain literal text.
- Employee imports: CSV/Excel source, empty Excel template and Excel error report.
  Existing create-only validation, protected posting and no automatic Auth account
  creation remain. Excel uses Text columns to preserve codes and contact numbers.
- Opening balances: CSV/Excel source/template and row-review/error report. The
  same protected previews, declarations, actor-bound retries and atomic batch
  posting from0.1.49 apply. A workbook is not an authorization or a direct grant.

## File boundaries

Imports accept one visible data worksheet with template headers starting in A1.
Keep employee codes, phone numbers and other identifiers as Text. Quantity cells
can contain reviewed numbers or template text. Employment dates can be ISO text
or ordinary Excel date cells without time/timezone. The fictitious1900leap day
and ambiguous numeric identifiers are rejected. Preserve actual worksheet row
numbers, including blank-row gaps, when showing validation errors.

Reject formulas/cached formula results, macros/embedded executable files,
external data links/connections, hidden worksheets/rows/columns, merged cells,
duplicate/mismatched addresses and invalid XML/entities. No workbook evaluation
or link fetching occurs. Import limits:2MiB compressed,16MiB total expanded,
8MiB per part,128parts,4KiB per field and the existing500employee/100opening rows.
Check declared dimensions and string/style allocation hints before the reader,
including hints in substituted namespaces. Excess data/formatting causes explicit
refusal; it is never silently truncated. Legacy `.xls`, `.xlsm` and `.xlsb` are
outside this increment.

Exports preserve formula-like user text as string cells. All report generation
uses fresh records/current scope; render before the final protected authorization
check, then write a new file. Existing files remain unchanged. Failure to represent
a source quantity/date safely blocks the export. Report exports are limited to
50,000rows,64columns and2million cells. Government upload adapters retain their
separate format contracts; an Excel report is not an agency return.

## Still queued in roadmap6

Attendance/leave request imports are not supplied by this Excel increment. Their
templates, source/time rules, normal approvals, protected previews, immutable
batch history and atomic posting still require implementation and verification.
Keep the full roadmap item open. Accounting/external-payroll exchange foundations
follow; no unresolved external connection or paid service is enabled here.
