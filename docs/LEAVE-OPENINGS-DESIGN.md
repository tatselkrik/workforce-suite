# Opening balances — proposed next implementation contract

Design notes for the authorized next increment, not completed behavior or new
user authority. No opening is posted by this document. Preserve the current
credit ledger and original charge/approval snapshots.

An opening must establish an explicitly reviewed balance without granting the
annual entitlement twice. Use a protected preview of the selected employee,
finite type, year and cutover date. Show grants/accrual, carryover, approved use,
pending reservations and existing credit deltas separately. Label the entered
number as the intended available balance after the displayed HRIS reservations.
Require a declaration about outstanding requests in the source system; do not
silently assume unimported external requests are already represented here.

The proposed application is a server-computed difference between that target
and the fresh projected balance, retained with its source basis, operator reason,
reference, import row identity and effective date. It is not a second grant.
Recheck the complete basis under the existing company lock before committing;
stale employee allowances, policy, requests or credits require a new preview.
Keep retries actor-bound and record one opening per employee/type/year. Later
corrections should retain the opening and use linked audited adjustments.

Carryover must be explicit: never silently turn an imported expiring carry
balance into permanent base credit. Resolve the opening/carry breakdown and
expiry accounting before writing its schema. Reject unsupported or ambiguous
cases in the preview; do not guess missing prior service, bookings or usage.
Begin with entered openings, then the same validated contract for batch imports
with all-row error reporting and no silent truncation or partial application.

Scheduled policy versions need a separate effective-date contract for calendar/
approval selection and grant/accrual/carry bases. Publishing a future policy must
not retroactively reinterpret past entitlements or already filed charges/plans.
Do not copy today's employee allowance into supposed historical facts. The
current legacy policy behavior remains until that contract passes verification.

Next: design the precise opening/carry and reservation basis, implement immutable
storage/protected preview/posting, then native UI and local/hosted verification.
No paid provider, bank action, external message or new deployment target is needed.
