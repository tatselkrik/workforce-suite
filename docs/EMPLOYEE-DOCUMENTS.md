# Employee documents and digital 201 files

Open **Employees → saved employee → Documents**. Administrators and HR Generalists can upload, label, search, download and archive files. New employees must be saved first. Account Manager uses the same employee editor. Managers and ordinary Employee accounts cannot access these files or their history; the future employee portal needs its own explicitly scoped document access.

**Upload document** accepts PDF, DOCX, PNG and JPEG, up to 10 MiB per file. Choose a title and category: Employment contract, Identification, Certificate, HR form, Acknowledgment or Other. Notes and issue/expiry dates are optional. Contracts can also have start/end dates. Dates describe the document; uploading a contract does not change employment fields or schedule an employment action.

The Documents tab saves each upload or metadata edit separately. **Save employee changes** still saves the employee/contact form. Saving a document does not implicitly save unfinished employee edits. Search matches titles and filenames; category and archive filters refine the list. Expired/expiring labels are based on the company date, with a 30-day upcoming window; no background alert or reminder is sent yet.

## Original files and retention

**Download original** asks for a destination and preserves the original file extension. The app verifies its size, SHA256 and supported structure before creating a new file. It refuses to overwrite an existing file. Documents are never automatically opened, publicly linked, rendered or cached on the employee's computer. A deliberately downloaded copy remains in the chosen folder under the user's control.

**Edit details** changes labels, notes and dates while retaining the original file. To replace a file, upload a new document and archive the old one. **Archive** hides the filed document from the default list; **Show archived** exposes it for download or restoration. Archiving an employee retains documents and history. Filed documents have no client delete or overwrite permission.

PDFs undergo basic signature/end checks; images must decode within the size/dimension limits. DOCX packages are checked for a Word main content type, package relationship, document/body XML, valid ZIP data and bounded expansion. Password-encrypted files, DOC/DOCM, known macro-enabled Word package types and VBA project parts are rejected. DOCX support is limited to UTF-8 XML parts and standard stored/deflate ZIP entries. Format checks are not antivirus scanning or a full Office/PDF compatibility validator. Embedded content is retained as part of the original file; company malware scanning is a separate future deployment control.

## Incomplete uploads

A reservation appears as **Incomplete upload** if upload or finalization cannot finish. Reopen it with **Retry upload** and choose the same original file. Matching ID, metadata and checksum make retries idempotent; a mismatching file cannot silently replace the reservation. A retry after an ambiguous network failure can safely discover an already uploaded or filed original.

**Discard incomplete** first commits a permanent metadata tombstone, then removes only that upload's unfinished Storage object through the Storage API. Cleanup can be retried. The tombstone prevents a concurrent finalization from filing bytes after cleanup. Discard is unavailable for filed documents; archive them instead. Metadata for discarded reservations remains retained.

## Access, audit and deployment

The private Storage bucket, document metadata and append-only events use active protected company membership and HR/Admin permissions. The server derives the actor, company, timestamps, revisions and immutable object path; user-editable Auth metadata cannot grant access. Metadata changes require the current revision. A concurrent stale save returns a conflict and makes no extra event.

Upload, details update, archive and restore events retain before/after metadata and the protected actor/role/time. These events are separate from Employment History. File bytes are not duplicated into audit rows. The client receives no Supabase server key or public document URL.

Apply both checked-in employee-document migrations to the dedicated deployment before offering version 0.1.13. The second adds DOCX while preserving existing file paths. The signed private update advertises the required schema version; installing an executable does not migrate another company's database. Backups and recovery of private Storage bytes must be planned separately from database metadata and desktop file recovery.
