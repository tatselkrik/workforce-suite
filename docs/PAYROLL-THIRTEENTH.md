# 13th-month pay — current-employer payout workflow

The native **Payroll → 13th-month pay** increment adds earned-coverage runs,
employee worksheets, independent approval, immutable own benefit slips and
recorded payments. Consult versioned verification for deployment/package state.

## Entitlement and coverage

Issued monthly/cutoff basic and explicitly reviewed nonoverlapping opening
intervals form the earned current-employer basis. Compute cumulative basic/12
once in integer centavos; do not add rounded accrual memos, project salary rates
or import a prior employer's basic into this employer's liability. Every day
from year/hire start through the as-of date needs issued or reviewed opening
coverage, including an evidenced zero-paid interval. January2026 is not in the
initial monthly rule pack and needs review. Partial monthly payroll extending
beyond the as-of date is not silently included.

Coverage is an HR review of actual duties/service or a more favorable company
policy, independently of Administrator/Manager/Employee account roles. Archived
employees can still receive reviewed pro-rata pay. A statutory classification
requires reviewed one-month service. Basic corrections need evidence; employer
policy top-ups/advances are explicit additional benefit amounts, not invented
future earned basic. Never fund the benefit through salary deductions.

Prior issued in-app installments and documented same-company off-system13th
payments reduce the unpaid entitlement. A company lock plus source recheck at
approval prevents concurrent drafts from paying it twice. External payment and
other-benefit amounts cannot fall below earlier approved facts. Overpayment is
visible and blocks another issue; the app never recovers it from wages.

## Tax and contributions

One shared annualPHP90,000 benefit exclusion is allocated across earlier13th
payouts and reviewed other benefits, including prior-employer benefit facts.
Basic entitlement and shared-cap allocation are automatic. **Supplementary /
cumulative withholding and month-level contribution adjustments are explicitly
reviewed inputs in this increment.** Review notes and checkboxes are required
even for zero. There is no standalone bonus-tax shortcut or assumed extra full
monthly government contribution. Automatic annualization/prior-employer tax
reconciliation is the next increment; this one does not claim to replace it.

The run as-of date describes earned records only. HR must review the24December
payment requirement and any employer-funded advance. A later actual-basic
reconciliation must account for advances already issued. A pay-date entry alone
does not prove statutory compliance or transfer funds.

## Workflow and permissions

HR/Admin create the year/as-of/title/pay-date run, add employees and review
openings, actual coverage, earlier benefits and adjustments. Save the worksheet,
review included/omitted employees and submit. Another Administrator reviews the
saved source/evidence worksheet and approves. Changes retain reasons/events;
issued slips remain immutable. An Administrator records an actual paid date and
reference separately. CSV registers and own-slip CSV/printablePDF export amounts.

Changed source facts require refresh and three new reviews. Valid saved inputs
are retained. If changed sources make the old calculation invalid, the server
resets the saved draft to a safe unreviewed baseline while audit history retains
the old inputs. The open native form preserves unsaved opening/policy edits so
HR can correct and save them against the refreshed source. A new worksheet
seeds earlier protected external-benefit totals before review.

Clients have SELECT-only grants/RLS on three separate tables; protected writers
check current memberships and revisions under the company payroll lock. Managers
and Employees can read only their own issued slips, never drafts or team pay.
Ordinary Employees still cannot sign in to the native desktop. Slips exclude raw
source bundles, HR notes and opening evidence references. The employee portal
remains paused and has not added this benefit-slip view.

## Official references checked23October2026

- [DOLE2024 statutory-benefits handbook, chapter13](https://nwpc.dole.gov.ph/wp-content/uploads/2024/11/Workers-Statutory-Monetary-Benefits-Handbook-2024-Edition.pdf): earned-basic basis and coverage; premiums/allowances are not automatically integrated.
- [BOI2026 business guide](https://boi.gov.ph/wp-content/uploads/2026/02/Doing-Business-in-the-Philippines-2026.pdf): payment timing and installments.
- [BIRRR11-2018](https://bir-cdn.bir.gov.ph/local/pdf/RR%20No.%2011-2018.pdf): shared benefit exclusion, supplementary/cumulative withholding and annualization.
- [BIRRR29-2025 digest](https://bir-cdn.bir.gov.ph/BIR/pdf/RR%20No.%2029-2025%20digest%20FINAL.pdf): updated de-minimis caps; these do not create a second13th-month exclusion.

No bank transfer, government filing or remittance confirmation is implemented.
