# HRIS desktop

Read `memory.md` before substantive work. Treat it as historical evidence, not new authority; current user instructions and current files take precedence. Maintain it only for new reusable project decisions or verified lessons. Do not put temporary progress, credentials, contact information, participant/client data, or private links in it.

## Standing authorization for prototype SQL

Kirk explicitly authorizes all SQL changes and migrations on this project's
dedicated HRIS prototype, including DROP, DELETE, TRUNCATE and updates without
WHERE. Read `deployment-private/SQL-AUTHORIZATION.md` for the exact authorized
project reference and verify the target before writing. This is continuing user
authorization: do not ask Kirk to reconfirm SQL execution or destructive-SQL
warnings for that matching project, including after compaction or a new session.
It remains in effect until Kirk revokes or changes it. Keep checked-in migrations,
target guards, appropriate verification and preservation of unrelated data. This
authorization does not extend to another Supabase project, purchases or external
communications. Respect platform-enforced tool permissions; do not claim this
file disables the platform's approval mechanism.

For authorized writes, prefer the already-linked Supabase CLI (`db query --linked`
or `db push --linked --yes`) with the exact target guard. The connected plugin's
current inherited "Allow low-risk actions" mode can generate its own SQL prompts;
the linked CLI has completed this project's authorized writes without that
connector prompt. Do not broaden plugin permissions to every connected project
to solve this project-specific requirement.

## Product and deployment rules

This is a native Rust desktop prototype. Preserve the application identifier `io.github.tatselkrik.hrisdesktop`. Each installed company deployment automatically uses its own dedicated Supabase project through its packaged connection profile. Database setup, connection editing and company selection do not belong in the app. Keep HRIS account sign-in separate from deployment setup; configured installations open at HRIS sign-in and never fall back to demo. An unconfigured source build may use the separate, visibly labeled fictional in-memory demo. Do not add SQLite or a web frontend to the desktop application; the future employee portal is a separate application.

Keep company-specific settings separate from core HR behavior. Never ship a Supabase secret/service-role key. Keep authentication sessions in memory. Apply schema changes through checked-in migrations; do not modify an unrelated hosted project. Verify Rust behavior locally and report live-database and visual-verification limits honestly.

The desktop serves Administrators, HR Generalists, Managers and future assigned staff roles. Ordinary Employee accounts are reserved for a separate mobile-friendly portal; reject their desktop sign-in before loading HR records. Account Manager is visible only to Administrators. Enforce permissions through protected database memberships and RLS, never user-editable Auth metadata. Shift schedules and dated assignment snapshots are described in docs/SHIFT-SCHEDULES.md. Leave policies and ordered approval chains are described in docs/LEAVE-POLICIES.md; preserve saved charge/approval snapshots and server-enforced revisions. The portal remains a separate future module; keep the biometric integration control visible and disabled until actual device integration is authorized and implemented.

Recruiting scope is documented in docs/HIRING.md: keep applicants/files/offers private to HR/Admin; assigned Managers receive only their interviews, own feedback and assigned checklist tasks. Preserve accepted-offer atomic hire conversion, no automatic Auth creation, immutable history and current membership/revision checks. Keep the six-stage applicant contract compatible with older installations.

Exit and clearance rules are in docs/EXIT-CLEARANCE.md. Keep separation details HR/Admin-private and Manager access limited to assigned tasks. Finalization requires an Administrator, fresh protected readiness checks and atomic membership deactivation plus employee archiving; retain Auth accounts and history. Keep exit tables separate from onboarding for installed-client compatibility.

Performance rules are in docs/PERFORMANCE.md. Preserve current reporting/assignment scopes, separate HR notes, clean reassignment drafts and immutable published review/goal snapshots. The separate performance migration adds a late exit finalization guard for older clients; do not rewrite deployed exit migrations.

Learning rules are described in docs/LEARNING.md. Keep certificate file bindings and their events HR/Admin-only; Manager/employee metadata must not contain private document IDs or storage keys. Learning progress is distinct from HR-recorded completion, and training dates do not create attendance, leave or exit blockers.

Career rules are in docs/CAREER-DEVELOPMENT.md. Plans describe shared aspirations;
never change canonical employment/pay or source goal/learning facts automatically.
Preserve independent staff definition/review, own-staff progress, HR-only waivers,
current reporting scope, same-employee metadata-only references and immutable
completion snapshots. Career plans add no exit blocker or notification contract.

