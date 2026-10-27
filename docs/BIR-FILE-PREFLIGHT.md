# Read-only BIR 1604C standalone Schedule 1 / 2 file checks

This increment checks an **unvalidated plaintext source file** against the
published layout: one four-column H1604C header and 1–500 details from one
schedule. Schedule 1 uses 49-column D1 / 36-column C1; the 0.1.56 extension uses
59-column D2 / 45-column C2 for Schedule 2. Mixed-schedule files are unsupported;
the checker never ignores another schedule or produces a partial pass.
It does not calculate tax or wages, establish MWE/PERA
or substituted-filing eligibility, certify2316s, reconcile HRIS payroll, validate
assigned identities or replace the official BIR module. Delivery is recorded
separately after proportionate verification and actual installation.

Desktop0.1.61 adds a **separate declared-compensation arithmetic review** after
complete structure/control success. It compares five supported sums per D1 row
or seven per D2 row, with independent counts and generic minimal findings.
These relationships are not expected HRIS pay or tax. Existing structure/control
errors and literal declaration readers keep their established contracts. See
[equations, exclusions and whole-file rules](BIR-DECLARATION-ARITHMETIC.md).

The reference is [RMC25-2024 AnnexA](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2025-2024%20Annex%20A.pdf),
PDF pages 6–9. The supplied module 7.4 is advertised for 2023 onward; this
preflight accepts2023through the current company calendar year. The
[2025 naming annex](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025%20Annex%20B.pdf)
defines the annual TIN/branch/MMDDYYYY/form filename. The2025SAWT annex does not
replace the1604C compensation layout.

## Compatibility limitation discovered in the supplied module

The downloaded7.4 archive has one NSIS installer. Bounded inert extraction found
readable FoxPro form methods for1604C. The published Schedule 1 control includes
PERA in column36, while one readable bundled method's excess-column check starts
at column36. Its Schedule 2 excess check starts at column44, conflicting with
published PERA/net-statutory-basic controls in columns44/45.
These are **reference conflicts**, not proof of a compiled-validator
bug: neither the installer nor a government program was executed, and the
relationship between readable methods and executed object code is unverified.

The local checker follows the published 49/36 and 59/45 layouts. It does
not silently drop PERA/net basic or reproduce the conflicting excess-column rule. Actual
official-module behavior needs fictional zero/PERA-control fixtures before an
agency-compatibility claim. Current encoding/region/identity semantics also remain
unverified. Do not derive current rates or assignment codes from old bundled tables.

## Native workflow and permissions

HR/Admin selects BIR under **Payroll → Government → Reconciliation**, then
**Check BIR 1604C file**. The file is read locally and never uploaded. Current
protected company registration is loaded to compare only its BIR TIN/branch.
Missing registration stays unverified; a different header is flagged against the
current record. This does not determine historical registration or filing rights.
Other agency identifiers, registration history/notes and employee names/identifiers
are not included in saved findings. Manager/ordinary Employee roles are denied.

Current access is checked before the protected read and after local parsing/
rendering. Company/account/role changes clear private view state. This is a
read-only view, so it creates no unsaved personnel or payroll draft. Failed source
checks clear a prior successful result. Saving CSV/Excel findings reloads the
source, verifies the displayed SHA256, repeats the checks and rechecks access
before new-file IO. Changed sources require rechecking; existing files survive.

## Bounded supported checks

- 2MiB maximum file,500details, bounded CSV fields/diagnostics and unchanged source
  bytes. Physical line numbers are preserved. No partial file is marked verified.
- ASCII plaintext with optional UTF-8 BOM (flagged for encoding review). No
  transliteration. Other encodings, mixed schedules, validation-marker/encrypted
  files and unsupported records fail explicitly rather than receiving a partial pass.
- Header/record order, exact column counts, matching employer/period prefixes,
  canonical calendar-date shapes, text/identifier widths, unique sequence and
  employee TIN/branch pairs, explicit published employment/separation/filing codes.
  These shape checks do not establish identity, employment or filing eligibility.
- All 31 D1 monetary fields map to C1 columns 6–36; all 40 D2 monetary fields map
  to C2 columns 6–45, including PERA and statutory basic net of contributions/dues.
  The D2 days/year factor is an explicit positive three-digit-bounded integer,
  never money or a control total. Its basis and MWE eligibility/wage floors remain
  unverified review items; they are not inferred from account role or payroll.
  Parse exact integer
  centavos without floating point or rounding. Unknown/bad fields cannot become
  zero or a complete subtotal. Compare exact totals without a tolerance or source
  adjustment; this stricter comparison is not an official rejection decision.
- Saved findings contain source hash/year, line, field and generic diagnostics.
  Raw employee rows, identifiers and compensation amounts are omitted. Diagnostic
  caps remain explicit and cannot convert further errors into a passing result.

No new database migration, rate pack, paid service, Office installation, official
filing, employer registration edit or money movement is part of this increment.
