# Focused workspaces0.1.64 — delivered28October2026

0.1.64 is installed on Kirk's PC and published to the existing private HRIS update
channel. The exact downloaded updater verifies against the existing Ed25519 key.
The installed executable, BUILD source/version, Windows Apps registration and normal
launch/close match. Company connection, logo, verification key and unrelated files
are preserved; the previous0.1.63 files are backed up. No open draft was discarded.
Work and the SAME usage-reset scheduler are paused after this bounded milestone.

## What changed

Role-focused navigation groups Workspaces, My work and authorized Administration.
Recruiters enter Hiring, Learning & Performance Officers enter Employees > Development,
and Payroll Officers enter Payroll. Combined responsibilities retain their union.
HR/Managers keep their scoped Employees workspace. Goals, Appraisals, Training,
Certifications, Career plans and authorized Review cycles remain together.

Administrators use Settings > Workspaces to show/hide six optional sections:
Organization chart, Development, Hiring, Exit & clearance, Payroll & payslips and
Reports. All are shown by default. Core navigation and authorized administration
remain. A hidden section retains its records, protected permissions, portal access
and open drafts; after finishing/cancelling a draft navigation returns safely.
Current-company revision/actor/full-payload receipts protect concurrent saves and
retries. The private immutable audit is Administrator-controlled. Visibility never
replaces the existing server permission checks. Ordinary Employees remain portal-only.

Payroll remains inside HRIS. No separate app, new HR-function module, dependency,
rate rule, paid service, hardware or portal change was introduced.

## Verification

- 772 normal Rust checks pass;30 intentional opt-ins remain excluded. The explicit
  hosted native save/replay,14role snapshot and real-installer/recovery checks pass.
- 15,734 PostgreSQL assertions and a fresh88migration replay pass;85Edge tests pass.
- Thirteen fictional native layouts were visually inspected, including specialist/
  combined roles, hidden sections, ordinary/compact/dark/125percent and selected
  sidebar visibility. These are separate from actual installed-user interactions.
- Migration88 is deployed.115prior public table sets and270priorAuth users survived
  deployment unchanged. Hosted actor/revision/idempotency, read/write scopes and
  fourteen role combinations pass with the role runtime enabled throughout.
- The initial hosted script stopped at an unspecified metadata/revocation assertion.
  A focused retest passes metadata update, forged-Administrator write denial and
  disabled existing-session denial, plus prior-record/Auth preservation. Original
  optional visibility was restored. Synthetic accounts remain disabled/archived,
  sessions ended; no existing account's role was changed for testing.
- Frozen source/archive privacy, normal release exclusion of QA hooks and configured
  startup pass. Exact silent installation, company/unknown-file preservation and
  isolated failed-installer recovery pass. Actual installed normal launch/close,
  BUILD, executable hash and Windows Apps version pass.
- Private publication, all88required migrations, authenticated exact size/hash/
  signature download, current staff feed and Employee/public denial pass. Existing
  releases remain immutable. Capacity preflight was about33.2MiBdatabase/290.5MiB
  storage before this small update; no billing/plan change was made.

Actual installed-user clicks, subjective layout, a real unsaved draft during a
concurrent visibility change and manual sidebar-wheel interaction remain for Kirk's
checklist. No publicCI or Authenticode success is claimed. Advisor output retains
35expected RLS/no-policy INFO findings (including two new private deny-by-default
workspace tables) and the prior leaked-password-protection WARN. Private tables
have no client grants and protected wrappers remain enforced; no broad policy or
paid setting was enabled. See [advisor guidance](https://supabase.com/docs/guides/database/database-linter?lint=0008_rls_enabled_no_policy)
and [existing password-protection warning](https://supabase.com/docs/guides/auth/password-security#password-strength-and-leaked-password-protection).

## Exact delivery

- Frozen source: `066202ac3bca1f5ae00c01d3c8425bd203571c85`.
- Required migration: `20261028122502_desktop_workspace_visibility.sql` (88total).
- Installed executable SHA256: `dc9b221fdaf54dbac19c2fac349840eb55823cf1081fbbe752b483ddff99ef8a`.
- Signed updater SHA256: `0a17add20cc5ea75afbe604dd044433ab9e69f687dbd5d26075bbcd97bc6804c`.
- Full setup SHA256: `42e2d809fb34a5509d153ddcb364fa0ad0c2738c9ae953a8bcced75e3ce32a94`.
- SourceZIP SHA256: `21ff5a56170c176f5f1fc0f7e5fcb1de75f0533806cb95d85fbada687c54bd4c`.

The private work evidence is retained with the0.1.64 package, deployment, source
privacy, visuals, hosted role/security, signing, installation and channel records.
Do not repeat these operations. Use [the testing checklist](WORKSPACES-0.1.64-CHECKLIST.md).
Payroll separation and later HR-function milestones remain queued until resumed.
