# Overtime approvals

Open **Attendance > Overtime approvals**. Staff can choose a completed work
date, see the calculated overtime candidate and request between one minute
and that candidate, with a reason. The database calculates the cap from the
record's saved shift/rules and current completed breaks. A candidate is time
available for review; submission does not change attendance or calculate pay.

## Review and history

**Review queue** shows pending requests within the current role's scope.
Managers review current active direct reports; HR Generalists and
Administrators review company requests. Nobody reviews their own request.
Approve between one minute and the requested amount, or reject with a reason.
The requester can cancel a pending request with a reason. Resolved proposals,
their submitted basis, actor, role, time, reasons and earlier approved amounts
remain in **Approval history** and **Submission & approval history**.

Only one pending or approved authorization is allowed for each attendance
record. An authorized reviewer can **Revoke approval with reason**; this
removes the current authorization, retains its earlier amount/history and
allows a replacement request. A resolved request cannot be edited or deleted
through the application. This is single-step overtime review, independent of
ordered leave approvals and attendance correction requests.

## Changes after submission

Submission saves the actual attendance, full non-voided break records and
calculated candidate. Approval checks those facts again on the server. A
later punch correction, break change or approved leave on that work date
requires a fresh proposal; a failed review leaves the pending request and its
history unchanged. Reject or cancel it and resubmit after refreshing.

If actual facts change after approval, the saved decision remains historical
and **Current approved overtime** becomes zero. A reviewer must revoke the
old authorization before a replacement can be submitted. Editing company
rules affects future records; explicitly applying current rules to an older
attendance record changes its revision and requires overtime review again.

Open attendance/breaks, absent saved schedules/rules, off days, approved
leave or schedule times needing review do not produce an eligible candidate.
Actual completed unpaid breaks outside the shift reduce the candidate.
The overtime minimum applies before rounding down, using the record's saved
rules. Required break rules do not invent actual break deductions.

## Access and deployment

Protected memberships, row-level security, server-owned identity/actor data,
company/attendance locking and optimistic revisions enforce these rules.
Client roles can read only their permitted records and cannot write request
or event tables directly. Ordinary Employee accounts remain portal-only;
their protected operations are prepared for the separate future portal.

Migration `20261022082855_overtime_approvals.sql` adds the request/history
tables and guarded RPC. It does not backfill approved time or alter existing
attendance, leave, documents, saved schedules or calculation policies.
Payroll, statutory overtime rates and device integration remain separate
modules; the biometric integration control stays disabled.