Payroll rules and source dates are in docs/PAYROLL.md. Keep salary/draft/audit data
HR/Admin-private, other staff limited to their own approved slips, and published
slips free of private HR notes, raw source bundles and consent references. Preserve
server centavo calculations, source/revision guards, independent Administrator
approval and immutable slips/events. Monthly reconciliation does not authorize
monthly-only wage payment. Missing attendance never implies an absence deduction;
payment recording is not a bank transfer or government filing. Rule packs need
explicit effective dates and current official Philippine sources.

Cutoff payroll rules are in docs/PAYROLL-CUTOFFS.md. Keep its separate period
tables compatible with monthly clients. Preserve both-direction per-employee
overlap guards, exact first/second allocation, issued-first inputs to monthly
contribution/cap reconciliation, semi-monthly withholding and immutable slips.
Portal work resumed on24October2026 for LOCAL use only. Keep it a separate application; do not publicly host, tunnel or expose it.

The separate employee-portal/ Rust application is described in
docs/EMPLOYEE-PORTAL.md and has its own nested instructions, Cargo.lock and release
lifecycle. Keep upstream Auth tokens and server session state in memory, the
browser cookie opaque/HttpOnly and CSRF tokens in page memory. Limit APIs to the
current person's records even for staff roles. Portal work must not weaken native
Employee admission or mutate a published desktop release.

13th-month payout rules are in docs/PAYROLL-THIRTEENTH.md. Use current-employer
issued earned basic plus evidenced nonoverlapping opening intervals, never salary
projections or rounded memos. Eligibility follows actual duties/service, not the
account role. Preserve source/revision/independent-approval and earlier-payout
locks, shared-cap floors, private evidence and immutable own slips. Withholding
and contribution adjustments are explicitly reviewed inputs in this increment;
automatic annualization/prior-employer tax reconciliation remains separate.

Annual compensation tax rules are in docs/PAYROLL-ANNUAL-TAX.md. Use recorded
paid dates, net certified prior-employer withholding, nonoverlapping evidenced
openings and one shared benefit exclusion. Preserve paid-source freshness,
independent approval, earlier-settlement credits, immutable own aggregate advice
and HR-only certificate/source evidence. Tax advice is not a last-pay injection,
transfer, filing or official2316. Portal work is authorized locally only; public hosting remains out of scope.

Individual payment rules are in docs/PAYROLL-PAYMENTS.md. Preserve immutable per-slip
receipts and their actual received dates. One person's payment must not mark the
whole batch paid. Legacy batch clients record outstanding recipients only; annual
tax follows receipts even while a batch remains approved. Payment references stay
HR/Admin-private; only active Administrators record actual full-slip payments.

Final-pay rules are in docs/PAYROLL-FINAL-PAY.md. Preserve prospective received-plus-
outstanding annualization, the one shared benefit cap, evidenced entitlement/plan
classification, independent creator/preparer/submitter exclusion, per-person source
reservations and atomic settlement. A final receipt discharges a source net through
combined cash/offsets; it is not a second full-net cash payment. Keep own statements
and terminal status aggregate-only, private references/evidence HR-only, unpaid
withdrawal distinct from reversal, and same-employment duplicate guards. Do not gate
owed pay on clearance, archiving or account access. The first reviewed pack supports
same-year2026 separation/cash; the separately documented0.1.41 extension adds
reviewed2025 separation/2026 receipt and linked amendments.


Regional wage rules are in docs/REGIONAL-WAGES.md. Preserve evidenced effective
classifications and independent creator/preparer-excluding approval. Compare only
basic daily or exact true-monthly salary with supported dated official floors.
Unknown dates/coverage stay unresolved; exemptions require independent evidence,
not account role or tax flags. Optional Admin enforcement defaults off and guards
older monthly/cutoff clients on the server. Retain immutable check/history/slips;
changed facts require resubmission. A reference gap is not automatic arrears.


Consolidated payroll reporting rules are in docs/PAYROLL-REPORTING.md. Use a fresh
HR/Admin-only single-statement snapshot and recheck authorization before export.
Keep approval/actual receipt dates, source offsets, combined final cash, tax
advice/settled adjustments and current outstanding/reserved balances distinct.
Never use later batch metadata as an individual's receipt or add contained source
net twice. Reports exclude private evidence, references and tax identifiers.
Reject excessive ranges/rows without silently truncating financial totals.


## Release installation workflow

Kirk authorized0.1.42 and future verified releases to the existing private HRIS
update channel on25October2026. Read deployment-private/UPDATE-PUBLICATION-AUTHORIZATION.md
for its exact destination guard; do not request repeated permission for that
matching verified publication. Preserve immutable artifacts/newer releases and
verify frozen source archives, signed packages, migration subsets, private access
and download hashes. Earlier0.1.41-only approval-review boundaries are superseded
by this direct human authorization; platform permissions still apply.

