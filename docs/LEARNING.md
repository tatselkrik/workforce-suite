# Training and certifications

Implemented in version 0.1.30 with migration `20261023003106`.

The location is Employees > Development, alongside Goals/Appraisals.
HR/Admin define training, dates/provider, required flag and completion/outcome
with optional learning minutes. Current staff can record progress on their own
training; Managers also record current-report progress. HR records completion,
cancellation and reopening. Completion is a factual past date and outcome,
not an accredited-course or statutory-compliance assertion. Dates do not create
attendance, leave or payroll entries and training does not block exits.

HR/Admin maintain certificate title/issuer/reference and issue/expiry dates,
revoke/reinstate and archive/restore with reasons and revision history.
Expiry is derived from the company date; archival preserves the validity state.
Managers/ordinary future-portal accounts read only own/current-report metadata.
Ordinary employees still cannot enter the desktop or invoke staff mutation RPCs.

Optional evidence links to an existing ready, unarchived private 201 document
for the same company and employee. Binding rows and their events are HR-only;
no file path/key/document identifier enters Manager or employee summaries or
public certificate history. Clearing/replacing a link retains its audit and the
original document. Later document archive does not erase the link or history.
Upload/download remains within the existing protected Document workflow.

Current membership, role/reporting scope, company serialization, exact revision,
strict payload fields, atomic audit and actor-bound birth retries are checked
by the server. Existing records survive employee archive; no new training or
certificate is created for an archived employee. HR can correct retained facts.
Career plans now reference limited learning metadata without changing learning
facts; see CAREER-DEVELOPMENT.md. Catalog/bulk enrollment, course providers/
invitations, external accreditation checks, performance inbox reminders and
portal UI remain later.

Verification is recorded in VERIFICATION.md and the versioned release report.
Local PostgreSQL tests simulate Auth/Storage metadata; actual Auth/Rust/REST
checks separately verify protected access and a private empty evidence binding.
No new S3 upload test is claimed; the existing 201 pipeline is unchanged.
