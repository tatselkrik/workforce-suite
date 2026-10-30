# Attendance breaks

Administrators and HR Generalists set **Break rules** when adding or editing a
shift under **Attendance > Shift schedules**. Each shift supports up to eight
rules with unique names, a duration of 1–720 whole minutes, paid/unpaid
classification and an optional required flag. Combined durations must be shorter
than the shift. These are company policy choices; this prototype does not decide
statutory break entitlements or calculate payroll.

## Saved policy and actual time

Assign the revised shift to apply its break rules from the selected effective
date. Assignments copy the rules, and a new attendance record keeps that copy.
Editing a template or changing an assignment does not change rules already saved
with attendance. Existing attendance without rules remains without invented
historical rules. An older client omitting `break_rules` when editing a template
preserves its existing rules.

Open **Daily attendance > Breaks** under the employee's name. The view shows
saved rules, completed paid/unpaid break totals, recorded times and, for completed
attendance, elapsed time excluding actual unpaid breaks. Paid breaks remain in
that time. Missing or short required breaks are reminders: the app never deducts
an untaken break automatically. Open breaks prevent a completed net total and
checkout. The daily roster's **Elapsed** column remains gross elapsed time.
Neither figure is approved payable time.

## Recording and corrections

Desktop staff can use **Start break** and **End break now** for their own open
attendance. These operations use server time and the saved rule's classification.
They cannot operate another employee's routine break controls. Ordinary Employee
accounts still cannot sign into the desktop; Workforce Employee uses the
same protected service contract.

HR/Admin can **Record break with reason** for an open or completed attendance
record. Choose a saved rule, or explicitly name and classify an unscheduled
break. Enter start/end dates and 24-hour times in the company time zone. An end
may be empty only while attendance is open. Times must be current or past,
inside the recorded attendance period, with no overlapping non-voided breaks.
Each saved rule can have one non-voided recording per attendance; additional
unscheduled breaks may be recorded separately.

Use **Correct times** with a reason to fix a break. Its original name, rule,
expected duration and paid/unpaid classification stay fixed. If the selected
rule or classification was wrong, **Void with reason**, then record a replacement.
Voiding excludes time from totals and retains the original row. **Show voided
breaks** and **Break history** expose retained entries, actor, server timestamp,
reason and before/after times to HR/Admin. Attendance edits cannot leave break
times outside attendance; correct or void the affected breaks first.

Managers can read their own and current direct reports' break summaries, but
cannot read HR correction reasons or manually change team breaks. Disabled or
unassigned accounts lose read/write access. Conflicting parent or break revisions
require refreshing and reopening the record; a failed save retains the draft.

## Database and boundaries

Migration `20261022035331_attendance_breaks.sql` adds RLS-protected break and event
tables with explicit read-only client grants, saved rule validation, overlap/open
constraints, a guarded private handler and a public security-invoker RPC.
Protected membership, ownership, locks, revisions, timestamps and history are
server enforced. Clients cannot directly insert, edit or delete break/history
rows. A parent-first lock order and attendance-bound trigger serialize break and
attendance corrections. File recovery does not undo this schema or its checkout
guard; an older client may need HR to resolve an open break.

Lateness, undertime, overtime, payroll, employee correction requests and automatic
reminders remain separate modules. Biometric integration remains visible and
disabled until actual hardware support is implemented and verified.
