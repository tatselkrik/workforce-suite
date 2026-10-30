# Employment history

Administrators and HR Generalists open **Employees → employee name or Edit → History**. Account Manager opens the same employee record. History shows saved employment changes newest first, including who saved each change, their role and the server save time in the company time zone. Expand an entry to see **Before** and **After**. Creation and starting snapshots show their saved values. The normal save controls still save pending edits made in the other employee tabs.

Tracked fields are employee code, full name, department, job title, date joined, employment type, work location, Reports to, annual leave allowance and active/archived status. A promotion appears as a saved job-title/department change; the app does not infer an employment action. Recorded time is when the database saved the record, not a separate effective date. Scheduled/backdated employment actions and supporting documents are future work.

Existing employees receive a **Starting snapshot** when this feature is deployed. This does not claim when they were hired or reconstruct earlier changes. New employees, including automatic staff-account identities, receive **Employee created**. Later employment edits receive **Employment updated**. Updating only contacts, photos, credentials or staff permissions creates no employment event. An unchanged save creates none.

History is read-only. Correct a mistake in the Employment fields and save again; the original entry remains. Archiving retains the timeline. Manager and ordinary Employee scopes cannot read this HR-only history, including their own; it does not appear in Profile. Workforce Employee does not expose HR-only employment history.

Events retain the employment values and manager/type/location names used at that save. Later related renames cannot rewrite old snapshots or actor names. Renaming a company choice alone creates no employee event. Multiple employment changes in one save form one event. A database sequence orders events even if times coincide. Concurrent employee saves retain both committed before/after states; this module does not add optimistic conflict prevention to the employee editor.

The database trigger records desktop, older-client and provisioning changes. Its narrow helper is private with a fixed search path; clients cannot submit audit actors/timestamps or insert, edit or delete events. The event and employee/contact/staff operation commit or roll back together. Actor data comes from protected active HR membership and the canonical employee name. Trusted bootstrap/operator writes without an HR actor are labeled **System**. Authentication metadata grants no permissions.

Deploy every checked-in migration through `20261021125457_employment_history.sql` before version 0.1.11. It adds the protected table, indexes, private helpers, immutable-event guard and starting snapshots without changing existing employment fields or RPC signatures. No Edge Function, dependency, sign-in or company-profile change is required. File rollback does not undo database migrations.

The fictional demo resets on close. Clearly labeled archived synthetic prototype records may remain for testing. Contracts, attachments/digital 201 files and organization charts follow as separate modules; this timeline is not a document store or a general audit of every HR action.
