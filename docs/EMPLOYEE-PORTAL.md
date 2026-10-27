# Separate employee portal

**27October2026 browser review:** actual Edge/Playwright checks now cover desktop,
390px/320px widths, dark mode, normal synthetic sign-in, profile/contact/photo,
session controls and sign-out. Two mobile issues remain:320px navigation clips
My profile, and selected-tab hover loses contrast. The previous blanket visual
deferral is superseded by [this bounded review](PORTAL-BROWSER-20261027-REVIEW.md),
not a full passing portal claim. Physical-phone/email confirmation and populated
slip/request/notification views retain their explicit unverified status.

Current verification scope25October2026: browser verification first, using
existing no-charge tools. Mobile-width browser layouts may be checked for free;
physical-phone and actual email-confirmation verification can remain deferred
without blocking browser-only completion. Preserve Auth confirmation rules.
Notifications appear inside the own portal inbox for now; external company
email and background browser/OS push delivery are deferred. Existing desktop
staff alerts continue. This scope change does not claim unperformed visual or
confirmation checks have passed, and the portal remains loopback-only.

Local package0.1.6 adds own payroll/appraisal/learning notices using the compatible
v2 inbox. Monthly statements open My payslips; other own notices are informational
until their record views exist here.48Rust/15JS checks, optimized hosted own-scope/
CSRF/read/revocation and packaged loopback startup pass. The running0.1.5 server
and drafts were preserved. Current visuals/real email confirmation remain
unverified. See [notification evidence](NOTIFICATIONS-0.1.46-VERIFICATION.md).
No public hosting, paid provider, billing change or external message was added.

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
- Own missing/incorrect attendance proposals for a selected past/current date,
  with company-local timestamps, saved parent revision and required reason.
  Recorded punches change only after another authorized desktop reviewer approves.
- Own overtime requests within the protected saved-rule candidate, current
  parent/break revisions, retained partial approvals and current approved minutes.
  Changed attendance/break/leave facts withhold current authorization until reviewed.
- Own pending-request cancellation and submission/decision history. The portal
  exposes no reviewer controls or team records, including for staff accounts.
- Personal notification inbox with unread count,25-item pages, unread-only
  filtering, saved read/unread markers, mark-shown and reset-read-history controls.
  Own leave decisions/upcoming approved leave, correction/overtime decisions and
  earlier open-shift reminders link to refreshed own records. Decision updates
  cover30company-local dates; upcoming leave covers today through the next7days.
  Background refresh runs while the page is visible and no draft/action is open;
  polling does not extend the30-minute idle session. Read status never approves
  a request. Staff tasks and private document reminders remain outside this inbox.
- Own circular photo preview, Save, Discard and Remove, shared with the existing
  native profile-photo record. The sidebar photo opens My profile. PNG/JPEG input
  is limited to5MiB/4096pixels, orientation is normalized and metadata stripped
  into a256square PNG. The server chooses the current user's private object path.
  Photo drafts block navigation/refresh and never discard unsaved contact edits.
- Transparent company logo/name on sign-in and in the company header. Only
  deployment-bound company branding may remain in server memory after sign-out;
  private profile-photo URLs/initials clear. Optional picture errors leave own
  records available. Private Storage downloads use fresh-origin requests so
  replacement/removal does not show a stale CDN image.
- Own account status, current-password verification, password change with matching
  confirmation, pending sign-in email changes and sign-out-other-sessions controls
  in My profile. Contact email remains separate. Employment and role fields stay
  read-only. Passwords are cleared after each attempted save and when signing out;
  contact/photo drafts are preserved independently.

Staff who use the portal receive the same own-record interface; their broader
desktop role does not cause team records or approval/admin controls to load here.
Employee desktop admission remains prohibited.

Account controls are implemented in the0.1.5 increment. Actual email delivery/
confirmation and current picture/notification/security-screen visual verification
remain pending, so milestone1 is partial.
Kirk resumed remaining roadmap work and the usage-reset scheduler25October2026,
after the0.1.3 pause and completed project storage cleanup.
Public HTTPS deployment and physical phone/network checks remain
separate from localhost development. The current prototype has no public URL.

## Operator startup

Email confirmation is separate from sign-in. Requests use S256 PKCE so the
provider does not redirect access/refresh tokens into the browser. The portal
deliberately discards that verifier and never exchanges the returned code or
creates a session from a callback. Confirm all required email links, then return
to the original portal and refresh account status, or sign in with the confirmed
email and current password. A callback only redirects to the main page and
ignores its untrusted parameters. Restarting or signing out is never undone by
opening a confirmation link.

The company's Auth mail service must permit delivery, and its redirect allowlist
should include the operator's exact local origin plus `/account/confirm-email`.
If the provider falls back to its default site URL, return manually to the
original portal after confirming. No mail provider, template, confirmation rule
or redirect allowlist is silently changed by this release. SSO/MFA enrollment,
forgotten-password recovery and a device/session inventory are separate scope.

