# What to check in 0.1.59

Use this checklist after installation. The separate versioned verification and
delivery records establish the exact installed build and private-channel status.

Open **Payroll → Government → BIR → Review declarations and sources** as HR/Admin.

1. Expect a setup message while the prototype employer BIR TIN/branch is missing.
   This feature must not invent or overwrite company registration facts.
2. With current company registration available, select the file's year and a local
   standalone Schedule1/2 `.dat`. Resolve whole-file structure/control errors first.
   Review labels must not call the source officially validated or reconciled.
3. Choose a file line. A unique current registered pair selects its current
   employee; unmatched/ambiguous pairs select no employee or payroll sources.
   Namesakes must not share source entries.
4. The left section shows31/40 literal monetary declarations with published column
   numbers, including PERA and Schedule2 net-basic. The days/year factor must not
   appear as money. The right section separately shows recorded sources with
   service periods, actual receipt dates, signed/unknown amounts and exclusions.
   No source total is automatically equated to a file column.
5. Check source paging, light/dark/compact layouts, long labels and aligned amounts.
   Existing font limitations for unsupported CJK characters remain explicit;
   exported Unicode/code text must remain intact.
6. Export fresh CSV/Excel to new filenames. `FILE_DECLARATION` rows carry local
   declared values; `RECORDED_SOURCE` rows carry recorded amounts/dates/treatments.
   No source observations must remain distinct from zero tax. Raw tax IDs,
   file-declared names, payment references and HR evidence must be absent.
7. Changing the source requires rechecking before export. Existing destinations
   must survive, and revoked/non-HR access must prevent rendering/export. The
   existing minimal BIR diagnostics and identity worksheets retain their scope.

No payroll/registration/source changes, expected-tax calculation, 2316
certification, official-module execution or filing is part of this view.
Actual BIR runtime checks remain deferred at Kirk's request.
