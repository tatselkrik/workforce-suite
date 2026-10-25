# Opening balances and dated policies — active implementation contract

Design notes for the authorized next increment, not completed behavior or new
user authority. No opening is posted by this document. Preserve the current
credit ledger and original charge/approval snapshots.

**Implementation status26October2026:** the native/protected storage, preview,
posting, dated policy and atomic CSV workflows are now implemented locally.
See [the implementation contract](LEAVE-OPENINGS.md). Hosted verification, visual
checks and release/installation remain pending; foundation-only wording below is
retained as design history and must not be treated as the current delivery state.
Current-year anchors also capture incoming gross carry; later employment defaults
must not recalculate that known amount through legacy prior-year projections.

Kirk authorized immediate implementation25October2026, continuation across the
usage reset if needed, then attendance/leave imports/native Excel and exchange
foundations. The earlier next-refresh-only hold is superseded. This document
does not itself grant deployment authority beyond the standing project guards.

## Calculation contract under implementation

`src/leave/openings.rs` now defines deterministic takeover arithmetic, with no
active app or database integration yet. Capture all active booked request IDs,
saved total/early units, current earned base including credits, known consumed
carry and an explicitly reviewed target available balance. The target already
accounts for displayed reservations. Record remaining imported carry separately
inside that target; expired carry cannot be imported as available or permanent
base. Carry requires a known prior-year service date and the configured cap.

Baseline requests keep their reviewed funding. Newly imported carry must not
retroactively fund baseline base-charged bookings. A baseline cancellation can
restore known consumed carry before expiry; after expiry only its base-funded
portion returns. New eligible requests consume remaining carry. Later accrual/
credit changes affect base once. Next-year carry uses only unused base, never
unspent imported carry. Changed baseline charge units require review; approvals
only move reserved units to used. Negative projected availability is exposed,
not silently clamped.

The forthcoming protected preview/post must freeze the employee/type annual
rate, accrual, hire-date and expiry basis and the credit total at posting. An
ordinary later policy/allowance edit must not recalculate that opening year's
past entitlement. The same-day credit total must be captured as a stored scalar,
so a later same-day adjustment does not disappear into a recomputed baseline.
Preview fingerprints must cover affected policy/employee/request/credit/opening
state. Posting rechecks under the existing company lock; retries stay actor-bound.
Reject unresolved future-year reservation impacts rather than underfund them.

## First dated-policy scope

`src/leave/versions.rs` defines prospective full snapshots. Calendar, labels,
activation and approval changes may start today/future midyear. Grant/accrual/
carry changes and new finite entitlements start on a future January1; reviewed
current-year entitlement changes use the existing audited credit workflow. This
initial restriction avoids inventing a midyear grant/proration rule.

Select calendar/type activation per charged date; preserve the request-start
label/approval-plan snapshot. Do not rewrite already filed requests. Append
versions in active effective-date order; only an unactivated future tail may
be withdrawn, and only when no active filed request references that version.
Withdrawals retain the original record and revision identity. Protected posting
must also prevent a scheduled change from underfunding existing reservations.
The legacy starting policy is a compatibility snapshot, not reconstructed prior
history. Protected dated entitlement storage/native UI remain to be implemented;
selectors alone do not complete roadmap4 or authorize a release.

The Rust foundation currently passes24targeted checks across takeover arithmetic,
prospective selection and a captured current-year entitlement basis. The latter
rejects dates before its known-from capture instead of inventing past grants.
Private deterministic PostgreSQL helper arithmetic is staged separately in
`20261025143356_leave_opening_arithmetic.sql`; it creates no ledger/public RPC and
changes no active balance behavior. Its isolated test covers conservation across
bookings/credits, expiry/cancellations and private execution denial. It is not
deployed or sufficient evidence for completing the feature.

## Original requirements retained

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
