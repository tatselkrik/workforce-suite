# Remaining HRIS roadmap — agreed order

**Latest resumption25October2026:** Kirk reactivated the same scheduler and
resumed the remaining roadmap continuously after delivered0.1.45. This
supersedes the finish-after0.1.45 pause. HRIS must remain completely free for now:
put features requiring paid subscriptions/services or new purchases on hold.
Continue independent local/open-source/free-tier work within no-charge limits;
do not enable billing, upgrades or overages. Review work since9:31p.m. with the
[consolidated checklist](REVIEW-SINCE-931PM-2026-10-25.md).

Kirk approved this order on24October2026 and resumed continuous work and the
existing usage-reset follow-up. This supersedes the stop-after0.1.41 boundary.
The original roadmap status remains in [ROADMAP.md](ROADMAP.md).

Earlier resumption25October2026: Kirk resumed the roadmap, reactivated the same
usage-reset scheduler and requested continuous work. The stop-after-portal0.1.3
and cleanup-only pause are superseded. Finish remaining local portal work first.

**Storage cleanup completed25October2026:** project size reduced from219.1GiB to
about1.82GiB. Source/company files, current releases and useful evidence were
preserved. See [storage maintenance](STORAGE-MAINTENANCE.md). This completed only
the cleanup task; Kirk subsequently resumed roadmap work and the scheduler.

Keep this ordered list as the working checklist. Cross out a milestone only
after its implemented scope passes proportionate verification and its authorized
release/delivery requirements. Record partial progress below its item; retain
completed wording and evidence links. Do not silently promote a placeholder or
unconfigured external connection to a completed integration.

1. **Finish the employee portal — local only.** Own attendance-correction and
   overtime requests; notifications; company/profile pictures; password/email
   controls. Existing own records, contacts, attendance, breaks, leave and monthly
   payslips remain supported. Status: in progress; no public hosting/LAN/tunnel.
2. **Government payroll exports and reconciliation.** Prepare reviewed BIR, SSS,
   PhilHealth and Pag-IBIG outputs and reconcile with approved payroll. Verify
   current official formats/rules. Actual government filing is separate.
3. ~~**Payroll, appraisal and training notifications.** Payslip/payment notices,
   review deadlines, training reminders and certificate-expiry alerts.~~ Delivered
   in desktop0.1.46 and local portal0.1.6; in-app only. External delivery is item8.
4. **Effective-date leave policies and balances.** Scheduled policy changes,
   entered/imported opening balances and audited credit adjustments.
5. **Approval delegation.** Temporary substitute approvers with retained history,
   current permissions, self-approval restrictions and clear validity periods.
6. **Attendance/leave imports and Excel support.** Validated imports, previews,
   error reports and native XLSX import/export; employee CSV imports already work.
7. **Accounting and external payroll integration foundations.** Configurable data
   exchange, reconciliation, retries and integration history. Actual connections
   require resolved company software/vendor targets.
8. **Email/push notification delivery.** A company-approved provider, resolved
   destinations, delivery/retry history and required specific sending authority.
   Paid providers/plans are on hold; use an approved no-charge option only.
9. **Actual biometric/device integration.** Select hardware/protocol and verify
   physical operation. Keep the visible control disabled until then. New hardware
   purchases or paid vendor licenses are on hold; an existing selected device may
   be evaluated within the standing physical-verification boundary.

## Progress and boundaries

- Desktop0.1.46 is installed and privately published, source
  `e7efe56e9dcd4486f6629ae9870517a8ab2ec768`;0.1.45 is retained for rollback.
  448default Rust checks plus exact installer/recovery,4,549PostgreSQL assertions,
  fresh61migration replay and four fictional layouts pass. Hosted checks verify
  current scope, individual receipts, old-client compatibility and revocation;
  all prior payroll/Auth/employer facts remain. Migration61 preserves92prior
  public record sets and267functions. See [notification checks](NOTIFICATIONS-0.1.46-CHECKLIST.md).
- The separate local portal0.1.6 package is delivered:48Rust/15JavaScript checks,
  optimized actual hosted own-notice/CSRF/read/revocation proof and package smoke
  pass. The running0.1.5 portal and its drafts/sessions were preserved. Portal
  visuals/actual mail confirmation and government agency acceptance remain partial.

- Desktop0.1.45 is installed and verified; retain0.1.44 as rollback. Kirk authorized
  0.1.42 and future verified releases to the same private channel on25October2026.
  Releases0.1.42–0.1.45 are published, authenticated downloads/signatures
  verified and0.1.45 serves as latest. Do not repeat completed installations.
- Portal milestone1: ~~own correction/overtime requests, cancellation and decision
  history~~ are implemented in the0.1.2 increment, with25 Rust tests,3,275 database
  assertions and seven actual hosted proof groups. See [portal verification](EMPLOYEE-PORTAL.md).
  Desktop layout/draft/history checks passed; the new phone-width check remains
  pending. The entire milestone stays open until account controls and all
  remaining verification are complete.
