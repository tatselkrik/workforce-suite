# Applicant pipeline — 0.1.26

**Hiring** is available to active Administrators and HR Generalists. Managers
and portal-only Employee accounts cannot load applicant records or history.
Assigned interviewer access is a separate future permission scope.

Add an applicant, then click their name to open one record. Keep full name,
position applied for, application date, optional email/phone/source and private
HR notes. Search the list, filter stages and optionally include archived records.
Application dates use the company time zone and cannot be in the future.

Save changes with a reason. The pipeline supports Applied, Screening, Interview,
Offer, Rejected and Withdrawn. Applied can advance to Screening or Interview;
Screening can advance to Interview or Offer; Interview can advance to Offer.
Any open stage can end as Rejected or Withdrawn. Reopen either terminal outcome
at Applied. Every change needs a reason. Save detail edits before changing a
stage or archiving. **Offer does not mean hired.**

Archive/restore retains the entire application and event history. Archived
records are read-only until restored. The History tab records each save, stage
change, archive and restore with protected author/role, server time, reason and
before/after values. History cannot be edited or deleted through the client.

Applicants are separate from employees and sign-in accounts. Adding or advancing
an application creates neither. Applicant contacts and HR notes stay outside
employee directories and general workforce reports. Resume attachments,
interview scheduling/feedback, offer documents, approved employee conversion,
onboarding and exit/clearance remain later modules. No job-board posting or
external communication is included.

## Persistence and concurrency

Migration `20261022172521_applicant_pipeline.sql` creates `applicants` and
`applicant_events`. Both use current protected HR membership for SELECT and
revoke direct client writes. The public invoker `manage_applicant` RPC calls a
private guarded implementation that serializes company writes, rechecks active
membership, validates a fixed editable-field set and derives audit identity.
Row updates and their history event commit together.

Edits, stages and archive/restore require the loaded revision. A stale request
returns conflict without changing either record or history. Refresh preserves
the local draft; explicitly reload the saved record before continuing. Failed
saves keep drafts open. Each new draft keeps one UUID across retries; replaying
the same original creation by its author returns the current record without
duplicating events or overwriting later changes. A changed retry is rejected.

Older clients remain compatible with the additive migration. The private update
requires this schema version. No Edge Function changes or new dependencies are
needed. Local PostgreSQL, Rust/HTTP, actual Auth and native layout evidence is
recorded in the release verification report; a local UI preview alone does not
establish hosted authorization.
