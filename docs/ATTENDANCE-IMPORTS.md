# Attendance correction request imports

Desktop0.1.51implements the attendance part of roadmap6. Its versioned review
checklist records the behavior and verification boundaries. Native Excel is
delivered in0.1.50; leave-request batch imports follow.

HR/Admin can prepare1–100CSV/Excel rows using employee_code, work_date,
checked_in_at, checked_out_at, reason and reference. Work dates use ISO dates
(or a date-only Excel cell). Timestamp columns stay Text with seconds and an
explicit Z/offset, up to microsecond precision. Blank checkout proposes an open
record. Use an active employee and an allowed past/current date/clock interval.
Recorded breaks, existing attendance and dated overnight shifts remain binding.

Opening/previewing creates no requests. Local errors preserve source row numbers;
the protected preview rechecks current company/membership, employee, attendance,
break, shift and pending-request facts. Duplicate employee/date rows and mutually
overlapping file proposals block the batch. Post binds the full payload and source
fingerprint under the normal company lock, all rows or none. Retry receipts bind
the original actor and complete payload; a timeout is not permission to create a
different batch. No raw file/path is uploaded or retained.

Rows create standard PENDING correction requests, not attendance. The actual HR
importer is submitted_by; there is no JWT/employee impersonation. The existing
self-only attendance_request API stays unchanged. Normal reviewers, source checks,
delegation, submitter/self exclusions, calculation capture and correction history
still apply at approval. There is no automatic approval or payslip rewrite.

The original importer, while still HR/Admin, may withdraw an unchanged pending
imported request through the separate audited import adapter. This does not
broaden the self-only cancellation API. A reviewed request cannot be withdrawn.
Individual normal approval/rejection/cancellation remains reflected in history.

Batch import notes and references are HR/Admin-private. A row's correction reason
and withdrawal reason appear in normal employee/reviewer history. Import ledger
tables have RLS, no direct authenticated table access, immutable entries and
protected scoped RPCs. Managers/Employees see only the ordinary requests allowed
by their existing own/team/delegated scopes, never private batch metadata.

History has explicit500batch/5,000item limits with no silent truncation. CSV/XLSX
review exports recheck protected access after rendering and preserve existing
files. Native drafts retain exact uncertain submissions/withdrawals, clear on
scope change and participate in unsaved-work protection. No paid service, account
creation, external/device connection, portal restart or public hosting is added.
