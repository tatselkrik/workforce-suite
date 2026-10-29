# Government file adapters — review in progress

**27October2026:** Desktop057annual receipt-source review and058current identity
review are installed/private-published. Supplied BIR7.4 members are now prepared
in a separate inert workspace; ten fictional runtime probes pass the expected
native-parser outcomes. The official application has not run: native Windows
control fails kernel-assets initialization (OS error3). See the
[offline preparation/findings and exact first manual check](BIR-OFFLINE-VALIDATION.md).
Readable control gating is an unresolved reference finding, not proof of a
compiled bug. Agency acceptance remains pending.

**Later26October update:** Kirk supplied the7.4 ZIP, patch and job-aid/circular
downloads in the prepared private reference folder. Do not request them again.
The native government Excel increment is delivered/installed in054. Bounded
inert inspection of the supplied installer found a published-versus-readable-form
PERA control-column conflict; no government program was executed. A read-only
published-structure preflight is installed/private-published in055/056as described in
[BIR file preflight](BIR-FILE-PREFLIGHT.md). It is not official validator acceptance.

As of 26 October 2026, desktop0.1.44 has delivered internal preparation CSVs;
the0.1.45 increment adds a bounded local SSS reference-draft control. Current
agency validator acceptance is not claimed. This note
prevents repeating completed source inspection and identifies the next work.

The [BIR 2024 Annex A](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2025-2024%20Annex%20A.pdf)
has now been visually inspected through PDF page 12. Its compensation schedules
are on pages 6–9; pages 10–12 continue other forms and finish with shared code
lists. Employment codes are R/C/CP/S/P/AL, separation codes NA/T/TR/R/D, and
substituted-filing flags Y/N. Do not infer these declarations from account role,
free-text employment type or archiving. Prior/current compensation categories,
MWE region/factor and per-person declarations still need evidenced input and
cross-checks. Check the exact schedule and validator before creating any file.
[RMC15-2025](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025.pdf) announces
module 7.4. Both pages of its
[2025 Annex A](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025%20Annex%20A.pdf)
and its one-page
[naming annex](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025%20Annex%20B.pdf)
were visually inspected on26October. Annex A contains the SAWT header/details/
control layout and shared declaration code lists; it does not contain a1604C
compensation schedule. Do not use SAWT as a replacement compensation schema or
assume the older compensation schedule is unchanged because these pages omit it.
Annex B distinguishes annual MMDDYYYY return periods from MMYYYY monthly/quarterly
naming periods and preserves the nine-digit TIN and four-digit branch as text.

