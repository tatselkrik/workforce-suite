# Review checklist: October 22–23, 2026

This covers changes committed after **October 22 at 3:54 p.m.**, through the
career development milestone in **0.1.31** (local time, UTC+8). Earlier employee,
201-file, organization, leave-policy, shift, break and time-calculation features
are outside this change window. The list follows the actual release history,
versions **0.1.19–0.1.31**.

These are suggested hands-on checks, not a requirement to repeat every automated
test. Use synthetic records. Confirm the version at the top right before checking
career plans. Ordinary Employee accounts still use the future separate portal;
HR, Administrators and Managers use this desktop.

## Changes to check

- [ ] **Attendance correction requests — 0.1.19.** Open Attendance > Correction
  requests. Submit an own missing/incorrect punch for a chosen day with a reason.
  Confirm submission leaves recorded attendance unchanged. A different eligible
  Manager/HR reviews it; approval applies the exact proposal and retains both
  submission and correction history. Reject/cancel, self-approval and stale
  attendance should behave as explained on screen.
- [ ] **Overtime approvals — 0.1.20.** Open Attendance > Overtime approvals. On a
  completed shift with an eligible calculated candidate, request some minutes.
  A different reviewer can approve fewer minutes or reject with a reason. Inspect
  revoke/history. Later punch/break changes should make an old approval historical,
  requiring a fresh review. Approved minutes are not a payroll calculation.
- [ ] **Detailed reports and CSV exports — 0.1.21.** In Reports, switch between
  attendance detail, leave charges and employee summaries. Change period, employee,
  department and status filters; export and compare the CSV with the selected view.
  Missing/incomplete attendance, saved-rule values and approved overtime should
  remain distinguishable.
- [ ] **Employee CSV imports — 0.1.22.** HR/Admin can use the template, preview a
  small fictional batch, inspect row errors and import valid new employees with
  contacts/emergency details. Confirm an invalid batch makes no partial changes,
  existing employees are not silently updated and the batch/history can be opened.
- [ ] **Personal saved report views — 0.1.23.** Save a filtered view, edit/apply it,
  sign out/in and confirm it remains. Test a rolling period and delete the view.
  Saved views belong to the signed-in account and do not widen its permissions.
- [ ] **Workforce analysis and selected report columns — 0.1.24.** Review current
  headcount by department, location and employment type. Change the trend period
  and daily/weekly/monthly grouping. Select and reorder report columns and compare
  the export. Historical trends describe saved attendance/leave, not historical
  headcount or a prediction.
- [ ] **Notification inbox — 0.1.25.** Open Notifications at the top. Inspect an
  eligible approval/reminder, mark it read/unread and sign in again. Read markers
  should persist. HR-only document-expiry reminders must not appear to Managers.
  This is an in-app inbox; it does not send external email or phone notifications.
- [ ] **Applicant pipeline — 0.1.26.** HR/Admin open Hiring, add a fictional
  applicant, search/filter, change stage with a reason, and archive/restore.
  Reopen saved change history. Applicants must remain separate from employees
  and accounts until the approved hiring conversion.
- [ ] **Recruiting files, interviews, offers and onboarding — 0.1.27.** Add/download
  a fictional résumé, schedule an interview and record the assigned interviewer's
  feedback. Have an Administrator approve an offer, record acceptance and convert
  it once into an employee. Review assigned onboarding tasks and their history.
  Conversion must not automatically create a sign-in account; Managers see only
  their assigned interviews, feedback and checklist tasks. No calendar/email invite
  or signed-offer service is included.
- [ ] **Exit and clearance — 0.1.28.** HR/Admin open Exit & clearance, create a
  fictional separation and assign checklist tasks. Review completion/waivers and
  readiness blockers. Finalization is Administrator-only: it disables access and
  archives the employee atomically while preserving the account and history.
  Use a synthetic employee for this check; it deliberately changes access/status.
