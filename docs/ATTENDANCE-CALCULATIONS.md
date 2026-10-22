# Attendance time calculations

Open **Attendance > Time calculations**. Choose a work date with the calendar,
search by employee name/code and open a record to see late arrival, undertime
(early departure), overtime candidates and time excluding completed unpaid
breaks. Managers see their own and current direct reports' records; HR and
Administrators see company records. Missing check-ins do not establish absence.

## Company rules

An Administrator uses **Company calculation rules > Edit rules**, also linked
from **Settings > Attendance calculation rules**. Set:

- Late-arrival and early-departure grace, each 0–120 minutes.
- Minimum outside-shift time for an overtime candidate, 0–240 minutes.
- Round overtime down to 1, 5, 10, 15, 30 or 60 minutes.
- Whether pre-shift work joins post-shift work as an overtime candidate.

Saving requires a reason and retains the previous values, actor and timestamp
in HR-only history. Defaults are zero grace, zero overtime minimum, rounding to
one minute and post-shift work only. These are prototype settings; configure
the company's actual policy before interpreting results.

Grace is subtracted before flooring to whole minutes. For example, an 08:00
start, 08:17 check-in and five-minute grace produces 12 minutes of late arrival.
Undertime measures departure before the planned end, after grace; it does not
infer a net-hours deficit from long breaks. Break reminders remain in **Breaks**.

Overtime candidates use actual recorded attendance outside the saved shift.
Only completed unpaid breaks overlapping those intervals are excluded. Paid
breaks stay counted. The minimum is applied before rounding down. For example,
107 post-shift minutes with a 15-minute unpaid break gives 92 minutes; a
30-minute minimum and 15-minute rounding produces a 90-minute candidate.
Untaken required breaks never create automatic deductions. Candidates require
review and are not approved/payable overtime or payroll amounts.

## Saved rules and review states

New attendance captures its calculation rule version, scheduled-workday
decision, company time zone and planned UTC boundaries on the server. Displayed
planned times use that saved time zone. Company policy, calendar, time-zone or
shift-assignment changes do not rewrite a recorded snapshot. Attendance-time
corrections retain it too. Existing attendance is not automatically backfilled.

HR/Admin can open a saved record and **Apply current rules with reason** when
saved shift hours exist. This explicitly replaces only that record's
calculation snapshot using the current rules/calendar and its saved shift.
Attendance times, saved shift and breaks stay intact. Before/after snapshots,
reason, actor and timestamp remain in **Calculation rule history**. Stale policy
or attendance revisions require refreshing before retrying.

Missing rules or shift hours leave time flags unavailable. Off-day work,
approved leave on a charged date and daylight-saving boundary gaps require
HR review. Approved half-day leave does not imply an hourly schedule. Leave
review uses the current protected approved-leave records and saved charge dates;
cancelling leave changes this review state. Ambiguous daylight-saving boundaries
use their later occurrence; nonexistent local boundaries are flagged instead
of blocking factual attendance. Open attendance can show late arrival but no
completed early-departure, overtime or net total. An open break also prevents a
completed net total.

## Permissions and deployment

Migration `20261022041651_attendance_calculations.sql` adds company policy and
two history tables with RLS and explicit read-only client grants. Public RPCs
are security invokers; private handlers enforce protected roles, company scope,
reasons and expected policy/attendance revisions. Server-owned capture and
history cannot be directly edited by clients. Only Administrators change
company rules; HR/Admin apply them to records and read audit reasons. Managers
can read the company rule summary and scoped attendance but not HR history.
Disabled/unassigned/anonymous clients have no access. Ordinary Employees remain
portal-only; their future own-record scope is protected in PostgreSQL.

File recovery does not remove the new database schema. Old clients can still
read their supported modules and new records receive server-owned snapshots.
Employee correction requests, overtime approvals and payroll remain subsequent
modules. The employee portal stays separate; biometric integration stays disabled.