Published module7.4 patch advisories concern ATCs and do not independently
establish a complete compensation file contract:
[June2025](https://bir-cdn.bir.gov.ph/BIR/pdf/Tax%20Advisory%20dtd%20June%2023%2C%202025%20re%20Updated%20Software%20Patch%20for%20Alphalist%20Module%20v%207.4.pdf),
[November2025](https://bir-cdn.bir.gov.ph/BIR/pdf/Tax%20Advisory%20re%20Updated%20Software%20Patch%20of%20the%20newly%20created%20ATCs%20for%20Alphalist%20Data%20Entry%20v%207.4.pdf).
The downloads landing page did not expose an inspectable validator link in its
rendered text; ordinary direct HTTP retrieval returned403 on26October. No access
restriction was bypassed and no BIR installer/validator was run. The module has
since been supplied locally by Kirk. Its actual runtime behavior and fictional
acceptance/error fixtures remain unverified; another download is not required.

The [current SSS downloads page](https://www.sss.gov.ph/download-forms-and-electronic-applications/)
links [R3FileGenerator_2023.zip](https://www.sss.gov.ph/wp-content/uploads/2024/08/R3FileGenerator_2023.zip).
The 485,284-byte archive was downloaded and inspected without executing a program
or following its internal-network link. SHA256:
`7a4382eb1db65daf860fdda44a463f3e4b53f4cda8b72bd8659d4177a860a8fc`.
It contains two DOCX documents, a library file, a nested RAR and an update EXE.
The readable documents describe a library replacement rather than a full flat-file
schema. The nested application was subsequently inspected as described below.
All five embedded DOCX screenshots were extracted within bounded archive limits
and visually inspected on26October. They show download locations and library
replacement steps only; they add no current machine-file specification or
validator evidence. Their depicted internal-network locations were not opened.
Do not treat a 2023 generator/library as evidence for current contribution rates.

The nested JAR and self-extracting update were inspected without launching their
installers/GUI. Their R3File, Contribution, Employee, Employer and Utility classes
are identical. An isolated headless harness invokes inspected DTO/string methods
on fictional values. Twelve month-position fixtures match the Rust integer-centavo
renderer. The reference's name-only TreeSet comparison can omit namesakes; this
prototype retains every unique employee/identifier. No old rate-table validator
or network helper was invoked. This is reference rendering, not agency acceptance.

The generator labels N (normal),1 (new hire),2 (separated) and3 (no earnings),
plus an explicit hired/separated date. Account role/archiving cannot establish
these declarations. The initial renderer supports explicit same-month events and
ASCII text fitting declared widths without truncation. Broader semantics/encoding
remain unresolved. The bounded [0.1.45 reference draft](SSS-REFERENCE-DRAFT.md)
adds source/receipt/registration readiness and explicit declarations; it is not
a verified agency upload control.

The current [PhilHealth downloads page](https://www.philhealth.gov.ph/downloads/)
links a one-page [RF1 form](https://www.philhealth.gov.ph/downloads/employer/rf1.pdf),
downloaded, hashed and visually inspected on26October. It is marked February2014
and shows ten employee rows, explicit employer/report types, structured names,
PIN, PS/ES shares, status/effective dates and receipt details. It refers to back
instructions, but the linked PDF contains only one page. Its historical salary-
bracket field is not authority to infer current salary bases/rates. A paper layout
is not a current EPRS upload contract. The
[employer procedures](https://www.philhealth.gov.ph/partners/employers/pay_procedures.php)
require EPRS; a current no-charge template/specification and validation evidence
remain necessary before implementing an electronic RF1 adapter.

The official Pag-IBIG
[MCRF encoding instructions](https://www.pagibigfund.gov.ph/document/pdf/dlforms/providentrelated/Instructions%20in%20Encoding%20of%20MCRF.pdf)
and [eSRS guide](https://www.pagibigfund.gov.ph/document/pdf/payments/ESRS.pdf)
again returned a browser challenge on26October. Do not guess column order or
bypass the challenge. Native XLSX is delivered in050; that does not establish
MCRF template compatibility.

## Current EPRS route review —27October2026

The official[2026-0050 advisory](https://www.philhealth.gov.ph/advisories/2026/PA2026-0050_v1.pdf),
signed17September2026, makes EPRS4.9.0 additional-payment features available from
1October2026. Employer access needs an approved request through the servicing
PhilHealth office. Covered additional-employee and salary-differential payments
use EPRS online channels with an SPA; the advisory excludes over-the-counter
payments for those categories. Do not extend that statement to unrelated
historical payments or infer that this company already has module approval.

The current employer procedures still require EPRS reporting. The2014RF1paper
form remains linked from Downloads, but does not define the current CSV schema.
The new advisory is route/access evidence, not an electronic file contract.
Neither proves accepted upload columns/encoding. No obsolete paper adapter was
added as a substitute. Ordinary searches for official AMS-CCL text contracts and
MCRF workbooks still yielded instructions, not the actual current templates.

Read-only reference/hash evidence is in ignored
`work/government-reference-20261027/philhealth-eprs49-source.json`.
The prepared BIR baseline folders still contain no reports, and no government
application is running. A separate read-only UI inventory entry point was tried;
it fails the same kernel-assets OS3initialization. Do not keep retrying it or run
government software under generic continuation. All completed062delivery stands.

## Next inputs and completion boundary

27October ordinary public retrieval: the official
[EPRS v2.1 guide](https://www.philhealth.gov.ph/downloads/employer/EPRS_v2.1_UserManual.pdf)
downloaded successfully (18 pages; PDF metadata dated8October2012). Its text and
rendered contents/remittance pages describe old screens and salary brackets,
without a CSV/upload column specification. It cannot supply a current file
adapter or current contribution rules. The employer procedures page currently
links ePOAF access registration, not a publicly documented upload template.
No registration form was submitted, software executed, employee data uploaded
or access challenge bypassed. The current file-format/acceptance gap remains.

The free local exchange foundations are delivered in053. Government milestone2
remains partial. The original public-source review added no application feature,
SQL, rate pack, release or agency-acceptance claim. Subsequent BIR preflight work
is documented in [BIR-FILE-PREFLIGHT.md](BIR-FILE-PREFLIGHT.md); delivery and
official-module acceptance must be recorded independently. PDFs, hashes and five SSS images are
retained in the ignored review folder with `public-source-review-20261026.json`.

Resume one agency at a time when its current official specification/offline
validator or a sanitized accepted reference is available. Kirk has already
supplied the current BIR7.4 module, patch and references; continue from those
local files, without requesting another download. The Pag-IBIG MCRF template
still needs ordinary official retrieval. Use fictional identities in local validators; fixture acceptance
still does not establish live filing acceptance. Keep the SSS control an internal
reference draft. Preserve issued figures and reviewed earned/received periods;
reject missing/ambiguous inputs. Never fabricate identifiers, classifications,
shares or certificates. Current employer portal credentials, employee-data
uploads, actual filing/payment and challenge bypass remain outside this task.


## Public-route refresh —29October2026

Kirk resumed public-source research. The official [2026 SSS Citizen's Charter](https://www.sss.gov.ph/wp-content/uploads/2026/07/2026-SSS-Citizens-Charter-1st-Edition-External-Services.pdf) documents registered Employer My.SSS PRN/e-CCL steps on PDF pages22–24. It does not establish a bulk-upload contract. Earlier2023 AMS editor instructions remain historical; the public downloads page still lists R3 File Generator2023.

BIR Downloadables still lists Alphalist; supplied local7.4 files remain prepared and unexecuted. PhilHealth's current employer page still directs reporting through EPRS without supplying a verified current upload-column contract in this review. Both known Pag-IBIG MCRF/eSRS guide URLs again returned browser/reCAPTCHA challenges; none was bypassed. No employer login/template, official-runtime acceptance, real filing or payment occurred. See [all verified links, free email/push limits and local emulator capability](FREE-PROTOTYPE-OPTIONS-2026-10-29.md). These bounded findings do not complete the deferred agency milestone.