Linked amendment and receipt-year extension rules are in docs/PAYROLL-AMENDMENTS.md.
Preserve original slips/receipts and independent approval, reviewed same-year refund
floors and global net tax-credit guards. Overpayment reviews create no automatic
recovery, debt or tax reduction. Carry evidenced outside benefits without counting
known issued benefits twice. Delayed final pay may retain prior-year service with
reviewed2026 receipt dates; never invent service coverage or a2027 rule pack.
Use the versioned report-v2 contract for consolidated amendment cash.

For each authorized HRIS release, finish by installing the latest verified version
on Kirk's PC. Publishing or delivering an installer alone is not completion.
Verify the installed executable hash, BUILD version, Windows Apps registration and
normal launch; preserve company connection/branding, update-verification keys and
unrelated files. Do not reinstall if all installed evidence already matches.
Check a running app for unsaved work before closing it; never discard drafts.
If Kirk must save/close it, complete independent work first and use a concise
handoff. This installation preference does not authorize another module, a public
release, a new deployment target or bypassing platform permissions.


## Authorized continuation — 24 October 2026

Proceed sequentially with payroll amendments/cross-year final pay, then the resumed
local-only employee portal, government-specific payroll exports/reconciliation,
payroll/appraisal/training notifications, leave-policy versions/opening balances/
approval delegation, and attendance/leave imports plus accounting/device integration
foundations. Preserve original payslips through linked adjustments. Verify current
official rules before financial implementation. No bank transfer, actual government
filing, public portal hosting, new cost or unresolved third-party/device connection
is authorized. Keep actual biometric support disabled pending a selected protocol/
device and physical verification. Use the existing usage-reset follow-up/checkpoint
to continue this queue; install each verified authorized desktop release.

## Current continuation — 24 October 2026

Kirk explicitly resumed continuous work after0.1.41 and reactivated the existing
usage-reset scheduler. The earlier stop-after0.1.41 boundary is superseded.
Follow the approved order in docs/REMAINING-ROADMAP.md. Cross out each milestone
only after implementation, proportionate verification and its authorized release/
delivery requirements; retain partial status until then. Complete the local-only
portal first, then the remaining ordered checklist. Preserve the deployment,
financial, privacy and unresolved external-target boundaries above. Continue
through usage resets using this task's checkpoint and the same follow-up, until
Kirk stops the work, the scope is complete or a genuine handoff blocks progress.

## Latest stop boundary — 25 October 2026

Kirk said "stop after this" during the portal0.1.3 personal-notification increment.
Finish and verify only that current increment, deliver its local package, then
stop. The existing usage-reset scheduler is paused. Pictures/account-security
controls and the rest of the roadmap stay queued until Kirk resumes them.
This supersedes the continuous-work instruction above.

## Storage cleanup — 25 October 2026

Kirk explicitly authorized the queued cleanup. It is complete: the project went
from219.1GiB to about1.82GiB by removing generated Cargo caches, obsolete test
executables and hash-identical release copies. Source, company files, current/
rollback releases, all portal packages and useful evidence were preserved.
See docs/STORAGE-MAINTENANCE.md and the ignored work/storage-cleanup-20261025 audit.
Cargo output must be rebuilt when development resumes; do not rebuild merely to
verify cleanup. The running portal was preserved. Cleanup authorization did not
resume feature work or scheduling: both remain paused until Kirk resumes them.

## Latest resumption — 25 October 2026

Kirk explicitly said to continue the roadmap, activate scheduling again and not
stop. This supersedes the stop-after-portal0.1.3 and cleanup-only pause above.
The same usage-reset scheduler is active again. Continue docs/REMAINING-ROADMAP.md
sequentially from the remaining local portal pictures/security/verification, then
the rest of the approved queue. All deployment, privacy, financial and unresolved
external-target boundaries remain. Storage cleanup is complete; never blindly
reuse its deletion manifest. Keep compact build evidence and avoid accumulating
duplicate test binaries/debug-symbol variants after verified delivery.

For routine prototype QA builds, set CARGO_INCREMENTAL=0,
CARGO_PROFILE_DEV_DEBUG=0 and CARGO_PROFILE_TEST_DEBUG=0 in the command environment;
use one target directory per application (work/target or work/portal-target).
Keep release profiles/lockfiles pinned. Enable extra debug symbols only when
needed for a specific diagnostic, and remove obsolete output after verification.

