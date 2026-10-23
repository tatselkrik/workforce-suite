# Separate employee portal

Read ../memory.md and the root AGENTS.md; current human authority takes precedence.
This is a separate Rust server/browser application, never embedded in the native
staff desktop. Preserve identifier io.github.tatselkrik.hrisportal. Each deployment
uses one operator-supplied company profile; no company/database chooser in the UI.

Use only the publishable Supabase key and the actual user's bearer for upstream
requests. Keep Auth tokens in server memory; browser receives an opaque HttpOnly
session cookie and an in-memory CSRF token. Validate live Auth and protected company
membership on data requests. Query only the session's own employee/company, even
when the account has broader staff permissions. Never expose private HR drafts,
team records or raw upstream Auth/error bodies. HTTPS is required outside loopback.
No credentials, sessions or employee records in logs, browser storage or disk.

The initial milestone is own profile/contacts, current attendance and approved
payslip access. Follow docs/EMPLOYEE-PORTAL.md for verification/remaining scope.
Do not claim public hosting, phone access or a native mobile app without evidence.
