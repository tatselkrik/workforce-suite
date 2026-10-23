# Philippine payroll and benefits

The first payroll module is a monthly reconciliation register for private-sector
employees, with reviewed attendance worksheets, salary versions, allowances,
deductions, employer benefits, independent Administrator approval and retained
payslips. A monthly register is a calculation period, not permission to pay only
once a month. Record interim salary payments as advances; actual bank transfers,
government submissions and payment-frequency compliance remain with the employer.

The cutoff increment adds separate semi-monthly runs, planned pay dates, monthly
contribution/cap reconciliation and per-cutoff payslips; see
[payroll cutoffs](PAYROLL-CUTOFFS.md). Use that workflow for the two regular
cutoff calculations. The monthly register and cutoff issue paths cannot overlap
for an employee/month.

## Rules and sources checked on 23 October 2026

- [DOLE monetary benefits handbook, 2024 edition](https://nwpc.dole.gov.ph/wp-content/uploads/2024/11/Workers-Statutory-Monetary-Benefits-Handbook-2024-Edition.pdf), guide computations pp. 2–3 and chapters 2–5: ordinary OT 125%; rest/special day 130%, special plus rest 150%; regular holiday 200%, regular holiday plus rest 260%; double regular holiday 300%/390%. Nonordinary overtime is another 30% of that day's hourly rate. Night differential is an additional 10% of the applicable normal/OT hourly rate for actual work from 22:00 to 06:00. Double special days use 150%/195%. Special working holidays use ordinary rates. [DOLE's August 2026 advisory explanation](https://dole.gov.ph/news/dole-sets-August-holiday-pay-rules-/) confirms the regular/special distinction and preceding-day eligibility for unworked regular holidays.
- [SSS Circular 2024-006, effective January 2025](https://www.sss.gov.ph/wp-content/uploads/2024/12/CI-2024-006-Publication.pdf): salary-credit brackets of PHP500, minimum PHP5,000, maximum PHP35,000; 5% employee and 10% employer, with employer-only EC PHP10 below MSC15,000, otherwise PHP30. MSC above20,000 belongs to the mandatory provident fund; report it separately. This remains the employed-member schedule on the [current SSS circular index](https://www.sss.gov.ph/sss-circulars/).
- [PhilHealth Advisory 2025-0002](https://www.philhealth.gov.ph/advisories/2025/PA2025-0002.pdf): 5% of fixed monthly basic salary, floor10,000/ceiling100,000, equally shared. Do not reduce the basis for absence/lateness or add overtime/allowances. [The government reports no 2026 rate increase](https://pia.gov.ph/news/no-hike-in-premium-rates-for-2026-says-philhealth/).
- [Pag-IBIG Circular460 reproduced with DBM Circular Letter2024-2](https://www.dbm.gov.ph/wp-content/uploads/Issuances/2024/Circular-Letter/CIRCULAR-LETTER-NO-2024-2-DATED-FEBRUARY-01-2024.pdf): maximum fund salary10,000 from February2024; employee1% at compensation up to1,500, otherwise2%; employer2%. Keep the basis distinct from PhilHealth and allow a documented payroll review.
- [BIR RR11-2018 AnnexE](https://bir-cdn.bir.gov.ph/local/pdf/Annex%20E%20RR%2011-2018.pdf): monthly compensation withholding table effective2023 onwards. Employee mandatory contributions reduce taxable compensation; employer contributions and voluntary deductions do not. Minimum-wage-earner coverage requires an explicit HR determination, not the app's account role.
- [BIR RR29-2025 digest](https://bir-cdn.bir.gov.ph/BIR/pdf/RR%20No.%2029-2025%20digest%20FINAL.pdf): updated de minimis benefits include monthly rice2,500 and laundry400. This module uses these caps only for months from February 2026; other allowances default taxable. It does not infer exemptions for all benefits or implement fringe-benefit tax.

## Boundaries and controls

Payroll profiles, drafts and audit history are HR/Admin-private. Managers have no
access to team salaries and may read only their own approved payslips. Ordinary
employees remain reserved for the separate portal. All hosted values are computed
again on the server with integer centavos and explicit rounding.

Payroll holidays are classified separately from the existing leave calendar,
which does not distinguish regular from special holidays. Classifying payroll
holidays must not rewrite saved leave charges. Review local proclamations, rest
days, coverage, paid leave, unworked-holiday eligibility and missing days explicitly.
Missing attendance is not automatically an absence or deduction. Salary divisors
are explicit; true monthly-paid defaults use365. Premium exemptions are a labor-law
coverage decision, independent of Administrator/Manager account permissions.
For an eligible unworked double regular holiday, daily-paid employees receive two
days and true monthly-paid employees receive one additional day, since one is
already included in salary. Unworked-holiday minutes cannot overlap paid leave or
unpaid time in the same worksheet row. The initial released pack is v2; pre-release
v1 issued synthetic slips retain their original facts.

Each employee worksheet retains salary, employee, holiday, attendance, break,
leave and OT source facts. Changed sources require an explicit source refresh and
new review before submission/approval. Issued payslips remain immutable; later
differences use documented adjustments in a later run. Another Administrator
approves a submitted register; the submitter cannot approve it. Payment recording
requires a date and reference and does not move money.

Approved payslips have native detail, printable PDF and UTF-8 CSV exports. The PDF
uses standard fonts and preserves supported Philippine Latin characters such as
enye; if a character is unavailable it fails explicitly and offers CSV rather than
silently changing a person's name. Fresh membership/record scope is checked before
export, and existing files are never overwritten.

Local CSV registers, statutory contribution/withholding worksheets and annual
summaries support reconciliation. They are not filed government returns, official
2316 certificates, SSS/PhilHealth/Pag-IBIG uploads or proof of remittance. Fringe-benefit tax, unusual coverage and regional minimum-wage compliance need
separate review;
the module shows a basic-pay/12 accrual memo rather than silently paying it.

Actual13th-month payouts are described in [13th-month pay](PAYROLL-THIRTEENTH.md). Their entitlement/shared-cap engine is separate from monthly accrual memos. Supplementary withholding and contribution adjustments currently require explicit review; [annual compensation tax/prior-employer reconciliation](PAYROLL-ANNUAL-TAX.md) now uses actual recorded payments and separate closing advice.
