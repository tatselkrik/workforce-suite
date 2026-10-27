# Accounts and permissions

Account Manager appears above Settings for Administrators. Protected Supabase memberships enforce the same permissions for direct API requests. User-editable account metadata never grants access.

| Role | Desktop access | Records and actions | Administration |
| --- | --- | --- | --- |
| Administrator | Yes | All HR records; record attendance/requests; review other employees' leave | Account Manager, access history and company settings |
| HR Generalist | Yes | All HR records; record attendance/requests; review other employees' leave | Personal Profile and display settings |
| Manager | Yes | Own record and direct reports; record own attendance/requests; review direct reports' leave | Personal Profile and display settings |
| Employee | No; future mobile-friendly portal | Database scopes are prepared for own record, attendance and leave | Portal UI is not built yet |

Nobody can review their own leave. Ordinary employees use the future mobile-friendly portal, not this desktop app. Managers and other assigned staff roles use the desktop. These are fixed prototype roles; a Payroll Officer or custom permissions should be introduced with the corresponding module rather than receiving speculative access now.

Personal addresses and emergency contacts have a narrower scope than the employee directory: Administrators and HR Generalists manage all contact records; other active members can read and edit only their own contacts through Profile. Managers cannot read direct reports' private contacts. See [Employee contacts](EMPLOYEE-CONTACTS.md).

## Administrator workflow

Account Manager lists employees with desktop staff roles. It shows their employee name/code first and sign-in email beneath. Disabled staff are hidden by default; **Show disabled** reveals them, including clearly labeled synthetic prototype accounts.

Use **Add staff employee** to open one employee form with Employment, Contact details, Emergency contact and Staff role & sign-in tabs. Enter the employment fields and contact details, then the sign-in email, role, initial password and confirmation. **Save employee & role** creates an unconfirmed Auth identity and saves the employee, contacts and protected role together. Unknown employment details may remain unset. Staff confirm email ownership before signing in and can change their initial password in Profile. Share passwords privately.

For someone already in Employees, open that person and use **Staff role & sign-in**; saving adds access to the same employee without creating another person. Clicking an Account Manager employee or its Edit action opens this same complete editor for later changes. There is no separate link/employee selector or Existing account setup step. Established identities cannot be reassigned. Pending active staff provide **Send confirmation** for delivery retries. Selecting **No staff role (employee portal)** removes desktop admission; the portal remains planned and that person no longer belongs in the staff-only list.

To suspend someone's access, open their employee → **Staff role & sign-in**, clear **Sign-in access enabled**, then **Save employee & role**. Their employee record and role are retained. Use **Show disabled** to find suspended staff and enable them again. Choosing **No staff role (employee portal)** removes desktop admission while retaining the employee; the portal remains planned. Your own/original Administrator access is protected.

The original installation administrator remains enabled with the Administrator role and receives an employee record during migration. Users cannot disable or demote themselves. Reporting links cannot form cycles. Each employee has at most one account, including disabled accounts. Deactivate an account before archiving its employee record. Assigning a reporting relationship does not itself grant desktop access or an account role.

Disabling membership immediately removes future server access to HR data, including requests made with an already-issued access token. The desktop rechecks access on Refresh and after idle periods; previously downloaded records are in memory and cannot be remotely recalled. A revoked access check clears the local company snapshot. Account Profile/Auth ownership is distinct from permission to access company records.

## Deployment

Apply all checked-in migrations in order. The Account Manager migration preserves existing owner access and seeds the owner for future trusted company bootstrap. It adds `company_members`, `account_access_events`, private role helpers and invoker-only public RPCs. Authenticated clients cannot directly insert, update or delete membership/audit rows. Access changes are serialized and audited. Scope checks use the current database membership rather than stale role claims.

Deploy `supabase/functions/account-admin/index.ts` and its `handler.mjs` module to the same dedicated project. Gateway JWT verification is disabled for this function; its handler validates every bearer with Auth and checks the protected current membership. The hosted `SUPABASE_SERVICE_ROLE_KEY` remains in the function environment and is never packaged with the desktop. Database assignments use the caller's JWT and recheck administrator access; failed assignments attempt to remove only the exact newly created Auth identity. A mail failure after a committed assignment keeps the pending account for retry.

Configure a usable email-confirmation destination and a company mail provider before rollout. The prototype project's default confirmation redirect was previously observed to point to localhost. Default Supabase mail delivery also has recipient/rate restrictions; successful account creation does not guarantee delivery. Do not bypass email verification to hide delivery failures.

Role changes are managed through Account Manager. There is no employee portal UI, account self-registration, company selection, hard-delete operation, device inventory or multi-stage approval chain in this module. Employee-role desktop authentication is rejected before HR records load, and its new desktop Auth session is revoked on a best-effort basis. In-memory demo account changes never create real users or send email. Normal builds exclude QA preview controls.

## Verification

Run `cargo test --locked`, `npm test` in `tools/db-tests`, and `node --test tests/account_admin_edge.test.mjs`. Database tests exercise all four roles, direct reports, denied operations, metadata tampering, disabled access, owner protection, cycles and private photos. Node tests cover the privileged provisioning boundary, failure cleanup, delivery retries and credential redaction. Native QA previews support `HRIS_PREVIEW_ROLE` and the `accounts` page for administrator/nonadministrator layout checks.


## Approved combined Development design — 27 October 2026

Kirk chose one **Development** workspace containing Goals, Appraisals, Training,
Certifications and Career plans. These functions are already grouped under
**Employees > Development** in the current desktop; Review cycles remain an
internal administrative section. Learning and Performance stay inside HRIS.

Use one **Learning & Performance Officer** specialist role in the planned
permissions extension. This supersedes the proposal for separate Learning Officer
and Performance Officer roles. A company can assign the combined responsibility
to one person; Managers continue their assigned team reviews and progress work.

**Implementation status:** the combined workspace exists. The specialist role
is approved design only; the installed application/database still use the four
roles documented above. Its protected permissions, Account Manager controls and
verification must be implemented together before the new role can be assigned.
Grouping the workspace must preserve current reporting/assignment boundaries,
private HR notes and certificate evidence, self-review exclusions and immutable
published review/history records.
