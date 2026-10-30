# Exit and clearance

Available in desktop version 0.1.28 with migration `20261022193159`. Open
**Exit & clearance** to start or search a case, assign tasks and review readiness.
HR can open the canonical employee record from the case. Managers receive
only their assigned checklist. Failed saves/refreshes preserve form drafts;
creation retries retain the original identifier and never overwrite later edits.

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
must finalize, on or after the last working date. Current readiness checks include
active employee status, required tasks, active direct reports, open attendance,
pending requests filed for that employee and protected owner/self accounts.
Readiness also blocks unfinished named leave approvals, active leave types naming
the employee, open assigned onboarding/clearance tasks and scheduled interviews.
Resolve or reassign those items before finalization; filed leave chains require
cancellation and resubmission if their named approver must change. The existing
account safeguards still apply. Finalization disables sign-in and
archives the employee in the same transaction as the exit/history changes;
an audit or archive failure rolls everything back. Accounts, employee records,
contacts, documents and previous history are retained. Restoration/rehire is a
separate deliberate workflow. No email or external message is sent.

Finalized cases remain read-only; cancelled cases retain their checklist and
history without changing employment or sign-in. Reopen a cleared case before
editing tasks or exit details. A new case may follow a cancelled case while the
employee remains active. Restoration/rehire is separate from exit finalization.

Company-specific legal, document and payroll requirements need an appropriate
clearance checklist. Exit readiness describes recorded checks rather than a full
compliance assessment. Final pay is handled separately in Workforce Payroll;
clearance does not transfer money or delete Auth identities. Filed leave approval
plans remain immutable, and operational substitutes use the protected delegation
workflow.
