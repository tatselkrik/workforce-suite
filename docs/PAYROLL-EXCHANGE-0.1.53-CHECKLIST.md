# HRIS Desktop0.1.53 — local payroll exchange

Installed and privately published. Use this guide with the accompanying
[verification record](PAYROLL-EXCHANGE-0.1.53-VERIFICATION.md).

Open **Payroll > Data exchange** with a current HR/Admin account.

- As Administrator, open **Column profiles**, create a profile, choose/reorder
  fields and rename headers. Save a reasoned version. HR can use profiles; only
  Administrators can change them. Identity/state/unit controls stay in every file.
- Under **Prepare export**, select the profile, date basis, dates, source and
  employee/search scope. Preview the current issued or received facts, then
  **Create export snapshot**. Missing legacy employee labels need review or an
  explicitly narrower scope; records are never silently omitted.
- Save the original CSV to a new file. Money is integer PHP centavos, so 100 means
  PHP 1.00. Cash, source offsets, final contained pay and tax advice remain separate.
  Unavailable values stay blank. A reconciliation digest identifies each source.
- Change or disable a profile, then reopen its old export in **Export history**.
  The original columns, facts, bytes and checksum should remain unchanged.
- Save its acknowledgement template. Enter `accepted` or `rejected` for reviewed
  rows, optionally adding private references/notes. Leave other statuses blank.
  Keep the employee/source/service-period context unchanged; use it to identify
  each row. The native batch review includes every row through pagination.
  Load the CSV, review the count and record the whole batch. A bad/stale row must
  prevent the entire batch from posting. Payroll payments do not change.
- As Administrator, reset an incorrect terminal outcome to pending with a reason.
  Reload its template before recording a corrected outcome. Prior events remain
  visible in reconciliation history.
- If a request loses its connection, use **Retry exact submission**. Use
  **Resolve retained attempt** to recover an existing result or close an unused
  identity against a delayed request. Draft contents remain available for review.
- Verify Managers/ordinary Employees cannot use company data exchange, even for
  their own payslip. Existing staff own-payslip access should remain unchanged.
- The direct accounting-service button stays disabled. This increment prepares
  local files and records manual reconciliation; actual vendor delivery/acceptance,
  account-specific journal mapping and a selected connection remain deferred.

Portal, phone/email confirmation, agency validator acceptance and biometric
hardware are separate recorded limitations. No paid service is enabled here.
