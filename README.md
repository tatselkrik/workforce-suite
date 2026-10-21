# HRIS Desktop

A native Windows HRIS prototype written in Rust. Each company installation automatically uses its preconfigured, dedicated Supabase database. Users sign in to **HRIS**; they do not enter database URLs, keys, or select a company. A separate unconfigured source build opens a clearly labeled **in-memory demo workspace** with fictional employees.

Application identifier: `io.github.tatselkrik.hrisdesktop`.

## What works

- **Overview:** active headcount, today's attendance, pending requests, and approved time away.
- **Employee records:** add/edit employees, search and filter the directory, customize annual allowances, and archive employees while retaining history.
- **Attendance:** record check-in/check-out, browse dates, reject duplicate daily check-ins and multiple open shifts. Live timestamps come from the database server and use the company time zone.
- **Leave:** request annual, sick, or unpaid leave; approve or reject pending requests; prevent overlaps and annual leave over-allocation.
- **Account Manager:** administrators create or link accounts, assign Administrator, HR Generalist, Manager or Employee access, link employee records and reporting managers, deactivate access and review access changes. Other roles cannot open this tab. See [account permissions](docs/ACCOUNT-MANAGER.md).
- **Settings:** company name, IANA time zone and default leave allowance, plus account display scale and 12/24-hour time preferences. Database configuration is handled during deployment.
- **Profile:** personal details, preferred name and optional contact phone; upload/change/remove a private photo; verified and pending account email; password and email changes after current-password verification; and sign-out controls for this device, other devices or everywhere.
- **Company sidebar:** a transparent deployment logo above the company name, followed by Overview, Employees, Attendance, Leave, administrator-only Account Manager and Settings. The circular photo or initials at the bottom left opens the full Profile page in the main content area.
- **CI:** checked-in GitHub Actions workflow tests Rust behavior and database access rules and builds a Windows executable. It does not deploy a database or publish a release.

The interface uses `egui`/`eframe`. The domain rules and Supabase Auth/REST adapter are also Rust. There is no browser UI, local web server, or SQLite database in the application. A Supabase Edge Function provisions accounts without exposing a server key to the desktop. Node.js runs the separate PostgreSQL and provisioning-handler tests.

## Try it

The private Windows setup installs version 0.1.4 for the current user in `%LOCALAPPDATA%\Programs\HRIS Desktop`, creates **HRIS Desktop** shortcuts on the desktop and Start menu, and registers an uninstaller in Windows Apps. Administrator rights are not required. Keep the company-specific setup private; it includes the deployment's public connection profile, but no account password or session.

Open **HRIS Desktop** and sign in with an assigned Administrator, HR Generalist or Manager account. Ordinary Employee accounts are reserved for a future mobile-friendly portal and cannot sign into this desktop app. Its company database is already selected by the installation. Account login remains necessary to access HR records; database setup is not part of that login.

Press **Enter** in the email or password field to sign in, or use the Sign in button. Submission is disabled while a sign-in is already running. Click the circular account photo/initials at the bottom left to open **Profile**. **Personal details**, **Account & security** and **Sessions** provide its account controls. Use **Sessions → Sign out of this device** to clear the session.

An unconfigured source build can run demo mode without an account. Sample changes reset when the application closes. To try the employee workflows in either workspace:

Suggested walkthrough:

1. Open **Employees**, add an employee, and edit their department or allowance.
2. Open **Attendance**, select that employee, record a check-in, then check out.
3. Open **Leave**, submit a weekday request for that employee, and approve it.
4. Return to **Employees** to see the updated annual balance.

## Build from source

On Windows, install Rust with the MSVC build tools. The repository pins Rust 1.95.0 and direct dependencies; keep `Cargo.lock` checked in.

```powershell
cargo test --locked
cargo build --release --locked
```

The executable is `target/release/hris-desktop.exe`. Run `cargo run --locked` during development. The current executable was built for Windows x64; other platforms have not been verified.

For a prepared private Windows distribution bundle, build the per-user setup with NSIS 3.11:

```powershell
makensis /DAPP_BUNDLE=C:\absolute\bundle /DSETUP_OUTPUT=C:\absolute\HRIS-Desktop-Setup.exe tools/windows/installer.nsi
```

The installer refuses an unfamiliar existing installation directory or shortcuts. It uses the application's fixed identifier for its uninstall registration. Its uninstaller targets only the named installed files and retains unknown files. Installation, file integrity, shortcuts, normal launch and an uninstall/reinstall cycle were tested locally. The uninstaller removed its own files, shortcuts and registration while preserving a synthetic unknown file in the installation folder; reinstallation restored the verified app.

