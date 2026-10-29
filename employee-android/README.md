# HRIS Employee for Android

Separate private employee app0.1.1, application identifier `io.github.tatselkrik.hrisemployee`.
It connects directly to the packaged company's Supabase using a publishable key and the signed-in person's protected own-record APIs. It does not require the PC browser portal to run. Administrators also see only their own employee records in this app.

The native Android shell embeds the existing Rust employee service and bundled interface on an ephemeral device-loopback socket. Existing attendance, leave, requests, payslips, inbox, contact/emergency details, profile pictures and account-security flows are reused. Kotlin/Compose supplies lifecycle, image/document pickers and Android printing. No staff administration/payroll preparation is exposed. No offline employee-data cache or saved password is provided; reopen after process death requires sign-in.

After signing in, open **Device alerts** and choose **Enable alerts**. Allow the Android notification permission, then use **Send test alert**. The dedicated free Firebase connection transports only a generic inbox hint; names, pay and HR details are fetched after current sign-in. New personal inbox items are checked every15minutes, within the free sending caps. Test alerts send immediately when capacity is available. Closing the app keeps alerts enabled; **Sign out** and **Disable alerts** switch them off. A grant expires after30days without authenticated confirmation. Android notification/power settings can delay delivery. The in-app inbox remains available independently. No external email provider, paid service, store publication or public/LAN PC portal is configured. The existing browser portal remains available and unchanged.

## Build

Pinned stable inputs are in Gradle files/locks, the Gradle wrapper and rust/Cargo.lock. Use `tools/build-employee-android.py` from the HRIS root on the configured Windows workstation. It builds arm64-v8a and x86_64 Rust libraries, verifies the company publishable profile, retains the private signing identity and collects dependency notices. `--release` requires committed clean inputs; `--candidate` is explicitly not a verified release. Private assets, signing files and build output must remain ignored. Builds consume the pinned dependency locks; use `--refresh-locks` only for an intentional dependency update, then commit the reviewed lock changes. Final releases check that source stays clean after assembly.

## Phone review

Install only the privately supplied signed APK on a compatible Android device (Android8/API26 or newer; arm64). Sign in with your existing company account. Review Home, Attendance, Leave, Payslips, Services, Inbox and My profile. Check image selection, CSV saving, Print/savePDF and draft behavior when switching apps. Android emulator evidence is separate from physical-phone verification. Never install the instrumentation test APK on an employee phone.
