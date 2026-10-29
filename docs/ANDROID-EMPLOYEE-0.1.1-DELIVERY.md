# HRIS Employee Android0.1.1 — private local delivery

30October2026, UTC+8. The signed APK is packaged and actually installed/verified on the owned emulator. No public store, new updater channel or remote CI publication was used.

## What changed

- Own-device **Enable alerts**, **Disable alerts** and **Send test alert**, with explicit Android permission and account-bound enrollment.
- Generic private background notifications; employment/pay details are fetched only after current sign-in. Closing the app retains the generic grant; sign-out disables it. Grants expire after30days without authenticated renewal.
- Existing own inbox sources are checked every15minutes, with deduplication, bounded retries and a1000attempt/day free cap. No Analytics, paid Firebase Functions, billing upgrade, external email or hardware connection was enabled.
- Measured pixel dialog bounds fix embedded WebView viewport-unit clipping. Company Supabase, employee scopes and the existing browser portal are preserved.

## Exact artifact

`dist/HRIS-Employee-Android-0.1.1/HRIS-Employee-0.1.1.apk`  
18,162,268bytes; versionCode2; Android8/API26+; arm64-v8a and x86_64.

SHA-256: `6f6f2c7847e9dff2719407f96c0f2858cdc78c663ef43297a299441375f4c93b`.

Frozen source: `0e228231fb022c9031d58f0b900449c6f89f0017`. Private source archive hash: `fa3b1a399434614a467a44743fddd9ace020eb45e489ded7c3b2e694033fd87a`. The archive contains702reviewed tracked files and excludes deployment/signing/generated private state.

The retained signer matches0.1.0. APKv2 signature, non-debuggable manifest, ZIP and all6native-library16KB alignments, dependency notices and absence of private keys/test credentials pass. Release lint has0errors; accepted warnings are ObsoleteSdkInt (redundant existing SDK guard) and UseKtx (style suggestion).

## Verification

-56Rust checks pass in default/browser and Android feature modes;1existing intentional ignore remains.83local PostgreSQL push checks/fresh97migration replay,27sender tests and8page tests pass. Prior full legacy suite passed13,268assertions.
-13hosted permission/preservation checks pass. The actual Vault-authenticated direct HTTPS callback returns200 without a caller-held push lock or persisted request queue. See ANDROID-PUSH.md for provider-owned queue constraints and installed http1.6 compatibility.
-Debug own-account workflows, notification opt-in/inbox/signout and actual dialog geometry/screenshot pass. The normal signed release was installed byte-for-byte, its process killed while backgrounded, then restarted by real FCM and posted the fixed PRIVATE system notification. It received no persisted HRIS Auth session.
-Normal release launch, UID-bound Rust/JNI loopback listener, no-session401, foreign-Host denial and private no-store responses pass. Temporary ADB forwarding was removed.
-All137prior record sets were preserved. Managed synthetic accounts are disabled/archived/session-free; dedicated emulator test data and ephemeral credentials were cleared. Android001 remains the immutable fallback; HRIS069, Payroll011 and PCportal018 were preserved.

## What Kirk can check

1. Install the APK and sign in with an existing company account; the app connects directly to its packaged company backend.
2. Open **Device alerts → Enable alerts**, allow notifications, then **Send test alert**.
3. Close/background the app, open an alert and sign in to review the inbox. Sign out and confirm alerts disable.

Physical-phone/OEM battery behavior remains unverified. Actual system-tray tap is subsequently verified on the owned emulator (see acceptance below). Actual email confirmation remains deferred. Full authenticated GUI evidence remains debug-instrumented; the exact signed FCM/startup/private-HTTP evidence is separate. Selected image upload and chosen CSV/PDF saves were subsequently verified; see acceptance below.


## Subsequent emulator file acceptance —30October2026

The actual photo-picker/upload/reload/removal, chosen CSV save and Android Print/save-PDF checks now pass, including settled photo and rendered one-page PDF visual inspection.137prior record sets are preserved and signed011 was restored. No production change/new APK was required. See [acceptance and testing checklist](ANDROID-EMPLOYEE-EMULATOR-ACCEPTANCE-2026-10-30.md). Earlier pending-file statements above describe the initial delivery; the later actual tray-to-own-inbox and sign-out checks also pass. The independent Android queue and SAME scheduler are now paused on completion; physical-phone/email checks remain deferred.
