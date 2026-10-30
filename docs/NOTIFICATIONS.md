# Notifications and reminders

Open **Notifications (unread count)** at the top right after signing in.
The sidebar stays unchanged. Save or close open editors first. Select All,
Approvals, My request updates, Reminders or Document expiry, or show unread
items only. Pages contain up to25 items. **Open record** marks the item read
and opens the relevant leave, correction, overtime, attendance date or employee
document list. Read/unread and Mark this page read never approve a request.

The inbox is generated from current protected records. Refresh, sign-in and
automatic connected refresh update the source state. Resolved review tasks
disappear; this inbox does not replace immutable approval or document history.
Read items remain actionable. Changed request revisions and document stages
produce new keys, so a previous read does not hide a later decision or expiry.

## Included items

- Pending leave at the user's currently eligible approval step; pending
  attendance corrections and overtime for their permitted review scope.
  Nobody receives a review task for their own request.
- Own approved/rejected/cancelled leave or correction decisions, and own
  overtime decision/revocation updates, when the latest saved event falls
  within the last30 company-local dates. Private reasons are not copied.
- Own approved leave starting within seven days, with a new stage on its
  starting date. Attendance check-out reminders require an own open record,
  a past work date and at least24 elapsed hours. Missing records never imply
  absence. These reminders do not change attendance facts.
- HR/Admin document-expiry and contract-end reminders for active employees'
  Ready, unarchived documents. Dates within30 days, today and past date are
  distinct stages. Incomplete/discarded uploads and archived people/documents
  are excluded. Expiry and contract end are separate dates/reminders.

Managers receive eligible team reviews, explicitly assigned leave approval
steps and their personal updates. A named leave reviewer can see a protected
receipt even when the requester is outside their employee-directory scope.
Private digital201 file reminders are HR/Admin-only even when the employee
reports to that Manager. Ordinary Employee accounts remain Android-only.

## Private read history

Migration `20261022145739` adds one private read-state row per account/company.
Only keys are stored, outside Auth metadata/JWTs; titles, contact details,
reasons and document contents stay in their original protected records.
RLS limits SELECT to the owning active desktop staff member. Direct writes
are revoked. The guarded invoker RPC derives the caller from Auth and checks
protected membership. Row-locked incremental operations preserve different
keys changed in separate sessions. Repeating a read operation is safe.

Read history supports5000 keys, with batches of1–100. **Read history > Reset
read history** clears only that account's markers, marking current items unread.
It never deletes HR records or another person's history. Read markers confer
no record access and inactive accounts immediately lose read/write access.

This module provides in-app notifications while HRIS is open and connected.
Email/push delivery, configurable escalation, background OS notifications and
external payroll/accounting/device connectors require separate implementation
and actual company service configuration.
