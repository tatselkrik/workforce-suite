# Benefits administration

Benefits administration is an HR Generalist and Administrator workspace under Employees. Employee, Manager, Recruiter,
Learning & Performance Officer and Payroll Officer alone have no benefits-admin
access. Use protected current Personnel capability, never Auth metadata, navigation
visibility or payroll permissions. Keep the context outside Snapshot, Payroll,
employee-app payloads, notifications and general reports. Benefit records are managed within the suite. Employees can already raise an own benefits question through Services.

HR maintains benefit plans with a unique code, category, description, eligibility
guidance, dependent limit and availability. Eligibility is an explicit recorded
HR review (Pending, Eligible or Ineligible) with its basis and captured current
employment/plan source. Guidance is not an automatic statutory eligibility engine.
Do not infer missing service dates, insurance acceptance, statutory entitlement,
coverage or vendor action. Plan revisions and complete prior snapshots remain
immutable; editing a plan does not rewrite an earlier enrollment review.

Enrollment records bind one employee and plan, captured plan revision and current
employee source, coverage dates, eligibility decision and Pending/Active/Declined/
Ended state. Activation requires recorded Eligible and an available current plan.
Plan/employment changes require fresh explicit review before a later edit; ending
existing coverage may retain its original source. Keep ended records terminal.
Reject overlapping non-declined coverage for the same person/plan; do not silently
replace prior coverage. Archiving an employee does not silently terminate coverage.
New activation requires active employment, while HR can retain/end past coverage.

Dependents have stable identities, name, relationship, optional date of birth and
individual coverage dates within the parent's interval. Keep them HR-private.
Allow namesakes, enforce plan count limits and retain before/after history when
corrected or removed. Do not collect identity numbers, medical details, credentials
or attachments in this increment. Every edit requires a reason and immutable audit.

Use guarded additive private tables/RPCs, current company/account lock, exact
actor/full-payload retry identities, optimistic entity revisions, source checks and
atomic before/after events. Recheck current authority before returning old receipts.
No direct authenticated/anonymous table grants or delete controls. Reject context/
history budget overflow explicitly. Native drafts preserve uncertain exact retries;
confirmed rejection requires fresh review; role loss clears content and edit undo.

This records company-reviewed administration only. Benefits changes never create
salary, allowance, deduction, remittance, payment, tax or payslip changes. Monetary
effects remain explicit separately reviewed Payroll inputs under existing controls.
An enrollment record does not contact an insurer, submit a filing or transfer money.
