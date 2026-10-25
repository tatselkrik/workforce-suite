# Audited leave credits — development scope

This is a partial increment of roadmap4, not a completed opening-balance import
or effective-date policy module. It adds no paid service or cash conversion.

Administrators record signed whole half-day units with an effective date and
reason. Only today through next year is accepted; prior entitlement is not
rewritten. These are explicit additions/deductions to base entitlement, not an
available-balance replacement or a second annual grant. Finite types only.
Positive unused base credits follow the existing carry cap/expiry and known-hire
rules. Unlimited leave needs no credit ledger.

Entries are immutable. A linked full reversal preserves the original, applies
on or after its date in the same year, and can occur only once. Reductions must
not underfund existing reservations at any remaining balance date. A reduction
with later-year leave is held for review because carryover may be affected.
Posting or reversing a credit never approves/cancels leave or changes saved
charge dates/approval plans. Read scope and role changes remain server enforced.

Private reasons/references and actor history belong to HR/Admin. Own/Manager
previews receive only scoped credit facts needed for exact balance calculations.
The portal's existing protected balance context uses the same server calculation.
Opening balances require a separately reviewed takeover basis to avoid counting
prior grants/usage twice; do not present this ledger as that feature.

Current state: local implementation/testing in progress. Migration62 is NOT
deployed and no new release is delivered. Desktop0.1.46 and local portal0.1.6
remain the latest verified packages. Scheduled policies/openings follow only
after this increment passes its SQL, native, hosted and delivery checks.
