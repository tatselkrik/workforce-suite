# Annual recorded pay details

The existing annual BIR source worksheet can show the recorded components of
each issued source. This is a read-only review aid, not a BIR field mapping,
expected tax calculation, certificate, return or evidence of agency acceptance.
It adds no payroll or registration facts and changes no rates.

`government_annual_pay_details(year)` wraps the existing protected annual ledger
and a minimal component projection in one statement. It is current Payroll Officers and Administrators only,
uses invoker RLS, and binds each detail to the same company, employee, kind and
source ID. The existing annual RPC and 18-column export remain compatible.
Missing historical fields stay unknown; an absent source cannot become zero.
The native decoder checks exact detail coverage, allowed keys, integer-centavo
bounds and per-kind sign constraints. Unknown or conflicting data fails explicitly.

Monthly/cutoff components are the issued basic pay, paid leave, unworked holiday,
premium, overtime, night differential, allowances and employee contributions.
Exempt allowances are contained in allowances; reimbursement is outside gross.
13th-month figures describe the current payout, not annual basic pay, annual
entitlement or prior payouts. Linked amendments show only their recorded
increment. Review-only amendments show no new income. Final-pay details show
leave conversion, separation/retirement, extra pay, returned deposits and
additional contributions; original pay contained in the settlement is excluded.
Returned deposits are not classified as income. Annual tax advice is a settlement
of tax on underlying pay, so it has no additional pay components here.

Source dates, actual individual receipt dates, missing receipts, postdated
receipts and excluded states retain the original ledger meanings. Values shown
for unpaid/withdrawn sources are not received income. Overlapping component
fields and different source kinds are never blindly summed or assigned to a
government-return box. No private notes/evidence, source bundles, tax identifiers
or current salary projections enter this projection or its separate CSV/Excel.

Exports fetch a fresh protected snapshot, render locally, recheck current access
and write a new file only. One row per source retains withholding once; blank
means not applicable and `Unknown` means an expected historical field is absent.
Empty exports keep their context rather than imply a zero return. Source limits
are inherited unchanged, with no silent truncation.
