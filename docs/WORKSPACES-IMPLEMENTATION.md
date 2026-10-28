# HRIS workspaces and Payroll implementation â€” approved27October2026

**Next scheduled milestone —28October2026:** Kirk authorized ONLY Separate
Payroll Desktop using the same Supabase and preserving all payroll features/history,
starting AFTER the usage reset. The SAME scheduler is active for29October2026
00:54:27UTC+8; implementation remains paused until then. Later HR-function work
remains queued. The completed0.1.64 delivery and earlier pause below are historical;
resume only Payroll separation on the scheduled wake, then pause after delivery.

**Current delivery —28October2026:** focused HRIS workspaces and optional-module
visibility are delivered, privately published and actually installed in0.1.64.
Account Manager0.1.63 remains enabled and compatible. **Work and the SAME scheduler
are PAUSED** after this authorized milestone. Older active/gate entries below are
historical. See [0.1.64 delivery evidence](WORKSPACES-0.1.64-DELIVERY.md).
Separate Payroll and later HR-function work stay queued until Kirk resumes.

Kirk approved the reviewed proposal and authorized implementation plus automatic
continuation across actual usage resets. This supersedes the preview-only hold
and scheduling pause for this queue. The older agency templates, official module
execution/acceptance, paid services, external delivery/vendor connections and
physical-device work remain deferred. This is an industry-neutral Philippine
prototype for small companies and larger employers, including BPOs.

## Ordered delivery checklist

- [x] ~~Staff roles, protected permissions and multi-role Account Manager.~~
  Delivered and enabled in0.1.63; evidence linked above.
  Keep Administrator, HR Generalist, Manager and Employee; add Recruiter,
  Learning & Performance Officer and Payroll Officer. A single employee/account
  can hold several staff responsibilities. Employee alone remains portal-only.
  Administrator remains highest application authority; developer access is an
  explicitly authorized company account, never an embedded support backdoor.
- [x] ~~Focused native workspaces and optional-module visibility.~~
  Delivered/installed0.1.64; see the delivery record above.
  Retain one Employees > Development workspace for Goals, Appraisals, Training,
  Certifications, Career plans and administrative Review cycles. Use one combined
  Learning & Performance Officer role. Preserve manager assignments and private
  HR notes/evidence boundaries. Company configuration must suit general employers
  without requiring BPO-specific client/campaign terminology.
- [ ] Separate native Payroll Desktop using shared Rust code and the same company
  Supabase. Keep one repository and existing financial sources/history. Payroll
  Officer is a distinct permission regardless of company size; one person may
  also hold HR Generalist. Preserve independent Administrator financial approval,
  self-review exclusions and exact source/revision/receipt controls. Verify full
  feature parity and local delivery before removing payroll screens from HRIS.
- [ ] Employee-relations case workflow with confidential, assigned access and
  immutable history. Preserve existing personnel/exit/payroll facts.
- [ ] Employee-service requests with ownership, routing, status and retained history.
- [ ] Expanded benefits administration for eligibility, enrollment and dependents;
  approved monetary effects remain explicit payroll inputs, never automatic pay
  or deduction changes from an unreviewed benefits edit.
- [ ] Expanded HR leadership reporting using defined, supportable historical facts.
  Keep unknown turnover/service dates explicit; no invented staffing forecasts.

Cross out only implemented, proportionately verified and delivered scope. Record
partial foundations without claiming a usable specialist role or completed release.

## Role boundaries

| Role | Intended scope |
| --- | --- |
| Administrator | Company administration, full authorized oversight and independent restricted approvals |
| HR Generalist | Personnel, attendance/leave and general HR services/relations/benefits; additional specialties are explicit assignments |
| Recruiter | Company applicants, interviews, offers and hiring/onboarding responsibilities |
| Learning & Performance Officer | Development administration and authorized metadata; separate HR-private notes/document evidence stay protected |
| Payroll Officer | Financial preparation, sources and reports; payroll approval remains independent Administrator work |
| Manager | Existing own/current-team and assigned-review/task scope |
| Employee | Existing own portal scope, without desktop admission |

Roles are access responsibilities rather than mandatory separate jobs. A company
may combine assignments on one account. Payroll preparation and approval remain
separate actors without creating a separate Payroll Approver role.

## Compatibility and implementation gate

The installed0.1.64and protected database support seven combinable roles.
The separate interactive preview illustrates proposed navigation only. Inspect
all affected RPCs, RLS/storage policies, source snapshots, Account Manager provisioning,
portal admission and native gates. Never grant a specialist the broad HR role merely
to make one screen load. Keep current membership checks independent of user metadata.
Use versioned additive contracts and fail closed for unsupported clients. Preserve
existing owner access, company identity, employment links, drafts and audit records.
Existing broad HR assignments need an explicit, audited compatibility mapping to
retain their current rights during transition; new assignments follow the new scope.

