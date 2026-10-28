# Employee services —0.1.67 delivery

HRIS0.1.67 is installed and verified on29October2026. The signed updater is
published and downloaded/verified through the existing private HRIS channel.
Separate local portal0.1.8 is packaged and its exact executable/embedded assets,
IPv6 loopback startup, branding and unauthenticated endpoint denial are verified.
Frozen source: `f4183822a3b2eee1e1ecc374214c8b2a0e5db94a`.

Open **Employee services > Load requests** in HRIS. HR/Admin route the company
queue; other staff see own and specifically assigned work. Requests retain shared
messages, handler/due dates, status changes, reasoned cancellations and history.
The portal **Services** tab always shows only the signed-in person's requests,
including when that person also holds a staff role. Confidential cases stay under
Employees > Employee relations. Requests do not edit canonical personnel or pay.
Use [the testing checklist](EMPLOYEE-SERVICES-0.1.67-CHECKLIST.md).

The separately delivered portal package is `work/portal-deployment-0.1.8/HRIS-Employee-Portal`.
Its Run-Portal.cmd uses `http://[::1]:3033` on this PC only, a distinct cookie host
from the preserved localhost/127.0.0.1 portals. Save drafts before changing portal
versions; old sessions/processes were not restarted or discarded. No public/LAN
hosting, tunnel, actual email confirmation or physical-phone claim is made.

Verification:827 default native checks/33 intentional opt-ins passed at
`c0b1ffda39234a091d3143db0e4ec030ef580433`; the later two-file portal JS/test fix
leaves every native production input unchanged.53 portal Rust checks/1 opt-in,
24 JS checks, full database workflow/fresh90migration replay and134 focused
service assertions pass.15 fictional native layouts and actual browser fixture
interactions at390/1440px were visually reviewed, including scrolled lower controls,
create/reply/cancel, draft guards and corrected company-local timestamps.

Hosted native/direct14-role, exact retries, assigned progress, reroute/revocation,
forged metadata and actual authenticated portal BFF own-only workflow pass.
Completed role checks were explicitly reused for the portal-only continuation.
Three failed QA harness attempts are preserved: strict fixture field rejection,
nonembedded version-literal assertion, and reading employment from the CSRF-only
sign-in response. Corrections follow existing product contracts; no permission or
validation guard was weakened. Retained synthetic histories remain visible to HR;
managed test accounts are disabled/archived/session-free.

Additive migration90 preserves133 previous public/private record sets,445 existing
function definitions/grants, prior Auth and enabled role runtime. Later hosted
verification preserves135 prior record sets, including the new retained history.
Security API:0 ERROR,1 unchanged Auth password-protection WARN and40 INFO RLS
notices; private tables deny direct clients. Auth settings remain unchanged.

Exact real silent installer, company/key/logo/unknown-file preservation and
failed-installer recovery pass. Actual installed executable/BUILD/source revision,
Windows Apps registration and responsive normal launch/close match. No unsaved
work was discarded. Payroll Desktop0.1.0 remains hash-identical. HRIS066 rollback
is retained. Frozen source/privacy, Ed25519 signature, remote schema subset,
authenticated download size/hash/signature and public/Employee denial pass.
No public CI or Authenticode claim; no paid service, financial rule change,
external employee communication, transfer or government filing occurred.

Next authorized work: benefits administration, then HR leadership reporting.
The same reset scheduler remains ACTIVE.
