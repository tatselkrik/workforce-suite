# Government file adapters — review in progress

As of 25 October 2026, desktop 0.1.43 has delivered private registration records.
No agency machine-file writer or validator acceptance is claimed yet. This note
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
module 7.4; its [naming annex](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025%20Annex%20B.pdf)
is distinct from the compensation schema. The current downloads landing page
did not expose a directly inspectable validator link in its rendered text.

The [current SSS downloads page](https://www.sss.gov.ph/download-forms-and-electronic-applications/)
links [R3FileGenerator_2023.zip](https://www.sss.gov.ph/wp-content/uploads/2024/08/R3FileGenerator_2023.zip).
The 485,284-byte archive was downloaded and inspected without executing a program
or following its internal-network link. SHA256:
`7a4382eb1db65daf860fdda44a463f3e4b53f4cda8b72bd8659d4177a860a8fc`.
It contains two DOCX documents, a library file, a nested RAR and an update EXE.
The readable documents describe a library replacement rather than a full flat-file
schema. The nested application was subsequently inspected as described below;
embedded DOCX screenshots remain uninspected.
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
remain unresolved; no R3 product download control is released from this evidence.

Next: finish the exact SSS/BIR format and offline-validator investigation using
fictional data, then implement a bounded adapter with explicit missing-input
errors. Preserve existing issued figures and reviewed earned/received periods;
never fabricate identifiers, classifications, employer shares or certificates.
Primary PhilHealth/Pag-IBIG limits remain in
[PAYROLL-GOVERNMENT.md](PAYROLL-GOVERNMENT.md). No government login, employee-data
upload, filing, payment or browser-challenge bypass is included.
