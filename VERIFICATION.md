# Prototype verification — 2026-10-20

## Supabase connection update

- All 17 Rust tests passed: 14 HR workflow/configuration tests, two local Auth/REST contract tests, and one company-profile precedence/fail-closed test. Formatting was applied with the pinned-compatible Rust 1.95.0 toolchain. Tests use fictional identities and credentials.
- All 55 local PostgreSQL schema, permission, singleton-company, and workflow assertions passed against both checked-in migrations.
- Both migrations were applied to the dedicated HRIS project. Local filenames match the server-assigned migration history: `20261020111810_initial_hris.sql` and `20261020113112_single_company_installation.sql`. Files were originally generated with the Supabase CLI before their versions were aligned to hosted history.
- The hosted SQL smoke test exercised actual PostgreSQL RLS, owner access, unrelated-account denial, anonymous denial, company uniqueness, employee validation, server attendance timestamps, duplicate check-in rejection, leave approval and allowance limits. All fixture identities and HR records were rolled back; post-test counts confirmed zero fixture identities and zero company/employee records.
- The public Auth health endpoint returned HTTP 200. Unsigned-in REST reads returned HTTP 401 for each of the four HR tables. These requests used a modern publishable key without an administrative key.
- The security advisor reported no lints. The performance advisor reported four informational [unused-index notices](https://supabase.com/docs/guides/database/database-linter?lint=0005_unused_index) on the fresh database; required/query indexes were retained.
- The requested administrator was created through the normal Auth signup endpoint, with email confirmation required. Trusted bootstrap assigned the sole account to one placeholder company. The live employee, attendance, and leave tables are empty. No password or session was stored in source, settings, or deployment files.
- The configured native startup screen was captured and visually inspected. Sign-in now appears before company defaults, with configured connection details collapsed. Email, password and sign-in controls fit in the visible window. This screenshot used a generic example profile rather than private deployment identifiers.

These hosted SQL tests use database roles and simulated request subjects, not a real signed-in user's JWT. Actual administrator password sign-in, authenticated desktop persistence and hosted concurrent clients remain unverified until email confirmation and live sign-in are completed. Hosted SQL results and public HTTP denial checks do not establish those remaining behaviors.

## Earlier prototype checks

The initial prototype separately passed inspection of Overview, Employees, Attendance, Leave, Settings, Add employee and Request leave, plus a normal optimized Windows startup/close check. Its original 15 integration tests and 52 local SQL assertions were superseded by the current expanded suites above. The original screenshot helper returned transparent pixels; the supported egui screenshot callback is used instead. Restricted local networking required the HTTP fixture test to run with normal loopback access. A cache hard-link warning fell back to file copying; it did not fail compilation.

The updated Windows candidate remains a private, unsigned prototype at version 0.1.0. Optimized assembly, startup, artifact hashes and source-revision evidence belong to its distribution manifest. No remote GitHub CI run, public release, installer, signing, automatic update, other operating-system check or company production rollout has been verified. No unrelated hosted project was changed.
