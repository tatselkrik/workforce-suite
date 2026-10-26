# Annual recorded BIR sources

This internal HR/Admin ledger reports withholding recorded on individual payroll
receipts in a selected year. It does not compute tax, produce an expected 1604C
amount, certify a 2316, reconcile prior employers or establish agency acceptance.
The independent local BIR file check retains its minimal diagnostic contract.

## Protected source and date contract

`government_annual_ledger(p_year)` is a read-only, single-statement projection.
Both new functions use security invoker with an empty search path and explicit
authenticated execution grants. The existing protected monthly source function
checks current active HR/Admin membership; Auth metadata grants no authority.
Employee labels use existing company RLS, including archived source employees.

Years begin in 2025. For the current year, query only January through the company
current month; earlier supported years include December. The report is **current
records as of today**, not a reconstruction of what HR knew at an earlier year
end. File-layout support for 2023/2024 does not imply payroll-source support.

The monthly source can repeat a postdated or missing-receipt fact. Group by
immutable source kind and ID, retain sorted observed months and count an identical
fact once. Conflicting observations reject the whole response. Observed months
describe query context; they do not establish receipt dates, service allocation
or contribution coverage. A missing individual receipt leaves its payment year
unverified even if the source appears in the selected year's queries.

Cap annual observations at 40,000 and unique sources at 20,000, before returning
an aggregate. Exceeding either cap rejects the whole report without truncated
totals. Native decoding independently checks the version, company, selected year,
elapsed months, identities, dates, amounts, source uniqueness and exact label set.

## Money and inclusion

Include only facts with an actual received date in the selected year and no later
than the report date. Preserve original service dates, which may be in a prior
year. Preserve signed centavos for amendments, annual settlements and additional
final-pay tax adjustments. Original source tax and the final additional adjustment
remain separate; combined final cash is not another withholding source.

Show approved/unpaid, missing-receipt, postdated, withdrawn and review-only records
separately. Approval and advice are not receipt. No receipts is not a recorded
zero. Unknown received amounts leave the complete total unresolved while showing
the known subtotal. A signed negative sum is not automatic debt, a government
credit, a refund instruction or a payroll recovery.

No new rates, tax rules, eligibility inference or financial formulas are added.
BIR contribution allocation is not invented: the existing allocation adapter
excludes BIR, so this ledger does not fetch private allocation/remittance history.

## Desktop and files

Payroll → Government → BIR → **Annual recorded sources** provides the year
selector, source cards, employee/source search and received-only screen filter.
Each card separates service dates, actual receipt, recorded amount, treatment and
source observations. Summary totals always cover the entire protected year.

Fresh CSV/Excel exports fetch and validate the complete protected ledger again;
screen filters never silently narrow their totals. Recheck current access before
the RPC and after rendering, immediately before creating a new file. Do not
overwrite an existing export. Export names/code, immutable source IDs, original
dates and exact decimal amounts as an explicitly internal source worksheet.
Omit government account/tax identifiers, HR evidence, payment references and
registration history. Excel values are literal text, including leading zeros,
Unicode and signed amounts; CSV follows the existing formula-prefix protection.
Empty exports retain year/as-of context and say no sources were observed.

This does not relax the separate BIR diagnostic export's promise to omit raw
employee identifiers, names and amounts. Actual file comparison and canonical
TIN/branch matching remain subsequent work with their own reviewed contract.

## Verification boundary

Deliver only after native model/adapter/UI checks, PostgreSQL authorization and
duplicate/conflict/budget cases, fresh migration replay, guarded hosted deployment,
exact release installation and private-channel verification pass. A fictional
fixture or successful internal ledger does not demonstrate official BIR-module
execution or acceptance. See the versioned verification report for actual results.
