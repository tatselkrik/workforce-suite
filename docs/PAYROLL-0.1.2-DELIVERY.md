# Payroll Desktop 0.1.2 — visual refresh delivered

Installed locally on 30 October 2026 at08:46Philippine time. Payroll now uses the approved HRIS teal light / charcoal dark design with matching native typography, navigation icons, surfaces, company/profile circles and compact toolbar. A small-window sign-in spacing fix keeps Payroll's longer admission message visible at enlarged scaling.

All fourteen financial sections, Payroll Officer/Administrator admission, independent approvals, immutable payslips/receipts/history and existing exports remain. Data exchange stays retired. There are no financial/rate/backend/schema changes or company-data writes. HRIS0.1.70 remains installed and all its files were hash-preserved during installation; Android011/local portal018 were untouched.

[Light native preview](design/payroll-0.1.2-light.png) · [Dark native preview](design/payroll-0.1.2-dark.png) · [Review checklist](PAYROLL-0.1.2-REVIEW.md). Preview figures are fictional. Use Settings > Display preferences to select Light/Dark and scaling.

## Verification

**872 workspace Rust checks across68suites pass**, with37intentional opt-ins ignored. Includes81UI checks: complete HRIS/Payroll theme equality, contrast, keyboard/disabled-draft navigation, selected financial sections/Settings in short and tall sidebars, and sign-in fit at normal/enlarged sizes. The HRIS-specific behavior of the shared helpers is preserved.

**22 fictional native views were actually inspected**, covering every financial section plus profile/settings, normal/compact, light/dark,125percent, salary/worksheet editors, combined role/hidden-HRIS-module configuration and sign-in. Sidecars verify actual product/version/theme/zoom. Sign-in intentionally defaults to light before authentication, including the scenario whose filename includes dark125. Government/report previews show demo/readiness states; they do not establish authenticated agency results or official acceptance. Normal vertical scrolling is expected for long forms/enlarged views.

The normal release excludes QA/session environment fixtures and company connection values. Configured launch/normal close and rejection of the HRIS updater pass. Exact isolated NSIS updater cases verify the own Payroll namespace/payload and reject HRIS/unknown markers while preserving company files and actual Windows registration. Actual installation verifies executable hash, BUILD, Windows Apps version, normal launch/close, complete Payroll011backup, company/branding/unknown-file preservation and all HRIS file hashes. No unsaved work was discarded.

Installed signed-in production click-through, subjective monitor review and public CI were not performed. This companion remains local with no configured Payroll signing/update channel; no HRIS update key is packaged or consumed. The NSIS setup is not Authenticode signed. No external publication, SQL migration, transfer, actual filing, paid service or new fixture account occurred.

## Frozen local artifacts

- Source commit: `9ed91b3bf653499dee52b736778120f0dba1e1a1`.
- Payroll executable SHA-256: `3bfcf2f775a29c193224f488505e7d5dd9c69eb14eb9ac11824fd721e500470e`.
- Setup SHA-256: `c414e31b48e8e843e6a137414a505fdac49b9a362e4b8ad5b1ed230526be79e1`.
- Local updater SHA-256: `3aa014db02869e20b0cc4cfe9f247253dfe065e9ca23342e6de4efc3a05fb14a`.
- Source archive SHA-256: `6b267715111142552a4bfb3a2176cd5096ba19440ba84bb2bc2466248d7b80fd`.

Immutable packages: `work/payroll-deployment-0.1.2` and `dist/Payroll-Desktop-0.1.2`. Prior installation backup: `work/install-payroll012-actual/backup`. Normal-startup, visual-review, frozen-test, installer, package and installation records are stored separately under work/payroll012-* and work/install-payroll012-actual. Later documentation commits do not change these frozen bytes.

The authorized Payroll visual refresh/local installation is complete. Pause the same reset follow-up; do not reinstall012, repeat HRIS070delivery, start another product redesign or revive deferred modules without Kirk's next instruction.
