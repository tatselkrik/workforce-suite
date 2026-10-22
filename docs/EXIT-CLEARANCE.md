# Exit and clearance — under development

This module is not included in installed version 0.1.27. Its database migration
is a local, unreleased candidate; desktop screens and hosted Auth verification
remain to be implemented. Existing recruiting and onboarding remain available.

Separate `exit_cases`, `clearance_tasks` and `exit_events` preserve compatibility
with installed onboarding clients. HR/Admin prepare a last working date,
separation category and private notes. Five initial required tasks cover handover,
property return, access review, HR records and human payroll/final-pay review.
The prototype does not calculate final pay or establish legal compliance.

Only HR/Admin receive case details. Managers receive their explicitly assigned
tasks with minimal saved employee name/code/last day and task instructions;
private separation reasons and notes remain outside that projection. Task
completion, HR waiver, reassignment and history use protected memberships,
revisions and immutable events. Cleared or cancelled cases lock their tasks.

HR can record clearance after resolving every required task. An Administrator
must finalize, after the last working date. Current readiness checks include
active employee status, required tasks, active direct reports, open attendance,+pending requests filed for that employee and protected owner/self accounts.
The existing account safeguards still apply. Finalization disables sign-in and
archives the employee in the same transaction as the exit/history changes;
an audit or archive failure rolls everything back. Accounts, employee records,
contacts, documents and previous history are retained. Restoration/rehire is a
separate deliberate workflow. No email or external message is sent.

Before release, add native readiness explanations, employee/task navigation,
guarded drafts, scoped transport tests and actual Auth checks. Verify assigned
approval handovers and company-specific exit requirements explicitly; the
initial readiness projection is not a complete offboarding compliance check.
