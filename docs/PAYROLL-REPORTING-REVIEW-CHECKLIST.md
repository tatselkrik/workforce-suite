# Payroll reporting review checklist — 0.1.40

- Open Payroll → Reports and Generate / refresh report.
- Switch between Approval date and Payment / settlement date; compare an earlier
  issued slip paid in a later month. The report should follow the actual date.
- Filter by source kind, employee or saved code/name/department. Try an empty range.
- Compare recorded payroll cash with individual receipts and combined final cases.
  A final source offset must not add another source-net cash payment.
- Expand issued figures. Annual-tax advice and settled adjustments must stay
  separate from payroll cash. Final statements identify contained source net.
- Check outstanding sources and their reserved subset. Withdrawn final statements
  remain visible as issued history and show no cash payment.
- Export fresh CSV and compare its generation time/filter/amounts. The export
  includes every matching row, even when native results have multiple pages.
- Confirm evidence, tax identifiers and private payment references are absent.
- Check light/dark themes and enlarged text. Manager accounts must not see this
  company reporting destination.
- Existing monthly-register/statutory exports remain inside their separate section.

Balances show current recorded status at generation, not historical accounting
closes. Reporting creates no payments, tax adjustments, remittances or filings.
