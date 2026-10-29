# Android employee app contract

Authorized29October2026. See employee-android/README.md and its nested AGENTS.md. Identifier io.github.tatselkrik.hrisemployee; separate lifecycle from HRIS/Payroll/Desktop portal. Existing dedicated company backend and immutable facts are retained, with no new schema for the Android wrapper.

Rust owns Auth tokens, own-record requests and server-session state in memory. The bundled WebView holds only the opaque HttpOnly session cookie and page-memory CSRF/drafts. A held ephemeral127.0.0.1 listener prevents bind races; foreign Host/Origin, external navigation, mixed content, file access, arbitrary JS interfaces, backups and release WebView debugging are disabled. Android cleartext is permitted only for device127.0.0.1; upstream Supabase remains certificate-verified HTTPS. No service key is shipped.

The process retains an Application-context WebView across Activity recreation without serializing private pages to disk. Application context is intentional; Activity access is weak and refreshed. Close clears browser/session state. Process death requires a fresh sign-in. Profile image selection uses the user-selected Android content URI; CSV uses the Android create-document chooser; printing uses Android's print framework. No implicit file overwrite or silent photo-library access is added.

The origin-scoped main-frame message listener accepts only bounded CSV payloads with a fixed payslip filename shape and print actions. Existing browser CSV behavior is retained. Missing required WebView capabilities fail visibly; renderer failure requires reopening/reviewing saved state before retry. Native colors follow system dark/light appearance.

Push is not connected. Do not request notification permission before a real free company connection exists. Later FCM must use a trusted protected sender, generic payloads and current own identity; no sender key belongs in the APK. Existing inbox notices continue. Preserve existing Auth confirmation rules; real email confirmation remains a separate unresolved verification.

## Verification status

Rust own-record regressions, JNI listener/security lifecycle, two ABI builds, Gradle compilation/lint and headless emulator acceptance have separate evidence under ignored work/android-employee. Release delivery records must identify actual passed checks, failed/replaced tests, installed APK hash/signature, source revision and unverified physical-device/PDF-save/push boundaries. Compilation and a declared callback do not prove delivery.