- [ ] **Goals and appraisals — 0.1.29.** Open Employees > Development. Add a goal,
  record progress, create a review cycle and assign an eligible reviewer. Save and
  submit feedback, then publish as HR. Inspect the saved goal facts and separate
  HR-only notes. Changing a reviewer must not carry the previous private draft to
  the replacement; published facts remain historical. Unfinished appraisals block
  exit finalization.
- [ ] **Training and certifications — 0.1.30.** Assign training, record progress,
  complete it as HR with a factual date/outcome and optional learning minutes, then
  reopen and inspect history. Add a certificate, check expiry, revoke/archive/
  restore and verify revocation is retained. HR may link an existing same-employee
  private 201 document; Managers receive metadata/progress without private file
  bindings or HR completion controls.
- [ ] **Career plans and milestones — 0.1.31.** Open Employees > Development >
  Career plans. Create a plan for another employee with aspirations, skills and
  shared notes. Add required/optional milestones and optional related goal,
  training or certificate context. Record progress and independently review a
  completion; only independent HR can waive. Complete the plan after at least
  one done milestone and no required milestone remains open. Reopen and inspect
  the earlier saved milestone facts in history. Changing a related source later
  must not rewrite those facts or change the employee's actual job/department/pay.

## A short cross-cutting check

- [ ] Check light/dark themes, enlarged display, long names/text, scrolling,
  visible save/discard controls and history column alignment.
- [ ] Check HR/Manager access with separate staff accounts. Managers should see
  only current reporting or explicitly assigned records. Account Manager remains
  Administrator-only; ordinary Employee desktop sign-in remains blocked.
- [ ] Refresh/reopen a saved synthetic record. A failed save should keep the draft;
  retrying an uncertain creation should reuse its original identity.
- [ ] Confirm the version/update banner after signing in. Updates require the
  private signed channel; available-update messages persist. Existing company
  connection and verification settings should survive an update.

## Remaining roadmap

| Original area | Prototype status and remaining work |
| --- | --- |
| Accounts, roles and approval chains | Core staff roles and protected permissions implemented; separate employee portal, custom role builder, delegation/escalation and broader configurable approval chains remain. Leave already has ordered configurable chains; attendance/overtime review is one step. |
| Employee details, contracts, attachments, history and 201 files | Core implemented. Effective-date employment actions, e-signing and document approval workflows remain enhancements. |
| Reporting relationships, managers, chart, locations and employment types | Core implemented. No automatic promotion or employment changes from a career plan. |
| Shifts, breaks, lateness, undertime, overtime and corrections | Core implemented. Biometric control is intentionally visible/disabled; real integration needs a chosen device/protocol and physical verification. |
| Holidays, workweeks, half-days, leave types, accrual, carryover and cancellation | Core implemented. Effective-date policies, opening-balance imports, posted credit adjustments and delegation/escalation remain enhancements. |
| Payroll and benefits | Not implemented: salary runs, deductions, allowances, payslips, benefits, reconciliation and local reporting. Company policy/effective dates and current statutory rules need verification before workplace use. This is the next main roadmap module, not started in this milestone. |
| Applicants, interviews, offers, onboarding and exit | Core implemented. External invitations/calendar delivery, electronic offer signing, company exit paperwork and human final-pay review remain. |
| Goals, appraisals, feedback, training, certificates and career development | Core implemented through this milestone. Employee portal UI, 360 feedback, bulk/catalog learning/provider connections and dedicated performance reminders remain enhancements. |
| Reports, imports/exports, custom reports and workforce analysis | Core reports/CSV, create-only employee imports, selected columns, saved views and recorded trends implemented. XLSX formatting, attendance/leave imports, controlled bulk updates and a broader custom report builder remain. |
| Notifications, reminders, expiry alerts and integrations | In-app inbox/core reminders implemented. Reliable company email/confirmation delivery and payroll/accounting/device integrations with retries and audit remain. |

Further development and the automatic resume scheduler are paused at Kirk's
request after this milestone. No new payroll or portal implementation starts
without a new instruction. The detailed roadmap is in [ROADMAP.md](ROADMAP.md).
