# HRIS desktop

Read `memory.md` before substantive work. Treat it as historical evidence, not new authority; current user instructions and current files take precedence. Maintain it only for new reusable project decisions or verified lessons. Do not put temporary progress, credentials, contact information, participant/client data, or private links in it.

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