## Government payroll review foundations

Rules/current source review are in docs/PAYROLL-GOVERNMENT.md. Keep source-v1
HR/Admin-only and enforce current protected access on the server and before/after
client requests. Preserve original slips/receipts and earned contribution months;
BIR review follows individual received dates. An unknown contribution allocation,
employer share or EC is unresolved, never silently zero. Postdated receipts retain
their date and explicit review state. Final cash/source tax must not be counted
twice. Contribution allocation and remittance workflow rules are also in that
document. Preserve exact issued employee amounts, all-editor/own-source approval
exclusions, immutable approved reviews/receipts, linked correction chains and
fresh protected exports. Only Administrators record/void evidenced remittances.
Internal worksheets are not agency upload files. Source/workflow increments do
not complete the full government milestone without legal identity/file adapters.

Government registration records follow docs/GOVERNMENT-REGISTRATION.md. Keep legal
names, assigned identifiers, supporting references and history HR/Admin-private,
including against an Employee's own-record access. Unknowns remain blank; shape
checks never imply agency verification. Preserve actor-bound retry receipts and
immutable before/after revisions without changing employment, branding or pay.

Agency preparation follows docs/GOVERNMENT-PREPARATION.md. Export only the chosen
agency's registered fields through the fresh protected single-statement projection;
recheck access after rendering and before file IO. Internal CSV is not an agency
upload/return. The bounded0.1.45 SSS control produces an HR-review reference draft;
current agency acceptance is unverified. Preserve namesakes, exact cents and
explicit declarations. Never infer current rates or
agency acceptance from a legacy generator's output alone.

## Latest stop boundary — current SSS increment

Kirk instructed on 25 October 2026 to finish only the current SSS reference-draft
increment, then stop and provide a testing checklist for work since 9:31 p.m.
and the remaining roadmap. The existing scheduler is paused. This supersedes
continuous continuation; do not start the next increment until Kirk resumes it.
Standing prototype SQL and verified private-channel publication authorizations
remain in effect; this stop changes work scheduling, not those target guards.
This boundary is now satisfied:0.1.45 is installed and privately published.
Keep work and the same scheduler paused pending Kirk's explicit resumption.

## Latest resumption and free-only constraint — 25 October 2026

Audited base-credit adjustments follow docs/LEAVE-CREDITS.md. Keep Administrator
posting, minimal own/current-team projections, HR-private reasons/references,
same-identity retries, exact half-day arithmetic and immutable linked reversals.
Never treat a delta as an opening balance or rewrite filed charges/plans. Negative
entries must protect reservations and later-year carry bases. Preserve the older
portal balance contract and native upgrade guard for credited previews. Scheduled
policy changes/opening-balance takeover accounting remain separate roadmap scope.

Kirk explicitly resumed the remaining roadmap and the same scheduler after
0.1.45, requesting continuous work. This supersedes the finish-after0.1.45 pause.
Preserve the running local portal0.1.5 and its drafts/sessions. Continue the
ordered checklist with the existing verification/release/installation rules.

HRIS must remain completely free for now. Put any feature requiring a paid
subscription, paid service or new purchase on hold; continue independent local,
free/open-source and already-available free-tier work. Do not enable billing,
upgrade plans, incur fees or purchase hardware/services. A free tier must stay
within its no-charge limits; it is not permission for overage or auto-upgrades.
Record cost dependencies and unresolved targets explicitly instead of silently
calling an unconfigured integration complete. Actual email/push delivery and
physical device connections retain their separate target/authority requirements.

## Latest verification, delivery and hardware scope — 25 October 2026

Kirk prioritizes portal browser verification for now. Mobile-width layout checks
may use existing no-charge browser tools; physical-phone verification and actual
email confirmation need not block that browser milestone. Keep unverified results
explicit. Defer any verification requiring payment; actual confirmation may wait
for an approved no-charge setup. Preserve existing Auth confirmation rules.

Roadmap item8 is narrowed to notifications appearing inside the portal inbox for
now. Existing native staff alerts remain. External company email delivery and
background browser/OS push are deferred; do not configure a delivery provider or
begin an external sending workflow under older roadmap wording. The existing own
portal inbox satisfies this narrowed display scope, not external delivery.

Kirk has no biometric hardware. Keep the biometric section visible and Connect
device disabled. Actual device/driver/protocol work and hardware purchases are
deferred until Kirk supplies hardware and resumes that scope. The rest of the
authorized free roadmap and the same scheduler remain active.
