# Workforce Suite 1.0.0-beta.1 delivery —30 October 2026

**Local delivery verified; private release publication pending.**

Workforce HR, Workforce Payroll and Workforce Employee are new1.0.0-beta.1 builds
from frozen runtime source `5b7c3bcaa8b403df9182982ee26c0aac9f70ee77`. Existing application IDs, compatible
installation/data paths, company/Supabase/Firebase configuration, signing identities
and original records/releases are preserved. Android's version code advances to4.
The separate PC browser portal retains its0.1.8 lifecycle and running sessions.

## Delivered apps

| Application | Local delivery | Executable/APK SHA-256 |
| --- | --- | --- |
| Workforce HR | Windows installed, BUILD/Apps/normal launch verified | `bcede7fb2991b6610aee557a2e7f7d6ab3d747f27347bc483c6ebd1078865dea` |
| Workforce Payroll | Windows installed, BUILD/Apps/normal launch verified | `2f430908d60c52535f2d5370d59d2b57a8bb6d5f7681bbab7bd9658a7f99533a` |
| Workforce Employee | Signed APK installed and verified on the owned emulator | `bd0276d32455877736f1a0dec1ff0a91c4133bd3e6fe5bf9f96028a34c046a18` |

Windows Apps and both owned shortcuts show Workforce HR/Workforce Payroll. Only
legacy shortcuts whose targets matched the exact owned applications were removed.
Previous installed files were backed up; company files, verification keys, branding
and unknown files passed preservation checks. No open user app or draft was closed.

## Verification

- 874 default native checks across68 suites pass;37 explicit opt-ins ignored.
- 9 local release/version/edge checks pass, including bounded prerelease ordering.
- Both browser/default and Android service modes pass59 Rust checks each, with1
 intentional ignore per mode. The browser's existing HTML/CSS/JS remain unchanged.
- 4 initial fictional native captures plus3 README captures were actually reviewed.
 Android final light and compact-dark fixture cases pass with10 and3 screenshots;
 final sign-in and selected home/pay/profile frames were visually inspected.
- Clean-source native builds and signed Android release/lint pass. The Kotlin unit
 task has no source tests; it is not reported as a passing Kotlin unit suite.
- Exact HR updater recovery passes; both update installers pass own/foreign/unknown
 namespace checks without changing actual registration. Both installed Windows
 executables match their BUILD versions/hashes and launch/close normally.
- Existing HR Ed25519 key verifies the exact beta updater. The stable Supabase
 updater catalog is unchanged; beta packages are manual private GitHub downloads.
 Payroll has no new automatic-update channel. Windows installers are not
 Authenticode-signed.
- Android retains its signer, is non-debuggable with backups disabled, and passes
 ZIP/ELF16KiB alignment for6 native libraries. The exact signed APK normally
 launches, serves the Android title/styles/icons from its UID-bound loopback Rust
 listener, rejects unauthenticated APIs/foreign Host, and retains CSP/no-store.
 The test APK and temporary ADB forwarding were removed; no emulator wipe occurred.
- Source/history and artifact privacy checks pass within their documented pattern
 review. Initial185commits/2,381reachable blobs were reviewed;3 reserved-example
 rejected-input fixtures were explicitly cleared. Deployment profiles, signing
 material, caches and generated packages are excluded from Git source.

## Source and release evidence

The private repository's main initially received187commits including the two beta
preparation commits, preserving all185original commits without force-push. Tags
`hr-v1.0.0-beta.1`, `payroll-v1.0.0-beta.1` and `employee-v1.0.0-beta.1` will identify
the exact frozen runtime source above. Later README/gallery/delivery-only commits
can update main without relabeling or rebuilding those frozen runtime artifacts.
Private tag, release and uploaded/downloaded asset verification is a separate gate;
publication is not claimed until its read-back/hash checks finish.

The [README](../README.md) includes a coordinated banner, version/platform badges,
feature map, expandable desktop galleries and Android screenshots, installation
steps and developer guidance. The9screenshot PNGs are reviewed unmodified fictional
captures; their hashes/scope are in [gallery provenance](screenshots/workforce-beta1/README.md).

GitHub Actions remains manual and was not dispatched: no remote CI pass is claimed.
No schema/migration, financial rule, company data, push-provider/grant/schedule or
new paid service/channel changed. Prior authenticated/FCM/file acceptance remains
historical evidence; it was not rerun for this naming release. Installed signed-in
GUI clicks, physical-phone/OEM behavior and actual email confirmation remain
unverified/deferred. Subjective monitor appearance remains Kirk's review.

## What to check

1. Open the new Workforce HR and Workforce Payroll shortcuts; confirm1.0.0-beta.1.
2. Sign in with the existing account and check branding, profile and role navigation.
3. Review existing employee records and issued Payroll slips/receipts without edits.
4. Check Light/Dark and scaling under Settings in both desktop apps.
5. Inspect the repository README and its app-tour screenshots.
6. On a future authorized phone test, install the signed Employee APK over the same
   existing package/signing identity; no uninstall or credential change is needed.

No new module or deferred external/hardware work follows this release task.
