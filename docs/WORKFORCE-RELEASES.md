# Versioning and packaging

Workforce Suite contains three independently versioned applications:

| Application | Source tag prefix | Package identifier |
| --- | --- | --- |
| Workforce HR | `hr-v` | `io.github.tatselkrik.hrisdesktop` |
| Workforce Payroll | `payroll-v` | `io.github.tatselkrik.payrolldesktop` |
| Workforce Employee | `employee-v` | `io.github.tatselkrik.hrisemployee` |

Each public release provides the generic source snapshot at its tag. Company
installers and signed APKs are packaged by a deployment operator using that
company's configuration and signing identity. See
[Deployment configuration](../config-examples/README.md).

## Compatibility

Preserve application identifiers, signing identities and compatible installation
and data paths across updates. Windows numeric file versions are distinct from
display prereleases. Android versionCode increases for each installable update.
The applications can advance independently when only one product changes.

Original personnel history, approvals, issued statements and receipts remain
immutable. A package update is separate from a database migration or data change.
Database changes use versioned migrations and deployment-specific review.

## Source and package verification

Public source uses pinned dependency locks and standard GitHub Actions runners.
The workflow checks source/history, the employee service, native code and database
contracts without deployment credentials or configured artifact uploads. Check the
result for the exact commit rather than assuming a tag or local build passed CI.

Company packaging verifies the source revision, manifest, signing identity, hashes,
required configuration, install behavior and recovery. Retain the evidence with
that deployment. Never label an existing binary as a newly built version.

## Update channels

Each desktop product has its own package and update identity. An HR update must
never be installed into Payroll. Android updates must match the existing package
identifier and signing certificate. Save current work before installing updates.
