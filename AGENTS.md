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
Portal work is currently paused; native payroll work does not resume it.

The separate employee-portal/ Rust application is described in
docs/EMPLOYEE-PORTAL.md and has its own nested instructions, Cargo.lock and release
lifecycle. Keep upstream Auth tokens and server session state in memory, the
browser cookie opaque/HttpOnly and CSRF tokens in page memory. Limit APIs to the
current person's records even for staff roles. Portal work must not weaken native
Employee admission or mutate a published desktop release.
