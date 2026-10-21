# Employee contact and emergency details

In **Employees**, click the employee name or **Edit**. One employee record contains **Employment**, **Contact details** and **Emergency contact** tabs. Administrators also see **Staff role & sign-in** to add or change access for that same employee; Account Manager opens this identical form. **Save changes** validates and saves employment and private contacts in one database transaction. A failed validation saves neither part; Cancel or Back discards unsaved changes. The save controls remain visible while the fields scroll.

**Employment** contains the employee code, full name, optional department/job title/date joined, annual allowance, status and **Reports to**. Reporting relationships are employee-to-employee, including people without accounts. Self-reporting and cycles are rejected. Reassign active direct reports before archiving a manager. Every account has an employee identity, including the original administrator; automatically created records leave unknown employment details blank for HR to complete.

**Contact details** contains an optional phone, personal email and home address (up to 500 characters). **Emergency contact** contains a name, relationship, primary phone and optional alternate phone. Enter name, relationship and primary phone together, or leave every emergency field blank. Phone numbers support international prefixes, separators and extensions; formatting checks do not verify that a number/email is reachable. Personal email does not change account email or send confirmation mail.

| Role | Read contact records | Change contact records |
| --- | --- | --- |
| Administrator | Everyone in the company | Yes |
| HR Generalist | Everyone in the company | Yes |
| Manager | Own employee only | Own contacts through Profile |
| Employee | Own employee at the database level; portal UI remains future work | Own contacts at the database level; no desktop/portal UI yet |
| Disabled/unassigned/anonymous | None | No |

Manager team-directory and leave permissions do not expose team home addresses or emergency contacts. Private tabs appear only for readable records. All API reads and writes enforce the same permissions through current protected memberships and RLS, including access revocation. Profile mirrors the employee record's full Employment fields read-only, including employee code, name, department, title, date joined, Reports to, allowance and status. Contact details and Emergency contact save that same private contact row; edits survive ordinary refresh while pending. HR employment changes appear on refresh. Profile photo, account security and sessions remain available. A narrow protected access projection supplies the own supervisor's name without giving a Manager access to that supervisor's employment record.

The contact migration creates `employee_contacts` with a company/employee foreign key, one record per employee, explicit column grants, RLS and server validation/timestamps. The later employee-record migration adds reporting, backfills account employees and provides the invoker-only `save_employee_record` RPC. Clients cannot change row/company/employee identity, set update timestamps or delete records. HR can clear optional fields by saving blanks; archiving retains contacts. Version 0.1.6 permits an unset hire date, which older clients cannot parse; coordinate its database deployment with installing 0.1.6. Do not assume reinstalling 0.1.5 reverses that database change.

In-memory demo contacts are fictional and reset when closed. Clearly labeled synthetic prototype records may remain in the dedicated testing database for repeat verification. Do not enter actual workplace contacts until company rollout requirements are settled. Employment history, contracts, document uploads/201 files, organization charts and multiple emergency contacts are later modules.
