# HRIS development roadmap

## Original roadmap status through0.1.41

Strikethrough means implemented within the prototype's documented limits. It
does not imply external filing, bank transfer, public portal hosting or verified
physical-device support. Release/installation evidence is in VERIFICATION.md
and the private versioned delivery records.

1. ~~Multiple HR users, employee/manager roles, restricted permissions and
   configurable leave approval chains.~~ The separate employee portal is partial
   and local only; own requests, notifications, pictures and account controls
   are implemented. Current visual/email-delivery verification remains pending.
2. ~~Contact/emergency details, contracts, attachments, employment history and
   digital201 files.~~ Contracts are private documents/metadata; electronic signing
   and document approval are separate future features.
3. ~~Reporting relationships, managers, organization charts, locations and
   employment types.~~
4. ~~Shift schedules, breaks, lateness, undertime, overtime, attendance corrections
   and the visible disabled biometric control.~~ Actual biometric integration
   remains pending selected hardware/protocol and physical verification.
5. ~~Holiday calendars, custom workweeks, half-days, accrual, carryover,
   cancellation and configurable leave types.~~ Further effective-date policy
   versions, opening balances/credit adjustments and approval delegation remain.
6. ~~Salary calculations, deductions, allowances, payslips, core benefits and
   local payroll reports.~~ Includes Philippine premiums, cutoffs,13th-month pay,
   annual tax, final pay, wage reviews and linked amendments. Government-specific
   exports/reconciliation remain;2025 separation/2026 receipt is the reviewed
   cross-year final-pay pack, not arbitrary historical/future-year coverage.
7. ~~Applicant tracking, résumé attachments, interviews/feedback, offers,
   onboarding checklists and exit/clearance workflows.~~
8. ~~Goals, appraisals, feedback, training, certifications and career development.~~
9. ~~Detailed attendance/leave reports, CSV exports, employee CSV imports,
   custom report columns/saved views and workforce analysis.~~ Attendance/leave
   imports and native XLSX workflows remain pending.
10. ~~In-app approval/request notifications, attendance/leave reminders,
    document-expiry/contract-end alerts and app-update notices.~~ Payroll,
    appraisal and training reminders, external email/push delivery and
    payroll/accounting/device connectors remain pending.

Kirk resumed continuous work after0.1.41 and reactivated the existing usage-reset
scheduler. The approved remaining order and completion checklist are in
[REMAINING-ROADMAP.md](REMAINING-ROADMAP.md); mark verified milestones there.

The HR workspace remains a native Rust desktop application for Administrators, HR Generalists, Managers and later assigned staff roles. Ordinary employees use a separate future mobile-friendly portal. Each company has its own dedicated Supabase deployment; the prototype is a reusable base for separately configured companies. Build and verify one module before moving to the next.

