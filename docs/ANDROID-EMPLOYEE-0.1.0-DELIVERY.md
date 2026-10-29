# Android employee app0.1.0 — private local delivery

The signed employee APK is staged at dist/HRIS-Employee-Android-0.1.0/HRIS-Employee-0.1.0.apk. It uses the existing company's Supabase directly and runs independently of the PC portal. The local browser portal0.1.8 and its running sessions are preserved. Background push is **not connected**; its free Firebase account/project connection remains pending.

Frozen production source: `6f5db64449565cb41b91c3faa8fbb47b5b81b62a`.
Application: `io.github.tatselkrik.hrisemployee`; version0.1.0/code1; Android8/API26 minimum; arm64-v8a and x86_64. APK17,729,488bytes; SHA256 `b57406298679da154fd49100cffa12a98c4c502f383c5a8eb6785d86d820ed4a`. Retained private signing identity, APKv2 signature, release non-debuggable manifest, ZIP/native16KB alignment and all four packaged native libraries are verified. Dependency notices are included; no secret/service key, test account or signing credential is packaged. The publishable company profile is intentional private-deployment input.

## Verified

- Rust listener/security lifecycle check passes; both optimized Android ABI builds pass. Existing default portal regressions:53pass,1intentional fictional-preview opt-in ignored. Desktop TLS remains the default; Android explicitly selects certificate-verified Rustls with ring/Mozilla roots.
- Gradle debug/release compilation, locked dependency resolution, R8 release assembly and lint pass with0errors. Remaining UseKtx warnings are advisory. The unit-test task has no source; no Android JVM unit-test pass is claimed.
- Two actual installed debug-emulator instrumentation workflows pass: Enter sign-in; current employee/payslip and exact net; protected employment fields; all major sections within mobile width; Android photo/document chooser cancellation and print-dialog opening; retained contact draft on Activity recreation; sign-out clearing; an Administrator restricted to own records and denied another person's payslip; foreign Host/navigation restrictions. Earlier test focus/asynchronous waits/MIME monitors were corrected. An actual six-column mobile navigation defect was fixed. Three synthetic screenshots were inspected.
-137prior record sets are preserved. The same three managed acceptance accounts end disabled/archived/session-free, and transient device credentials are removed. No hire, payroll, payment or real account was recreated.
- The exact signed release is installed in the headless Android16/API36 emulator. Its installed APK hash, version/non-debuggable state, normal launch and UID-bound Rust listener pass. Protected HTTP checks confirm bundled sign-in/CSV source, no-store headers, unauthenticated own API denial and foreign Host rejection. Temporary localhost ADB forwarding was removed.
- A debug instrumentation runner cannot attach to the R8 release because Kotlin runtime class names were optimized. This test-harness limitation is retained explicitly; the normal release and its HTTP checks pass. Full authenticated UI evidence comes from the debug build of the same production inputs. Release screenshots are intentionally protected.

## What Kirk can check on a phone

1. Install the signed APK; sign in with an existing company account. It connects without opening the PC portal.
2. Review Home, Attendance, Leave, Payslips, Services and My profile; open the inbox from Notifications.
3. Open an own payslip. Try CSV saving and Print/savePDF in Android's chooser/framework.
4. Try a profile-photo selection and contact/emergency update. Employment fields stay managed by HR.
5. Start a contact draft, switch away/rotate and return. Discard or save it before leaving the page.
6. Sign out and confirm private details clear. Device alerts currently explain that background push is not connected.

Physical-phone behavior, actual selected-photo upload, completed device CSV/PDF saving, real email confirmation and delivered background push remain unverified. No public store/CI release, paid plan, external email, government filing, biometric connection or public/LAN portal is claimed.

Evidence is retained under ignored work/android-employee: acceptance-results.json, instrumentation.log, source-precheck.json, android-package-results.json, release-installed-results.json, lint/build/signature/alignment logs and synthetic screenshots. Source ZIP SHA256 `5905a301c41a8e09ead8c260b821d61e35e4799bb181ecb540c4d3120fa67049`. Release APK/source/signing identity must be preserved.

## Payroll0.1.1

Payroll0.1.1 is installed on Kirk's PC from the same frozen source. The declined external Data exchange entry is removed; compatibility/audit history and ordinary financial reports/exports remain. No schema, wage/tax calculation, salary, payslip, receipt or payment logic changed. Targeted navigation geometry passes in both themes/four heights/all remaining tabs; locked optimized build passes.

The exact local update installer passes own/HRIS/unknown namespace checks and company/key/branding/unknown-file preservation. Actual executable/BUILD/Windows Apps/normal launch match. All HRIS files and existing Payroll company files are preserved; no drafts discarded. Executable SHA256 `41a3e45e6be525eb2385319d7861bf6e98da96d561895e477f52503ff849ba57`. Update installer SHA256 `5b1458405f14d0426c71ce512870e9e96bb67bca88626975b73b24285fdb56cd`. Payroll remains a local companion without a new publishing channel; signed-in GUI interactions/normal close were not automated. Preserve Payroll0.1.0 rollback and HRIS0.1.69.
