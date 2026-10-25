# Review checklist — native Excel0.1.50

0.1.50 is installed and privately published; see the separate verification file.
Use fictional records for import checks. Full attendance/leave-import scope stays open.

1. **Reports:** run attendance, leave, employee summary, headcount or trends.
   Export Excel. Confirm the same filters, row count and selected column order as
   the displayed report. Dates/counts/half days use appropriate Excel cell types.
2. Check an employee code with leading zeros, a Unicode name, multiline text and
   text starting with `=`, `+` or `@`. Values remain literal text. Timestamp strings
   retain their zone/offset; the workbook contains no generated formulas.
3. Export to an existing filename. The app refuses replacement and preserves it.
   Change report filters and run again before export; a fresh scoped report is
   generated and access is rechecked before saving.
4. **Reports → Employee import:** save the Excel template, enter fictional data
   and choose the workbook. Review rows before applying. Codes/phones stay Text;
   importing employees does not automatically create sign-in accounts.
5. Try a duplicate code or invalid contact/manager/type. Review the worksheet row
   number and export errors to Excel. Invalid rows prevent the whole employee
   import; existing employee records stay unchanged.
6. **Leave → Opening balances → Import file:** save/use the Excel template.
   Preview all rows and review the source declaration before Administrator posting.
   CSV and Excel follow the same target/carry and all-or-none rules from0.1.49.
7. A Manager cannot import employee/opening records. Manager reports retain the
   current own/team scope. HR can prepare openings; only an Administrator posts.
8. Try formulas (even cached), hidden data, merged cells, more than one sheet,
   numeric employee codes or a workbook exceeding import limits. Import is refused
   with a clear error. Paste reviewed values into a fresh template and retry.

No new SQL, external conversion service, paid feature, accounting/device connection,
email or public portal is included. Attendance/leave request batch imports remain
next; current native attendance/correction and leave approvals remain available.
