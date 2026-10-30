# Employee relations

Employee relations lives in Employees > Employee relations in HRIS. Current
Administrators oversee all company cases; HR Generalists see and act only on cases
assigned to them. An HR Generalist can create a case assigned to themselves.
Only an Administrator can assign/reassign its handler. Current protected
membership, active handler employment and company scope are checked on the server;
user-editable Auth metadata, line management and Payroll access grant no case access.

Cases record one employee (including retained former-employee history), a short
title, category, priority and initial confidential description. Notes are append-only.
Follow-ups have due dates and retained completion notes. Open, In review, Awaiting
follow-up, Resolved and Closed have explicit transitions/reasons. Pending follow-ups
must be completed before resolution/closure; a closed case needs Administrator
reopening. These are internal tracking states, not legal findings or disciplinary
actions. Nothing automatically changes personnel, payroll, exit, attendance or leave.

Keep cases, follow-ups, full request retry receipts and immutable events in the
private schema with no direct client table privileges. Expose versioned invoker
wrappers over guarded private transactions only. Bind each request identity to
company, current actor and the complete payload; preserve optimistic case revisions,
company/account locking, current assignment checks even for retries, and atomic
events. Cases/follow-ups are retained; no delete control is provided. Exceeding
bounded context/history budgets must fail explicitly rather than silently omit data.

The native workspace loads its separate confidential context on demand; never put
it into shared Snapshot, employee-app payloads, Payroll, general reports, notifications,
Auth metadata or local files. Once loaded, refresh it during normal protected
refreshes. Revoked assignment/access clears cached case contents and affected
drafts. Preserve ordinary drafts through non-authority failures and uncertain
saves: retry the exact frozen request; definitive rejection requires explicit review.
Attachments, automatic disciplinary decisions, employee self-service cases, external
communications and statutory case-handling advice are outside this increment.
