# Accounts and permissions

Account Manager appears above Settings for Administrators. Protected Supabase memberships enforce the same permissions for direct API requests. User-editable account metadata never grants access.

| Role | Desktop access | Records and actions | Administration |
| --- | --- | --- | --- |
| Administrator | Yes | All HR records; record attendance/requests; review other employees' leave | Account Manager, access history and company settings |
| HR Generalist | Yes | All HR records; record attendance/requests; review other employees' leave | Personal Profile and display settings |
| Manager | Yes | Own record and direct reports; record own attendance/requests; review direct reports' leave | Personal Profile and display settings |
| Employee | No; future mobile-friendly portal | Database scopes are prepared for own record, attendance and leave | Portal UI is not built yet |

Nobody can review their own leave. Ordinary employees use the future mobile-friendly portal, not this desktop app. Managers and other assigned staff roles use the desktop. These are fixed prototype roles; a Payroll Officer or custom permissions should be introduced with the corresponding module rather than receiving speculative access now.

Personal addresses and emergency contacts have a narrower scope than the employee directory: Administrators and HR Generalists manage all contact records; other active members can read only their own linked contacts. Managers cannot read direct reports' private contacts. See [Employee contacts](EMPLOYEE-CONTACTS.md).

## Administrator workflow

In Account Manager, use **New account**, enter the email, role and full employee name, then choose an initial password and confirmation. The default **Create employee automatically** creates both identities together; there is no separate employee-creation prerequisite. If HR has already recorded this person, select their existing unused employee instead. **Create account & send confirmation** creates an unconfirmed Auth identity, assigns access and requests its confirmation email. Share the initial password privately. Staff confirm email ownership before signing in and can change the initial password in Profile. Employee accounts are marked **Portal (planned)** and cannot sign into the desktop; creating one does not provide a working portal yet.

Use **Existing account** to assign an Auth account already provisioned for the deployment; it also receives an employee automatically if needed. It does not create or change that account's password. Use **Edit access** for later role/status changes. Employee identity cannot be silently reassigned. Set employment details and **Reports to** in **Employees**. Pending active accounts provide **Send confirmation** for delivery retries.

The original installation administrator remains enabled with the Administrator role and receives an employee record during migration. Users cannot disable or demote themselves. Reporting links cannot form cycles. Each employee has at most one account, including disabled accounts. Deactivate an account before archiving its employee record. Assigning a reporting relationship does not itself grant desktop access or an account role.

Disabling membership immediately removes future server access to HR data, including requests made with an already-issued access token. The desktop rechecks access on Refresh and after idle periods; previously downloaded records are in memory and cannot be remotely recalled. A revoked access check clears the local company snapshot. Account Profile/Auth ownership is distinct from permission to access company records.

## Deployment

Apply all checked-in migrations in order. The Account Manager migration preserves existing owner access and seeds the owner for future trusted company bootstrap. It adds `company_members`, `account_access_events`, private role helpers and invoker-only public RPCs. Authenticated clients cannot directly insert, update or delete membership/audit rows. Access changes are serialized and audited. Scope checks use the current database membership rather than stale role claims.

Deploy `supabase/functions/account-admin/index.ts` and its `handler.mjs` module to the same dedicated project. Gateway JWT verification is disabled for this function; its handler validates every bearer with Auth and checks the protected current membership. The hosted `SUPABASE_SERVICE_ROLE_KEY` remains in the function environment and is never packaged with the desktop. Database assignments use the caller's JWT and recheck administrator access; failed assignments attempt to remove only the exact newly created Auth identity. A mail failure after a committed assignment keeps the pending account for retry.

Configure a usable email-confirmation destination and a company mail provider before rollout. The prototype project's default confirmation redirect was previously observed to point to localhost. Default Supabase mail delivery also has recipient/rate restrictions; successful account creation does not guarantee delivery. Do not bypass email verification to hide delivery failures.

Role changes are managed through Account Manager. There is no employee portal UI, account self-registration, company selection, hard-delete operation, device inventory or multi-stage approval chain in this module. Employee-role desktop authentication is rejected before HR records load, and its new desktop Auth session is revoked on a best-effort basis. In-memory demo account changes never create real users or send email. Normal builds exclude QA preview controls.

## Verification

Run `cargo test --locked`, `npm test` in `tools/db-tests`, and `node --test tests/account_admin_edge.test.mjs`. Database tests exercise all four roles, direct reports, denied operations, metadata tampering, disabled access, owner protection, cycles and private photos. Node tests cover the privileged provisioning boundary, failure cleanup, delivery retries and credential redaction. Native QA previews support `HRIS_PREVIEW_ROLE` and the `accounts` page for administrator/nonadministrator layout checks.
