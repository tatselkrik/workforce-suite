# HRIS development roadmap

The HR workspace remains a native Rust desktop application for Administrators, HR Generalists, Managers and later assigned staff roles. Ordinary employees use a separate future mobile-friendly portal. Each company has its own dedicated Supabase deployment; the prototype is a reusable base for separately configured companies. Build and verify one module before moving to the next.

Broader product benchmarks combine many of these modules: [BambooHR](https://www.bamboohr.com/platform/) lists employee data/reporting, time, hiring, payroll and performance; [SAP HCM](https://www.sap.com/products/hcm.html) spans core HR, payroll and talent; [Sprout](https://sprout.ph/) combines HRIS, payroll, analytics and talent workflows. This roadmap is a development plan, not a claim of feature parity.

## Stage 1 — Accounts and permissions

Current work: administrator-only Account Manager, above Settings. Initial roles are Administrator, HR Generalist, Manager and Employee. Administrators create or link accounts, assign employee records/reporting managers, enable or disable access, and view access changes. Protected database memberships enforce permissions. Manager review is one step for direct reports; nobody approves their own leave. Multi-step approval chains remain future work.

Employee accounts are prepared for the portal and cannot sign into the desktop. Build the mobile-friendly portal separately for employee record access, attendance, leave requests, profile/security and notifications, using the same protected company membership scopes. A portal implementation is not part of this first desktop module.

## Stage 2 — Employee records and organization

Add contact and emergency details, employment types, locations, contracts, attachments, employment history and digital 201 files. Then extend reporting relationships to employee records without accounts and provide organization charts. Give sensitive documents their own access and retention rules.

## Stage 3 — Leave policies and approvals

Add company holiday calendars, custom workweeks, half-days, configurable leave types, accrual, carryover and cancellation. Extend the basic manager/HR review into configurable approval chains, with reviewer/date history and delegated review. Preserve existing leave balances during policy migrations.

## Stage 4 — Attendance rules

Add shift schedules, breaks, lateness, undertime, overtime and correction requests with approval history. Biometric integration is visible but disabled in this prototype. Actual device support requires choosing a device/protocol and verifying it against physical hardware.

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
