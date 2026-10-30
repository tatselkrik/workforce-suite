# Workforce Employee architecture

Workforce Employee is the Android employee application, with identifier
`io.github.tatselkrik.hrisemployee`. It connects to the packaged company Supabase
deployment through protected own-record APIs. Every account, including a staff
or Administrator account, receives only its own employee information.

## Runtime and interface

The Kotlin and Compose shell embeds the shared Rust employee service and bundled
WebView interface. The service binds an ephemeral device-loopback address; its
configured origin must match the actual literal IP and port. Strict Host, Origin,
CSP, CSRF and no-store checks protect requests. The service is not exposed outside
the device. Its Rust library lives in `employee-portal/`.

The application supports attendance, leave, correction and overtime requests,
published pay information, Services, Inbox, profile pictures,
contact and emergency details, account security and device alerts. Staff
administration and payroll preparation belong to the desktop applications.

An application-context WebView survives Activity recreation without serializing
private pages or drafts to disk. Activity access is weak and refreshed. Closing
clears the app session; process death requires a new sign-in. The service cancels
held or incomplete connections during shutdown and bounds token cleanup.

## Files and privacy

Profile images use an explicit Android content selection. CSV uses the Android
create-document chooser and PDF uses Android's print framework. These are deliberate
exports; there is no automatic employee-record cache or saved password. Backups,
WebView debugging, external navigation, mixed content and arbitrary bridges are
disabled in release packages.

The origin-scoped main-frame listener accepts only bounded CSV payloads with
permitted filename shapes and print actions. Missing WebView capabilities fail
visibly. Renderer failure requires reopening and reviewing saved server state
before retrying an uncertain action.

Light and dark appearance follows Android settings. AndroidX handles back
navigation, document selection and notifications. See the
[Android build guide](../employee-android/README.md) and
[device alert contract](ANDROID-PUSH.md).