Implement and test local permission foundations before hosted rollout. An assignment
must not be described as usable until its complete native/server data and action
boundaries pass verification. Check direct API denial and revocation, not only menus.

For each authorized app release, verify/package/install the exact candidate and keep
rollback. Existing HRIS private update publication is already authorized at its exact
private guard. A separate Payroll identity/feed must be resolved and authorized before
publishing there; complete independent local development and delivery first.

## Recovery and current evidence

Use AGENTS.md's current context-freshness protocol and this chat's private checkpoint.
The SAME resume-hris-after-usage-reset scheduler is PAUSED after delivered0.1.64.
On a future authorized resumption, check both five-hour
and weekly limits and arm the actual five-hour reset +2minutes before substantive work,
checkpoint at meaningful milestones/around80% usage, and retime before exhaustion.
The computer and desktop app must be available for local continuation. Do not repeat
completed desktop0.1.62, portal0.1.7 or migrations through72.

## Local role foundation progress â€”27October2026

The seven-role capability model, bounded/canonical role sets, protected actor-bound
contexts, legacy-compatible base-role projection and version-negotiated native
admission are implemented locally. An additive private registry captures prior
membership rights with immutable compatibility history. Specialist activation
remains gated off: no new role is assignable in the installed app yet.

Local evidence: the first full normal Rust run passed690checks with28intentional
opt-ins ignored. After access binding changes,11role-model,4existing-access,
9delegation,7new access-binding and6local HTTP adapter cases passed. The local
PostgreSQL suite passed10,251workflow/preservation assertions, fresh73migration
replay and129role-foundation assertions. Shared capability cases match native/SQL.
These are local checks; no hosted migration, role activation, release or installation
has occurred. Role editing and complete module read/write boundaries remain next.

## Local assignment transaction progress â€”27October2026

The disabled rollout now has typed protected Account Manager views and an atomic
role/access transaction. It binds the current Administrator, employee, full request,
role revision, compatible base role and membership timestamp. Exact retries return
an immutable receipt; source changes require a fresh review. Sign-in disable/restore
preserves assignments, captures status history and retains owner/self/hierarchy
protections. Uncertain delivery retains the same request identity and contents.
The native adapter rechecks actor access before returning refreshed assignments.
Older clients cannot overwrite specialist roles through their four-role selector.

Local evidence: the full Rust run and database results are recorded in the private
task checkpoint; the PostgreSQL workflow/preservation suite passed10,764assertions,
fresh74migration replay,129foundation checks and76transaction checks. The runtime
policy remains FALSE. Migrations73/74 are local only; no hosted role activation,
new desktop release, installation or usable specialist role is claimed. Native
Account Manager editing/provisioning and complete domain read/write boundaries
remain required before rollout.
## Local Account Manager editor finished; work paused â€”27October2026

The existing-account editor and native worker routing are implemented locally.
Administrators can prepare multiple responsibilities, sign-in access and a reason
against the exact saved account source. Uncertain delivery retains one immutable
retry; a confirmed transaction rejection keeps the draft for explicit re-review.
Temporary post-save refresh failures retain the pending request. Only a matching
receipt closes it. Actor changes block a save; personal/company data are cleared
on access changes. The app's unsaved-work guard includes these drafts.

Final local verification:722normal Rust checks passed,28explicit opt-ins ignored.
This includes real loopback HTTP request/receipt/access tests, stale/rejected and
uncertain-draft cases, and automated editor geometry/immutability at480,760and1100
pixels in light/dark themes and pending/editable states. Headless checks are not
manual inspection of the installed application. The earlier unchanged SQL passed
10,764workflow/preservation assertions, fresh74migration replay,129foundation and
76transaction assertions.

The runtime gate is still FALSE. Migrations73/74 were not deployed; no new desktop
release was packaged, published or installed. Desktop0.1.62/portal0.1.7 deliveries
remain unchanged. New specialist roles are not yet usable. Atomic new-account
provisioning and complete module/RLS/RPC/storage/portal/updater permissions remain
queued before activation, followed by the remaining approved order above.

Kirk requested pausing after this editor increment. The same scheduler is PAUSED;
resume only after a newer explicit human instruction and the current checkpoint.
## Current local permissions progress â€”28October2026

See STAFF-PERMISSIONS.md for the verified local payroll/development/directory scopes
and remaining activation requirements. Specialist activation remains FALSE. Continue
only Account Manager provisioning/permissions through verified delivery, then pause.
