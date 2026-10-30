# Attendance correction requests

Open **Attendance > Correction requests**. Staff can submit their own missing
or incorrect check-in/check-out for a selected work date. Ordinary Employee
accounts use Workforce Employee for Android; its protected request
scope is ready, but its mobile UI is not built here.

## Submit and follow your request

1. In **My requests**, choose the work date using the calendar.
2. Choose **Request correction**. Existing recorded times are shown when present.
3. Enter check-in/check-out dates and 24-hour times, plus a reason. Leave checkout
   empty only for an open shift. Overnight check-ins follow the saved schedule.
4. Submit, then open the request for its saved proposal and decision history.

Submission does not change attendance. Only one pending request is permitted
per employee/work date. A pending proposal is immutable: cancel it with a reason
and submit a fresh request to change the details. Rejected and cancelled requests
remain in history. Users cannot remove their submission or decision events.

## Review and application

**Review queue** shows pending requests the current account may review. This is
one review step: a Manager can approve/reject current active direct reports;
Administrators and HR Generalists can review company requests. Nobody reviews
their own request, including HR/Admin. An HR/Admin can handle a request when
there is no available direct Manager. Reporting and membership permissions are
checked again at the time of the decision. This flow is separate from configurable
leave approval chains; custom attendance chains/delegation are not implemented.

Open a request, compare **Recorded when submitted** with **Requested times**,
and choose **Approve with note** or **Reject with reason**. A decision reason
is required. Approval applies the exact submitted timestamps and records the
decision and linked correction audit in the same transaction. A failed validation
does not save a decision or partially change attendance. Rejection leaves the
actual attendance untouched. Approved requests cannot be approved twice or
reversed through this screen; an erroneous recorded time needs a new request
or a reasoned direct HR correction.

The actual record, revision and company time zone must still match the submitted
baseline. For missing attendance, the schedule must also be unchanged. If another
check-in, checkout, HR correction or explicit rule application changed the record,
reject/cancel the stale proposal and resubmit from the current state. An existing
record retains its saved shift and calculation rules after approval. New missing
attendance captures the current server-owned rules when approved.

Approval rechecks current breaks, hire dates, archived status, future timestamps,
daily uniqueness, open shifts and overlap constraints. Break times must stay
within attendance, even if they were recorded after submission. Resolve invalid
breaks through HR before retrying; this request changes punch times, not breaks.

## Retention and deployment

**Submission & approval history** retains requester/reviewer names and roles,
server timestamps, reasons and approved before/after attendance. **Request history**
lists resolved requests within the current permitted scope. Requesters read their
own history; Managers read current active direct reports; HR/Admin retain the
company history. Disabled/unassigned accounts and anonymous clients have no access.
Direct HR corrections remain available separately and do not require a request.
General HR correction reasons stay HR-only; request history is a separate,
scoped record shared with the requester and their authorized reviewers.

Migration `20261022050704_attendance_correction_requests.sql` adds request/event
tables with RLS/read-only grants and a guarded RPC. A private transaction proof
allows a Manager to apply only the exact approved proposal through the existing
correction safeguards; it grants no general manual correction access. A protected
trigger links one immutable correction event to its request. Migration
`20261022081216_attendance_request_context_index.sql` covers the private proof's
foreign key. Existing attendance/corrections are preserved; earlier requests
are not invented. File recovery does not remove these schema changes.

This flow does not approve overtime or calculate pay. Those remain separate
modules. Biometric integration remains visibly disabled.
