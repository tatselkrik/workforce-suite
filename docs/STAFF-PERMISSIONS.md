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

Current local evidence: the full normal Rust run passed756checks with28intentional
opt-ins ignored, including14shared role combinations and unchanged-base-role
revocation. Staged84SQL passes12,353workflow/preservation assertions; dedicated
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

Operational delegation now freezes both protected role sets in a separate
immutable binding. Existing grants are backfilled from their original saved role
codes, never today's assignments. Current full role/binding/status checks suspend
mismatches even when the compatible base code is unchanged. Matching restored
roles can resume an unrevoked grant inside its dates; explicit revocation remains
final for that grant. Named leave-plan assignments remain request-specific and
cannot be forwarded. HR operational authority requires HR/Admin substitutes;
Administrator authority requires another Administrator. Other specialties cannot
use a named assignment as attendance/overtime authority. Financial delegation
remains excluded. The separate version2 context retains version1 compatibility.
New local evidence:83chronological migrations,615delegation assertions and11native
delegation regressions pass; the unchanged public data/function grants pass the
12,353assertion preservation suite. The new tests include actual named leave
submission/decision, both-side same-base role changes, restoration, revocation,
immutable receipts and direct private access denials. No hosted rollout occurred.

## Notification and updater increment

The native notification inbox uses current protected Desktop admission, Development
administration for appraisal publication and company learning/certificate reminders,
and current Manager reporting scope for team reminders. Private 201-file alerts
still require Personnel Documents. Payroll notices remain own-advice notifications;
HR notes, assessments, certificate identifiers and private evidence are omitted.
The explicit portal surface stays own-only for all roles, including Administrators.
Changing specialties invalidates no-longer-permitted notices/read operations even
when the compatible base role stays Employee. Mixed legacy/lifecycle read updates
roll back atomically on an unavailable key; each person's portal/native read markers
remain separate.

The private update catalog and Storage download policy admit all current desktop
staff specialties while rejecting ordinary Employees, unassigned identities,
anonymous callers and disabled/archived staff. Membership metadata is not authority.
Authenticated staff, including Administrators, gain no catalog publication or updater
upload authority. Signature/package verification and published artifacts are unchanged.

Targeted local evidence:398PostgreSQL assertions pass across14shared role combinations
and84chronological migrations;15native notification regressions pass, including three
new matrix/privacy/revocation tests. Fixtures honor one-company deployment and
deactivate-before-archive rules. Tests retain all notice/update source rows. These are
local managed Auth/Storage simulations, not hosted API/S3 or installed-screen proof.
Final normal native verification passes756tests with28intentional opt-ins ignored;
staged84SQL passes12,353workflow/preservation assertions. The new test file passes
its formatting check and the owned diff passes whitespace checks. Logs are retained
under ignored work/staff-notifications-{native,native-full,schema}.log and
staff-notifications-permissions-db.log. Specialist activation, hosted migration and
installed0.1.62remain unchanged. The pre-existing repository-wide formatting gate
and unrelated reports import warning remain for the later complete-release task.

The repository formatting check currently reports90files, including untouched
pre-resume source. Its required final release check remains unresolved; no CI
success or delivery is claimed by these local checks.

## Account-write safeguards, local85increment

Legacy role/status saves now synchronize the protected assignment/revision/history
in the same business transaction as the original membership/access event. Existing
records/receipts remain immutable; deployment reconciliation captures current
membership and preserves unknown earlier status. Safe legacy disabling retains
specialties; only the new contract restores or replaces them. After activation,
new account creation requires the updated provisioning contract. The hosted
handler checks that gate using the caller's protected context before Auth creation.
Neither legacy nor new handlers delete an identity after an uncertain business save.

The private scoped saver accepts validated prospective roles only from reviewed
v2 entry points; it is not granted to authenticated or service-role callers. It
preserves owner/self/last-Administrator, established employee identity and archive
locks. Specialist desktop staff may retain reports while true Employee-only access
requires reassignment. Reporting metadata does not confer Manager review authority;
explicit Manager duty works even with the legacy HR compatibility code.

Local85evidence:30dedicated reconciliation/identity/supervisor/audit rollback checks,
76existing role-transaction checks,77Edge regressions and12,353staged preservation
assertions pass. The chronological85chain replays. No hosted change occurred.

## Protected account history, local86increment

Access history is available per account in Account Manager. Its read-only projection
binds the current Administrator, company, selected employee, role revision and member
source time. It includes complete role and original account-access events with a
1,000-record limit for each stream; excess history is rejected without omission.
No Auth/email/credential fields or raw legacy JSON enter this projection. Missing
earlier roles/access states remain Not recorded. Actor names are explicitly current
employee names rather than reconstructed historical identities.

Native validation rejects foreign/stale/incomplete history and rechecks the actor
after HTTP reads. History refresh retains pending role drafts and clears a stale
history view after source changes. Existing provisioning recovery keeps exact retries,
checked outcomes and reasoned cancellation; staff rows now expose the existing
explicit Send confirmation control when appropriate. No automatic mail was added.

Local86evidence:77database history assertions across14roles,86chronological replay,
12,353preservation assertions and74focused native staff/provisioning/history tests
pass. This includes real loopback binding/revocation, immutable history/unknowns,
pending draft preservation and narrow320/480/760light/dark header geometry. A narrow
heading overflow was found and fixed. Headless geometry is not installed-screen
verification. Hosted activation and installation remain unchanged.

## Remaining before activation and delivery

Review every direct RPC, RLS/storage path and native snapshot/action/menu. Add the
remaining scoped roster and current-role labels, reconcile legacy account writes
with protected assignment/history, and complete the remaining native specialist workflow/export/navigation boundaries.
Verify own/team/assigned scopes, revocation, older clients, portal and updater
access before enabling roles. Then apply guarded migrations, verify hosted
behavior, package/publish/install the exact verified release and PAUSE work plus
the same scheduler. Wider workspace changes and Payroll extraction stay queued.

## Current one-task stop boundary

Kirk narrowed this run to the notification/updater increment. Its local
verification and saved source are complete; work and the same scheduler are paused. Do not continue
the remaining Account Manager tasks or ship the incomplete role rollout. The wider
scope above remains a queue, rather than instructions to resume automatically.


## Completed local native guard audit

The remaining cutoff/benefit/tax/final/amendment workflow and export paths now use
Payroll Preparation rather than the old HR code. A fourteen-role native matrix
exercises actual create payloads and paid-source selection, plus another person's
final-statement export and retained own advice. Recruiting-plus-Manager keeps the
Recruiting workspace; other staff keep assigned hiring tasks. Report preferences,
organization directory/history filters and document-alert categories use their
protected scope. No financial calculation or rate changed.

The v1 delegation DTO cannot represent Employee-base specialist principals/delegates.
Migration87explicitly requires an app upgrade for those records instead of returning
invalid or silently omitted history. The separate version2 projection stays complete.
Existing three-base-role delegation records retain their legacy contract.
