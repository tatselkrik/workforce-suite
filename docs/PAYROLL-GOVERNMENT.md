# Government payroll exports and reconciliation

Open **Government** in Workforce Payroll to review contribution allocations,
record remittances and generate reconciliation worksheets. Registered legal
identities, original pay sources, coverage months and individual receipt dates
remain separate facts. Internal CSV and Excel outputs are review worksheets,
not official returns or proof of agency acceptance.

## Source and money rules

Use fresh protected Payroll Officers and Administrators snapshots, exact centavos and immutable issued
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

## Agency formats

See [Government file format references](GOVERNMENT-FILE-REVIEW.md) for the
applicable published structures and the distinction between preparation,
format-specific checks and official validator acceptance.

## Contribution reviews and remittance records

Payroll Officers and Administrators open a current agency/month snapshot. Monthly/cutoff known shares stay
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