For company branding, optionally include `company-logo.png` beside the executable in the deployment bundle. Use a transparent PNG up to 2 MiB and 2048 × 2048 pixels; it is scaled proportionally without adding an opaque background. The company name comes from company Settings. Missing or invalid logos use company initials as a fallback. Company-specific logos stay outside generic source control.

## Deployment setup (administrator only)

The dedicated **HRIS** Supabase project is initialized in Singapore (`ap-southeast-1`) on the Free plan, with one company assigned to the confirmed administrator Auth account. Apply every checked-in migration, including account-photo Storage policies, when preparing a company deployment. Earlier live checks verified sign-in, employee storage, attendance, leave approval/balances, refresh and sign-out; their temporary records were removed afterward. See `VERIFICATION.md` for the evidence and limits of each update.

1. Use the assigned HR administrator account; its email is already confirmed. This Auth account is separate from the Supabase dashboard login.
2. Package `hris-connection.json` beside `HRIS-Desktop.exe`. It contains only the project URL and publishable client key. The generic source includes `hris-connection.example.json` instead of a company-specific profile. The installer carries this profile automatically; users do not configure it.
3. Open the installed app. It shows the normal **HRIS sign-in** screen. Enter the assigned account's email and password locally. Successful sign-in opens Overview and loads only that company's records. Passwords and sessions are never saved.
4. Confirm **CONNECTED** in the sidebar, then update the placeholder **HRIS Company** name and policy defaults in Settings. Demo employees are never uploaded.
5. Add a test employee, refresh, restart the app, sign in again, and verify persistence before entering actual workplace data.

For a separate company's deployment, use a separate dedicated Supabase project. Apply all files in `supabase/migrations/` in order, create and confirm its first HR Auth user, and run `supabase/bootstrap-company.sql` as the project administrator. Bootstrap requires exactly one Auth user and no existing company, so it cannot silently choose another account or replace existing configuration. The desktop client cannot create companies. Deploy `supabase/functions/account-admin/` following [Account Manager setup](docs/ACCOUNT-MANAGER.md) to enable account creation and confirmation delivery. Local migration versions match the hosted migration history; migration files were originally generated by the Supabase CLI.

The packaged company profile is the only connection source. Old remembered device settings and `HRIS_SUPABASE_*` environment variables are ignored. No connection editing or company switcher appears in the application. An invalid profile, or a missing profile in an installed app, blocks startup instead of opening demo or selecting another database. For local development, place a private profile beside the development executable; omit it only when intentionally using the separate demo build.

Passwords and access/refresh tokens stay in memory. Every company launch requires HRIS account sign-in. Signing out clears company records and returns to sign-in, attempts server-side session revocation, and discards local tokens even when the network is unavailable. It never opens demo mode in a company installation.

Personal details and display preferences use the signed-in account's editable Auth metadata; they never determine permissions. Access comes from protected company membership rows, with the original owner retained as an active Administrator. Photos are normalized from a PNG/JPEG up to 5 MiB and 4096 × 4096 pixels into a centered 256 × 256 PNG, stripping original photo metadata. Rendering applies a circular mask. The private `profile-photos` bucket allows each active member to read/write only their own fixed `<user-id>/avatar.png`, with a 2 MiB PNG limit. Photos are retrieved using the user's bearer token and kept in memory rather than a local photo cache. Do not use public photo URLs or administrative keys.

Password changes require the current password, matching confirmation and 12–128 characters for the new password. Email changes also verify the current password and remain pending until Supabase's email confirmation completes. Email delivery and confirmation follow the project's Auth/SMTP configuration; the app does not bypass confirmation or send administrative updates. Other/global sign-out revokes refresh sessions; already issued access tokens can remain valid until expiry. The app does not claim to enumerate other devices. Demo personal/photo/display changes remain in memory and real account-security controls are disabled.

The desktop client sends the publishable key in `apikey` and the signed-in user's access token in `Authorization: Bearer`. Row-level security enforces the current member's company, role and employee/report relationships on every HR request. Immutable owner/company/employee identifiers are protected by column grants. Anonymous HR access and record deletion are denied. Only Administrators can provision accounts; company creation and deletion remain deployment operations. Access is refreshed when loading data and every 60 seconds while idle; a revoked membership clears the company view.

## Prototype boundaries

