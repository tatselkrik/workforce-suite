# Project storage cleanup — 25 October 2026

Kirk authorized this local cleanup separately from paused feature development.
The scheduler and feature work were paused at cleanup completion; Kirk later
resumed them25October. No application code, database, installed release or
running portal was changed by cleanup.

| Measurement | Before | After |
| --- | ---: | ---: |
| Project folder |219.12GiB|1.82GiB|
| Drive free space |67.22GiB|278.59GiB|

The folder's logical size fell by217.30GiB; measured drive free space increased
by211.36GiB. Logical file sizes and drive allocation are different measurements,
and other processes may also affect drive free space.

Removed only the guarded cleanup manifest: three Cargo output directories
(`target/`, `work/target/`, `work/portal-target/`),361obsolete test executables
and105byte-identical redundant release copies. Debug symbols and accumulated
build/test variants accounted for most of the growth. Tools and pinned dependency
lockfiles remain; global Rust/package caches and other projects were untouched.

Preserved source/Git history, migrations, private company settings/signing
material, current desktop0.1.41 and rollback0.1.40, all versioned portal packages,
one canonical copy of every distinct older release binary, and screenshot/log/
JSON/PDF verification evidence. Two tiny configuration files found in build
output were separately copied and hash-verified under the ignored audit folder's
`preserved-cache-files/` before removing their original cache directories.

The running portal0.1.1 package stayed open. The independently packaged portal0.1.3
is retained at `work/portal-deployment-0.1.3/HRIS-Employee-Portal/Run-Portal.cmd`.
This cleanup did not upgrade/restart that running session or alter its drafts.

## Verification and audit

All469targets were removed without a deletion error. Before the intentional
documentation/status updates,8,367retained project-file hashes and all9installed
application-file hashes matched their baseline;336tracked files, Git HEAD and
worktree were unchanged. Current desktop source and portal source/private ZIPs
passed integrity checks; the existing portal still served its loopback page.

The local ignored audit is `work/storage-cleanup-20261025/`: `plan.json` names
the exact targets and retained canonical copies, `retained-hashes.json` stores
preservation hashes, `deletion-result.json` records deletions and `result.json`
records measurements/checks. It is a record of this one cleanup, not permission
to reuse the deletion list against a changed workspace. Historical QA screenshots
and reports remain; reproducing their deleted test executables requires a build.

No build/test suite, reinstall or new visual check was run afterward. Rebuilding
would recreate the removed output. Previous portal visual limitations remain.

## Resume development

Run from the project root when development is explicitly resumed. Cargo recreates
the output directories from source and the existing lockfiles; the first build
takes longer and consumes fresh disk space. Existing packaged apps run without
these Cargo directories.

```powershell
# Native desktop
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'work/target'
cargo test --locked --offline

# Separate employee portal
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'work/portal-target'
cargo test --manifest-path employee-portal/Cargo.toml --locked --offline
```

The global Cargo dependency cache was preserved. If a required package is later
missing, use the same locked commands with network access instead of `--offline`.
Keep release binaries in their versioned packages; retain screenshots/reports
without accumulating an executable copy in every temporary QA folder. Reinspect
current targets and running processes before any later cleanup.
