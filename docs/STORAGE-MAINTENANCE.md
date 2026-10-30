# Build storage management

Rust and Android builds can generate large caches and debug symbols. Keep
reproducible build output separate from source, company records, signing keys,
deployment configuration, current packages and recovery copies.

## Compact Rust checks

Use the project's compact settings for local development:

```powershell
$env:CARGO_INCREMENTAL = "0"
$env:CARGO_PROFILE_DEV_DEBUG = "0"
$env:CARGO_PROFILE_TEST_DEBUG = "0"
cargo test --workspace --locked
```

Reuse one target directory per application or verification purpose. Avoid keeping
duplicate debug-symbol variants or obsolete test executables after their evidence
has been retained. Pinned lockfiles allow dependency/build caches to be recreated.

## Before removing output

Inspect sizes and active build processes first. Resolve exact paths and distinguish
generated output from deployment files. Preserve installed executables, current
signed packages, manifests, company files, signing identities, recovery evidence
and unsaved work. A previous cleanup list is not authority to delete current files.
