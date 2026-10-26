# Read-only BIR 1604C Schedule 1 file checks

This increment checks an **unvalidated plaintext source file** against the
published Schedule 1 layout: one four-column H1604C header,1–500D1 details with
49columns and one36-column C1 control. It does not calculate tax, establish PERA
or substituted-filing eligibility, certify2316s, reconcile HRIS payroll, validate
assigned identities or replace the official BIR module. Delivery is recorded
separately after proportionate verification and actual installation.

The reference is [RMC25-2024 AnnexA](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2025-2024%20Annex%20A.pdf),
PDF pages6–7. The current supplied module7.4 is advertised for2023onward; this
preflight accepts2023through the current company calendar year. The
[2025 naming annex](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025%20Annex%20B.pdf)
defines the annual TIN/branch/MMDDYYYY/form filename. The2025SAWT annex does not
replace the1604C compensation layout.

## Compatibility limitation discovered in the supplied module

The downloaded7.4 archive has one NSIS installer. Bounded inert extraction found
readable FoxPro form methods for1604C. The published Schedule 1 control includes
PERA in column36, while one readable bundled method's excess-column check starts
at column36. This is a **reference conflict**, not proof of a compiled-validator
bug: neither the installer nor a government program was executed, and the
relationship between readable methods and executed object code is unverified.

The local checker therefore follows the published49/36-column layout. It does
not silently drop PERA or reproduce the conflicting excess-column rule. Actual
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
  transliteration. Other encodings, Schedule 2/MWE, validation-marker/encrypted
  files and unsupported records fail explicitly rather than receiving a partial pass.
- Header/record order, exact column counts, matching employer/period prefixes,
  canonical calendar-date shapes, text/identifier widths, unique sequence and
  employee TIN/branch pairs, explicit published employment/separation/filing codes.
  These shape checks do not establish identity, employment or filing eligibility.
- All31D1 monetary fields map to C1 columns6–36, including PERA. Parse exact integer
  centavos without floating point or rounding. Unknown/bad fields cannot become
  zero or a complete subtotal. Compare exact totals without a tolerance or source
  adjustment; this stricter comparison is not an official rejection decision.
- Saved findings contain source hash/year, line, field and generic diagnostics.
  Raw employee rows, identifiers and compensation amounts are omitted. Diagnostic
  caps remain explicit and cannot convert further errors into a passing result.

No new database migration, rate pack, paid service, Office installation, official
filing, employer registration edit or money movement is part of this increment.
