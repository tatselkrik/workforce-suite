# What to check in 0.1.57

Sign in as Administrator or HR, then open **Payroll → Government → BIR →
Annual recorded sources**.

1. Choose 2026 and Generate / refresh. Check the receipt-year total and the
   separate counts for received, approved/unpaid, missing receipts and postdated
   records. Source service dates may belong to an earlier year.
2. Open a source's reference/observations. Its actual receipt date stays separate
   from the months in which the report observed it. Repeated observations count
   one immutable source once.
3. Try employee/source search, Received only and page navigation. These filter the
   screen; the summary and exported annual worksheet retain the entire year.
4. Choose 2025. Empty or unrecorded amounts must not imply a zero-tax declaration.
   Missing individual receipts say their payment year is unverified.
5. Export a fresh CSV and Excel worksheet to new filenames. Check leading-zero
   codes, exact signed amounts and separate service/receipt dates. Unreceived
   records have no amount in the Received-in-year column. Keep these HR files
   private. Two fresh exports may have different generation timestamps.
6. Confirm Managers cannot open Government. Use Back to reconciliation to return
   to the existing monthly reviews/remittances and BIR file check.

No payroll or tax rules changed. This is an internal recorded-source ledger;
automatic file comparison, expected 1604C amounts and official BIR validation
remain pending. Source files, slips, receipts and remittances are unchanged.

Codex verified native logic/HTTP adapters, actual hosted HR/Admin access and
revocation, PostgreSQL permissions/bounds and actual native fictional layouts.
Signed-in button/file-dialog interactions and your preferred appearance remain
manual checks. The existing native font can show a box for unsupported CJK
characters; Excel/CSV preserve the original Unicode text.
