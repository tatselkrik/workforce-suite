# Workforce Suite

A private prototype for one company's HR team, payroll staff and employees. Three applications share reviewed Rust code and the same dedicated Supabase deployment. The initial named release is **1.0.0-beta.1**; later versions can advance independently.

| Application | Platform | Purpose |
| --- | --- | --- |
| **Workforce HR** | Native Rust desktop, Windows distribution | Employee records, hiring, attendance/leave, learning/performance, confidential cases, employee services, benefits and leadership reporting |
| **Workforce Payroll** | Separate native Rust desktop, Windows distribution | Payroll preparation, independent approval, payslips, payment records and government review/reporting |
| **Workforce Employee** | Private Android app | The signed-in person's own attendance, leave, payslips, service requests, profile and generic notifications |

The company connection is packaged by the deployment operator. There is no database/company chooser in the apps. Employee accounts use the employee app; staff access is enforced by protected memberships and server permissions. Payroll uses its own responsibilities and independent financial approvals.

## Repository layout

- `src/`: Workforce HR executable and shared Rust implementation.
- `payroll-desktop/`: Workforce Payroll executable over that shared implementation.
- `employee-android/`: Android shell and embedded Rust bridge.
- `employee-portal/`: Shared own-record service/interface; the retained PC browser portal remains separate and loopback-only.
- `supabase/`: Versioned database migrations and protected functions.
- `tests/`, `tools/`, `docs/`: Verification, packaging and documented contracts.

Existing package/application identifiers and deployment/data paths are deliberately retained across the display-name change. Old releases remain immutable. See [release/version policy](docs/WORKFORCE-RELEASES.md) and [current roadmap and deferred work](docs/REMAINING-ROADMAP.md).

## Build and check

The existing toolchains and dependency lockfiles are pinned. On the configured Windows workstation with Rust/MSVC:

```powershell
cargo test --workspace --locked
cargo build --workspace --release --locked
```

The desktop outputs retain their compatible executable names: `hris-desktop.exe` and `payroll-desktop.exe`. Android's pinned SDK/JDK/Gradle requirements and private packaging command are described in [its guide](employee-android/README.md). The signing key, company profile and Firebase configuration stay outside Git. Never publish an instrumentation APK or substitute a renamed old binary for a release build.

GitHub Actions is manual while this project must remain free. Confirm no-charge capacity before dispatching jobs; no automatic database deployment or paid runner is configured. Local checks and exact source/artifact/install evidence are recorded separately from unrun remote CI.

## Deployment and privacy

This repository and beta downloads are private. Deployment profiles, credentials, signing keys, company records, database dumps, caches and generated installers/APKs are excluded from source. Builds use publishable client configuration and current-user authorization; server credentials must never enter an app. Keep each company's Supabase and deployment settings separate from core source.

Beta releases use separate tags: `hr-v1.0.0-beta.1`, `payroll-v1.0.0-beta.1` and `employee-v1.0.0-beta.1`. The existing HR automatic-update feed remains stable-only; beta packages are delivered through private GitHub prereleases. Stable-version ordering, installation namespaces and verification keys are preserved. Payroll and Android do not consume HR updates.

This is a beta prototype. Payroll payment/remittance recording does not transfer money or file government returns. Agency-validator acceptance, physical-phone/OEM behavior, email confirmation, paid services and actual biometric hardware retain their documented verification/deferred boundaries. The visible biometric connection remains disabled.
