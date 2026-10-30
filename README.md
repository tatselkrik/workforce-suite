<p align="center">
  <img src="docs/design/workforce-suite-banner.svg" alt="Workforce Suite — HR operations, payroll and employee self-service" width="100%">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/release-1.0.0--beta.1-007b83?style=flat-square" alt="Release 1.0.0-beta.1">
  <img src="https://img.shields.io/badge/desktop-Windows-2474ab?style=flat-square" alt="Windows desktop">
  <img src="https://img.shields.io/badge/employee-Android%208%2B-3b8067?style=flat-square" alt="Android 8 and later">
  <img src="https://img.shields.io/badge/core-Rust-a75b40?style=flat-square" alt="Shared Rust core">
  <img src="https://img.shields.io/badge/distribution-private-576875?style=flat-square" alt="Private distribution">
</p>

<p align="center">
  <strong>HR operations, dedicated payroll and everyday employee services.</strong><br>
  Three applications share the same company records, with access shaped by each person's responsibilities.
</p>

<p align="center">
  <a href="#the-apps">The apps</a> ·
  <a href="#application-tour">Application tour</a> ·
  <a href="#installation-and-updates">Install</a> ·
  <a href="#developer-setup">Developer setup</a> ·
  <a href="#documentation">Documentation</a>
</p>

![Workforce HR overview in the light theme, using fictional demonstration records](docs/screenshots/workforce-beta1/hr-overview.png)

<p align="center"><sub>Actual native application preview · Fictional demonstration data · Teal light and charcoal dark themes</sub></p>

## The apps

