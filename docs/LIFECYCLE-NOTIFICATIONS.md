# Payroll, appraisal and learning notifications — implementation contract

Delivered in desktop0.1.46 and the separate local portal0.1.6. It introduces no paid
delivery provider, external messages, money movement or payroll calculation.
Portal visuals/email confirmation and exact government-file acceptance remain
explicitly pending while this independent authorized work proceeds.

New notices use a protected minimal projection. Payslip/advice publication and
individual payment recording are distinct events; batch status never establishes
another person's receipt. Own notices exclude amounts, bank references, private
notes, evidence, tax identifiers and source bundles. Final-settlement recording
must not describe a contained source as a second cash payment.

Assigned current reviewers receive appraisal deadlines; eligible HR/Admin may
receive submitted reviews awaiting publication. Own published-review notices
contain no ratings or private notes. Training start/due and certificate-expiry
reminders follow current own/reporting/HR scope, active employees and unresolved
statuses. Completion/cancellation/reassignment/archival removes obsolete tasks.
Recorded certificate validity is not an accreditation/compliance assertion.

Keep lifecycle read markers separate from legacy desktop/portal marker tables.
Older released clients must still load their original keys. Use protected
surface-specific markers and caller-derived identity; a portal staff user still
gets own notices only. Mark-read/reset must not approve, change pay, attendance,
reviews, learning or source history. New/mixed marker writes are transactional.

Native notice links open the matching authorized record. The portal may show
safe informational notices for own records whose full portal view is not yet
implemented; do not imply an unavailable document can be opened. Preserve the
running0.1.5 portal and its drafts; new portal delivery has a separate lifecycle.

Verification must cover owner/foreign/disabled/role-change/forged metadata,
old-client compatibility, exact receipt date versus event date, private-field
omission, duplicate keys, read-state concurrency and time-zone/stage boundaries.
Implementations and current status are recorded below as work is verified.

Verified25October2026:448default native Rust checks,48portal Rust checks and15JS
checks;4,549PostgreSQL assertions and fresh61migration replay. The additive
migration preserves92prior public record sets and267functions. Hosted normal
synthetic pay approval/individual receipt, current HR/Manager/reporting scope,
forged role/target denial, legacy portal compatibility, native decoding and actual
optimized portal own inbox/CSRF/read/revocation pass. Existing payroll, employer
registration and Auth facts remain unchanged. The synthetic receipt is clearly
labeled prototype QA and sends no money. No real person's account was changed.

Desktop0.1.46 is installed with matching hash/BUILD/Windows Apps and normal
launch/close, exact installer recovery and private-channel download/signature
verification. Portal0.1.6 is delivered separately, preserving running0.1.5 drafts.
See [evidence](NOTIFICATIONS-0.1.46-VERIFICATION.md). Portal current visuals and
actual mail confirmation remain unverified. Email/push delivery is a separate
held/unconfigured scope; no provider or billing settings changed.
