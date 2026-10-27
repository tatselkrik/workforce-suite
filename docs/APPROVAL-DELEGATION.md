# Temporary operational approval delegation

Open **Leave > Approval delegation**. Administrators select an original approver,
a substitute, a workflow and inclusive start/end dates, then save with a reason.
Configure leave, attendance corrections and overtime separately. HR/Admin see
configuration history; Managers see only assignments involving themselves.
Reasons/references and configuration actors remain HR/Admin-private.

Delivered in0.1.48, installed and privately published. The migration, native
application, hosted Auth/workflows and exact installer/recovery checks pass.
See [verification](APPROVAL-DELEGATION-0.1.48-VERIFICATION.md) and the
[review checklist](APPROVAL-DELEGATION-0.1.48-CHECKLIST.md).

Delegation adds a substitute; the original approver retains original authority.
It does not transfer an account role, reporting line, personnel-file access,
account-management permission, salary approval or other financial authority.
Payroll approval and independent-approval exclusions are unchanged.

## Authority and dates

- Only active desktop staff with active employment can be selected. A Manager
  can use a Manager, HR or Administrator substitute. HR needs HR/Admin, and an
  Administrator needs another Administrator. Ordinary Employees cannot delegate
  or review through this desktop module.
- Dates follow the company's time zone, include both endpoints, cannot be
  backdated and cover at most366 days ending within the next year. Overlapping
  assignments for the same original approver/workflow require revocation first.
- Both staff accounts, employee bindings and recorded roles are rechecked at
  every decision. A role/binding/status change suspends the grant; restored
  matching eligibility can reactivate it within its dates unless revoked.
- The original approver must still have original authority for that exact
  request/step. A substitute cannot forward delegated authority. Reviewing one's
  own employee request remains blocked in all three workflows. Correction and
  overtime review also retains its existing submitting-account exclusion; the
  existing HR-assisted leave workflow is unchanged.
- Saved manager/person assignments stay in filed leave plans. Reviewer steps
  that depend on current reporting still use current reporting. Delegation
  neither changes the plan nor skips an approval step.
- **Revoke…** stops further delegated decisions immediately with a retained
  reason. An expired/revoked grant does not undo decisions already made.

## Requests and history

Eligible requests appear in their normal Leave, Attendance correction-request or
Overtime review lists. The detail identifies the original approver and displays
retained delegated-decision receipts. The actual substitute remains the actor
on the original decision/event; the saved original approval assignment remains.

Foreign-team substitutes receive only the relevant request/basis, source-match
flag and retained task history. Employee directories, contacts, raw attendance,
documents, salaries and government identifiers keep their existing scopes. A
protected current-source flag supports local previews without broadening those
tables. The server rechecks actual attendance/break/leave facts when deciding;
stale proposals remain unapprovable. Existing issued-payroll/source guards remain.

Configuration and receipts are immutable records. Revoke and replace rather than
editing/deleting history. Saves retain an actor-bound request identity across
network failures. Refresh to discover the outcome; retry the same retained draft
without creating a duplicate. Concurrent configuration changes require reviewing
the updated settings before submitting again.

The separate portal retains its existing own-record contracts. This module adds
no external messages, paid service, device connection or public deployment.

## Specialist-role update, local implementation only

The pending Account Manager release binds each grant to both complete protected
role sets. A specialty change suspends its use even when the compatible base role
stays Employee or Manager. Existing grants retain their saved legacy mapping and
history. Staff without HR/team authority can cover only a specifically named
leave approval; every action must still prove the original principal's exact
request/step authority. The native version2 projection displays complete staff
roles without adding Account Manager access to substitutes. This is verified
locally; installed0.1.62 and hosted specialist activation remain unchanged.
