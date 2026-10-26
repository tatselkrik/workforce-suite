# Attendance/leave imports and native Excel — implementation plan

Current delivery: native Excel0.1.50, attendance-correction imports0.1.51and
leave-request imports0.1.52are installed and privately published.
See [attendance imports](ATTENDANCE-IMPORTS.md) for its independent approval,
immutable history and exact actor/payload retry contract.

This plan was implemented after desktop0.1.49. Roadmap item6is complete; the
current queue continues with accounting/external-payroll exchange foundations.

Start with native XLSX file support for the existing employee/opening import
previews and attendance/leave/workforce report exports. CSV remains supported.
Use pinned free Rust libraries without Microsoft Office or a paid subscription.
Preserve literal identifiers, Unicode, dates and exact reviewed source values;
write text explicitly so formula-like employee fields are never executable.

Imports use a single visible data worksheet with the existing template columns.
Preserve actual worksheet row numbers in errors. Reject formulas/cached formula
results, macros, external connections, ambiguous merged/hidden data, invalid
types and excessive compressed/uncompressed sizes or dimensions. Never evaluate
a workbook, fetch a link or silently truncate rows. CSV/XLSX reach the same
existing protected preview, all-or-none posting and role checks. Export only a
fresh scoped report, recheck current access, create a new file and retain existing
files. Agency-specific upload formats remain separate contracts.

Then add attendance and leave request import templates, protected previews,
row-level error reports and atomic actor-bound batches. Imports must use the
normal correction/leave approval workflows; they must not directly overwrite
attendance, mark leave approved, bypass dated balances/approval plans or change
issued payroll. Bind preview to current sources, enforce permissions again when
posting, refuse stale/partial batches and retain an immutable import history.
Define the exact time/date and duplicate rules before adding these server writes.

Accounting/external-payroll exchange foundations follow this milestone. Actual
vendor connections, paid services, external messages and device work remain
outside this increment. No new database write is authorized by this design note;
the existing exact-target prototype authorization governs implementation.
