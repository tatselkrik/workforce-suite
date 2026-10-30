# HRIS Employee Android0.1.2 — design delivery

30October2026, UTC+8. Privately packaged and installed on the owned emulator. This visual update matches the approved HRIS/Payroll teal light and charcoal dark design. No public store, new channel or remote CI publication was used.

## What changed

- Matching native app chrome and embedded screens, rounded cards, clearer type, 48-pixel fields/buttons, circular company/profile branding and local vector navigation icons.
- **Home / Time / Leave / Pay / Help / Profile** keep the existing destinations. Time opens Attendance; Pay opens Payslips; Help opens Employee services. Full accessible names remain.
- Android appearance changes retain the live WebView and unsaved draft. The explicit context theme follows the [official WebView behavior](https://developer.android.com/develop/ui/views/layout/webapps/dark-theme).
- Android-only presentation feature: the PC browser's base HTML/CSS/JS and original print rules remain unchanged. Company connection, own-record permissions, business rules, notification pipeline and file/print code are preserved.

## Exact package and verification

`dist/HRIS-Employee-Android-0.1.2/HRIS-Employee-0.1.2.apk`, 18,193,204 bytes; versionCode 3; Android 8/API 26+; arm64-v8a and x86_64. Frozen source `96bf1172232aed24ce3b717d7624ab7ac6966e84`. APK SHA256 `741d5437aabd561d0404192e5833ca0c3bd17ae7b4e3cc076f91c1456ce1a94f`. Source archive SHA256 `c734d0242b0fd8fd407e2e0826ea4db0a2e988753229f8aadf89da3f6f44f5bb`; 729 reviewed tracked files.

Both native ABIs were rebuilt for the presentation change. The signed release reused only hash-verified unchanged native inputs. Existing signer/APK v2, non-debuggable/backup-disabled manifest, ZIP/all 6 native-library 16 KB alignment, dependency attribution and no private keys/test instrumentation pass. Release lint has 0 errors; existing ObsoleteSdkInt and UseKtx warnings remain. The local commit is frozen; no public CI result is claimed.

Default/browser and Android feature Rust suites each pass 59 checks, with 1 existing intentional ignore each. 24 page checks and 1 Rust bridge check pass. Kotlin's unit-test task has no source; it is not counted as a passing test suite.

Four actual owned-emulator design cases pass: 411-wide phone light/dark; 320-wide compact dark with WebView textZoom 130; 800-wide tablet light. 26 native frames were captured; 12 representative current frames were actually inspected. Geometry, five icon loads under unchanged CSP, no horizontal overflow, visible alert-dialog bounds, navigation draft guard and live appearance switching preserving the same view/unsaved text pass. Screens use fictional test-only state, no company sign-in or data writes. Device-alert dialog state is mocked; these are not delivery/provider tests. QA-VERIFICATION.json lists exact captures, reviewed images and APK identities.

The exact signed APK was installed and normally launched at `2026-10-30T04:02:15.162096+00:00`. Installed SHA/version/code, UID-bound Rust listener, no-session 401, foreign-Host denial, unchanged private CSP/no-store and final styles/five artwork assets pass. Emulator appearance was restored; data was not wiped; test APK and temporary ADB forward were removed. Signed 0.1.1 remains the verified immutable fallback.

## Kirk's review checklist

1. Install **HRIS-Employee-0.1.2.apk** on a compatible Android device using the existing app/signing identity.
2. Check sign-in/company logo, Home, Time, Leave, Pay, Help, Notifications and Profile in light and dark appearance.
3. Start a contact draft, change Android appearance and return; the draft should remain. Save or discard normally.
4. Review profile/photo controls, payslip layout and Device alerts. Existing image/CSV/PDF/background workflows retain their prior verified behavior; their full signed 0.1.2 workflows were not rerun for this styling change.

Physical-phone/OEM behavior and actual email confirmation remain unverified/deferred. Actual authenticated 0.1.2 GUI, FCM reception and chosen photo/CSV/PDF saves were not retested. Previous 0.1.1 acceptance is separately recorded in [the emulator acceptance report](ANDROID-EMPLOYEE-EMULATOR-ACCEPTANCE-2026-10-30.md). No SQL, financial, provider/grant/dispatch, billing, HRIS 0.1.70/Payroll 0.1.2 installation or PC portal 0.1.8 session changes were made. The scoped design task is complete; its same usage-reset follow-up is paused on completion.
