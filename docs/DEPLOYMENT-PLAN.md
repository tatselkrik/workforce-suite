# Employee contacts deployment — 0.1.5

Status: the additive contact migration is deployed and hosted permission checks passed. Kirk authorized direct SQL changes/testing in this dedicated prototype and retention of labeled synthetic fixtures. This authorization does not extend to unrelated projects or a future company's production rollout.

Apply all migrations in order, including `20261021033544_employee_contact_details.sql`. It creates a separate private contact table, explicit column grants, validation/timestamp trigger and current-membership RLS. Existing employee directory rows, account policies and the active `account-admin` function stay compatible. No server credential, account email/password change, new cost or data deletion is needed. Retain the labeled synthetic contact fixture for repeat testing.

45 Rust tests, 166 local database assertions, 22 handler regression tests, hosted role/validation checks, native minimum-size/125% layouts and live Rust-adapter save/new-sign-in persistence passed. See `VERIFICATION.md` for exact limits. Package the normal executable with the existing private company connection, back up the previous installation and verify payload hashes, registration, shortcuts and startup. If rollback is needed, restore 0.1.4 without dropping the additive contact table.

## Historical Account Manager deployment review — 0.1.4

Status: explicitly approved and deployed on 2026-10-21; hosted role/provisioning checks passed and temporary test data was removed. The private installation report records the subsequent guarded Windows update.

## Approved hosted changes

Applied `supabase/migrations/20261021021602_account_manager_roles.sql` only to the existing dedicated HRIS project. Its filename matches the server-assigned version.

- Add protected company memberships and account-access history; backfill the existing owner as an active Administrator.
- Add narrow private authorization helpers and invoker-only public access/account RPCs.
- Replace owner-only HR access with current membership scopes: Administrator/HR Generalist for company HR records, Manager for own/direct reports, Employee for future own-record portal access. Employee accounts are rejected by the desktop.
- Preserve the original owner and prevent client-written memberships, self-demotion, self-approval, duplicate active employee links and reporting cycles. Serialize employee links/archiving and leave-budget decisions.
- Extend private own-photo policies to active members. No public photo bucket or server credential is introduced.

Deployed `supabase/functions/account-admin/index.ts` with `handler.mjs` as the new `account-admin` function, active version 1, in the same project. It validates each bearer with Auth, checks current Administrator membership, creates unconfirmed Auth identities using its hosted server credential and assigns access using the caller's JWT. It sends confirmation requests and preserves pending accounts if delivery fails. No existing function was replaced.

The change affects live access rules and adds privileged server-side account provisioning. It does not delete existing HR data, change the company connection, change the administrator password/email, switch the Free plan or touch another company's project. The migration is transactional. The preflight found one company, one confirmed owner and no HR records; recheck that state at deployment time.

## Verification and installation sequence

1. Recheck the target, hosted migration history, owner membership and record counts. Apply the reviewed migration and align its local filename to the returned version.
2. Deploy the reviewed function. Verify active administrator access and anonymous/nonadministrator denial, run rollback-only hosted role/workflow fixtures and inspect security/performance advisors. Verify confirmed account provisioning separately without bypassing email ownership.
3. Recheck the existing account and record counts after cleanup. Preserve the original account credentials, deployment profile and Free plan.
4. Rebuild the private package from the final exact source revision if migration naming or verification documentation changed. Install only after HRIS is closed and its existing installation marker, executable and profile are verified; back up the existing named files first. Verify installed hashes, Windows registration, shortcuts and startup.

The original owner remains compatible with 0.1.3, which can be retained or restored during investigation without dropping the new membership/history tables. Do not automatically roll back by deleting accounts or audit history. Any hosted permission failure requires diagnosis before installation proceeds.

## Current evidence and limits

40 Rust tests, 126 local PostgreSQL assertions and 22 provisioning-handler tests passed. Formatting, optimized Windows build and native minimum-size/125% visual checks passed. Tests are local and use fictional identities; hosted Account Manager workflows have not run.

The migration, active function, rollback-only hosted role suite and real account-provisioning checks passed after explicit approval. The original owner remains active. Signup delivery/confirmation, HR restrictions, disabled-token/private-photo access, actual Manager desktop admission and Employee rejection passed; guarded cleanup restored the original counts. See `VERIFICATION.md` for evidence and test limits. The default localhost confirmation redirect and company mail-delivery configuration still need a usable deployment flow before workplace rollout. No public release, push or remote CI result is claimed.
