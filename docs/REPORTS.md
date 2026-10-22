# Reports and CSV exports

Open **Reports**, before Account Manager in the sidebar. Choose Attendance
detail, Leave detail or Employee summary, then a date range and optional
employee/department filters. Use the calendars, This month or Last 30 days.
Run report reloads current permitted records. Results show their generation
time, use 25-row preview pages and scroll in both directions. Export CSV
reloads current data again, replaces the preview with that report and saves
all rows to a new UTF-8 CSV file readable by spreadsheet applications.

## Attendance and employee totals

Attendance detail shows work dates, local punches and their time zones,
saved shifts/rule versions, gross minutes, completed unpaid breaks, minutes
excluding those breaks, calculation state, late/early-departure minutes,
overtime candidates, current approved overtime, approved leave and pending
correction counts. Overnight punches retain the recorded work date. Unknown
final totals remain blank. Gross/net elapsed time and overtime authorization
are separate from payable time and payroll.

Employee summary aggregates available values and explicitly counts open
records, incomplete net totals and records needing calculation review.
Previous overtime approvals whose attendance, breaks or leave no longer
match their basis contribute zero current approved minutes; saved decisions
remain in the overtime history.

Include days without check-in adds active employees' dates from their known
join date. It distinguishes scheduled, off-day, unassigned, full/half-day
approved leave and not-yet-due dates. A missing check-in is a review flag;
it does not establish absence. Scheduled past days without check-in excludes
approved leave. Today is treated as not yet due because a shift may still be
open or upcoming. Archived employees retain actual records and filed leave
but have no inferred missing days or termination date.

Employee labels, department, job title, location, type and status reflect
current employee records. Existing attendance uses saved rules/schedules;
missing-day schedules use dated assignments and the current holiday calendar.
These reports do not reconstruct historical departments or employment dates.

## Filed leave

Leave detail emits one row per saved charge date inside the selected period,
including half-days, filed type labels, status, original request range/total,
policy version and approval progress. A request spanning the filter boundary
is clipped by charge date; its original filed total remains separately labeled.
Rejected and cancelled charges remain history and do not consume balances.
Use the status filter to select a decision state. Leave reasons and private
contact/emergency details are excluded from all report exports.

## Access, files and limits

Administrators and HR Generalists use their company scope. Managers use only
their own and current direct reports' records. Ordinary Employees remain
portal-only. The worker reloads RLS-protected data and checks membership scope
again before writing any file. Changed/revoked access blocks export and clears
the desktop session. A report filtered to an unavailable employee is rejected.

CSV uses UTF-8 with BOM, CRLF rows and quoted/escaped fields. Potential formula
prefixes in text receive an apostrophe before spreadsheet interpretation.
Export requires a `.csv` filename and preserves existing files; choose another
filename when exporting again. An incomplete new file is removed on write
failure. The app does not save reports, passwords or sessions automatically.

The prototype limits each report to 366 dates and 50,000 output rows;
attendance/summary also cap processing at 50,000 employee-days. Narrow the
period or employee/department filter for larger companies. Report inputs are
indexed in memory and reuse existing attendance/leave calculation rules.
No database migration or new permission grant is required for this module.

Create-only employee CSV imports with previews/error reports are available to
HR/Admin; see [employee imports](EMPLOYEE-IMPORTS.md). Custom report definitions,
XLSX formatting and broader workforce analysis remain subsequent work. A CSV
export is not an import template or payroll instruction.
