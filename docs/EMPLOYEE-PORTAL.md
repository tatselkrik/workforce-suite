# Separate employee portal

The native desktop remains the staff workspace. `employee-portal/` is a separate
Rust/Axum service and responsive browser interface, with its own version and
application identifier `io.github.tatselkrik.hrisportal`.

## Implemented portal controls

- Company account sign-in, including Enter in the form, and local sign-out.
- Read-only own employee code, name, department, title, join date, employment type,
  work location, reporting manager and annual policy allowance. The allowance is
  not presented as a remaining leave balance.
- Own contact/emergency updates with Save and Discard. Employment and sign-in
  email cannot change through these fields. Unsaved forms block navigation.
- Own server-timed check-in/check-out, recent 93 recorded work dates, any open
  shift and a date picker. Missing records do not automatically establish absence.
- Start/end the open shift's saved break rules using server time and current
  attendance/break revisions. Checkout stays blocked during an active break.
  Actual completed unpaid breaks, rather than an assumed break, affect work time.
- Own leave types and balances as of the selected start date, company workweeks
  and holidays, full/half-day charge estimates, filing and cancellation with a
  reason. Saved charges and approval receipts remain visible. The server guards
  policy/record revisions, overlap, ownership and already-started approved leave;
  contact HR to review a cancellation after approved leave has begun.
- Own issued payslips, approved amounts, component descriptions, UTF-8 CSV and
  browser printing/Save as PDF. This portal does not calculate or change payroll.
  Payment remains separate from an approved calculation.

Staff who use the portal receive the same own-record interface; their broader
desktop role does not cause team records or approval/admin controls to load here.
Employee desktop admission remains prohibited.

Next increments add attendance-correction and overtime
requests, notifications, company/profile pictures and account password/email
controls. Public HTTPS deployment and physical phone/network checks remain
separate from localhost development. The current prototype has no public URL.

## Operator startup

The operator supplies the same private two-field company connection profile used
by the desktop. Users never see connection, company selection or database setup.
The server uses only a publishable key and the signed-in user's access token;
neither a service-role key nor another person's Auth token is required.

```powershell
cargo test --manifest-path employee-portal/Cargo.toml --locked
cargo run --manifest-path employee-portal/Cargo.toml --locked -- C:\private\hris-connection.json
```

The default origin/listener is `http://127.0.0.1:3030`. Open that address locally.
For deployment, supply the externally visible HTTPS origin and a listener as the
second/third arguments, for example `https://hr.example.invalid 127.0.0.1:3030`.
An operator-managed TLS reverse proxy must preserve the public Host header, disable
response caching and route only this dedicated company deployment. Plain HTTP is
accepted only for a loopback origin with a loopback listener. Do not expose a
loopback-development configuration to a LAN or public network.

The native desktop and portal have independent release lifecycles. Building this
crate does not change the installed desktop, its identifier or updater artifacts.
Dependencies/lockfiles are separate. Axum0.8.9 and Tokio1.54.1 were checked against
their official documentation when the portal was created; the new crate avoids
the yanked Tokio1.54.0 rather than changing the released desktop's lockfile.

## Authentication and privacy

Opaque random session IDs live in HttpOnly, SameSite=Strict, host-scoped cookies;
HTTPS uses the `__Host-` cookie prefix and Secure. Supabase access/refresh tokens
remain only in the Rust process. CSRF tokens stay in the page's memory. The
application does not persist credentials, sessions or automatic employee-record
caches in browser storage or local files. CSV/PDF exports are private files the
person explicitly chooses to save.
Closing/restarting the server loses its in-memory sessions. This prototype uses a
single server instance; replicas would need a separately designed session store.

Live Auth user and protected membership are checked before data requests, with
company/employee identity consistency and explicit own-record filters even for
staff roles. Responses validate returned identities and project allowed fields.
Private salary profiles, HR notes, source bundles and consent references are not
portal responses. There is no arbitrary table/RPC proxy or administration route.

Mutations require the configured Origin and session CSRF token. Hosts are checked;
responses use no-store/private, restrictive CSP, no framing and no referrer.
Supabase redirects are disabled and upstream errors are replaced with bounded,
non-sensitive messages. Sign-in has a bounded per-account attempt window and
concurrency limit. The prototype caps active sessions at512, serializes refresh
per session and ends local sessions after30minutes of inactivity. Logout attempts
server-side local revocation. Previously issued upstream access tokens follow
Supabase's expiry/revocation rules; current disabled membership still denies data.

Protected APIs and RLS remain authoritative. Version0.1.0 needed no migration;
the0.1.1 source adds own-only leave context/submit/cancel wrappers in
`20261023103105_employee_portal_leave.sql`. Public wrappers use security invoker
and call protected routines; current employee identity is inferred server-side.
The installed desktop's existing leave and break routines remain compatible.

## Verification

The Rust suite exercises request origin/host restrictions, HttpOnly/Secure cookie
settings, CSRF, own-contact identity binding, actual local HTTP Auth/REST fixtures,
Manager own-record projections, refresh, disabled memberships, foreign-row
rejection, redaction and sign-out. Local HTTP fixtures are fictional, not live Auth.

The ignored `visual_preview_server` test serves an isolated fictional loopback
preview on3031; POST `/qa/stop` ends it. It is absent from the normal server binary.
Browser checks cover Enter sign-in, desktop/390px phone layouts, aligned profile
and payslip rows, dirty-form navigation, discard, CSV download and sign-out.
The0.1.1 fictional phone preview also verifies half-day estimates, retained
cancelled requests and checkout disabling/re-enabling around a break. Seventeen
Rust tests and1,891 database assertions pass. The function-only hosted migration
preserved all67 existing public record sets. These checks do not by themselves
establish an actual Employee Auth start/end-break test.
See the task's separate live verification record for hosted Auth and data checks;
do not infer them from mocked tests or screenshots. Browser printing, physical
phones, public TLS/hosting and multi-instance operation are not established by
the localhost preview.

The first hosted check used actual Employee Auth through the Rust server: contact
updates preserved employment, supplied foreign employee IDs were rejected/ignored,
server timestamps controlled check-in/out, only the person's issued slip appeared,
and bad origin/CSRF, administration paths and disabled membership were denied.
Local sign-out cleared access. Twelve unrelated core record sets matched; synthetic
employees were archived/access disabled afterward and verification sessions ended.
No real account password changed. Ten boolean proofs were recorded privately.

The0.1.1 hosted check adds eight boolean proofs: own policy/balance projection,
actual half-day filing, cancellation with retained visibility, stale/duplicate/
CSRF guards, foreign break-parent denial, logout/disabled access, twelve unrelated
core sets preserved and synthetic cleanup. It caught a missing existing
`x-hris-leave-contract: 2` header; the portal now opts into that read contract so
cancelled/custom leave remains visible without changing legacy-client RLS.
Actual hosted break start/end, browser printing and public deployment remain
outside these eight proofs.

Sources: [Axum](https://docs.rs/axum/0.8.9/axum/),
[Tokio1.54.1 release](https://github.com/tokio-rs/tokio/releases/tag/tokio-1.54.1),
[Supabase server-side Auth](https://supabase.com/docs/guides/auth/server-side/creating-a-client)
and [server-checked user/session validity](https://supabase.com/docs/guides/troubleshooting/should-i-set-a-shorter-max-age-parameter-on-the-cookies-8sbF4V).