These are [Supabase user-update](https://supabase.com/docs/reference/javascript/auth-updateuser)
and [PKCE email-confirmation](https://github.com/supabase/auth/blob/master/internal/api/verify.go)
flows. A rejected or uncertain save is shown explicitly; refresh account status
before retrying. Other portal cookies on this server are removed immediately by
the session control; other clients' issued access tokens retain their service
expiry behavior. Ten password-verification attempts per five minutes do not
prevent reading one's already authenticated records.

The operator supplies the same private two-field company connection profile used
by the desktop. Users never see connection, company selection or database setup.
The server uses only a publishable key and the signed-in user's access token;
neither a service-role key nor another person's Auth token is required.

The private package also includes an optional `hris-branding.json` next to the
connection profile. It contains only a profile fingerprint, company ID/name and
optional normalized logo. The server rejects a mismatched or malformed branding
file. Generic source archives exclude it. An unbranded source build learns the
current company's name/logo after an authenticated refresh and keeps only those
company assets in memory for sign-in; employee photos are never part of that cache.

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

The0.1.5 account-security increment passes47Rust tests and13JavaScript code tests.
Nine hosted proof groups exercise normal synthetic Employee password change,
old-password rejection/new-password login, password restoration, unchanged opaque
cookie, other-own-cookie revocation with another Manager preserved, projected
own account status, forged-target/CSRF/current-password guards, disabled-membership
denial and unchanged preexisting Auth users. Synthetic accounts remain disabled/
archived with sessions removed. No real person's password or email changed.

The hosted email-change test request was rejected by the configured Auth service;
mail delivery and both-address confirmation are UNVERIFIED. Pending-state/S256
request/callback behavior passes local HTTP tests. Those fixtures do not establish
actual mail delivery. The callback cannot establish any login, including after
sign-out. There is no schema change; the database remains at56versions and its
suite was not rerun. Browser/native UI tools still fail before initialization;
current account/picture/notification/phone visuals remain unverified. Milestone1
stays partial while independent authorized roadmap work continues.

The0.1.4 picture increment passes38Rust tests and9JavaScript code-level checks.
Nine actual hosted Auth/Storage proof groups against the optimized loopback
executable cover branded startup metadata, own normalized uploads, EXIF/text
metadata removal, CSRF/format/scope guards, foreign-account read/write denial,
immediate removal visibility, separate Manager photo/removal, sign-out privacy,
disabled membership and preservation of preexisting objects (excluding read-access
timestamps). The hosted company currently has no logo; its absence/branding
metadata were verified live, while actual logo pixels/transparency/cache behavior
use local HTTP/image fixtures. The current screen/phone layout remains unverified
because the browser runtime still fails kernel-assets initialization.

Two early live runs caught a real stale-image response after successful deletion.
A guarded read showed both synthetic Manager objects had been deleted. Fresh
`cacheNonce` origin requests fixed the mismatch; the complete subsequent run
passed. This follows [Supabase cache-bypass guidance](https://supabase.com/docs/guides/storage/cdn/smart-cdn#bypassing-cache).
Synthetic accounts remain disabled/archived with sessions removed and history
retained. No actual password changed, database migration, native desktop release
or existing portal restart was performed. The schema remains at56versions; the
database suite was not rerun for this client-only change. Compact build settings
avoid accumulating full debug-symbol/incremental variants after storage cleanup.

The0.1.3 notification increment passes32Rust tests,5JavaScript code-level tests
and3,414local PostgreSQL assertions. Eight actual hosted Auth/API proof groups
against the optimized loopback executable cover own redacted reminders, independent
correction/OT decisions, approved upcoming leave, pagination, persistent read/
unread/reset and concurrent marker updates, CSRF/field/foreign-record guards,
staff own-only scope and immediate disabled-membership denial. Synthetic accounts
remain disabled/archived with sessions removed and their history retained. The
first live harness used the wrong legacy-RPC parameter name; the corrected full
run passed. No actual account password changed.

Additive migration`20261024160758` is deployed (56versions), with85oldpublicrecord
sets and235existing functions unchanged. New markers have RLS, public RPCs remain
invokers, and private helper grants are restricted. No new advisor warnings/errors
or unindexed foreign keys appeared;3olderwarnings and5olderFKINFO remain.

Current notification-screen and phone-width visual checks are UNVERIFIED: both
browser and native UI runtimes fail kernel-assets initialization with a Windows
path error. The JavaScript tests use a small code-level DOM fixture, not a browser
render. Package metadata records this limitation. Earlier screenshots below apply
only to their original increments. This is a local prototype increment, not proof
of public hosting, remoteCI, browser printing, a physical phone or Authenticode.
UTF-8 punctuation is corrected in this source; delivered0.1.2 archives stay intact.

The0.1.2 increment passes25 Rust portal tests, one CSV safety test, JavaScript
syntax checks and3,275 local PostgreSQL assertions. Seven actual hosted Auth/API
proof groups exercise own context, forged-field/CSRF/revision guards, no premature
punch write, independent Manager correction and partial overtime approval, retained
missing-day cancellation/history, own-only staff scope and revoked-account cookies.
Synthetic histories remain with accounts disabled/archived and sessions removed.
Earlier live harness attempts stopped on a plain-text422 response and a reused
retained fixture shift name; corrected complete verification passed.

The additive read-only context migration `20261024133750` is deployed (55versions),
with all85 prior public record sets preserved and existing writers unchanged.
There are no new advisor warnings/errors or unindexed foreign keys; three earlier
warnings and five earlier FK INFO remain. Company-local input rejects nonexistent/
ambiguous DST times. Desktop browser inspection verified form alignment, retained
own history, draft navigation blocking and discard restoring controls. The new
phone-width check remains pending because the browser runtime failed to restart;
earlier phone-layout evidence below applies to earlier increments only.

This portal release does not change the installed desktop0.1.41. Its normal
executable excludes the fictional preview route and fixtures. Exact local package,
startup and file-hash evidence are recorded with the private versioned artifacts.

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
