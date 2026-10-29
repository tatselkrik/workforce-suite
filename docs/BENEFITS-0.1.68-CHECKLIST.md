# Benefits administration —0.1.68 review checklist

In HRIS open **Employees > Benefits > Load benefits**. Use fictional active
employees and fictional plan/dependent information for testing.

- Add a plan with a unique uppercase code, category, description, eligibility
  guidance, dependent limit and availability. Record a reason and confirm review.
- Create a Pending enrollment and recorded eligibility review for a selected
  employee and plan. Eligibility is an explicit HR decision with its basis.
- Review/edit the enrollment to Active only after marking it Reviewed eligible.
  Check coverage dates and dependent limits; conflicting coverage is rejected.
- Add/edit dependents with stable identities, relationship, optional birth date
  and coverage within the employee's interval. Namesakes remain separate people.
- Edit a plan. Earlier enrollment reviews retain their captured plan revision.
  Explicitly refresh/review current plan and employment details before a later edit.
- End active coverage with a final date and reason. The original review and
  dependent identities remain; dependent end dates are limited to the parent's
  final date. Ended history is retained, without a delete/reactivate control.
- Open Retained history and Saved fields. Reasons, actor and company-local time
  remain visible. Archiving does not silently end coverage or change payroll.
- Check light/dark and compact views; scroll to the lower save/review controls.
  An uncertain save keeps one frozen Retry same save. A confirmed rejection
  requires current saved details to be reviewed or the draft to be discarded.
- Administrator/HR Generalist can administer benefits. Manager, Employee,
  Recruiter, Learning & Performance Officer or Payroll Officer alone cannot.
  The separate Payroll Desktop and local employee portal expose no benefits-admin data.

These are recorded company reviews. There is no insurer/vendor connection,
automatic statutory eligibility decision, salary/deduction change or external
communication. Monetary effects remain separately reviewed Payroll inputs.
