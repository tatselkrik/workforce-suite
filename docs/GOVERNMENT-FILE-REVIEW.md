# Government file format references

Government payroll preparation separates internal reconciliation, format-specific
file checks and official validator acceptance. A structurally consistent file or
an internal worksheet does not establish acceptance by an agency.

## BIR

The implemented BIR checks use explicit schedule, record, control and declaration
rules. Preserve TIN and branch identifiers as text. Account roles, free-text
employment labels and archiving cannot establish employment declarations,
substituted-filing eligibility or minimum-wage status.

References include [RMC 25 2024 Annex A](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2025-2024%20Annex%20A.pdf),
[RMC 15 2025](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025.pdf),
[its Annex A](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025%20Annex%20A.pdf)
and [file naming Annex B](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025%20Annex%20B.pdf).
The SAWT layout in the 2025 annex does not replace a compensation schedule.
Format-specific checks remain distinct from running the official validation module.

See [File preflight](BIR-FILE-PREFLIGHT.md),
[Declaration review](BIR-DECLARATION-REVIEW.md),
[Identity review](BIR-IDENTITY-REVIEW.md) and
[Offline validation](BIR-OFFLINE-VALIDATION.md).

## SSS

The [SSS downloads page](https://www.sss.gov.ph/download-forms-and-electronic-applications/)
includes the R3 File Generator reference. Historical generator structure does not
establish current contribution rates or upload acceptance. The
[2026 Citizen Charter](https://www.sss.gov.ph/wp-content/uploads/2026/07/2026-SSS-Citizens-Charter-1st-Edition-External-Services.pdf)
describes employer PRN and electronic contribution-list procedures; it is not a
complete bulk-upload schema.

The [SSS reference draft](SSS-REFERENCE-DRAFT.md) is for internal review. It is
not a verified agency upload file. Explicit declarations must have supporting facts.

## PhilHealth and Pag IBIG

The [PhilHealth employer procedures](https://www.philhealth.gov.ph/partners/employers/pay_procedures.php)
describe EPRS reporting. The [RF1 paper form](https://www.philhealth.gov.ph/downloads/employer/rf1.pdf)
is not proof of a current electronic upload-column contract.

Pag-IBIG references include the
[MCRF encoding instructions](https://www.pagibigfund.gov.ph/document/pdf/dlforms/providentrelated/Instructions%20in%20Encoding%20of%20MCRF.pdf)
and [eSRS guide](https://www.pagibigfund.gov.ph/document/pdf/payments/ESRS.pdf).
Upload compatibility requires the applicable agency template and validator result;
column order, encoding and declarations must not be guessed.

## Reconciliation

Use [Government payroll review](PAYROLL-GOVERNMENT.md) to compare original payroll
sources, reviewed contribution allocations and recorded remittances. The suite
records reviewed facts and prepares local outputs; it does not submit returns or
transfer money. Keep preparation, payment receipts and filing acceptance distinct.
