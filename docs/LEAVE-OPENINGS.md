# Effective-date policies and opening balances

Implementation for the next desktop increment. **Not delivered until the release
verification/install record confirms it.** This document does not grant deployment
authority or replace the private target guards.

## Native workflow

Leave now has **Opening balances** alongside Requests, Leave policies, Credit
adjustments and Approval delegation. Its own views are Enter opening, Import CSV
and History. HR/Administrators can review protected previews; only Administrators
post openings or policy changes. Managers see minimum own/current-team opening
history without HR reasons, references, authors or audit receipts.

An opening takes over an existing employee's reviewed **available balance today**.
The number already accounts for the HRIS pending/approved reservations displayed
in the preview. It is not an additional annual grant. Register outstanding source
requests here before takeover, and review their representation explicitly. An
opening is limited to one employee/type/current year; later corrections use the
existing immutable credit ledger. Backdated grants are not reconstructed.

Enter the unspent expiring carry **inside** the target separately. It must fit the
captured carry cap, including carry already consumed by baseline requests, and
needs a known prior-year service date. Expired carry cannot be entered as available.
Later-year active reservations must be reviewed before an opening is posted.

The protected preview shows before/after base, carry, approved use, pending
reservations and availability, together with the carry expiry, captured rate and
number of retained requests. Changing inputs removes the earlier preview. A
changed employee, source request, credit, policy, opening or setup revision forces
fresh review at posting. Uncertain writes retain their identity for exact retries.

## CSV openings

The UTF-8 template has six columns in this order:

| Column | Meaning |
| --- | --- |
| employee_code | Existing employee code; never create an employee implicitly |
| leave_type_code | Existing finite type code shown beside the import controls |
| available_days | Reviewed nonnegative available days, in whole/half days |
| remaining_carry_days | Expiring carry inside that target; blank means zero |
| reason | Private review reason, 5–500 characters |
| reference | Optional private reference, at most 160 characters |

Files are bounded to 2 MiB and 1–100 data rows. Quoted CSV and physical source lines
are retained. All row errors are shown; duplicate employee/type rows are flagged.
No valid subset is silently imported. The Administrator reviews the entire
protected batch and posts all rows in one transaction. Stale fingerprints, a
revoked role or any invalid row leave no partial openings. Exact actor-bound batch
retries return the original complete row inventory. Error/review exports use the
existing spreadsheet-formula-safe CSV writer and preserve existing destination
files. Native XLSX/attendance/leave imports are the next separate roadmap item.

## Dated company policies

Administrators append full policy snapshots in effective-date order. Calendar,
names, type activation and approval changes can take effect today or later. Grant,
accrual and carry changes, including new finite types, start on a future January1
within five years. This bounded first contract does not invent midyear proration;
reviewed current-year changes use credit adjustments.

New native requests select the calendar/type activation on each charged day and
save the request-start label and approval chain. Already-filed dates, charges,
assigned approvers and decisions are retained. New annual terms must still fund
existing reservations. Only an unactivated future tail without active saved
policy references may be withdrawn; its snapshot and withdrawal receipt remain.
Reservations are checked again after withdrawal, including requests starting
under an earlier version. Changing effective dates means withdrawing an eligible
future tail and posting a new version, not editing history in place.
Withdrawal also preserves dated credit obligations: a newly introduced type
cannot disappear while it has a nonzero dated credit effect. A same-date linked
reversal may offset that effect while retaining both entries. Credit posting
selects the type at its effective date, including future scheduled types.

The baseline is a compatibility snapshot of the policy known at enablement, not
fabricated earlier legal history. Once the first opening/version enables this
company's dated workflow, undated policy editing is rejected. Unchanged companies
retain the earlier undated contract until they opt in through that workflow.

Attendance fallback calendars, holiday-sensitive shifts and the payroll source
holiday list also resolve the affected date. Payroll still requires its separate
HR-reviewed statutory holiday classification; company closures do not select
pay rates automatically. No multiplier, contribution, tax rule or issued payslip
is changed. Exit readiness includes named approvers in current and unwithdrawn
future snapshots, while already-filed assigned steps remain separate blockers.

## Entitlement conservation

The captured current-year basis stores known-from date, annual rate, accrual,
service date, captured earned rate, type/carry terms and **incoming gross carry**.
Freezing only the annual rate would still allow a later employment-default edit
to recalculate past-year carry. The captured carry is an explicit known balance,
not invented historical grants. Once frozen, later prior-year changes need a
reviewed current-year credit rather than silently changing that incoming amount.
Ordinary allowance/service-date edits preserve this year's captured basis and
serve as future defaults. Within a captured year, queries before the known-from
date are rejected. Earlier uncaptured years retain the explicit legacy
compatibility calculation; they are not attestations of historical grants.

An opening also stores the earned base **including the credit total at posting**,
the IDs/saved charges of active baseline bookings, and known carry funding already
consumed. Later same-day credits therefore affect base once. Approval moves units
from reservation to used without demanding the same expired carry a second time.
Cancellation restores the original funding; expired carry cannot become base.
Only unused base feeds the next year's capped carry, not unspent imported carry.

The integer half-day model and private PostgreSQL projection share conservation
vectors. Negative projections are exposed rather than silently clamped to a grant.
Stored charges and consent/audit facts are not recalculated from later labels or
employment defaults.

## Protected contracts and compatibility

`leave_dated_context` is a current-membership/date-bound staff projection, with
HR-private audit separated from minimum own/team calculation facts. Tables have
RLS with no direct authenticated access. Actor metadata is taken from protected
memberships; user-editable Auth metadata supplies no authority. Company advisory
locks, revisions, source fingerprints, immutable records and actor-bound retries
guard the mutation RPCs. Snapshots and imports reject excessive sizes instead of
silently omitting rows.

`submit_leave_checked` independently derives the policy/setup/credit revisions,
five-field balance and charged dates and compares them to the native preview.
After enablement the legacy native submit route is blocked even with a forged
new-client header. Older installed clients can still sign in, read history and
update. Delegated reviews retain the existing original-authority wrapper and
immutable decision receipts; they gain no personnel or policy-edit permission.

The existing portal receives the same own-only five-field balance/type contract.
It selects the dated start policy. Since older portal builds show one calendar,
cross-version spans require split requests rather than a misleading preview.
No running portal process is restarted as part of this desktop increment.

No paid scheduler or provider is needed: effective versions are selected by company
date during protected requests. No public portal exposure, external notification,
payroll change, filing, payment, hardware or new service is introduced.
