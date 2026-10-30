# Workforce analysis

Reports includes **Workforce headcount** and **Attendance & leave trends**.
Both use the existing fresh-data/report export worker. Administrators and HR
use company scope; Managers see only their own/current direct-report records.
Ordinary Employee accounts cannot sign into the desktop. Saved views and custom
columns do not grant additional record access.

## Current workforce snapshot

Group current employee records by department, work location or employment
type. Charts compare active records and show archived counts separately.
Unassigned values appear as Not set; inactive option labels remain visible.
Employee/department and archived filters apply before counting. The chart
shows the eight largest active groups; the table and CSV include all groups.

This is a snapshot at generation time, using the company-local date. It does
not reconstruct historical headcount, hires, turnover or past departments.
Archived records have no inferred termination date. CSV From/To metadata uses
the actual snapshot date, even when a saved view has older filter dates.
Future-dated joining fields do not alter the recorded Active/Archived status.

## Recorded attendance and leave trends

Select a period and Daily, Weekly (Monday) or Monthly buckets. The first and
last buckets include only selected dates. Attendance uses the recorded work
date, including overnight records. One employee with an attendance record on
one work date contributes one attendance employee-day. Completed and open
records remain distinct. Zero recorded days are not classified as absences.

Leave uses saved charge dates and exact half-day units. Approved and pending
charges are separate, including future planned dates when selected. Rejected
and cancelled charges do not contribute. Current decision states apply; this
is not a reconstruction of what was approved on a past reporting date. Private
leave reasons and contact fields are excluded.

Minute totals reuse each attendance record's saved calculation rules and
current protected overtime authorization basis. Records needing calculation
review are counted; unavailable minutes contribute no known minutes. Late
records count those with at least one calculated whole late minute. Overtime
candidates, current approvals and payable time remain separate. Changed facts
withhold current approval minutes as in the detailed attendance report.

Charts compare recorded employee-days with approved/pending leave days on a
shared count scale. These series can overlap. Hover a bucket for its dates,
record counts and saved leave totals. They are not attendance rates, payroll
totals or inferred absence rates. Tables and CSV retain the detailed values.

## Columns, saved views and limits

Expand Columns to choose fields and use Up/Down to order them. All columns
restores the default; Clear selection blocks generation until a field is
chosen. Preview and CSV use the same selected order. CSV always retains its
company/report/period/generation metadata. Charts use the full scoped report.
Save a view to retain the grouping, bucket size, filters and column order.
See [reports](REPORTS.md) for persistence and permission checks.

Periods are limited to 366 dates and trends to 50,000 employee-days. Current
headcount accepts at most 50,000 employee records. Narrow filters for larger
companies. Duplicate employee/work-date facts block reliable trend generation.
No new dependency or employee-data permission grant was introduced.
