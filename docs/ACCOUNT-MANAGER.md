# Account Manager and protected staff duties

Account Manager appears above Settings only for Administrators. Current protected
database memberships and role assignments enforce permissions on direct API,
Storage and native operations. User-editable Auth metadata never grants access.
Every account belongs to one employee; staff duties can be combined on that account.

| Role | Scope |
| --- | --- |
| Administrator | Account Manager, company administration, authorized oversight and independent financial approval/payment |
| HR Generalist | Employee records, private contacts/documents, attendance, leave and general HR reports |
| Recruiter | Applicants, interviews, offers and recruiting/onboarding administration |
| Learning & Performance Officer | Combined Development administration; separate HR notes and private certificate/document evidence stay protected |
| Payroll Officer | Salary/payroll/government preparation and reporting; independent Administrator approval/payment remains required |
| Manager | Own/current direct reports and specifically assigned reviews/tasks |
| Employee alone | Own records in Workforce Employee for Android; desktop admission is rejected |

A new HR Generalist does not automatically receive Recruiting, Development or
Payroll. Existing legacy HR assignments retain their former rights through an
explicit four-specialty compatibility bundle. Administrator is application
authority; the application contains no developer backdoor.

## Administrator workflow

Account Manager lists staff with their employee name/code and assigned duties.
Use **Show disabled** for suspended staff. **Edit employee** opens employment and
contact details. **Manage access** edits staff responsibilities, enabled sign-in
and a required reason against the exact saved revision. Personal/company edits
and unresolved account requests participate in the unsaved-work guard.

**Add staff employee** opens one form with Employment, Contact details, Emergency
contact and staff access sections. **Save employee and access** saves the employee,
contacts and duties in one protected database transaction. For an existing person,
use **Staff role & sign-in** in their employee record. Established identities cannot
be reassigned. An unassigned existing Auth identity can be reused without changing
its password, confirmation or metadata.

Auth is a separate service. A credential-free reserved attempt fixes its Auth UUID;
SQL and Auth are not one cross-service transaction. Exact retries recover the same
receipt. Uncertain delivery retains the identity and pending request. Linked
replacement or reasoned cancellation retains history. No compensating Auth deletion
or password reset occurs. Passwords and tokens remain in memory.

Email confirmation uses the explicit **Send confirmation** action and the deployment's Auth confirmation settings. Retries do not automatically send another message or bypass confirmation.

## Suspending access and retaining history

Clear enabled sign-in in **Manage access**, enter a reason and save. Employee,
roles and historical records remain. Employee alone removes desktop duties while
retaining Android sign-in eligibility. Disable sign-in before archiving an employee.
Owner/self/last-Administrator safeguards and employee identity locks remain enforced.
A reporting relationship does not grant a Manager role and cannot form a cycle.

Changed or disabled membership affects subsequent server requests even with an
existing access token. The desktop rechecks authorization and clears its company
snapshot on revocation; previously downloaded memory cannot be remotely recalled.
Private contacts are HR/Admin or own-only, never ordinary Manager team data.

## Access history and financial boundaries

**Access history** opens immutable responsibility and original access events:
source/revision, roles, status, reason and current actor labels. Earlier unknown
facts remain unknown. Actor names are current names, not reconstructed old names.
Refresh preserves an unsaved role draft and rejects a substituted/stale source.

Specialty/Manager duties do not grant payroll approval, payment/remittance recording
or account administration. Current protected authority, independent actors, creator/
submitter exclusions and immutable original payslips/receipts remain required.
Delegation does not extend account/personnel/payroll permissions. Older delegation
clients explicitly require upgrade for records their DTO cannot represent.

## Server contract

The account-admin handler independently verifies Auth
and current protected Administrator membership. No service/signing secret is packaged.
The operator-only tools/staff-role-activation.sql checks the owner, complete canonical
assignments, schema and enabled release before setting the private company gate.
