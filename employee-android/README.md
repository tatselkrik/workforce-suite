# Workforce Employee for Android

Workforce Employee provides attendance, leave, correction and overtime requests,
published payslips, Services, Inbox and personal profile controls.
The current release is `1.0.0-beta.2`, with application identifier
`io.github.tatselkrik.hrisemployee` and Android versionCode 5.

The app connects to its packaged company's Supabase configuration. Every account,
including an Administrator, receives only its own employee information. Staff
administration and payroll preparation use the desktop applications.

## Using the app

Install the signed company APK on Android 8 or later and sign in with your company
account. The app keeps authentication in memory. Closing or process termination
requires sign-in again; employee records and passwords are not cached for offline use.
Android light and dark appearance carries through the app.

Use Android's document chooser for exports and image selection, and its print
framework for PDF. See the [suite user guide](../docs/USER-GUIDE.md) for workflows
and [the Word edition](../docs/Workforce-Suite-User-Guide.docx) for a downloadable copy.

## Device alerts

After signing in, open **Device alerts** and select **Enable alerts**. Allow the
Android notification permission, then use **Send test alert**. Firebase Cloud
Messaging carries only a generic inbox hint; private details are fetched after
current sign-in. The in-app Inbox remains available independently.

Personal inbox items are checked every 15 minutes within configured sending caps.
Closing the app retains enabled alerts; **Sign out** and **Disable alerts** turn
them off. Grants expire after 30 days without authenticated confirmation. Android
notification and power settings can delay reception. See
[Device alert reference](../docs/ANDROID-PUSH.md).

## Architecture

Kotlin and Compose manage lifecycle, protected WebView, pickers, printing and
notifications. The bundled interface uses an embedded Rust own-record service on
an ephemeral device-loopback origin. That service is part of the Android app.
See [Runtime and access boundaries](../docs/ANDROID-EMPLOYEE.md).

## Build and packaging

Pinned inputs are in the Gradle files and locks, wrapper and `rust/Cargo.lock`.
Use `tools/build-employee-android.py` from the repository root on a configured
Windows workstation. It builds arm64-v8a and x86_64 libraries, verifies the company
publishable profile, retains the signing identity and collects dependency notices.

`--release` requires committed clean inputs; `--candidate` produces an explicitly
unverified candidate. Use `--refresh-locks` only for an intentional dependency
update. Final packaging checks that source remains clean after assembly.

Supply deployment profiles, Firebase configuration and signing keys locally using
the [configuration guide](../config-examples/README.md). Keep them and configured
APKs outside Git. Updates must retain the package ID and signing certificate.
