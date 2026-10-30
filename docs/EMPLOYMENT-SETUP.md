# Employment types and work locations

Administrators manage company-specific choices in **Settings → Employment setup**. Add a choice, edit its name or clear **Available for new assignments** to disable it. Disabled choices stay visible here and can be enabled again. Names must be unique within their list, including inactive choices, and contain 1–80 characters. Starter choices are Full-time, Part-time, Contract, Internship, Main office and Remote; they are editable labels, not an assignment or legal classification of any existing employee.

Administrators and HR Generalists assign an **Employment type** and **Work location** in the employee's **Employment** tab. Save changes saves employment and contact details together; the same fields are carried through the staff-account editor. **Not set** is valid for unknown information. Existing employees and automatically created employee identities are left unclassified until HR assigns choices.

**Profile → Employment** shows the saved canonical values read-only. Managers and ordinary employee account scopes cannot change company-managed employment fields. Ordinary employees use Workforce Employee for Android. Account Profile metadata and sign-in roles do not determine employment type/location.

The employee list provides separate type/location filters, **Not set**, inactive choices and **Clear filters**. Search includes these labels, and both assignments are visible in the list. Existing employee-status, department and access scopes still apply.

Disabling a choice retains its existing employee assignments. HR can keep that inactive assignment while editing other fields, or clear/replace it. It cannot be newly assigned to somebody else. Renaming a choice updates its displayed label wherever assigned; the current label is not a historical employment record. See [employee documents](EMPLOYEE-DOCUMENTS.md) and [employment history](EMPLOYMENT-HISTORY.md) for related records. No delete control is provided.

Choice editors retain their original revision. If another Administrator changes that choice, cancel, refresh and reopen before saving. Unsaved choice forms block an update/restart. Choice writes go through a guarded Administrator RPC, and the employee validation and choice writer share the company's transaction advisory lock to coordinate assignment/deactivation. Database foreign keys enforce company/type/location identity; permissions and active membership are checked on the server.

Deploy every checked-in migration, including `20261021121404_employment_types_and_locations.sql`, before version 0.1.10. It adds protected dictionaries, indexed nullable employee references, a guarded choice RPC, starter-choice bootstrap and support in the existing atomic employee/staff save path. Older clients that omit the two new keys retain the existing assignments; explicit JSON null clears a field. No privileged client key or new Edge Function deployment is needed.
