# Hiring workflows
**Hiring** gives Administrators and HR Generalists private applicant records, résumés, offers and onboarding cases. Managers see their assigned interviews, their own interview feedback and assigned onboarding tasks. Android-only Employee accounts cannot sign into this desktop. Managers never load applicant HR notes, attachments or pay/offer terms.

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
employee directories and general workforce reports. Hiring from an accepted,
Administrator-approved offer creates one employee and onboarding checklist
atomically. It does not create an Auth account; provision access separately in
Account Manager when appropriate. The existing six pipeline stages remain
compatible with older clients; a separate Hired indicator and conversion record
retain the employee link. Pipeline stage changes are blocked after hiring.

## Résumés and application files

Open an applicant, then **Résumés / files**. Add a PDF, DOCX, PNG or JPEG up to
10 MiB with a title and optional notes. Download original bytes to a new local
file, edit metadata, and archive/restore without replacing the original.
Pending uploads retain their identity across retries; retry the exact file or
discard the incomplete reservation. Discarded reservations cannot be finalized.
The separate private applicant bucket never mixes files with employee 201 files.
Files are not automatically copied to the employee record when hiring.

Downloads use a fresh query identifier and no-cache headers to recheck access.
Live testing found a previously authorized identical Storage URL could return a
cached response immediately after disabling its HR account. Fresh requests were
denied; the desktop bypasses that cache. Previously downloaded local files cannot
be recalled by disabling an account. The bucket remains private and no signed
public download URLs are issued.

## Interviews

Use an applicant's **Interviews** tab or the global **Interview agenda**. HR
schedules a title, active staff interviewer, company-local date/start/end,
location and instructions visible to that interviewer. Durations must be
positive and no more than eight hours; scheduled assignments cannot overlap
for the same interviewer. Rescheduling, cancellation and completion require a
reason and current revision. Future interviews cannot be completed.

The assigned interviewer can record a 1–5 rating, Proceed/Hold/Not recommended
and a written assessment after the start. Corrections require a reason and
revision. HR sees all retained assessments; a reassigned Manager sees only
their own feedback. Reassignment removes the previous Manager's interview and
feedback access. Private applicant notes and résumés stay with HR.

## Offers and approved hiring

From **Offers**, prepare job title, department, start date, exact base amount,
three-letter currency, monthly/annual/hourly pay period, terms/benefits and an
optional expiry. An Administrator approves and locks terms. HR records external
delivery and candidate acceptance/decline with reasons. The app does not send
email, obtain a digital signature, calculate payroll or claim statutory
compliance. Revised approved terms require withdrawal and a new draft. One
active offer per applicant prevents contradictory simultaneous offers.

After acceptance, **Hire and start onboarding** asks for employee code, reporting
manager, employment type and work location. Approved job/department/start and
current applicant name/contact details populate the employee. Emergency details
remain unset. Employee, contacts, immutable employment history, conversion and
five initial required tasks commit together. A failed write leaves no partial
hire; an exact retry returns the original employee/checklist.

## Onboarding

Open the applicant's **Onboarding** tab or global **Onboarding** queue. The
initial checklist covers contact/emergency verification, contract/documents,
orientation, equipment/access and introductory training. HR adds custom tasks,
sets instructions/due date/required status and assigns active desktop staff.
HR or the assigned Manager records completion with a reason; only HR waives
requirements or closes/reopens the case. All required tasks must be resolved
before closing. Closing locks tasks until HR reopens the case. Reassignment
and disabled membership immediately restrict fresh task/history reads.

Each task and case retains protected actor, reason, server time and before/after
history. Task instructions are visible to assigned staff; keep private HR/pay
notes in the applicant or offer instead. Applicant self-service, job-board posting, calendar/email invitations, signed offers and
exit/clearance remain subsequent modules.

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

Migrations `20261022180842` and `20261022183649` add private attachments, interviews, offers and onboarding using protected current memberships, invoker RPCs and private guarded writers.

Older clients remain compatible with the additive migrations. The private update
requires this schema version. No Edge Function changes or new dependencies are
needed. Local PostgreSQL, Rust/HTTP, actual Auth and native layout evidence is
recorded in the release verification report; a local UI preview alone does not
establish hosted authorization.
