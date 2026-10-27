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

Current local evidence: the full normal Rust run passed751checks with28intentional
opt-ins ignored, including14shared role combinations and unchanged-base-role
revocation. Staged82SQL passes12,353workflow/preservation assertions; dedicated
checks pass298capability,209payroll,562development,298Personnel,247Recruiting and
168operational/context/principal/correction/import assertions. These checks use
fictional records and managed Auth/Storage fixtures; they are not hosted or
installed-screen verification. No financial calculation/rate was changed.

## Additional local Personnel and Recruiting boundaries

Personnel alone permits employment/contact maintenance, private 201 files,
employment history and separation-case administration. Other staff can maintain
own contacts and complete their exact assigned clearance tasks, without private
case details, other assignments or waivers. Private file RLS also protects direct
Storage requests. A read-only employment directory is separate from the generic
operational employee scope; it does not grant attendance/leave histories, balances,
private contacts, document evidence or salary access. Native daily attendance and
balance displays retain that operational scope rather than infer missing records.

Recruiting controls applicants, assessments, resumes, offers, hiring and company
onboarding. Other current staff can access exact assigned interviews/tasks and
write their own feedback. Offer approval remains Administrator-only. Accepted
conversion atomically creates one employee/contact record and five tasks, preserves
retry/history and creates no Auth account. A Recruiter without Personnel cannot
subsequently edit the employee or open their private contacts. Fourteen shared
cases cover combined Manager duties and the explicit retained legacy HR bundle.

The descriptive actor-role audit columns retain the four-code membership contract;
those codes are not permission authority. No existing audit row was rewritten.
The version1 desktop snapshot rechecks the entire protected role set/revision after
scoped reads; changed access clears cached forms even when its base code is unchanged.
Long current-role labels are truncated in the sidebar with their full hover text.

Attendance, shifts, breaks, corrections and attendance imports require the
attendance capability; leave imports, policy administration and private balance
audits require the leave capability. Reviewer selection evaluates the selected
person's current protected roles rather than the caller's. Named staff approval
remains request-specific, with self/submission and payroll exclusions retained.
Protected operational context reads admit specialist desktop accounts; generic
own/team scope is not widened by their read-only employment directory. Report
exports invalidate when the protected role set/revision changes, including when
the compatible four-code role is unchanged. Descriptive import/audit actor codes
record the actual actor without granting authority.

The repository formatting check currently reports90files, including untouched
pre-resume source. Its required final release check remains unresolved; no CI
success or delivery is claimed by these local checks.

## Remaining before activation and delivery

Complete delegation, notifications/updater and legacy-write boundaries;
review every direct RPC, RLS/storage path and native snapshot/action/menu. Add the
remaining scoped roster and current-role labels, reconcile legacy account writes
with protected assignment/history, and finish role/access audit/recovery usability.
Verify own/team/assigned scopes, revocation, older clients, portal and updater
access before enabling roles. Then apply guarded migrations, verify hosted
behavior, package/publish/install the exact verified release and PAUSE work plus
the same scheduler. Wider workspace changes and Payroll extraction stay queued.