Broader product benchmarks combine many of these modules: [BambooHR](https://www.bamboohr.com/platform/) lists employee data/reporting, time, hiring, payroll and performance; [SAP HCM](https://www.sap.com/products/hcm.html) spans core HR, payroll and talent; [Sprout](https://sprout.ph/) combines HRIS, payroll, analytics and talent workflows. This roadmap is a development plan, not a claim of feature parity.

## Stage 1 — Accounts and permissions

Implemented: administrator-only Account Manager, above Settings. Initial roles are Administrator, HR Generalist, Manager and Employee. Every account receives an employee record; administrators assign roles, enable or disable access, and view access changes. Protected database memberships enforce permissions. Ordered leave approvals now use configurable per-type steps with protected reviewers; nobody approves their own leave. See [leave policies](LEAVE-POLICIES.md).

Employee accounts cannot sign into the desktop. The separate local portal now supports own records, contact updates, attendance/breaks, leave filing/cancellation and issued payslips. Its remaining requests, notifications, pictures and profile/security controls are queued separately; see [employee portal](EMPLOYEE-PORTAL.md).

## Stage 2 — Employee records and organization

Implemented: one employee editor with private contact/emergency details and reporting relationships, including employees without accounts (see [contact permissions](EMPLOYEE-CONTACTS.md)). Configurable employment types/work locations, employee assignments, read-only Profile fields and employee filters are implemented. Automatic HR-only employment history with starting snapshots and immutable before/after events is implemented (see [employment history](EMPLOYMENT-HISTORY.md)). Private contracts, attachments/digital 201 files with metadata, original downloads, retention and history are implemented (see [employee documents](EMPLOYEE-DOCUMENTS.md)). An expandable organization chart uses saved reporting relationships, scoped employee cards and department/location filters (see [organization chart](ORGANIZATION-CHART.md)). Effective-date employment actions, e-signing and document approval workflows remain future work. The organization chart peer alignment is verified at normal and enlarged scales.

## Stage 3 — Leave policies and approvals

Implemented: company holiday calendars and custom workweeks, full/morning/afternoon requests, configurable leave types, annual/monthly entitlement, capped expiring carryover, cancellation, pending reservations and per-type 1–5 ordered approval steps with retained reviewer/date history. Existing requests retain original charges; new policy versions do not rewrite filed plans. See [leave policies](LEAVE-POLICIES.md). Delegation, automatic escalation, effective-date policies, imported opening balances and posted credit adjustments remain future work. Attendance calculation rules are now implemented separately; leave charges remain unchanged.

## Stage 4 — Attendance rules

Direct HR attendance corrections with required reasons and before/after history are implemented. Shift schedules, dated employee assignments, retained schedule history and overnight attendance attribution are implemented. Paid/unpaid shift break rules, own break controls, reasoned HR corrections/voids and actual unpaid totals are implemented (see [attendance breaks](ATTENDANCE-BREAKS.md)). Configurable late-arrival/early-departure grace, overtime minimum/rounding and optional pre-shift time are implemented with saved rules and reasoned HR application/history (see [attendance calculations](ATTENDANCE-CALCULATIONS.md)). Overtime values are candidates for review, not approved/payable time. Own attendance correction requests with single-step Manager/HR review, no self-approval and retained submission/decision history are implemented (see [attendance requests](ATTENDANCE-REQUESTS.md)). Own overtime requests capped by the server calculation, partial Manager/HR approvals, rejection, cancellation, revocation and retained decision/basis history are implemented (see [overtime approvals](OVERTIME-APPROVALS.md)). Changed attendance, breaks or approved leave withholds current authorization until reviewed again. Payroll remains separate. Biometric integration is visible but disabled in this prototype. Actual device support requires choosing a device/protocol and verifying it against physical hardware.

## Stage 5 — Reports and data exchange

Implemented: detailed attendance reports, filed-charge leave reports, employee summaries and date/employee/department/status filters with role-scoped CSV exports (see [reports](REPORTS.md)). Saved-rule calculations, missing-day review flags, partial/historical overtime and incomplete totals remain distinct. Implemented: HR/Admin create-only employee CSV imports with row previews/error exports, active reference validation, atomic batch writes, guarded retries and immutable source links/history (see [employee imports](EMPLOYEE-IMPORTS.md)). Implemented: personal saved report views with fixed/rolling periods, per-account persistence and current-access checks. Implemented: current headcount breakdowns and recorded attendance/leave trends with charts, plus selected ordered report columns and private revision-guarded saved preferences outside sign-in tokens (see [workforce analysis](WORKFORCE-ANALYSIS.md)); attendance/leave imports and bulk updates remain later data-exchange modules. XLSX formatting remains future work.

## Stage 6 — Notifications and connections

Implemented: a role-scoped in-app notification inbox with private read/unread history, current approval tasks, personal request updates, upcoming leave, unfinished check-outs and HR-only document-expiry/contract-end reminders; see [notifications](NOTIFICATIONS.md). Configure a usable email-confirmation destination and reliable company email delivery. Connect payroll/accounting/attendance systems through explicitly configured integrations, with retries and audit history.

## Stage 7 — Hiring, onboarding and exit

Add applicant tracking, interviews, offers, onboarding checklists, exit tasks and clearance workflows. Keep applicant data separate from employee records until hiring is approved.

Implemented: HR/Admin-only applicant records, search/stage filters, reasoned stage changes, archive/restore and immutable before/after history with revision guards and safe creation retries. Applicants remain separate from employee records and accounts; see the [applicant pipeline](HIRING.md). Implemented: private résumé/application attachments, scoped interview scheduling and feedback, Administrator-approved offers, accepted-offer employee conversion and assigned onboarding checklists with immutable history. No external email/calendar invitations or signed-offer workflow is included. Implemented: separate private exit cases, assigned clearance checklists, reasoned completion/waivers, readiness blockers and Administrator-only atomic access deactivation/employee archiving with retained history; see [exit and clearance](EXIT-CLEARANCE.md). Company-specific exit paperwork and human final-pay review remain separate.

## Stage 8 — Development and performance

Implemented: employee/current-report goals with reasoned progress and retained history, company review cycles, eligible assigned reviewers, assessment/strengths/development feedback, HR publication, immutable submission-time goal facts and separate private HR notes. Reassignment resets unfinished assessments and protects prior reviewer drafts; current membership/reporting scopes apply. Unfinished appraisals block exit finalization, including older clients. See [performance](PERFORMANCE.md). Implemented: assigned training, current-report/own-staff progress, HR-recorded completion/outcomes/minutes and certificates with expiry, revocation/archive history and HR-only existing201document links; see [learning records](LEARNING.md). Implemented: shared career plans, required/optional milestones, independent reviews, HR-only waivers and retained completion/context snapshots; aspirations never change employment or source learning records. See [career development](CAREER-DEVELOPMENT.md). Employee portal, 360 feedback and performance inbox reminders remain later work.

## Stage 9 — Payroll and benefits

Implemented in version 0.1.33: effective salary profiles, reviewed monthly reconciliation worksheets, private-sector Philippine overtime/holiday/night premiums, recurring and per-run allowances/deductions, employer benefit memos, independently approved immutable payslips, recorded payments and PDF/CSV reconciliation exports. Contribution and withholding rules were checked against official sources on 23 October 2026. See [payroll](PAYROLL.md) for effective dates and limits. This module does not transfer money, file government returns or replace company payroll reconciliation. Subsequent implemented increments are described below.

Implemented in version0.1.34: separate1st–15th/16th–month-end runs, planned pay dates, reviewed
worksheets, independent approval, immutable period payslips, recorded payments,
CSV/PDF and monthly contribution/exemption reconciliation are implemented in the
current source. See [payroll cutoffs](PAYROLL-CUTOFFS.md) and versioned release
evidence for hosted deployment and installation. Actual13th-month payouts are implemented in the0.1.35 source with reviewed
opening coverage, installments, shared benefit exclusion and independent approval;
see [13th-month pay](PAYROLL-THIRTEENTH.md). Withholding/contribution adjustments
remain explicit reviewed inputs. Implemented in the0.1.36 source: automatic annual compensation-tax reconciliation,
evidenced prior-employer balances, shared annual benefit reconciliation,
independent approval and immutable aggregate PDF/CSV advice; see
[annual tax](PAYROLL-ANNUAL-TAX.md). Implemented in0.1.38: [final-pay reviews and settlement](PAYROLL-FINAL-PAY.md), with reviewed vacation/separation/retirement amounts, prospective last-compensation tax, independent approval, immutable own statements, source-net offsets and unpaid withdrawal. [Individual payment records](PAYROLL-PAYMENTS.md) remain compatible.

Implemented in0.1.39: [regional wage compliance](REGIONAL-WAGES.md), with
evidenced effective classifications, independent approval, dated wage checks
and optional Administrator enforcement. Implemented in0.1.40:
[consolidated payroll reports](PAYROLL-REPORTING.md), separating immutable
approved amounts, actual individual receipts, source offsets, combined final
cash and current outstanding/reserved balances.

Implemented in0.1.41: [linked amendments](PAYROLL-AMENDMENTS.md) preserve original
payslips/receipts and independently approve supplemental cash or a separately
reviewed discrepancy without automatic recovery. Reviewed2025 separations may
retain actual2026 receipt dates; no service interval or earlier-year tax credit
is invented. Consolidated report-v2 includes amendment cash once. Government
exports/reconciliation and configured integrations remain queued. Work and the
scheduler stop after finishing and installing this version.

## Separate employee portal — local prototype, work resumed

Build a mobile-friendly employee application against the same protected company memberships. Start with own employment/contact details, attendance and breaks, leave/correction/overtime requests, own approved payslips, notifications and account security. Keep payroll drafts, team salaries, recruiting, private HR notes and administrative controls outside employee access. The native desktop remains the staff application. Device and external-service connections remain unconfigured until the necessary company and hardware choices are available.

First local milestone implemented: the separate Rust portal signs in an employee,
shows only their own read-only employment/contact records, saves their contacts,
records server-timed attendance and reads issued payslips with CSV/browser
printing. Fourteen local Auth/HTTP/security tests and ten actual hosted Auth/API
proofs passed; phone-width browser checks use fictional records. No schema change
was needed. The0.1.1 source adds own leave types/balances, full/half-day filing,
cancellation/history and saved-rule server-timed breaks. Seventeen Rust tests,
1,891 database assertions and fictional phone-width checks pass; the additive
leave migration is deployed with67 public record sets preserved. Eight additional
actual hostedAuth/API proofs cover leave filing/cancellation and access guards.
Private build/package records are separate from these source checks. Next add correction/overtime requests,
notifications, profile/security controls and company branding for local use only.
Public HTTPS hosting requires separate authorization and is outside the current queue.
See [employee portal](EMPLOYEE-PORTAL.md).

Latest local increments: portal0.1.2 adds own correction/overtime proposals and
decision history;0.1.3 adds personal notifications and saved read/unread state.
Automated and actual Auth/API checks pass; current notification/phone visual
inspection remains unavailable. Pictures/account-security controls stay pending.
Kirk subsequently completed storage cleanup and resumed work/scheduling25October.
Portal0.1.4 adds private own circular photos and company sign-in branding, with
automated/live checks and current visual verification still pending. Password/
email controls follow next. Use [the ordered checklist](REMAINING-ROADMAP.md)
for current scope/status rather than historical stop/resume paragraphs above.

## Government reconciliation — locally installed0.1.42

~~Reviewed contribution allocations, immutable government remittance/void/linked
correction records, native monthly reconciliation and fresh review CSV~~ are
implemented, verified and locally installed. The government milestone remains
partial: legal identities and exact agency formats/validators follow. Current
portal0.1.5 account controls are separately delivered, with actual email confirmation
and current browser/phone visuals unverified. The ordered checklist is current;
older stop/pause paragraphs above are historical. Work and scheduling remain active.
See [0.1.42 delivery evidence](GOVERNMENT-0.1.42-VERIFICATION.md).

The sequence may change when the company has a more urgent requirement. Payroll, biometric devices and external integrations require company-specific decisions; they are not generic plug-ins already working in this prototype.
