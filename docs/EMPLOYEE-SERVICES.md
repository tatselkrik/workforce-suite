# Employee services — current implementation contract

Employee services tracks requests, routing and outcomes. It is separate from
confidential employee-relations cases. Use the existing protected roles; do not
introduce another role or give a specialist broad HR permission to load a ticket.

HRIS offers a service workspace to desktop staff. HR Generalists/Administrators
coordinate the company queue and route a request to an active desktop staff member.
Other staff see their own requests and specific requests assigned to them; a line
management relationship grants no additional service access. Ordinary Employees
use the separate LOCAL portal. The portal always shows only the current person's
requests, even when the account is an Administrator or assigned handling officer.
Use dedicated own-only server wrappers and browser field allowlists, not a browser
switch between own and staff scope. Keep upstream tokens/server state in memory,
opaque HttpOnly cookies and existing origin/CSRF/current-identity checks.

Categories start with employment documents, employee-information updates, benefits
questions and general HR support. A request has a title, shared description,
priority, current assigned handler, optional due date, status and retained shared
message/history. All messages in this first increment are visible to the requester
and authorized handling staff; there is no private internal-note field. No case
content, salary source, private201file/storage key or tax identifier is imported.
Document requests do not generate or certify official documents in this increment.

Native HR may submit for another current/former company employee, recording the
actual submitter rather than impersonating the requester. A self-service submission
always infers its own employee on the server and cannot select an assignee or due
date. Requested profile/contact/employment changes never edit canonical fields
automatically; authorized staff review and use the existing record workflow.

Statuses are Submitted, In progress, Waiting on employee, Completed and Cancelled.
Handling staff can progress active work, with an explicit shared outcome. Requesters
can reply/cancel their active own requests; HR can cancel on behalf. Only HR can
reopen terminal work. HR routing/unassignment needs a reason and a fresh request
revision. Reassignment or loss of the handling role removes that assignment's
access, while the person's own-request access remains. Preserve former-employee
history and never gate owed pay or canonical employment on a service request.

Use additive private tables with no direct client privileges, guarded invoker
wrappers, current company/employee/role/assignment checks, immutable events,
optimistic revisions and actor/full-payload retry receipts. Old receipts still need
current scope; failed auditing rolls back changes. Bound rows/bytes and fail
explicitly rather than silently truncate. Keep request data outside shared Snapshot,
Payroll and general notifications/reports; a later explicit scoped inbox can be
considered within the queue. No external messages/providers, purchases or paid
services are part of this workflow.

Current work is a local foundation, not a completed release. Native and own-portal
adapters/UI, additive SQL, current-role/assignment/direct-API tests, safe local portal
delivery, exact private HRIS release and actual installation must pass before the
roadmap entry is crossed out. Preserve the currently running portal and its drafts;
never restart it casually or expose it to LAN/public/tunnel access.

The requester browser projection contains only ticket identity/code/title/category,
priority/status/due date, a captured handling staff display name, source revision,
timestamps, reply/cancel controls and the shared message thread. It excludes company,
employee, Auth/actor/assignee identifiers, routing directories, full before/after
states, private receipts and staff controls. The own projection rejects a staff
context even for an Administrator. Handler names are canonical snapshots captured
at routing, paired with the private assignee identity and cleared on unassignment;
a former name is history, not proof of a current assignment or permission.

The native adapter contract uses `employee_service_context` (no arguments) and
`save_employee_service(p_change)` for Staff scope only. It checks fresh protected
access before and after requests, rejects a portal-audience response, binds the
exact actor/payload/receipt, and distinguishes a confirmed transaction rejection
from uncertain delivery or an invalid receipt. Payroll and ordinary Employee
accounts cannot reach these context reads through the native adapter. These local
HTTP fixtures do not prove that the pending database endpoints exist or authorize
a hosted deployment. Own portal wrappers must infer identity independently and
must never accept an arbitrary staff/own audience parameter from the browser.

The additive migration is `20261029022501_employee_services.sql`: it was created
with the linked CLI and its generated UTC timestamp was advanced to one second
after the existing migration89 predecessor to retain chronological replay order.
No previously deployed migration was renamed or rewritten. Private tables deny
direct client access; public invoker wrappers use fresh protected private guards.
Exact retry receipts recheck current ownership/handling/routing responsibility;
removing HR responsibility cannot preserve a former routing action via an old ID.
The separate own wrappers infer identity and return only shared thread fields.

Current local coverage includes a fresh90migration replay with133previous record
sets and445function definitions/grants preserved,134service assertions,14role
configurations,14native model cases,8native HTTP cases,5portal HTTP/model cases
and8browser logic cases. Native visual/complete-regression/hosted/release delivery
remain gates; this paragraph is evidence of a local increment, not completion.

Reference review29October2026: current Supabase function privileges/search-path
and the September PostgreSQL minor-release/April DataAPI grant changes were read.
This migration uses ordinary UUID/date/text/JSONB/B-tree constructs, explicit
function grants and private tables. It changes no extension, Auth setting or
financial rule. See https://supabase.com/docs/guides/database/functions and
https://supabase.com/changelog/postgres-15-19-17-11-breaking-changes .
