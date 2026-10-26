# Government review Excel exports

In **Payroll → Government → Reconciliation**, generate the agency/month, choose
**Excel (.xlsx)** and use **Export fresh review** or **Export agency preparation**.
CSV remains available from the same format selector. Both controls load a new
protected server projection; they do not export a stale screen cache.

The review worksheet contains the source/inclusion rows, known subtotals, complete
or unresolved liability, selected-month remittance allocations and differences.
Voided receipts remain visible. A multi-month receipt contributes only its
selected-month allocation, and missing amounts remain UNKNOWN/UNRESOLVED. An
unreceived approval, postdated receipt, final-pay source offset or annual tax
advice must retain its existing meaning; Excel changes no financial rule.

The preparation worksheet adds only the selected agency's registered identifiers,
legal names and registration revisions. It retains missing-registration flags;
it never guesses identifiers or name components. Other agencies' identifiers,
private payment references/evidence, registration notes, actor IDs and history
are excluded. Both exports are limited to current HR/Admin access.

CSV and Excel share one unescaped source-row pipeline. CSV retains its existing
UTF-8 BOM, CRLF, quoting and formula protection. Excel writes literal text cells,
including leading-zero identifiers, names beginning with formula-like characters,
timestamps and exact two-decimal PHP amounts. It creates no spreadsheet formulas,
macros or external links. Totals are captured review values; editing a workbook
does not recalculate them or update HRIS. Use Generate/refresh and export again
for a new report. The native reader is not a government-report import workflow.

The header is frozen, wrapped and filterable. Exports fail explicitly on excessive
rows/columns, malformed source scope, invalid XML text or unsupported extensions;
no rows are silently dropped. Render before the final protected access check and
write a new file only. A revoked role blocks saving; existing files are preserved.

These are **internal review worksheets**, not BIR/SSS/EPRS/Pag-IBIG upload files,
agency validation results, returns, payment instructions or evidence of filing.
The SSS reference draft retains its separate `.txt` contract. Exact agency
adapters/validator acceptance remain tracked in [the format review](GOVERNMENT-FILE-REVIEW.md).
No Office subscription, new dependency, paid conversion service or database
migration is needed. Delivery is established separately by the versioned evidence.
