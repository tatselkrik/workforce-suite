# HRIS 0.1.70 — visual refresh delivered

Installed and privately published on 30 October 2026. Installation completed at 04:57 Philippine time; channel verification completed at 04:58. This delivers the approved D teal light / C charcoal dark direction in HRIS Desktop.

## Changes

- Consistent light/dark surfaces, borders, status colors and clearer native typography.
- Vector navigation icons, calm selected sections and circular company/profile treatment.
- Compact toolbar for version, date, notifications, refresh and authorized Payroll launch.
- Responsive Overview metric cards; aligned attendance and pending leave panels, stacked on smaller windows.
- Matching sign-in, employee forms, dialogs, settings and module styling. Enter to sign in, current themes, roles, optional modules and draft guards remain.

Settings > Display preferences controls Light/Dark and scaling. Segoe UI is loaded from Windows when available; bundled egui fonts provide the fallback. No font files, paid service or new dependency were added.

[Light native preview](design/hris-0.1.70-overview-light.png) · [Dark native preview](design/hris-0.1.70-overview-dark.png). These show fictional demo records. [Review checklist](HRIS-0.1.70-REVIEW.md).

## Verification

The frozen source passed **872 default Rust checks across 67 suites**, with 37 intentional opt-ins ignored. This includes 81 UI checks covering contrast, keyboard navigation, role/module visibility, sign-in, avatars, draft handling and geometry. Full separate-Payroll legacy style equality passes. One stale test expecting the already-retired Data exchange tab was corrected; Payroll production behavior was not changed.

**23 native fictional views were actually inspected**, including normal/compact, 100/125% scale, light/dark, employee editing, attendance/leave, Account Manager, settings/profile, hiring/development, cases/services/benefits, reports/leadership, Manager scope, sign-in and a leave dialog. Sidecars verify actual themes and zoom levels. Normal vertical scrolling is expected in compact enlarged layouts.

The normal release excludes the QA screenshot/session fixtures and company connection values. Configured launch/normal close, exact installer success/recovery, existing-key Ed25519 verification, installed executable/BUILD/Windows Apps registration, company-file preservation, authenticated private download hash/signature, staff feed and public/ordinary-Employee denial pass. No new accounts were created; the three exact managed synthetic actors finish disabled, archived and session-free. The protected role runtime remains ready.

Limitations: Account Manager's native demo capture shows its protected-fetch error state; automated editor/permission checks are separate evidence. Installed signed-in production click-through, subjective appearance on Kirk's monitor and public CI were not performed. The package is not Authenticode signed; the private updater uses its existing Ed25519 identity.

## Immutable delivery

- Source: `7a63a85bfa42439c68966a4db120e803a6000545`.
- Executable SHA-256: `9b04ba85298d13ab21d64fb9806f170b6890ca5e5958cba94d77fe55508d7c8f`.
- Signed updater SHA-256: `bf4db5b47ce0dcaf4ae1e765285b6647f77d9639827b0abe8d727216b6e29a87`.
- Setup SHA-256: `c0bb3642f5398ce5e43494644519c105618a611a2c480b64ffe3ef318f702fa6`.
- Source archive SHA-256: `f64b3b4840dd78f26bbc456b0403d968ce52489bc087ba4916cc6d5b95f50188`.

Private artifacts are in `work/company-deployment-0.1.70` and `dist/HRIS-Desktop-0.1.70`; the actual previous installation is backed up under `work/install-design070-actual/backup`. Do not modify these immutable release artifacts. BUILD, source archive, package/signing/installer/installation/publication and visual-review records identify the exact verified source and bytes. Later documentation commits do not change the frozen release.

No database migration, financial rule, backend permission or canonical company data change. Existing required HRIS schema 92 was verified within deployed schema 97. Payroll 0.1.1, Android 0.1.1 and local portal 0.1.8 keep their existing binaries/designs and histories. Their deferred scope is not resumed.

The HRIS-only approved visual scope is complete. Pause the same reset follow-up on completion; do not repeat this installation/publication or start another product's redesign without Kirk's instruction.
