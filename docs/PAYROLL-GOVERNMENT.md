# Government payroll exports and reconciliation

Implementation is in progress. This milestone is not delivered or installed.
The new `payroll_government` module currently provides source validation and
reconciliation arithmetic; an additive read-only source RPC is also implemented.
Remittance storage, reviewed allocations, native UI,
agency-specific file generation and end-to-end verification remain outstanding.
Eleven Rust checks pass, including strict decoding of local PostgreSQL vectors.
The local PostgreSQL suite passes3,591assertions. The source migration is deployed
at schema version57 with all86prior public record sets and241existing functions
unchanged. Hosted checks validate93published fact rows across four agencies,
protected HR/Admin access, Employee/Manager denial, forged metadata rejection,
invalid parameters and immediate disabled-membership denial. All24existing payroll
table sets and prior Auth users matched; synthetic histories remain disabled/
archived/session-free. The native library suite passes78tests with2ignored;
the explicit government-vector run includes its otherwise ignored conformance
check. Native UI/delivery remain outstanding; a source API alone does not complete milestone2.
Existing monthly statutory CSV files remain generic payroll worksheets.

## Source and money rules

Use fresh protected HR/Admin snapshots, exact centavos and immutable issued
salary/cutoff/benefit/final/amendment facts. Retain per-person actual receipt
dates. Never infer one employee's receipt from batch status or another person's
payment. Monthly/cutoff overlap, foreign identities, duplicate source facts and
excessive rows/amounts fail closed. Archived employees remain reportable.
Postdated recorded receipts retain their original date under an explicit
`dated_after_as_of` review state; they are not current received withholding.

SSS, PhilHealth and Pag-IBIG reconciliation uses an explicit contribution month.
The fact that salary was paid later does not move the earned contribution month.
BIR withholding review uses the actual receipt month; unpaid approved calculations
remain visibly separate. A legacy paid batch without an individual receipt is
unresolved. An approval, annual tax advice or scheduled payment is not a receipt.

Unknown employer/EC shares and unallocated benefit/final/amendment contributions
must remain unresolved; do not silently replace them with zero or guess coverage
from approval/cash dates. Known subtotals may be displayed while a complete
liability/difference remains unavailable. Source data does not always contain all
fields required for agency files; original slips must not be rewritten to add them.

Final settlement contains original source amounts: include the source's own
withholding once, plus only the final additional tax adjustment. Never add the
combined final net as a second contribution or withholding charge. Retain signed
reviewed linked adjustments and settled annual tax adjustments separately;
review-only discrepancies and withdrawn payouts create no recovery or tax credit.

A remittance record must describe a real evidenced payment, separately from an
employee's wage receipt. Voids retain history and do not reduce the liability.
An excess is a discrepancy for review, not an automatic credit/refund or permission
to reduce a later return. Reconciliation never initiates filing or payment.

## Official source review — 25 October 2026

| Agency | Inspected evidence | Implementation consequence |
| --- | --- | --- |
| BIR | [1601-C form](https://efps.bir.gov.ph/efps-war/EFPSWeb_war/forms/1601C/1601c_v3.xhtml) and [form instructions](https://bir-cdn.bir.gov.ph/local/pdf/1601C%20final%20Jan%202018%20with%20DPA.pdf) describe compensation withholding and explicit adjustments. | Prepare a review worksheet; do not label an internal CSV as an eFPS/eBIRForms return. Prior-month adjustments need their original period and reviewed return treatment. |
| BIR annual | [RMC25-2024 Annex A](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2025-2024%20Annex%20A.pdf), annual section starting PDF page6, requires structured names, employer/employee TIN/branch fields and distinct prior/current compensation categories. [RMC15-2025](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025.pdf) announces module7.4. | Do not split a full name heuristically or infer missing certificate categories. The7.4 AnnexA first page is SAWT, not the1604C compensation schema. Match the correct form/schedule and finish reading its continuation before coding an upload file. |
| BIR naming | [RMC15-2025 Annex B](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025%20Annex%20B.pdf) lists annual filenames using TIN, branch, return period and form type. | Preserve leading zeros; distinguish annual return-period formatting from monthly/quarterly formats. This check alone is not official validator acceptance. |
| SSS | [Pay contributions](https://www.sss.gov.ph/pay-contribution) describes employer e-CL review and PRN through My.SSS. [Official downloads](https://www.sss.gov.ph/download-forms-and-electronic-applications/) still list R3 File Generator2023 and the contribution collection list. | An internal worksheet is not proof of My.SSS acceptance or PRN generation. Reconcile employee/employer shares and employer-only EC separately; obtain the exact current upload specification/validator before a machine-file claim. |
| PhilHealth | [Employer payment/reporting procedures](https://www.philhealth.gov.ph/partners/employers/pay_procedures.php) requires EPRS. [Downloads](https://www.philhealth.gov.ph/downloads/) links RF1. | Maintain EPRS/RF1 fields and employee/employer shares. An older article mentioning CSV is insufficient to certify a current upload format. |
| Pag-IBIG | Search-indexed [MCRF encoding instructions](https://www.pagibigfund.gov.ph/document/pdf/dlforms/providentrelated/Instructions%20in%20Encoding%20of%20MCRF.pdf) and [eSRS guide](https://www.pagibigfund.gov.ph/document/pdf/payments/ESRS.pdf) identify remittance coverage, member/employer identification and shares. Direct MCRF retrieval returned a browser challenge. | Complete primary-document/template inspection before an upload-compatibility claim; no challenge bypass or guessed spreadsheet layout. Native XLSX remains a later ordered milestone. |

Three original BIR PDFs, SHA256s, extracted text and inspected page renders are
retained in the ignored source-review evidence folder. No government account was
opened and no employee data was uploaded. This review adds no new payroll rate
pack, penalty/deadline calculation, tax filing, transfer or provider connection.

## Remaining implementation

1. Add reviewed contribution allocations, preserving the deployed source-v1
   contract through new checked-in migrations.
2. HR-only legal identity/company registration data, validated without changing
   original payslips or weakening older clients.
3. Auditable remittance records/voids and source freshness checks.
4. Native review screens, fresh worksheet exports and scoped source/issue details.
5. Exact agency files after complete specification/validator verification; block
   missing/ambiguous inputs rather than silently creating incomplete returns.
6. Proportionate local/database/hosted/privacy checks, versioned desktop delivery
   and installation. Public portals and actual government filing remain out of scope.
