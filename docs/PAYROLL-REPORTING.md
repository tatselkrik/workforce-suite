# Consolidated payroll reporting

Payroll → Reports combines immutable monthly, cutoff,13th-month, annual-tax,
final and linked amendment statements. Source0.1.41 uses report-v2 for supplemental
cash, original links, signed statutory adjustments and review-only discrepancies;
the prior endpoint remains available to older installations. See
[amendment boundaries](PAYROLL-AMENDMENTS.md). Only current HR/Admin may generate or export company reports.
Managers retain their own slips but do not receive this report destination/API.

Filter up to366inclusive days by actual approval date or actual recorded
payment/settlement date. Earned periods remain separate columns. Optional employee,
source kind and saved name/code/department filters include retained archived
employment. An empty result is explicit. More than5000matching rows requires a
narrower filter; totals and CSV are never silently truncated. Native pages show
50rows while every matching row is included in summary/CSV.

A single protected server statement returns compact financial facts from one
consistent snapshot. The client verifies company/filter/row/centavo invariants.
It checks current protected access before the request and again before exporting.
Changing account scope clears results. Fresh CSV generation queries the server
again and displays that generation's time. No private evidence, worksheet/source
bundles, bank/payment references, tax identifiers or actor IDs enter these reports.
The existing explicit monthly statutory worksheets remain separate exports.

Recorded payroll cash consists of direct salary/13th and supplemental receipts plus the actual
combined net of settled final cases. A source receipt with final_settlement origin
discharges that source through the combined case; its net is an offset, never an
additional full-net cash payment. Annual-tax advice and its settled signed
adjustments stay separate from payroll cash. Advice alone never transfers funds.

Issued gross/net and contributions/withholding summarize salary/cutoff/13th
sources. Final statements show combined net, contained source net and the
additional net difference, which can be negative. Withdrawn statements remain in
issued history with no actual received date/cash. Do not add overlapping statement
nets or tax advice into a grand cash total.

Outstanding salary/13th balances lack a recorded receipt; approved final
reservations are a subset, not a payment. A missing individual receipt on an old
paid batch is flagged with unknown date/cash/balance, rather than using another
person's later batch date. Balances are current at generation for selected
records, not reconstructed historical closing balances. The report does not
automatically recalculate payroll, create arrears, remit contributions, transfer
funds, file returns or generate an official2316. CSV uses exact centavos and safe
spreadsheet escaping; portal work remains paused.
