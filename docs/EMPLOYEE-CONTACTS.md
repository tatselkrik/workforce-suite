# Employee contact and emergency details

Create an employee in **Employees**, then choose **Contacts** on their row. The two tabs share one **Save contacts** action, which validates and saves both tabs together. Cancel or close discards unsaved edits. The employee code/name, attendance, allowance and account sign-in remain separate.

**Contact details** contains an optional phone, personal email and home address (up to 500 characters). **Emergency contact** contains a name, relationship, primary phone and optional alternate phone. Enter name, relationship and primary phone together, or leave every emergency field blank. Phone numbers support international prefixes, separators and extensions; formatting checks do not verify that a number/email is reachable. Personal email does not change account email or send confirmation mail.

| Role | Read contact records | Change contact records |
| --- | --- | --- |
| Administrator | Everyone in the company | Yes |
| HR Generalist | Everyone in the company | Yes |
| Manager | Own linked employee only | No; request changes from HR |
| Employee | Own linked employee at the database level; portal UI remains future work | No |
| Disabled/unassigned/anonymous | None | No |

Manager team-directory and leave permissions do not expose team home addresses or emergency contacts. The **Contacts** button is available only for readable private records. All API reads and writes enforce the same permissions through current protected memberships and RLS, including access revocation. Account Profile remains an independently editable personal display/security page.

The additive migration creates `employee_contacts` with a company/employee foreign key, one record per employee, explicit column grants, RLS and server validation/timestamps. Clients cannot change row/company/employee identity, set update timestamps or delete records. HR can clear optional fields by saving blanks; archiving an employee retains contacts for authorized HR access. Existing directory rows are not rewritten and old installed clients remain compatible with this additive table. Rollback can initially mean reinstalling the prior desktop; leave the additive table intact to preserve data.

In-memory demo contacts are fictional and reset when closed. Clearly labeled synthetic prototype records may remain in the dedicated testing database for repeat verification. Do not enter actual workplace contacts until company rollout requirements are settled. Employment history, contracts, document uploads/201 files, organization charts and multiple emergency contacts are later modules.
