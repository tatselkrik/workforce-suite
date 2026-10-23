# Payroll cutoffs — version 0.1.34 review

1. Open **Payroll → Cutoffs → Cutoff runs**. Create a completed historical month with cutoff1 (1st–15th), a title and planned pay date; add a synthetic employee with a salary profile.
2. Open the worksheet. Review actual dates, attendance proposal, holidays/leave, OT authorization and night minutes. Missing attendance must not deduct pay automatically. A mid-cutoff hire needs explicit documented proration. Save the reviewed worksheet.
3. Submit after checking included/omitted employees and pay date. Self-approval must fail; another Administrator approves. HR review/exception notes remain private.
4. Create cutoff2 (16th–month-end). It must require the issued first cutoff for an existing employee. Refresh after source changes and review again; unsaved form adjustments must survive a source refresh.
5. Compare basic and recurring amounts across both halves, monthly contribution deductions on the second, monthly exemption caps, semi-monthly withholding and the monthly reconciliation CSV. Overnight OT authorizations must not be reused after first-cutoff usage unless an explicit HR exception is reviewed.
6. Export an issued cutoff payslip as PDF and CSV. Check company, employee, exact dates, planned pay date and aligned amounts. A Manager must see only their own issued slips and no draft salaries.
7. Try overlapping issued monthly-register pay for the same employee/month: both directions must reject it. Issued slips must stay unchanged after later salary/source edits.
8. Record an actual payment date/reference only for a synthetic demonstration or an independently verified payment. Recording moves no money and does not file returns.

This prototype uses fixed semi-monthly halves and a stated second-cutoff contribution policy. Actual company pay schedules and any other policy need review. The paused employee portal has not added cutoff slips. Monthly-register annual exports exclude cutoff pay; use the explicitly labeled cutoff reconciliation for those records. Actual 13th-month payouts, annual tax adjustment/prior-employer opening balances and final pay remain next milestones.

Release evidence distinguishes local automated calculations/access tests, actual hosted Auth/Rust workflows, fictional native previews, normal configured startup and exact-installer checks. No claim is made that a signed-in native form was operated automatically or that public CI or Authenticode signing ran.
