# CSV and Excel exchange

Workforce Suite reads and writes bounded CSV and native `.xlsx` files locally.
All imports use a review preview followed by the relevant protected transaction;
selecting a file does not create accounts, bypass approvals or grant permissions.

| Workflow | Reference |
| --- | --- |
| Employee records | [Employee imports](EMPLOYEE-IMPORTS.md) |
| Leave opening balances | [Opening balances](LEAVE-OPENINGS.md) |
| Attendance correction requests | [Attendance imports](ATTENDANCE-IMPORTS.md) |
| Leave requests | [Leave imports](LEAVE-REQUEST-IMPORTS.md) |
| Reports and custom columns | [Reports](REPORTS.md) |
| Local payroll exchange | [Payroll exchange](PAYROLL-EXCHANGE.md) |

Download a template from the corresponding screen. Preserve exact headers and
keep employee codes, phone numbers and other identifiers as text. Preview errors
identify original worksheet rows. Correct the source and preview again before
posting; failed validation must not silently omit rows.

CSV exports neutralize formula prefixes, while Excel exports preserve user text
as literal string cells. Importers reject formulas, macros, external connections,
hidden or merged worksheet structures and unsupported formats. Files are not
evaluated or fetched over the network. See [Native Excel](NATIVE-EXCEL.md) for limits.

An internal report or payroll exchange file is not an agency return, payment,
vendor acknowledgment or proof of third-party compatibility.
