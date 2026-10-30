# Shift schedules

Open **Attendance > Shift schedules** to create a named shift. Administrators
and HR Generalists manage shifts and assignments. Managers can read schedules
for themselves and their current direct reports; they cannot assign or edit them.
Ordinary Employee accounts use Workforce Employee for Android.

Choose working days, start/end times in 24-hour format, and whether company
holidays are scheduled workdays. An end before the start finishes the next day;
equal times and shifts of 24 hours or longer are rejected. Paid/unpaid break
rules and actual break recording are available; see [attendance breaks](ATTENDANCE-BREAKS.md).
Lateness, undertime/early departure and overtime candidates are available in
**Time calculations**; see [attendance calculations](ATTENDANCE-CALCULATIONS.md).
Elapsed attendance still includes breaks and is not payable time.

## Assignments and history

Open **Assignments**, choose a date using the calendar, find an employee and
choose **Assign / change**. Select an active shift, effective date and reason.
The assignment applies until the next effective date. A later save for the same
date takes precedence; all earlier events remain in **Schedule history** with
the server actor, time and reason. A past effective date explicitly corrects the
schedule shown for that period. Assignments cannot precede a known hire date;
archived employees cannot receive new assignments. Their history remains.

An assignment copies the shift's name, working days, hours, holiday rule, break rules and
company time zone. Template edits and deactivation do not rewrite that copy.
To apply revised hours, assign the revised template from the required date.
Choosing **Company calendar — no shift hours** ends individual scheduling from
that date. Earlier dates keep their earlier assignment. The company workweek
is the fallback, without assumed start/end times.

The holiday calendar is the current company calendar; assignments retain their
holiday-handling rule, not a frozen list of holiday dates. Individual schedules
do not recalculate leave: new leave requests still use configured leave policies
and filed requests retain their saved charge dates and approval chain.

Changes to employee assignments, selected template versions or the company time
zone during editing cause a stale-save conflict. Refresh and reopen before
saving. Overlapping shift transitions are rejected, including overnight shifts
that would run into the following assignment. A different time zone is captured
only when a new assignment is saved. A daylight-saving gap at an assignment
boundary requires another date/time; ambiguous fall-back boundaries use the
later occurrence for overlap checks. There is no rotating multi-week pattern,
split shift, shift swap or schedule publishing workflow in this first module.

## Attendance and overnight shifts

Daily attendance shows planned hours alongside the employee. An in-progress
scheduled night shift checked into after midnight belongs to the starting date.
The end boundary is exclusive; a check-in outside a scheduled window uses its
current company-calendar date. Employees can still record attendance on an
unscheduled day. A missing check-in alone does not establish absence.

New attendance records capture their plan on the server. Later assignment edits
do not change that snapshot. Existing records start with no recorded plan;
earlier schedules are not reconstructed. Open overnight records are checked out
from their original work date. HR corrections include a check-in date so a
missed after-midnight check-in can retain the night shift's starting work date.
Routine timestamps, protected identity/revisions, overlap prevention, correction
reasons and before/after audit remain server enforced.

The migration is `20261022030213_shift_schedules.sql`. Public save RPCs are
security invokers; private handlers enforce active protected HR membership,
company locks and optimistic revisions. Tables use RLS and read-only client
grants; clients cannot forge snapshots, actors or history. Old clients can still
sign in, check updates and read their supported modules. File rollback does not
remove the new database schema or undo schedule-aware overnight attribution.

Biometric integration remains visible and disabled until a device is selected
and integration is implemented and verified against that hardware.
