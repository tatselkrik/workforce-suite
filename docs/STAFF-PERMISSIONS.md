# Protected staff permissions

Current protected membership and responsibility assignments grant access in
Workforce Suite. User-editable Auth metadata, menu visibility and an account's
descriptive legacy role label never grant authority. Missing, unknown, foreign,
archived or revoked scope grants no new access.

See [Account Manager](ACCOUNT-MANAGER.md) for the responsibility matrix and
[Staff provisioning](STAFF-PROVISIONING.md) for recoverable account setup.

## Responsibility boundaries

- Personnel permits employment and contact maintenance, private 201 files,
  employment history and separation administration. Other staff retain their own
  contact controls and specifically assigned clearance tasks without private cases.
- Recruiting controls applicants, interviews, feedback, offers, hiring and
  onboarding. Assigned staff can complete their own interviews or tasks. Offer
  approval remains Administrator-only. Converting an accepted offer creates the
  employee and standard tasks atomically, without creating an Auth account.
- Learning and Performance administers goals, cycles, appraisals, training and
  certificate metadata. Private HR notes, document evidence and file bindings
  retain separate Personnel permissions. Published review snapshots are immutable.
- Payroll preparation, salary drafts, government preparation and financial reports
  require Payroll responsibility. Independent Administrator approval and payment
  or remittance recording retain creator, preparer and submitter exclusions.
- Attendance and leave administration require their protected capabilities.
  Operational approval delegation never grants personnel, account or payroll powers.

New HR Generalists do not implicitly receive Recruiting, Development or Payroll.
An explicit audited legacy specialty mapping preserves compatible existing duties.
Combined responsibilities retain the union of their permitted scopes.

## Own team and assigned scopes

A Manager's scope follows current reporting relationships. Assigned interviews,
clearance tasks, service requests and appraisals grant only the specific required
access. A read-only directory does not grant attendance or leave history, private
contacts, files or salary data. Workforce Employee remains own-only for every role.

The operational directory includes only needed employee labels and protected
responsibilities, not Auth account IDs, private contacts or Account Manager data.
Reviewer selection checks the selected person's current protected duties; every
write independently checks them again.

## Revocation and compatibility

The server and native adapters check the current responsibility set and revision.
Role changes invalidate cached context and exports even if the compatible base
role code is unchanged. Storage, direct RPCs, employee-app APIs and updater
admission enforce the same current boundaries.

Delegation freezes protected role sets in immutable bindings. Role or employment
mismatches suspend access; restored matching roles may resume an unrevoked grant
within its dates. Older clients that cannot represent specialist principals must
require an upgrade rather than omit or misinterpret protected history.

## Accounts history and retries

Account changes preserve exact actor and payload bound retry receipts, revisions,
original employee identity and immutable access history. Unknown earlier role or
access facts remain unrecorded rather than reconstructed. Actor labels are current
employee names and are not assertions about historical names.

No metadata change, hidden control or stale receipt bypasses current permission.
Confidential cases, benefits records and leadership reports retain their own
separate projections and role checks.
