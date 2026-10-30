# Workforce Payroll architecture

Workforce Payroll is a native Rust desktop application in the same repository as
Workforce HR. Both use the company's Supabase deployment and shared Rust domain,
financial, adapter and screen code. Original registers, issued payslips,
individual receipts and review history remain in their existing protected records.

## Application boundaries

- Workforce HR uses application identifier `io.github.tatselkrik.hrisdesktop`.
  It manages personnel and HR workflows and displays the person's own issued pay
  information. Its **Open Payroll Desktop** action uses a verified local install.
- Workforce Payroll uses `io.github.tatselkrik.payrolldesktop` and the
  `payroll-desktop/` workspace package. It has its own Windows installation,
  shortcuts, package version and update identity.
- Workforce Employee for Android displays only the signed-in person's published
  pay information. Staff accounts receive that same own-record scope.

Each application signs in independently and keeps upstream tokens in memory.
Company configuration is packaged separately from credentials. Unconfigured source
builds can use the clearly labelled fictional demo; configured installs fail closed
when their required company configuration is missing.

## Financial workflows

The shared implementation covers monthly registers, salary and recurring items,
payroll holidays, payslips and PDF, reporting, statutory reference packs, cutoffs,
13th-month pay, annual tax, individual payments, final pay, regional wage checks,
linked amendments, government review and remittances, and local data exchange.
CSV, Excel and PDF outputs use the same protected source and revision checks.

Current Payroll Officer or Administrator responsibility is required for payroll
administration. Independent Administrator approval and creator, preparer and
submitter exclusions remain enforced. Issued statements and receipts are immutable.
Other staff responsibilities and hidden navigation controls never grant access.

## Loading and permissions

The Payroll loader fetches financial context, the directory needed for employee
selection, own profile labels, company branding and personal financial notices.
It does not load private personnel files, hiring or exit cases, confidential
relations cases, appraisals, training, HR policy or Account Manager data.

The worker rejects operations outside the product even if a caller bypasses its
menu. Workforce HR uses an own-pay projection and cannot prepare registers or
export financial administration data. Protected memberships, RLS and guarded RPCs
remain the server authority across both applications.

Financial notices preserve their original keys and read receipts. Account,
security, session and appearance controls concern the same signed-in person.
Company, personnel and access administration belongs in Workforce HR.

## Installation and updates

Install Workforce Payroll on each PC that needs financial administration. The HR
updater does not install or update Payroll. Preserve each application's package,
signing and update identity; one product must never consume another's update.
Existing application IDs and installation data paths remain compatible.

See [Versioning and packaging](WORKFORCE-RELEASES.md) and the
[user guide](USER-GUIDE.md) for operation and distribution.
