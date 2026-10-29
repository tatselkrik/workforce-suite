# HR leadership — HRIS0.1.69 delivery

**Installed and privately published on29October2026.** Open **Reports → Leadership** as Administrator. The authorized Payroll separation, employee-relations, employee-services, expanded-benefits and leadership queue is complete; its usage-reset continuation automation was deleted on completion. Older active-queue entries are historical.

## Delivered behavior

Current active/archived workforce, department/location/employment-type groups and record-completeness counts are separate from selected-period observations. Monthly rows retain clipped joining dates, finalized exit last-working dates, recorded completed/open attendance and saved approved/pending leave charges, including half-days. Hiring/transition and combined Learning & Performance workload counts use current saved states with explicit date bases.

The Administrator-only aggregate projection stays separate from Snapshot, Payroll and portal. No raw employee/applicant identities, private notes, confidential cases, services, benefits/dependents or salary/tax/payment facts enter it. Fresh CSV/native XLSX exports retain definitions/time zones and require current access before file IO. Existing files are never overwritten. Current groups are not historical headcount; archived status is not separation; missing attendance is not absence. See [contract](HR-LEADERSHIP.md) and [checklist](HR-LEADERSHIP-0.1.69-CHECKLIST.md).

## Exact release and installation

- Frozen source: `6e58313d73cd7275927dc795aede234a17884cbf`; initial full native/model/adapter/SQL verification source `1afd8b65b2116496168f908df39639a34d107bda`. Only the leadership UI layout/test file changed afterward.
- Installed executable SHA256: `647b81577180d8b6aae86df2cba126a1e242b24b4611a9ff4d6475a7c29c3f37`.
- Signed update SHA256: `86e4a4fe2fe8430751d4f94546c6be3d0deb7bcb9720fd6eac271c66f92344f2`.
- Frozen source archive SHA256: `5ed1b42255d5d2d1ea873a51c8fb012558a1f50079fd0e5e89c5112737c3b065`.
- Exact installer and failure-recovery opt-in pass. Existing Ed25519 identity, signature and current Rust verifier match.
- Actual installed executable/BUILD/version/source, Windows Apps registration, responsive configured launch and normal close pass. Installation completed `2026-10-29T04:06:42.9298456Z`. Company connection, branding, keys, unknown files and unsaved work were preserved;0.1.68 rollback is retained.
- Authenticated private-channel download size/hash/signature, immutable source/migration subset, newer-release preservation and ordinary-Employee/public denial pass. Latest staff feed is0.1.69. No public release/CI or Authenticode claim.
- Separate Payroll0.1.0 executable remains hash-identical; local portal0.1.8 and its existing sessions/drafts were not restarted or changed.

## Verification and preservation

868 default Rust checks pass across68suites, with37 intentional opt-ins ignored at the initial source. Two current UI checks pass, including nine width/scale geometry scenarios;14 current fictional native views were actually reviewed, covering light/dark, compact125percent, groups/months/hiring/bottom, empty state and non-Admin visibility. Only a UI file differs from the full verified source; protected models, adapters, SQL and worker are unchanged.

The complete20-suite local PostgreSQL run passes16645 assertions and fresh92-migration replay, including80 focused leadership checks. Three SQL projections decode/export through Rust. Coverage includes leap-year/clipped dates, unknown/future joins, inactive option labels, current half-day leave states, malformed saved charges, source/group budgets, retained archived history, nonzero hiring/development workflows, read-only execution and revocation.

Additive migration92 (`20261029033424_hr_leadership_reporting.sql`, SHA256`af5b5fc49587a99cd9b71a495c662e127d6d133e26c6d5f95a6d6e65f1f54b87`) is deployed.137 prior record sets and469 original functions/grants, Auth facts and runtime-ready TRUE are preserved. It adds only two read functions and never changes financial rules or company records.

Actual hosted native Admin CSV/XLSX and eight protected role combinations pass. HR, Manager, Recruiter, Learning & Performance, Payroll Officer, all combined non-Admin roles, Payroll product, forged metadata, existing-token role removal, disabled Admin and anonymous access are denied. Current totals agree with the database. Two fictional verification actors were disabled/archived and their sessions cleared; prior company facts/Auth remain unchanged.

Advisors report no errors or new leadership finding. The existing Auth leaked-password-protection warning and42 intentionally private RLS-with-no-policy INFO findings remain. Existing performance findings are25 unindexed-FK INFO,148 unused-index INFO and8 permissive-policy WARN; no table/policy/index was changed in this increment. Auth settings/billing were not changed.

## Evidence and limits

Ignored evidence: `work/leadership-rust-default-results.json`, `leadership-db-full-results.json`, `leadership-current-ui-proof.json`, `leadership-visual-review.json`, `leadership-migration-results.json`, `leadership-hosted-results.json`, `leadership-source-precheck.json`, `leadership-normal-startup-results.json`, `leadership-installer-test.log`, `leadership069-signing-results.json`, `install-leadership069-actual/installation-results.json`, `leadership069-publication-results.json` and `company-deployment-0.1.69/`.

Actual installed signed-in button interactions and subjective readability on Kirk's screen were not claimed; native authenticated adapters and fictional native renders were verified separately. Agency acceptance, actual phone/email confirmation, external providers/delivery and physical devices remain deferred. No paid service, billing change, filing, transfer, public portal hosting or external employee communication occurred. This release does not resume deferred work.
