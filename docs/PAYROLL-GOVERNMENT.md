# Government payroll exports and reconciliation

Source0.1.42 implements **Payroll → Government**, reviewed contribution allocations,
recorded remittances and fresh reconciliation CSV worksheets. This is a partial
government-reporting milestone: private legal identities and exact agency upload
formats/validators remain outstanding. Delivery and installation are established
separately by the versioned release evidence. Existing monthly statutory CSV files
and the new government CSV are internal review worksheets, not official returns.
The [0.1.42 delivery report](GOVERNMENT-0.1.42-VERIFICATION.md) confirms local
installation. Private update-channel publication remains pending authorization.

The additive workflow migration is deployed at58schema versions, preserving all86
preexisting public record sets and243functions. The local PostgreSQL suite passes
3,785assertions and a fresh chronological replay of all58migrations. Hosted real
Auth/REST checks cover HR preparation, independent Administrator approval, missing
attestation rejection, duplicate-reference races, idempotent voids, linked receipt
corrections, RLS/direct-write denial and immediate membership revocation. A native
diagnostic validates six fresh protected snapshots with114source rows and their
CSV output in memory. All24earlier payroll record sets and prior Auth users match.
Synthetic remittances end voided and accounts disabled/archived/session-free.
Fictional native screen inspection includes normal/compact/dark125percent layouts,
allocation/approval/remittance/void/history and Manager denial. Actual signed-in
native button interactions and government validator acceptance are not claimed.

Post-deployment advisors report3warnings: Auth leaked-password protection disabled
and2multiple-permissive-policy warnings. They also report5unindexed foreign-key
INFO findings,5deny-by-default RLS INFO findings and113unused-index INFO findings
after the workflow migration. No new warning or unindexed foreign-key finding
was introduced. Existing Auth settings and those earlier table policies remain. The
private verification record retains exact findings. References:
[password protection](https://supabase.com/docs/guides/auth/password-security#password-strength-and-leaked-password-protection),
[policy lint](https://supabase.com/docs/guides/database/database-linter?lint=0006_multiple_permissive_policies),
[foreign-key lint](https://supabase.com/docs/guides/database/database-linter?lint=0001_unindexed_foreign_keys).

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

1. HR-only legal identity/company registration data is implemented in the0.1.43
   candidate and under verification; see [the registration contract](GOVERNMENT-REGISTRATION.md).
   It is not yet a delivered release.
2. Exact agency files after complete specification/validator verification; block
   missing/ambiguous inputs rather than silently creating incomplete returns.
3. Complete the remaining government-specific verification and versioned delivery.
   Public portals and actual government filing remain out of scope.

## Contribution reviews and remittance records — source0.1.42

HR/Admin opens a current agency/month snapshot. Monthly/cutoff known shares stay
read-only. Special13th-month, amendment and final-contribution amounts may have
explicit reviewed coverage and employer/EC shares. Every employee centavo must
sum to the immutable issued deduction/adjustment; zero cannot be split into new
charges/refunds. Covered months must be unique, within evidenced service and any
known original month. Unknown shares remain unresolved until an evidenced review
is approved. A negative correction needs an existing known per-person charge;
another employee's positive charge never covers it.

Draft/save/submit/return/cancel uses retained revisions. Every creator, editor and
submitter remains in the exclusion history. Approval requires an Administrator
who contributed to no version of that draft and is not the source employee.
Approved/cancelled reviews are immutable; changed coverage uses a linked successor.
Source/revision/current membership checks are enforced on the server. A later wage
receipt does not invalidate an unchanged contribution basis. Withdrawal does.
Fresh snapshots keep raw source-v1 and effective reviewed facts separately.

Only an active Administrator records an actual evidenced agency/bank remittance.
The whole receipt and1–24covered-month shares must reconcile exactly. Dates cannot
be future. Agency/reference uniqueness is case/space normalized and serialized
against concurrent entry. Exact operation retries do not duplicate a receipt.
Voiding requires a reason, retains the original and excludes its shares; it never
reverses a real payment. A correction replaces the final voided entry, retaining
the chain even when correcting a reference typo. No direct client table writes
are granted, including to Administrators.

**Export fresh review CSV** fetches and validates a new protected snapshot and
rechecks current access before writing. Its22columns distinguish source inclusion,
known amounts, complete liability, selected-month remittance share and discrepancy.
Unknown complete totals remain `UNRESOLVED`; an original amount on an excluded
source is not an included liability. Private payment/review references, evidence,
notes and actor IDs are omitted. Employee labels are current and explicitly named
as such. A worksheet neither submits a return nor authorizes a transfer/refund.
