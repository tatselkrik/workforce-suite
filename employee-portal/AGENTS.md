# Shared Android employee service

Read the root AGENTS.md and local operator instructions if present. This package
is a library used by Workforce Employee for Android. Keep `autobins = false`
and expose no standalone executable or service launcher.

Reuse the signed-in person's protected own-record APIs, current live membership,
memory-only upstream tokens, opaque HttpOnly cookies and page-memory CSRF tokens.
Never expose HR, team or private payroll context or raw upstream errors, even to
an account with staff responsibilities. The embedded HTTP origin must match its
actual bound loopback IP and port. Preserve strict Host/Origin/CSP/no-store rules,
bounded parsing, uploads and exports, and Android lifecycle cleanup.

Contact and account changes require current identity and appropriate password
proof. Preserve uncertain-save retries, saved leave approvals and exact own
payslip projections. Explicit user-chosen exports differ from automatic caching.
Keep native desktop and company deployment authority distinct from library behavior.
