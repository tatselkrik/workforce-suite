# Attendance entries and corrections

Daily attendance follows the company's time zone. Choose a work date from the calendar to review that day's records, missing check-ins and approved leave. A missing check-in does not establish absence without schedules and holiday rules.

## Routine attendance

**Check in** and **Check out now** use the current database server time. Check-in creates today's record. An overnight shift is checked out from its original work date. Administrators and HR can record these manual exceptions for employees; Managers can record their own routine attendance. Ordinary employees will use the separate planned portal.

## Record a missing entry

An Administrator or HR Generalist selects the date and employee, then **Record past attendance** or the row's **Record** button. For a verified time on today's date, use **Record with reason**. Enter check-in in 24-hour time, optional check-out and its date, and a reason of 1–500 characters. Use a later check-out date for an overnight shift. Save correction applies the entry immediately and creates its first history entry. This is an HR correction flow; employee requests and approval chains are not implemented.

An empty check-out leaves the shift open and prevents another check-in until it is closed. Use **Correct** to enter a missed historical check-out. **Check out now** records the current time; it does not backdate a missed departure.

## Correct an existing record

Choose **Correct** beside the employee's attendance. Review the currently recorded times, edit check-in or check-out, and give a reason. The employee and work date are fixed so a correction cannot silently move somebody else's record. To review a changed record after a conflict, cancel the form, refresh and reopen it. An unsaved correction prevents an update/restart.

**History** shows who saved each correction, their role, server save time, reason and the previous/new times. The first missing entry shows “No attendance record” before it. History is read-only and visible only to active Administrators and HR Generalists. Routine server-timed check-outs update the current record; they are not reasoned HR corrections. A later correction records the then-current check-out in its before/after values.

Future times, dates before hire, duplicate daily entries, multiple open shifts, overlapping shifts and check-out before check-in are rejected. Times use the company time zone; ambiguous or nonexistent daylight-saving times require another unambiguous local time. Fractional seconds in an unchanged existing time are preserved.

## Retained employee history

Archiving preserves the employee and all attendance/leave history. It blocks new routine attendance and leave. HR may correct archived historical attendance before today; this does not reactivate employment or sign-in. Disabling sign-in only suspends access and does not change employment status. The current offboarding sequence is to disable access, reassign any direct reports, then archive. No employee deletion is offered.

## Deployment

Apply every checked-in migration, including `20261021111354_attendance_corrections.sql` and `20261021113409_attendance_conflict_response.sql`, before deploying version 0.1.9. The additive migration keeps legacy server-timed client grants. Custom timestamps are written only by a guarded HR RPC; direct custom-time/revision/history writes are denied. A private, transaction-scoped context authorizes that one write, and the change and audit commit together. Revision checks prevent overwriting another correction or a live check-out. PostgreSQL's exclusion constraint prevents overlapping shifts even with concurrent writers. No desktop server key is required.

The public RPC translates stale revisions to HTTP 409 (`PT409`). Do not expose an explicit retryable `40001` through PostgREST: it can retry indefinitely in PostgREST 14. See [Supabase troubleshooting](https://supabase.com/docs/guides/troubleshooting/high-cpu-and-infinite-transaction-retries-when-using-custom-error-codes-in-rpc-functions-77326b).
