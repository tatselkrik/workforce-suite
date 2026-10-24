# Remaining HRIS roadmap — agreed order

Kirk approved this order on24October2026 and resumed continuous work and the
existing usage-reset follow-up. This supersedes the stop-after0.1.41 boundary.
The original roadmap status remains in [ROADMAP.md](ROADMAP.md).

Latest instruction25October2026: Kirk resumed the roadmap, reactivated the same
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
3. **Payroll, appraisal and training notifications.** Payslip/payment notices,
   review deadlines, training reminders and certificate-expiry alerts.
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
9. **Actual biometric/device integration.** Select hardware/protocol and verify
   physical operation. Keep the visible control disabled until then.

## Progress and boundaries

- Desktop0.1.41 is already installed and verified; do not repeat its release.
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
  runtimes fail initialization. Pictures/security remain queued, so milestone1
  stays partial. Work and scheduling resumed after the completed storage cleanup.
- Portal increment0.1.4 implements private own circular photos/preview/save/
  discard/removal and deployment-bound company sign-in branding.38Rust/9JS checks
  and9actual hosted proof groups pass. A stale-image removal issue was fixed with
  fresh Storage origin reads. Current visual verification remains pending, and
  password/email controls follow next; milestone1 stays open.
- Portal increment0.1.5 implements own password/email status/changes and signing
  out other sessions.47Rust/13JS tests and9hosted proof groups pass, including a
  synthetic password change/restoration and retained foreign sessions/real Auth
  users. Actual email confirmation delivery and current UI visuals are unverified;
  milestone1 remains partial. Continue independent government-export work while
  these limitations remain recorded; no confirmation/provider rules are weakened.
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