- Portal increment0.1.3 adds the own notification inbox/read history with32Rust
  tests,5JavaScript code checks,3,414database assertions and8actual hosted proof
  groups. Its additive migration preserves85priorrecordsets and235functions.
  Current notification/phone visual verification is pending because both UI
  runtimes fail initialization. Pictures/security followed in0.1.4/0.1.5; milestone1
  stays partial. Work and scheduling resumed after the completed storage cleanup.
- Portal increment0.1.4 implements private own circular photos/preview/save/
  discard/removal and deployment-bound company sign-in branding.38Rust/9JS checks
  and9actual hosted proof groups pass. A stale-image removal issue was fixed with
  fresh Storage origin reads. Current visual verification remains pending;
  password/email controls followed in0.1.5. Milestone1 stays open.
- Portal increment0.1.5 implements own password/email status/changes and signing
  out other sessions.47Rust/13JS tests and9hosted proof groups pass, including a
  synthetic password change/restoration and retained foreign sessions/real Auth
  users. Actual email confirmation delivery and current UI visuals are unverified;
  milestone1 remains partial. Continue independent government-export work while
  these limitations remain recorded; no confirmation/provider rules are weakened.
- Government milestone2 has started with official-source review and a separate
  reconciliation engine passing11Rust checks, including PostgreSQL-vector decoding.
  The local database suite passes3,591assertions. Source-v1 is deployed at57schema
  versions with86recordsets/241functions preserved; hosted checks cover93facts,
  four agency projections and current protected-role denial/revocation. All24prior
  payroll table sets and earlier Auth users matched. Native library78tests pass;
  The next0.1.42 reconciliation increment now has reviewed allocations, immutable
  remittance/void/correction storage, native screens and fresh privacy-limited CSV.
  Its workflow migration is deployed58 with86recordsets/243functions preserved;
  3,785local database assertions and fresh58migration replay pass. Actual hosted
  checks include independent approval, a concurrent duplicate-reference race,
  revocation and native validation of six snapshots/114source rows. All24prior
  payroll sets and earlier Auth users match; synthetic payments end voided.
  ~~Reviewed contribution allocations, remittance/void/correction history, native
  reconciliation screens and fresh review CSV~~ are delivered and locally installed
  in0.1.42. Eleven fictional layouts, exact installer inventory/recovery, installed
  hashes/BUILD/Windows Apps and normal launch/close are verified. File adapters remain queued;
  the complete government milestone is partial. See
  [government reporting boundaries](PAYROLL-GOVERNMENT.md).
- ~~HR-only legal identity/company registration records~~ are delivered and
  installed in0.1.43. Explicit name parts, registered addresses/identifiers,
  supporting review and immutable before/after history preserve employment,
  company branding and issued payroll.428default Rust checks,3explicit vector
  checks, the real installer/recovery test,3,974database assertions and fresh59
  migration replay pass. Hosted checks validate3native-decoded snapshots and
  current role/revision/idempotency guards. All24payroll sets, earlier Auth and
  employer facts remain unchanged. See [0.1.43 evidence](GOVERNMENT-0.1.43-VERIFICATION.md).
  Exact agency files/validator acceptance remain queued; milestone2 stays partial.
- ~~Agency-specific internal preparation CSV~~ is delivered/installed in0.1.44.
  A fresh protected projection exports only the selected agency's legal fields,
  missing-input flags and reviewed source/summary amounts.434default Rust checks,
  four explicit vector checks, exact installer/recovery,4,110PostgreSQL assertions
  and fresh60migration replay pass. Hosted checks cover four native-decoded
  agency responses, role/revocation/privacy and unchanged payroll/Auth/employer
  facts. Four native layouts pass; installed payloads/Windows Apps/launch match.
  SSS reference rendering matches12fictional month vectors; its bounded draft
  control followed in0.1.45. Exact agency files/validator acceptance remain queued; the
  full government milestone stays partial. See [0.1.44 evidence](GOVERNMENT-0.1.44-VERIFICATION.md).
- ~~Bounded SSS R3 reference draft~~ is delivered, installed and privately
  published in 0.1.45. The 442 default Rust checks, exact installer/recovery and
  four fictional native layouts pass. Fresh source/registration/receipt binding,
  explicit declarations and current-role checks guard a new local `.txt` file.
  My.SSS acceptance, broader encoding, reserved-slot semantics and other agency
  file adapters remain pending. See [0.1.45 evidence](GOVERNMENT-0.1.45-VERIFICATION.md).
- Effective-date policies, balances and delegation follow the existing saved
  charges/plans and immutable history rules; they do not rewrite older requests.
- Install each verified authorized desktop release, preserving unsaved work and
  company files. The separate portal has its own release lifecycle.
- Prototype SQL authorization remains in AGENTS.md and the private target file;
  verify the exact linked project and preserve unrelated records.
- No public portal exposure, bank transfer, actual government filing, new cost,
  model/plan change or unresolved third-party/device connection is authorized.
- Electronic signatures/document approval and360 feedback are later optional
  extensions, outside this ordered implementation queue.
- Reuse the same usage-reset automation and task checkpoint. Continue until Kirk
  stops the work, the agreed scope is complete or a genuine handoff blocks it.
