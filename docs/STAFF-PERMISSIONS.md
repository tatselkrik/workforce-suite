# Protected staff permission rollout

Status28October2026: LOCAL implementation in progress. Runtime gate remains FALSE.
Hosted migrations through72 and installed desktop0.1.62 remain unchanged. This is
not yet a usable or delivered specialist-role release.

The seven-role/multiple-responsibility contract is in WORKSPACES-IMPLEMENTATION.md.
Account provisioning/recovery is in STAFF-PROVISIONING.md. Only a current protected
membership/assignment grants authority; Auth metadata is never a role source.
New HR Generalists do not implicitly gain Recruiting, Development or Payroll.
The audited legacy four-specialty mapping retains existing broad HR rights.

## Implemented local boundaries

- Capability kernel uses current active membership and employee status. A missing,
  foreign, unknown, archived or revoked scope grants no new access.
- Payroll preparation, salary/draft data, government preparation and reports use
  the protected payroll responsibility. Independent Administrator approval,
  payment/remittance recording, source checks and issued facts are unchanged.
- Learning & Performance administers goals, cycles, appraisals, training and
  certificate metadata. Private HR notes and certificate file bindings/events
  retain separate HR/Admin permissions. Shared progress, current-team review,
  independent decisions and immutable published snapshots remain enforced.
- A fresh, bounded workflow directory supplies operational employee IDs, names
  and protected responsibilities. It contains no Auth account IDs, emails,
  contacts or Account Manager data. It supports specialist reviewer selection;
  actual writes recheck the assigned person's current protected authority.
  Unknown, malformed, duplicate or foreign directory scope is rejected.

Current local evidence:748normal Rust checks pass with28intentional opt-ins
ignored;12,353PostgreSQL migration/workflow/preservation assertions pass against
staged79migrations;136actual payroll/government/RLS checks and463development,
private-field, legacy workflow and directory assertions pass. These checks use
fictional records and managed Auth/Storage fixtures; they are not hosted or
installed-screen verification. No financial calculation/rate was changed.

## Remaining before activation and delivery

Complete Personnel/201-files, attendance/leave/approval and Recruiting boundaries;
review every direct RPC, RLS/storage path and native snapshot/action/menu. Add the
remaining scoped roster and current-role labels, reconcile legacy account writes
with protected assignment/history, and finish role/access audit/recovery usability.
Verify own/team/assigned scopes, revocation, older clients, portal and updater
access before enabling roles. Then apply guarded migrations, verify hosted
behavior, package/publish/install the exact verified release and PAUSE work plus
the same scheduler. Wider workspace changes and Payroll extraction stay queued.
