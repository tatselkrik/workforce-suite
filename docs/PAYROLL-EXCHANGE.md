# Local payroll data exchange

This is a free file-exchange foundation, not a connected accounting service,
bank instruction or agency file adapter. Actual vendor connections remain
deferred until a company selects its software and approves the target.

Administrators maintain append-only versions of CSV column profiles. Each profile
selects and renames whitelisted report fields; it cannot contain SQL, expressions,
credentials or URLs. Money is exported as integer Philippine centavos. Required
controls identify the exchange, source record and row digest. Spreadsheet formula
prefixes in text are escaped visibly with an apostrophe; unknown amounts remain
blank. Cash, source offsets, source net, contained final-pay net and tax advice
retain separate meanings. There is no all-purpose total-pay column.

Current active HR/Admin can preview and prepare a frozen export from the protected
payroll-report-v2 projection. Preparation rechecks the profile version and source
fingerprint. Its immutable snapshot, profile, exact UTF-8 CSV bytes and SHA-256
digest survive later payroll payments, profile edits and membership changes.
Historical downloads are labeled historical and recheck current HR/Admin access
before writing a new local file. An export is not proof of delivery or acceptance.
Prepared exports never change payroll, receipts, salaries or tax calculations.
Legacy sources missing their saved employee labels fail with a review message;
the exchange never silently drops them or substitutes today's employment facts.
An explicitly narrower employee/search filter can export complete records.

Reconciliation imports use the HRIS acknowledgement template, not an undocumented
vendor format. Each row carries the exchange identity, record key, captured row
digest, expected acknowledgement revision, accepted/rejected status and optional
private reference/note. Read-only employee code/name, source kind and a text service
period make the template understandable; altered context is rejected. Native batch
review and reset selection show those human labels. All reviewed rows are available
through pagination. All rows validate and post atomically. An uncertain retry
must use the same identity, actor and complete payload. A different actor/payload
cannot reuse a receipt. Terminal outcomes require an Administrator's separately
reasoned reset to pending before another outcome; prior events remain immutable.
Manual acknowledgement records do not certify a vendor response or mark pay paid.

Uncertain submissions can be resolved through a protected, serialized lookup.
An unused identity is closed with an immutable actor/payload digest; a delayed
original request must then fail. A committed identity returns its saved outcome.
Never treat a bare absence query as permission to create another request while
an old request could still arrive. Retired identities are bounded and private.

No direct table access is granted. Protected RPCs check active employment and
membership, including after a company lock. Manager, Employee, forged metadata,
revoked membership and other-company requests cannot read or write exchange data.
Private audit reasons, local acknowledgement references and saved payroll snapshots
never enter own/team projections. Limits fail explicitly without truncating totals:
1,000 rows/export; 100 exports/company; 50 profiles, 100 versions/profile;
1,000 rows/acknowledgement; bounded CSV/payload sizes and reconciliation history.
The new exchange history has an additional conservative32MiB company budget.
It fails explicitly instead of removing history or initiating a paid upgrade.
512KiB is reserved within that budget for closing unused retry identities, so
recovery remains available when ordinary history is full. Protected historical
downloads remain available while a write outcome is unresolved.

Native UI: Payroll > Data exchange. Admins edit profiles; HR/Admin preview, prepare,
save the exact frozen file, download a reconciliation template, import reviewed
acknowledgements and inspect original export/history. The direct connection control
stays unavailable with a target-required explanation. No external send occurs.

Implementation and delivery are in progress. Do not mark roadmap item7 complete
until local/server permission, retry, race, exact-byte and immutable-source checks,
native UI inspection, guarded migration and release/installation have passed.
