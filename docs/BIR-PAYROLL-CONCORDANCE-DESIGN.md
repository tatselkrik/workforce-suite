# BIR payroll source comparison — proposed next increment

Status: investigation only after delivered 0.1.56. No API, migration, financial
formula, source comparison or new release is implemented by this document.

The existing local file check verifies shape and exact internal controls. The
next useful government-reconciliation step is a separate HR/Admin view of file
declarations beside current recorded payroll sources. It must not call a
structurally consistent file reconciled or officially validated.

## Findings that constrain implementation

- The published 1604C schedules distinguish previous withholding, present
  withholding, December withholding, refunded overwithholding, actual withholding
  and PERA. A current-employer payroll subtotal cannot simply be equated to the
  file's actual-withholding column. Supplied readable form methods reference
  previous/present/PERA/December/refund fields together and use integer
  truncation. This is unexecuted reference evidence, not current financial law;
  do not copy its formulas or lose centavos.
- Existing `government_payroll_sources` uses actual individual receipt dates for
  BIR; unpaid approval, missing receipt, review-only, withdrawal and postdated
  receipt retain separate states. Final original-source tax and additional final
  adjustments are distinct; combined net cash must not become a second tax fact.
- A postdated fact can be shown under both its service month and future receipt
  month by the current monthly query's OR predicate. Blindly concatenating and
  summing twelve monthly snapshots would duplicate it. Annual sourcing needs a
  fresh single-statement contract, stable kind/source identity, explicit received
  year/as-of rules and fail-closed conflict checks.
- Current preparation projects only the chosen agency's canonical registration
  fields. Match unambiguous employee TIN/branch pairs, retain namesakes and mark
  missing/duplicate/unmatched identities for review. Current identity matching is
  not historical or agency-assignment certification.
- Further contract review confirms allocation-source lookup explicitly excludes
  BIR (`government_allocation_source` only accepts SSS/PhilHealth/Pag-IBIG).
  Thus current BIR effective facts need no contribution-month redistribution;
  preserve the existing base source contract instead of projecting private
  contribution review/remittance history into the annual view.
- Current payroll source reporting starts in 2025 and rejects future calendar
  months. A current-year annual wrapper cannot blindly call all twelve months,
  and file-layout support from 2023 does not establish payroll-source support
  for 2023/2024. Unsupported/unrecorded years must not become a zero balance.

## Next safe implementation decision

Review the effective-fact/correction contract in the source and reconciliation
migrations before defining a minimal protected annual projection. Keep source
inclusion, signed adjustments, unavailable amounts and source dates visible.
Bound rows before aggregation and reject truncation. Preserve HR/Admin-only
access before/after reads and before new-file export; do not return private HR
evidence, remittance references or other agencies' IDs.

Start with clearly labelled recorded-source/declaration views. Do not infer an
annual tax basis, certificate categories, MWE eligibility, substituted filing or
prior/current employer allocation. Resolve and verify the exact official mapping
before adding a claimed expected file amount. Any comparison export has its own
explicit internal worksheet contract; preserve the existing minimal diagnostic
export's promise to omit raw employee identifiers, names and amounts.

## First increment and verification cases

A bounded annual **recorded-source ledger**, not an expected tax return, can
precede automatic file reconciliation. Derive supported elapsed months within
one protected statement, deduplicate identical kind/source IDs, reject conflicting
duplicates and retain original service dates beside actual receipt dates. Expose
received-in-year totals separately from approved/unpaid, missing-receipt,
postdated, withdrawn and review-only facts. Preserve signed centavos; missing or
unsupported inputs are not zero. Do not total both original source net cash and
contained final cash. Current BIR registrations can be a minimal selected-agency
projection, with the existing strict scope/revision/canonical-ID validation.

Required cases include postdated same-key duplication across service/receipt
months; prior-year service/current-year receipt; signed annual-tax/amendment
settlements; final original tax plus additional adjustment once; missing receipt;
unsupported year; row-bound rejection without truncation; namesakes and ambiguous
or missing identities; HR/Admin positives versus non-HR/metadata forgery denial;
and revocation after render preventing new-file IO. Stored payroll, receipts,
company registration and other agencies must remain unchanged. Review the
current official mapping before presenting any declaration field as an expected
amount; none of these cases establishes agency acceptance.

Sources already inspected: published RMC25-2024 Annex A pp6–9; supplied7.4 readable
`val_1604c` method (Schedule1 lines580–641 in the compact private extraction);
`20261025010201_government_payroll_sources.sql`; and
`20261025021320_government_preparation_exports.sql`. Their paths are evidence,
not authority to execute a stale command. Existing government/privacy/immutable
payroll rules and current human instructions remain controlling.
