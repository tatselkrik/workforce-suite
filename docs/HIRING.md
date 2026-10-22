# Applicant pipeline foundation - unreleased

The installed application remains version0.1.25. This foundation is source
work for the next hiring module. Applicant screens, database persistence,
history and hiring conversion are not implemented or deployed yet.

Applicants remain separate from employees and sign-in accounts. The first
record collects name, optional email/phone, applied position, source, application
date and private HR notes. It does not collect employee emergency contacts or
create company membership. Initial access is HR/Admin-only; assigned Manager
interview access can be added later with its own protected scope.

The domain model supports Applied, Screening, Interview, Offer, Rejected and
Withdrawn. Forward review transitions and terminal outcomes need reasons;
reopening starts at Applied. Offer does not mean hired. Stage payloads require
the current revision and an unarchived record. Drafts retain one UUID across
retries and serialize only editable fields, excluding company, role, account,
employee, actor and revision assignments.

Five targeted Rust tests verify field/date bounds, transitions/reasons, private
scope, stable draft identity, protected-field exclusion and stale/archive guards.
These are local domain checks; they establish no server authorization or UI
completion. Existing version0.1.25 artifacts are unchanged.

Next implement additive applicants and immutable event tables. SELECT must be
limited by protected current HR membership; revoke direct client mutations.
A public invoker RPC should call a private guarded implementation, derive the
actor/company from membership, lock/recheck the company and application, and
independently enforce field validation, current revision and stage transitions.
Capture before/after state, reason, protected actor and server time in each
event. Create starts Applied; archive/restore preserves the record and history.

Verify invalid/stale writes are atomic and test Manager/portal/disabled/forged
metadata/crosscompany denial and immutable history before deployment. Then wire
the native list/editor/history and actual Auth/visual checks. Resume attachments,
interviews, offers, approved hire conversion and onboarding follow separately.
No public job posting, email delivery or external messages are included.
