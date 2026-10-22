# HRIS development roadmap

The HR workspace remains a native Rust desktop application for Administrators, HR Generalists, Managers and later assigned staff roles. Ordinary employees use a separate future mobile-friendly portal. Each company has its own dedicated Supabase deployment; the prototype is a reusable base for separately configured companies. Build and verify one module before moving to the next.

Broader product benchmarks combine many of these modules: [BambooHR](https://www.bamboohr.com/platform/) lists employee data/reporting, time, hiring, payroll and performance; [SAP HCM](https://www.sap.com/products/hcm.html) spans core HR, payroll and talent; [Sprout](https://sprout.ph/) combines HRIS, payroll, analytics and talent workflows. This roadmap is a development plan, not a claim of feature parity.

## Stage 1 — Accounts and permissions

Implemented: administrator-only Account Manager, above Settings. Initial roles are Administrator, HR Generalist, Manager and Employee. Every account receives an employee record; administrators assign roles, enable or disable access, and view access changes. Protected database memberships enforce permissions. Ordered leave approvals now use configurable per-type steps with protected reviewers; nobody approves their own leave. See [leave policies](LEAVE-POLICIES.md).

Employee accounts are prepared for the portal and cannot sign into the desktop. Build the mobile-friendly portal separately for employee record access, attendance, leave requests, profile/security and notifications, using the same protected company membership scopes. A portal implementation is not part of this first desktop module.

## Stage 2 — Employee records and organization

Implemented: one employee editor with private contact/emergency details and reporting relationships, including employees without accounts (see [contact permissions](EMPLOYEE-CONTACTS.md)). Configurable employment types/work locations, employee assignments, read-only Profile fields and employee filters are implemented. Automatic HR-only employment history with starting snapshots and immutable before/after events is implemented (see [employment history](EMPLOYMENT-HISTORY.md)). Private contracts, attachments/digital 201 files with metadata, original downloads, retention and history are implemented (see [employee documents](EMPLOYEE-DOCUMENTS.md)). An expandable organization chart uses saved reporting relationships, scoped employee cards and department/location filters (see [organization chart](ORGANIZATION-CHART.md)). Effective-date employment actions, e-signing and document approval workflows remain future work. The organization chart peer alignment is verified at normal and enlarged scales.

## Stage 3 — Leave policies and approvals

Implemented: company holiday calendars and custom workweeks, full/morning/afternoon requests, configurable leave types, annual/monthly entitlement, capped expiring carryover, cancellation, pending reservations and per-type 1–5 ordered approval steps with retained reviewer/date history. Existing requests retain original charges; new policy versions do not rewrite filed plans. See [leave policies](LEAVE-POLICIES.md). Delegation, automatic escalation, effective-date policies, imported opening balances and posted credit adjustments remain future work. Attendance calculation rules are now implemented separately; leave charges remain unchanged.

## Stage 4 — Attendance rules

Direct HR attendance corrections with required reasons and before/after history are implemented. Shift schedules, dated employee assignments, retained schedule history and overnight attendance attribution are implemented. Paid/unpaid shift break rules, own break controls, reasoned HR corrections/voids and actual unpaid totals are implemented (see [attendance breaks](ATTENDANCE-BREAKS.md)). Configurable late-arrival/early-departure grace, overtime minimum/rounding and optional pre-shift time are implemented with saved rules and reasoned HR application/history (see [attendance calculations](ATTENDANCE-CALCULATIONS.md)). Overtime values are candidates for review, not approved/payable time. Own attendance correction requests with single-step Manager/HR review, no self-approval and retained submission/decision history are implemented (see [attendance requests](ATTENDANCE-REQUESTS.md)). Next add a separate overtime approval workflow. Biometric integration is visible but disabled in this prototype. Actual device support requires choosing a device/protocol and verifying it against physical hardware.

## Stage 5 — Reports and data exchange

Add detailed attendance/leave reports, spreadsheet exports, validated imports with previews/error reports, then custom reports and workforce analysis. Apply the same role scopes to reports and exports as to on-screen records.

## Stage 6 — Notifications and connections

Add in-app notifications, reminders and document-expiry alerts. Configure a usable email-confirmation destination and reliable company email delivery. Connect payroll/accounting/attendance systems through explicitly configured integrations, with retries and audit history.

## Stage 7 — Hiring, onboarding and exit

Add applicant tracking, interviews, offers, onboarding checklists, exit tasks and clearance workflows. Keep applicant data separate from employee records until hiring is approved.

## Stage 8 — Development and performance

Add goals, appraisals, feedback, training, certifications and career development, with privacy appropriate to each record type.

## Stage 9 — Payroll and benefits

Add salary calculations, deductions, allowances, payslips, benefits and local reporting workflows after attendance/leave policies are stable. Verify current Philippine rules, effective dates, approval controls and reconciliation with a qualified payroll practitioner before workplace use. Do not infer statutory rates from prototype defaults.

The sequence may change when the company has a more urgent requirement. Payroll, biometric devices and external integrations require company-specific decisions; they are not generic plug-ins already working in this prototype.