| Application | For | Workspace | Current release |
| --- | --- | --- | --- |
| **Workforce HR** | Administrators, HR staff and assigned managers | People, hiring, time and leave, employee services, development and reporting | [1.0.0-beta.1 · Windows](https://github.com/tatselkrik/workforce-suite/releases/tag/hr-v1.0.0-beta.1) |
| **Workforce Payroll** | Payroll Officers and Administrators | Pay preparation, independent approval, payslips, receipts and statutory review | [1.0.0-beta.1 · Windows](https://github.com/tatselkrik/workforce-suite/releases/tag/payroll-v1.0.0-beta.1) |
| **Workforce Employee** | Employees, including staff viewing their own records | Attendance, leave, approved payslips, requests, profile and device alerts | [1.0.0-beta.1 · Android](https://github.com/tatselkrik/workforce-suite/releases/tag/employee-v1.0.0-beta.1) |

Each company installation uses its own packaged Supabase configuration. Users sign in to their company account without selecting a database or company. The three apps share that deployment; a different company receives a separate deployment.

The initial release versions match. Later releases can advance independently without forcing every app to update together.

## Core capabilities

| Area | Included workflows |
| --- | --- |
| **People and employment** | Employee directory, reporting relationships, organization chart, contact and emergency details, employment history, contracts, attachments and digital 201 files |
| **Hiring and separation** | Applicants, résumés, interviews, feedback, offers, onboarding and exit/clearance tracking |
| **Time and leave** | Shift schedules, breaks, attendance corrections, overtime requests, holidays, workweeks, half-days, dated policies, balances and approval chains |
| **Learning and performance** | Goals, appraisals, feedback, learning records, certifications and career plans |
| **HR services** | Confidential employee-relations cases, routed employee requests, benefits eligibility, enrollments and dependents |
| **Payroll** | Salaries, allowances, deductions, Philippine premium calculations, cutoff payroll, 13th-month pay, annual-tax review, payments, final pay and linked amendments |
| **Reports and exchange** | Scoped attendance/leave reports, workforce and leadership summaries, configurable columns, bounded CSV/XLSX imports and exports |

Access is enforced through protected server permissions. Confidential cases, payroll administration and employee self-service retain separate scopes even though they share the same deployment.

## Application tour

These are actual app renders with **fictional demonstration data**. Desktop previews use the native screenshot harness; employee previews use the owned Android emulator. They do not show company records or prove authenticated production acceptance. [Screenshot sources and scope](docs/screenshots/workforce-beta1/README.md).

### Workforce HR

An overview of current work leads into employee records and focused HR workspaces. Company branding stays separate from the application name.

<details>
<summary><strong>Employee directory — records, organization and development</strong></summary>

![Workforce HR employee directory, filters and focused tabs](docs/screenshots/workforce-beta1/hr-employees.png)

</details>

<details>
<summary><strong>Company sign-in — existing account and familiar branding</strong></summary>

![Workforce HR sign-in with circular company branding and beta version](docs/screenshots/workforce-beta1/hr-signin.png)

</details>

### Workforce Payroll

A separate desktop keeps financial administration focused while retaining the same employee records and original payroll history. Preparation and independent approval remain distinct.

<details>
<summary><strong>Monthly register — preparation, review and exports</strong></summary>

![Workforce Payroll monthly register in the charcoal dark theme](docs/screenshots/workforce-beta1/payroll-monthly-dark.png)

</details>

<details>
<summary><strong>Display preferences — shared visual language, separate app</strong></summary>

![Workforce Payroll settings in the charcoal dark theme](docs/screenshots/workforce-beta1/payroll-settings-dark.png)

</details>

### Workforce Employee

The Android app connects directly to the company's backend. It does not depend on a running PC portal. Staff accounts receive the same own-record view when using it.

<p align="center">
  <img src="docs/screenshots/workforce-beta1/employee-home.png" alt="Employee home with attendance and employment summary" width="240">
  <img src="docs/screenshots/workforce-beta1/employee-pay.png" alt="Employee's own approved payslips" width="240">
  <img src="docs/screenshots/workforce-beta1/employee-profile.png" alt="Employee profile and private profile-photo controls" width="240">
</p>

<p align="center"><sub>Home and attendance · Own payslips · Profile and contact details</sub></p>

<details>
<summary><strong>Android company sign-in</strong></summary>

<img src="docs/screenshots/workforce-beta1/employee-signin.png" alt="Workforce Employee company sign-in on Android" width="320">

</details>

## What's new in 1.0.0-beta.1

- **A coordinated suite:** Workforce HR, Workforce Payroll and Workforce Employee now have clear names and independent release tags in one repository.
- **Consistent appearance:** teal light and charcoal dark themes across the three applications.
- **Compatible upgrades:** existing application IDs, signing identities, company connections, installation/data paths and original records are retained.
- **Prerelease ordering:** desktop version comparison recognizes beta versions and a later stable `1.0.0` correctly.

This is a new beta build of the existing prototype. Detailed release, installation and verification evidence is recorded in the [beta delivery report](docs/WORKFORCE-1.0.0-BETA.1-DELIVERY.md).

## Installation and updates

Download the appropriate installer or signed APK from its **private release** in the apps table. Repository access is required. Match the download against its accompanying `SHA256SUMS.txt` before installing.

1. Save work and close the desktop app being updated, or obtain the Android APK on the intended device.
2. Install over the existing version. Keep the same application identity and signing key; do not uninstall simply to change the displayed name.
3. Open **Workforce HR**, **Workforce Payroll** or **Workforce Employee** and sign in with the existing company account.

Windows shortcuts and Apps registration use the new names. Compatible installation folders and executable basenames retain their earlier HRIS/Payroll names. The Android package identifier and signer are unchanged; this release advances its version code to **4**.

Beta updates are installed manually from these private releases. The existing HR automatic-update feed remains stable-only. Payroll and Android do not consume HR update packages. Windows installers are not Authenticode-signed; the HR update package has its separate retained Ed25519 verification signature.

## How the suite fits together

```mermaid
flowchart LR
    HR["Workforce HR<br/>Windows desktop"] --> Company[("Company Supabase<br/>Auth · protected data · files")]
    Payroll["Workforce Payroll<br/>Windows desktop"] --> Company
    Employee["Workforce Employee<br/>Android + own-record Rust service"] --> Company
    Alerts["Firebase Cloud Messaging<br/>Generic device alerts"] --> Employee
```

Desktop financial screens use shared Rust code rather than separate calculation implementations. Android embeds the own-record service behind a private device-loopback interface. The retained PC browser portal has an independent, local-only lifecycle.

## Developer setup

The repository keeps the established source layout and pinned lockfiles:

```text
workforce-suite/
├── src/                 Workforce HR and shared Rust implementation
├── payroll-desktop/     Workforce Payroll executable
├── employee-android/    Android shell, lifecycle and Rust bridge
├── employee-portal/     Own-record service and retained local browser portal
├── supabase/            Versioned migrations and protected functions
├── tests/               Workflow, transport and release checks
├── tools/               Build, packaging and verification utilities
└── docs/                Contracts, delivery evidence and screenshots
```

On a configured Windows development workstation with Rust/MSVC:

```powershell
cargo test --workspace --locked
cargo build --workspace --release --locked
```

The outputs retain the compatible names `hris-desktop.exe` and `payroll-desktop.exe`. See the [Android guide](employee-android/README.md) for its pinned SDK/JDK/Gradle toolchain and private packaging workflow. Deployment profiles, Firebase configuration and signing keys must be supplied privately; they are excluded from Git.

**GitHub Actions is manual.** Confirm no-charge capacity before dispatching a run. Local verification and installed-artifact evidence are recorded separately from remote CI; no passing remote CI badge is claimed. Building an app does not deploy database migrations.

## Prototype boundaries

- **Private by design:** credentials, signing keys, company records, deployment profiles, database dumps and build caches are excluded from source. Client packages contain publishable configuration; server keys must never enter an app.
- **Recorded transactions:** payroll payment and remittance records do not transfer money or file government returns. Agency file preparation is distinct from official validator acceptance.
- **Verification scope:** current releases have local/emulator evidence. Physical-phone/OEM behavior, actual email confirmation and subjective monitor review remain separately documented.
- **Deferred connections:** biometric integration stays visible and disabled while no hardware is selected. Paid services and declined external accounting/payroll integrations are outside the active prototype scope.

## Documentation

| Start here | Reference |
| --- | --- |
| Releases, tags and compatibility | [Release/version policy](docs/WORKFORCE-RELEASES.md) |
| Current release evidence and checks | [1.0.0-beta.1 delivery](docs/WORKFORCE-1.0.0-BETA.1-DELIVERY.md) |
| Remaining and deferred work | [Roadmap](docs/REMAINING-ROADMAP.md) |
| Payroll architecture | [Separate Payroll Desktop](docs/PAYROLL-DESKTOP.md) |
| Android build and operation | [Employee app guide](employee-android/README.md) |
| Android notifications | [Protected push pipeline](docs/ANDROID-PUSH.md) |
| Screenshot provenance | [Reviewed gallery](docs/screenshots/workforce-beta1/README.md) |

<p align="center"><sub>Workforce Suite · Private company prototype · Built with Rust</sub></p>
