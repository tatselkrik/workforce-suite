# Leave policies and approval chains

Open **Leave > Leave policies**. Administrators choose **Edit policies and
approval chains**; Settings also links to this page. HR and Managers can read
the company's policy summary. Ordinary Employee accounts remain reserved for
Workforce Employee and cannot sign in to the desktop.

## Calendar and leave types

- Select any company workdays from Monday through Sunday. Add dated holidays
  with a name; holidays and non-workdays do not consume leave.
- Keep annual, sick and unpaid leave, and add company-specific types. Rename
  or deactivate a type; existing codes and filed requests are retained.
- Mark a type paid or unpaid. This is a policy label, not payroll computation.
- Choose an annual grant, monthly accrual or no balance limit. Annual leave
  can use each employee's annual allowance; other types use their configured
  yearly allowance. Amounts use exact half-day units rather than floating-point
  deductions.
- A full-day request may cover several dates within one calendar year. Morning
  and afternoon requests use one date. Separate morning and afternoon requests
  can coexist, while a duplicate half or an overlapping full day is rejected.
  Split a request crossing New Year into two requests.

Each request saves its charge dates, half-day amounts, type label, policy
revision and approval plan. Later calendar/type/chain edits do not recalculate
those saved charges or plans. Policy versions retain the configuration and
server-recorded actor/time. Existing requests begin with their original
Monday–Friday charges and an explicit starting snapshot; earlier actors and
approval times are not reconstructed.

## Accrual, carryover and balances

Monthly accrual posts for full calendar months employed, at month-end. The
yearly allowance is multiplied by eligible completed months / 12 and rounded
down to a half-day. A partial joining month does not earn a full month's credit.
HR must set **Date joined** before an accrued-leave request can be submitted.
An unknown hire date does not generate accrual or prior-year carryover.

Carryover is capped per type and expires at the end of the chosen month.
Eligible leave consumes carryover first. Only unused prior-year base entitlement
can carry forward; older carryover does not carry a second time. No entitlement
is inferred before the deployment's tracking start year. For this prototype,
that year starts with the company record or earlier retained leave records.

Balances use the current type policy and employee allowance. Editing grants
can therefore change current availability; already filed charge dates stay
unchanged. Entitlements retain the established grant/accrual calculation. Dated
base-credit adjustments now have a separate immutable ledger under Leave >
Credit adjustments; see [audited credits](LEAVE-CREDITS.md). Imported opening
balances and policy effective dates use the [dated policy and opening-balance workflow](LEAVE-OPENINGS.md). Leave cash conversion is separate from balance administration. Never enter a total remaining balance as a credit delta.

The request preview shows the balance projected through the requested start
date, including month-end credits earned by then. Pending requests reserve their
full charge. Intermediate approvals keep that reservation; the final approval
moves it to used leave. Rejected or cancelled requests release the reservation
or deduction. Submission and approval both recheck availability on the server.
The Employees directory shows annual availability through today.

## Ordered approvals

Configure one to five steps for each leave type, and use **Move up** to reorder:

- **HR or direct manager** keeps the original single-review behavior.
- **Direct manager** uses the requester's saved Reports to employee.
- **HR / Administrator** allows either assigned HR or Administrator staff.
- **Administrator** requires that role.
- **Specific staff member** assigns an active desktop staff employee.

Every step needs an eligible active staff account when a request is submitted.
Self-approval is blocked at every step. Manager and specific-person assignments
are captured on the request; changing the company's chain affects new requests.
Only the current eligible reviewer can approve or reject. The request remains
pending until every step approves. The same staff member can act at multiple
eligible steps; separation-of-duties requirements are not an additional rule.

Open a request to see its plan, review note, status and retained history.
Approvers outside the reporting team receive the request identity and history,
without access to the employee's personnel record or private contacts. They
retain a receipt after their decision. Disabling their staff account removes
that access. Missing/disabled assigned reviewers block their step: HR can cancel
the request and submit a replacement under a corrected chain. See [approval delegation](APPROVAL-DELEGATION.md) for temporary substitutes and [notifications](NOTIFICATIONS.md) for current reminders. There is no automatic approval escalation.

## Cancellation and corrections

The requester may cancel pending leave or approved leave that has not begun.
HR/Administrators may also cancel earlier approved leave to correct a record.
Enter a reason. Requests, approval decisions and events remain; cancelling
twice cannot restore days twice. Filed dates and charges are immutable; cancel
and submit a corrected request instead of editing history.

Attendance uses the current calendar for non-workday labels and saved approved
charge dates for full/half-day leave labels. Missing check-ins still do not
establish absence. Individual schedules are configured separately under Attendance; they do not recalculate leave charges. Break tracking and time calculations are available under Attendance; see [attendance calculations](ATTENDANCE-CALCULATIONS.md).

## Deployment and permissions

The checked-in leave migrations are `20261021174408`, `20261021205511` and
`20261021210727`.
Apply them before installing this desktop version. Direct client INSERT/status
PATCH on `leave_requests` is retired. New clients use `submit_leave`,
`review_leave`, `cancel_leave` and `save_leave_policy`; public wrappers remain
security invokers. Private handlers recheck protected membership and serialize
mutations with company settings/account/reporting changes. Reviews,
cancellations and configuration use optimistic revisions, with `PT409` for
stale saves. Clients cannot forge charges, approval plans, actors or audit rows.

New clients send `X-HRIS-Leave-Contract: 2`. A read filter keeps unknown custom
and cancelled enum values out of older clients, so they can still sign in and
check the signed update feed. This header grants no access: membership and
request/reviewer scope are enforced independently. New clients see their full
authorized history.

All new tables have RLS and explicit grants. Policy edits are Administrator-only;
requests retain employee/HR/team scope plus narrowly assigned reviewer access.
Authentication metadata grants no permission. Audit versions/events and filed
requests cannot be deleted by clients. No service key or authentication session
is packaged or persisted.

The Windows installer preserves company configuration. Reverting desktop files
does not revert these database migrations; an older desktop receives a
compatibility-filtered leave view and cannot use the retired write contract. Keep database recovery
separate from installer recovery.

Run the Rust suite and `node tools/db-tests/schema.test.mjs` locally. The database
suite first verifies legacy behavior, then upgrades and verifies migration
preservation and the new workflow. `hosted-leave-smoke.sql` exercises real hosted
PostgreSQL/RLS with transaction-local simulated subjects and rolls all fixtures
back; it does not test Auth tokens/email. A separate authenticated Rust adapter
probe verifies real persistence, saved charges, approvals, cancellation and
fresh sign-in using labeled retained fixtures.
