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
No credentials, sessions or automatic employee-record caches in logs, browser
storage or disk. Explicit CSV/PDF exports are private files the person chooses
to save; keep them distinct from automatic application storage.

Own profile/contacts, attendance/breaks, leave requests and approved payslip access
are the current scope. Portal leave wrappers infer the current employee and guard
policy/record revisions; staff roles still receive only own-record controls here.
Follow ../docs/EMPLOYEE-PORTAL.md for verification and remaining scope.
Do not claim public hosting, phone access or a native mobile app without evidence.


Kirk resumed portal development on24October2026 for LOCAL use only, including the
remaining own correction/overtime requests, notifications, profile/security and
branding scope. Do not publicly host, tunnel or expose the server, or resume
unrelated desktop employee access. Preserve the existing own-record/token/CSRF
boundaries and nested release lifecycle.

Kirk subsequently instructed this chat to stop after desktop0.1.41. Portal work is
queued and PAUSED until a new human instruction resumes it; local-only deployment
and all security boundaries remain in force.