- One company per dedicated database. Company setup is an operator action, not an app sign-up option. Four fixed roles are available; Employee is portal-only and the portal UI is not built yet. Custom roles, multi-stage approval chains and a general HR audit trail remain future work. Managers use the desktop to review direct reports' leave in one step; nobody can approve their own leave.
- Leave uses Monday–Friday workdays. Holidays, different workweeks, carryover, accrual, partial days, cancellations, and country-specific statutory rules are not implemented. The sample 15-day allowance is configurable sample policy, not a legal entitlement claim. Requests spanning calendar years must be split.
- Attendance supports one shift per employee per company date. Elapsed time includes breaks; payroll, overtime, schedule rules, and manual time corrections are outside this version. The visible Biometric integration card is deliberately disabled.
- Live mode requires internet access; there is no offline queue or local cache of employee data. Use Refresh to load changes from another device. Errors remain visible, and the app never silently falls back to demo after a failed live request.
- UI and network requests run on separate threads. A failed write/refresh may require a manual Refresh before retrying; duplicate and state-transition rules protect against repeated submissions.
- The private Windows setup is unsigned. Automatic updates, automatic database deployment, and production readiness certification are not implemented.

Company identity, time zone, and allowances are stored as configuration/data, separate from the UI and core rules. A later company rollout can extend these policies without adding an administrative key to the executable. See [the staged HRIS roadmap](docs/ROADMAP.md) for the remaining employee, attendance, leave, payroll, recruitment, development and reporting modules.

## Verification

Rust workflow and HTTP contract tests:

```powershell
cargo fmt --all -- --check
cargo test --locked
```

Local PostgreSQL migration/RLS tests (Node.js 22+):

```powershell
cd tools/db-tests
npm ci --ignore-scripts --no-audit --no-fund
npm test
```

Account provisioning handler tests:

```powershell
node --test tests/account_admin_edge.test.mjs
```

The database suite executes all migrations using PGlite/PostgreSQL, including `btree_gist`, and simulates Supabase's Auth identities, API roles and managed Storage metadata contract; it does not run an S3 service. It covers role scopes, direct-report reviews, disabled accounts, owner protections, access history and metadata tampering. Node tests exercise the provisioning handler with controlled Auth/REST responses. Rust HTTP fixtures cover Auth/REST, profile persistence, private photos, wrong-password rejection, pending email changes, access revocation and session cleanup. Live Profile/security checks passed on 0.1.3; temporary data was removed and the original account restored. Consult `VERIFICATION.md` for the current version's precise local, hosted and native UI evidence. Remote GitHub CI has not been run.

The hosted Account Manager migration and provisioning function were deployed and verified on the dedicated prototype. `tools/db-tests/hosted-accounts-smoke.sql` is a rollback-only role test guarded for one initialized company and no existing HR records; run it only as the trusted project operator. Actual signup confirmation, HR restrictions, disabled-token/photo access, Manager desktop admission and Employee desktop denial passed. Temporary accounts/records were removed. The earlier native HR/Profile checks remain historical evidence; see `VERIFICATION.md` for each version's exact scope and limits.

For native UI screenshots during development:

```powershell
cargo build --locked --features qa-screenshot
$env:HRIS_SCREENSHOT_TO = 'C:\absolute\path\hris-preview.png'
./target/debug/hris-desktop.exe
```

`HRIS_PREVIEW_PAGE` can select `employees`, `attendance`, `leave`, `accounts`, `settings` or `profile` in this QA build. `HRIS_PREVIEW_FORM` can open `employee`, `leave` or `account`. `HRIS_PREVIEW_ROLE` can preview `hr_generalist`, `manager` or `employee` with fictional role-scoped records. `HRIS_PREVIEW_PROFILE=security` or `sessions` selects a Profile section. `HRIS_PREVIEW_AVATAR` can load a synthetic local QA photo into the visibly labeled demo without authenticating. `HRIS_PREVIEW_SIZE=compact` uses the minimum window size, `HRIS_PREVIEW_SCALE=125` checks enlarged display, and `HRIS_PREVIEW_SCROLL=bottom` captures lower content. `HRIS_PREVIEW_COMPANY_NAME` can override the fictional demo name. These environment controls are excluded from a normal production build.

Implementation references: [eframe native applications](https://docs.rs/eframe/0.37.0/eframe/), [Supabase row-level security](https://supabase.com/docs/guides/database/postgres/row-level-security), [Supabase publishable API keys](https://supabase.com/docs/guides/getting-started/api-keys), [Supabase password authentication](https://supabase.com/docs/guides/auth/passwords), and [PGlite extensions](https://pglite.dev/extensions/).
