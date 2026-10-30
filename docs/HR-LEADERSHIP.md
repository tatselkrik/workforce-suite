# HR leadership reporting

Open **Reports → Leadership** as an Administrator. This separate protected report provides aggregate oversight; existing HR and Manager operational reports retain their existing scopes. Account roles and optional-module visibility do not grant leadership access. Payroll administration remains in Payroll Desktop.

## Facts and time bases

Choose at most 366 dates, ending no later than today in the company time zone. Current active/archived employee counts and department/location/employment-type groups describe the generation-time records, including future joining dates. Unknown joining dates and missing active-record classifications remain explicit. Inactive option labels remain readable; an unassigned group is distinct from a recorded label named “Not set.”

Monthly rows use clipped selected intervals. Joining counts use the currently recorded joining date; finalized exit-case counts use the recorded last-working date and current finalized state. Archiving is not an inferred separation. These observations do not reconstruct past headcount, departments, time to hire, turnover, forecasts or past approval states.

Attendance counts recorded employee/work-date rows, keeping completed and open punches distinct. Missing records are never absences. Leave uses immutable saved charge dates and half-day units for currently approved or pending requests; rejected/cancelled records do not contribute. No entitlement or attendance calculation is rerun.

Hiring and Development cards count current saved workflow states across retained records, including archived employment. Overdue means a recorded due date before company today; unknown due dates are not overdue. Only HR-recorded training completion counts as completion. Certificate counts exclude archived/revoked and future-issued records. A due date exactly today is included in the next-30-days window, not expired. These are workload counts, not appraisal scores, competency claims or provider acceptance.

## Privacy and freshness

The server reads one stable statement snapshot after checking current active Administrator membership and active employment. The dedicated typed projection contains counts and current group labels only, apart from its company/actor binding. No employee/applicant names, codes or IDs, contacts, private notes, confidential cases, service requests, benefit/dependent records, salary, tax or payment facts enter the report. It stays outside the shared Snapshot, Payroll and Workforce Employee.

Each generation/export obtains fresh facts; the client rechecks protected access before and after the request and immediately before writing. Revocation, failed requests, changed filters and other data refreshes clear the cached preview. Exports are new local CSV or native XLSX files; existing files are preserved. Exported rows include dates, generation time, time zone, scope and metric definitions, with no actor/company IDs. CSV formula prefixes are neutralized and Excel group labels stay literal text.

## Bounds

Each source is limited to 50,000 selected records, saved leave charges to 50,000 dates and groups to 600. Invalid saved charge shapes, duplicate dates, dates outside the filed request or inconsistent charged totals fail explicitly. Exceeding limits rejects the report instead of silently truncating totals. No financial rules, canonical records, workflow decisions or immutable history are changed.
