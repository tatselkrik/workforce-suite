# Review checklist — work since 9:31 p.m.

This covers portal increments 0.1.2–0.1.5 and desktop 0.1.42–0.1.45. It does not
repeat earlier payroll, hiring or performance features. Use fictional records.
The final 0.1.45 delivery status is in its
[verification note](GOVERNMENT-0.1.45-VERIFICATION.md).

## Desktop — latest installed release after delivery

- [ ] **Version:** check the top-right version is **0.1.45**. Company connection,
  branding and existing records should remain. Reopen/sign in if an older process
  was left open; no account password was changed by this increment.
- [ ] **Payroll → Government:** switch among SSS, PhilHealth, Pag-IBIG and BIR,
  select a month and generate/refresh. Check known versus unresolved liability,
  included/excluded sources, earned contribution months and actual wage-receipt
  dates for BIR. See the [0.1.42 steps](GOVERNMENT-0.1.42-CHECKLIST.md).
- [ ] **Coverage/share reviews:** save an evidenced draft, submit, approve with
  a different eligible Administrator, inspect history and revise through a linked
  successor. Issued employee amounts must remain unchanged.
- [ ] **Recorded remittances:** as Administrator, test a clearly fictional
  receipt, monthly allocations, void and linked replacement. Original history
  must remain. HR Generalists must not record/void receipts. End these synthetic
  tests with receipts voided; these records do not send money.
- [ ] **Registration details:** test company/employee legal names, assigned
  identifiers, supporting review and retained revision history. Unknowns remain
  blank; these fields must not rename canonical employment or company branding.
  See the [0.1.43 steps](GOVERNMENT-0.1.43-CHECKLIST.md).
- [ ] **Both CSV exports:** generic fresh-review CSV excludes legal identifiers;
  agency-preparation CSV includes only the chosen agency's required registered
  fields, missing-input flags and reviewed sources/totals. Preserve leading zeros
  by importing identifiers as text. See the [0.1.44 steps](GOVERNMENT-0.1.44-CHECKLIST.md).
- [ ] **SSS R3 reference draft:** test readiness errors first. A ready month needs
  known reconciled shares, legal IDs and one active full-month receipt. Review
  fixed amounts/identifiers, explicit names/initials/declarations and same-month
  event dates; save to a new `.txt`. Existing files and original records must
  remain unchanged. Test cancel, source checking, stale-input rejection and
  review reset after an edit. See the [0.1.45 steps](GOVERNMENT-0.1.45-CHECKLIST.md).
- [ ] **Access and layout:** Managers must not see Government/private legal
  registrations. Check sign-out clearing, unsaved-form protection, normal/compact
  layouts and your chosen theme.

## Separate portal — local packages, not part of the desktop installer

Use the latest local **0.1.5** package via the
[operator startup instructions](EMPLOYEE-PORTAL.md#operator-startup). A previously
running older portal is preserved and does not update merely because the desktop
was installed. Keep the listener loopback-only.

- [ ] **Own attendance corrections/overtime:** file/cancel requests, inspect
  reasons/history and review them through another eligible desktop staff account.
  Filing a correction alone must not alter the original punches; changed parent
  facts must withhold stale overtime approval.
- [ ] **Personal inbox:** unread count, pages, unread-only filtering, read/unread,
  mark-shown and reset history. Reminders link only to the current person's data;
  marking read must not approve a request.
- [ ] **Pictures/branding:** preview/save/discard/remove a circular own photo;
  company logo/name appear at sign-in/header. Sign-out clears personal images.
- [ ] **Account security:** current-password verification, mismatched-password
  rejection, pending sign-in email status and sign-out-other-sessions. Use a
  synthetic account for password changes; restore its original value afterward.
  Actual email confirmation delivery and the newest browser/phone layouts remain
  unverified; these are still outstanding checks.
- [ ] **Own-record scope:** staff roles using the portal receive only their own
  records and no team approval controls. Employee accounts remain blocked from
  the native staff desktop.

Storage cleanup also removed obsolete generated output: about219.1GiB became
about1.82GiB immediately after cleanup. Later builds necessarily use additional
space; that historical size is not a current folder-size measurement. Source,
company files and retained current/rollback packages were preserved.

Internal CSVs and the SSS reference draft are not verified government upload
files. No government return or money transfer was performed. The existing
scheduler is paused at Kirk's finish-current-increment boundary; see the
[remaining ordered roadmap](REMAINING-ROADMAP.md).
