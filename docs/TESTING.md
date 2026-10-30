# Testing

Run checks against the source revision being reviewed. Keep source tests, configured
backend checks, signed packages, installation and physical-device behavior distinct.
The [GitHub Actions workflow](../.github/workflows/ci.yml) defines the public CI jobs.

## Native applications

Use the pinned Rust toolchain and compact settings from the project instructions:

```powershell
$env:CARGO_INCREMENTAL = "0"
$env:CARGO_PROFILE_DEV_DEBUG = "0"
$env:CARGO_PROFILE_TEST_DEBUG = "0"
cargo test --workspace --locked
cargo build --workspace --release --locked
```

Some integration and screenshot tests are explicit opt-ins. Review their required
fixture, target and output guards before invoking them. A skipped opt-in is not a
passing integration or layout check.

## Employee service

The Android app embeds the shared employee-service library. Its Rust and JavaScript
tests cover own-record projections, request handling and interface behavior. Use
the exact commands and feature flags in the CI employee-service job. Android
packaging and instrumentation use the pinned inputs described in the
[Android build guide](../employee-android/README.md).

## Database contracts

`tools/db-tests/` replays checked-in migrations against local fixtures and exercises
permission, revision, retry and preservation contracts. Its npm pretest creates
the required scratch directory in a fresh checkout. These fixtures do not establish
the state of a configured hosted company deployment.

Hosted checks require a resolved deployment target and appropriate operator
authority. Use clearly identified synthetic records and preserve unrelated facts.
Never infer infrastructure access from possession of this public source.

## Documentation and public source

Check relative links, headings, configuration examples and rendered Word pages.
The Markdown and Word user guides must contain the same content. Review generated
file metadata and attachments before publication.

```powershell
node tools/public-source-check.mjs --history
```

This scan complements a review of screenshots, history and release contents.
It does not establish that every sensitive-data pattern has been detected.

## Deployment acceptance

Check sign-in and revocation, current role and own/team boundaries, unsaved work,
critical approval workflows, exact package hashes and signing identity, install
recovery, normal launch, and device-specific file and notification behavior.
Government format checks remain distinct from official validator or filing acceptance.
