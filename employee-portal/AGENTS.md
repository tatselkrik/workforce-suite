# Shared employee service

Read the root AGENTS.md and local operator context if present. This directory's
historical package name is retained for compatibility, but it is a library used
by Workforce Employee for Android. The standalone browser executable is retired;
keep automatic binary discovery disabled and do not add hosting instructions.

Reuse the signed-in person's own protected Supabase APIs, current live membership,
memory-only upstream tokens, opaque HttpOnly cookies and page-memory CSRF tokens.
Never expose HR/team/private payroll records or raw upstream error bodies, even
when the account has staff permissions. The embedded HTTP origin must match its
actual bound loopback IP and port. Preserve strict Host/Origin/CSP/no-store rules,
bounded parsing/uploads/exports and Android lifecycle cleanup.

Contact/account changes require current identity and appropriate password proof.
Preserve unknown-save/retry handling, saved leave approvals and exact own payslip
projections. Explicit user-chosen exports differ from automatic record caching.
Keep the native desktop, public source and local deployment boundaries distinct.
