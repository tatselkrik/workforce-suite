# Workforce Suite

This is the public source repository for Workforce HR, Workforce Payroll and
Workforce Employee for Android. Private company deployments stay local, outside
Git. Do not create a second remote for private files or publish configured builds.

## Local operator context

Before substantive work, read `memory.md` if present as historical context. In an
operator checkout, also read `deployment-private/OPERATOR-AGENTS.md` if present;
it preserves deployment-specific rules and exact-target authorities. Those local
files are ignored and must never be committed. Current user instructions override
historical notes. Keep short task checkpoints under ignored `work/` when enabled.

## Product boundaries

- HR and Payroll are separate native Rust desktop applications sharing reviewed
  financial/domain code. Do not add a web frontend or embedded database to them.
- Android is the employee-facing application. `employee-portal/` contains its
  shared own-record Rust library. Keep `autobins = false` and expose no standalone
  server launcher.
- Preserve deployed application IDs, signing identities, legacy install/data
  paths and protected membership/RLS contracts. Company profiles are supplied by
  the operator, never selected by an employee inside an app.
- Employee Android access is own-only, including staff accounts. Native admission
  excludes ordinary Employees. Payroll administration requires its protected
  responsibility and independent financial approval. Never authorize from
  user-editable Auth metadata or merely hidden controls.
- Preserve original employment/approval snapshots, issued payslips, receipts,
  confidential cases, scoped attachments and immutable history. Consult the
  relevant feature contract in `docs/` before changing behavior.
- Keep tokens/passwords/session state in memory. Android serves bundled content
  from the exact bound device-loopback origin; never expose that service to a LAN
  or public host.

## Changes and verification

Use pinned lockfiles and the existing toolchain. Keep unrelated changes intact.
Use `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0` for compact local checks, with one target per app.
Specify UTF-8 for Python text files and tool/API stdout. Test meaningful behavior
and permission boundaries; report actual checks separately from unrun/physical UI
verification. Do not change payroll rules without current official sources.

Database changes need checked-in migrations and a resolved authorized target.
Public source conveys no authority to write to somebody else's infrastructure.
Never infer permission for transfers, filings, providers, purchases or billing.

## Public Git and releases

The single remote repository is `workforce-suite`. Verified source changes are
committed and pushed there. Run `node tools/public-source-check.mjs --history`
before publication; review screenshots and relevant history as well as current
files. Profiles, Firebase configuration, secrets, signing keys, company files,
configured installers/APKs and detailed vulnerability reports stay ignored/local.
Keep README and docs focused on product use, configuration and developer reference.
Keep personal requests, approvals, chat acknowledgments, work logs, rollout plans,
historical delivery evidence and detailed vulnerability findings in ignored local
operator records. User guides belong in docs in matching Markdown and Word formats.

Preserve source history/tags. Public releases contain generic source only.
Company-specific packages are built, verified and installed locally; never relabel
an older artifact. Source publication, backend deployment and app installation are
separate actions. Public CI uses standard no-charge runners without artifact/cache
uploads; do not run private/paid jobs, enable billing or claim unrun CI passed.
