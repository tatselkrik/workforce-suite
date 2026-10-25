# Payroll, appraisal and learning notifications — implementation contract

This is the next free in-app milestone after0.1.45. It introduces no paid
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
