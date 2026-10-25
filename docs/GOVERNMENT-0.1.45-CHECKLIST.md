# Version 0.1.45 review checklist

Use fictional records. This increment adds a **local SSS R3 reference draft**,
not a verified agency upload, payment or government filing.

1. As HR/Admin, open **Payroll → Government**, choose **SSS** and a contribution
   month, then **Generate / refresh → Prepare SSS R3 reference draft**.
2. Missing legal/SSS registrations, unresolved shares, an unreconciled amount,
   multiple active receipts or a receipt covering several months must explain
   why preparation is blocked. A recorded TR/SBR must fit ten characters; do
   not truncate a longer PRN or bank reference to make it fit.
3. On a ready fictional month, check the fixed SSS identifiers, SS/EC amounts and
   recorded receipt. Enter reviewed employer/name representations and explicit
   middle initials. They affect only the local draft, not saved registration.
4. Choose **Normal**, **New hire** or **Separated** for every contributing person.
   New-hire/separation dates must fall in that month. The bulk Normal button is
   an explicit declaration; review exceptions afterward. No-earnings employees
   are not inferred from zero deductions in this increment.
5. Check the review box and save to a **new `.txt` filename**. Unsupported widths,
   non-ASCII characters or missing declarations must block export. Existing
   files must remain unchanged. Canceling the chooser must retain the form.
6. Edit a name after checking the review box: the review must reset. **Check
   current sources** must retain typed input. Changed registrations, receipts
   or source facts must require a new form before export. **Discard edits and
   load checked sources** explicitly starts over.
7. Check navigation/closing protection while editing and the form at your usual
   window size/theme. A Manager must not gain access to this screen or export.
8. Existing **fresh review CSV** and **agency preparation CSV** must still work.

Current My.SSS validator acceptance, broader encoding and reserved-slot semantics
remain unverified. Keep this draft for HR review; do not treat it as filing-ready.
