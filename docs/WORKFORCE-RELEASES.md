# Workforce Suite release plan

Kirk approved one private monorepo and three application display names on30October2026. The existing code layout remains: root/src for Workforce HR and shared Rust code; payroll-desktop for Workforce Payroll; employee-android plus the embedded own-record service in employee-portal for Workforce Employee. The retained PC portal stays local with its independent lifecycle.

All three start at1.0.0-beta.1, then advance independently. Tags: hr-v1.0.0-beta.1, payroll-v1.0.0-beta.1 and employee-v1.0.0-beta.1. Preserve the original Git history, binary releases, package/application IDs, deployment/data paths and signing keys. Display names and new build versions change; canonical HR/payroll/auth/history facts do not. The Android code advances from3to4. Windows installers retain legacy installation namespaces and use an explicit numeric file version alongside the prerelease display version.

Review tracked source and reachable history for private material before private GitHub upload. Source excludes deployment profiles, keys, tokens, user/company files, dumps, build caches and packaged binaries. New private GitHub prereleases include verified installers/APK, release notes and checksums. Verify exact remote commits/tags/privacy/assets; no existing immutable release is replaced. The exact private repository is guarded in deployment-private/GITHUB-WORKFORCE-AUTHORIZATION.md.

Local native/Android/version/installer and privacy checks are required. GitHub CI stays manual until no-charge execution is assured; never enable billing or spend to obtain CI. Do not claim unrun CI or subjective/physical-phone checks. The same usage-reset automation may continue only this release scope after current-context recovery; pause it on completion or genuine handoff.

## Compatibility and delivery channels

The HR automatic-update service keeps its existing stable numeric manifest/feed contract. Private GitHub prereleases carry these beta builds; do not publish beta rows into that stable catalog. Native version comparison accepts bounded SemVer prereleases so a later stable1.0.0 upgrades beta1 and an older0.1.x is never offered as an upgrade. Local HR signing may sign the beta installer using the retained key; no new signing identity or hosted channel is created. Payroll has no new automatic-update channel.

Windows beta1 uses numeric installer file version1.0.0.1 with display version1.0.0-beta.1. Application IDs, executable basenames, data/cache directories and installation folders retain their deployed values. New shortcuts/Windows Apps labels use the Workforce names. Remove a legacy shortcut only after its target is proven to be the exact owned installation; preserve unfamiliar shortcuts and unrelated files. Android keeps its package/signer/Firebase identity and increases versionCode to4.

The initial source/history review covered185commits and2,381reachable blobs (~78MB uncompressed), with only three reserved-example rejected-input URL fixtures cleared. Reviewed beta source is committed and uploaded privately, preserving that history. Exact tag/asset and installation evidence is recorded in [the beta delivery report](WORKFORCE-1.0.0-BETA.1-DELIVERY.md); repository creation alone is never release proof.

The README follows Kirk's existing repository presentation: a concise product introduction, version/platform badges, an expandable application tour, reviewed screenshots, installation guidance and developer documentation. Use actual fictional app captures and retain their provenance; never publish employee data or claim passing CI with an unrun workflow badge.
