# Work arrangements and daily work

Workforce HR manages dated attendance policies, office stations, tasks and daily
reports. Workforce Employee for Android shows only the signed-in person's work.

## Attendance methods

In **Attendance → Setup → Work arrangements**, HR saves a new policy version for an
employee. Choose an effective date, optional end date, reason and arrangement:

| Arrangement | Routine attendance |
| --- | --- |
| On-site | An enrolled office PC with a fresh webcam photo and personal PIN |
| Remote | The employee's own signed-in Android account |
| Hybrid | Office station on selected ISO weekdays; own Android on other work dates |

New versions are drafts until deliberately activated. Office restrictions require
an active station that has completed a photo-backed punch. Earlier punches and
their saved policy remain intact. A temporary version ends on its entered date;
the earlier applicable version then resumes. An open shift uses its original work
date and policy for checkout, including overnight work. With no active policy,
existing punch methods continue.

Employment records show the loaded attendance arrangement. Save employment edits
before opening the full policy workspace. Reasoned HR corrections remain the
exception route for missing punches, camera failures or a policy dispute.

## Office station

1. An Administrator enrolls the PC under **Attendance → Setup → Office stations**.
2. HR sets an active employee's personal attendance PIN and gives it to them privately.
3. Open the attendance kiosk and let an Administrator unlock the station session.
4. The employee enters their code and PIN, checks the live webcam preview and records check-in or checkout.
5. Wait for the saved receipt before leaving. If delivery is uncertain, retry the same capture.

The kiosk is an attendance-only mode of Workforce HR. The temporary Administrator
sign-in closes after unlocking. Its restricted grant stays in memory, expires
after 16 hours, and is revoked when the station is closed, disabled or unlocked
again. Closing while offline may leave the grant valid until its expiry. It does
not provide HR management access. A grant authorizes that unlocked session; it
does not provide hardware attestation or facial identity verification.

The employee's code and PIN clear after a confirmed action or an idle interval.
Five incorrect PIN attempts lock that PIN for 15 minutes; HR can reset it. A
fresh photo is required for each punch. No facial recognition or continuous
recording is performed. A failed photo upload never counts as attendance.

Photos are private HR evidence, compressed to at most 64 KiB each. HR can open
them from **Daily attendance → Photos**. **Clean expired attendance photos**
removes images older than seven days and abandoned uploads, in batches of up to
100, while retaining receipts, hashes and cleanup attribution. Cleanup runs when
HR requests it. The service stops new captures at a 128 MiB retained/pending
evidence budget until HR reviews storage.

## Tasks and reports

Managers use **Attendance → Daily reports → Tasks** to assign current team members
a task, expected outcome, priority and due date. HR attendance administrators can
oversee this work. Independent review excludes the reviewer's own submissions.

Employees use **My work** in Android to update task progress and prepare a daily
report: summary, completed work, unfinished work, blockers and next actions. They
may include their own assigned task snapshots and explain necessary unassigned
work in the report. Use the shift start date for overnight work.

A Manager can open a submitted report from **Overview → Needs your review** or
**Daily reports**. The review shows the saved submission, task snapshots and prior
decisions alongside the review note and **Approve** and **Return for revision**
buttons. From an Overview review, use **Back to review queue** or **Review next**
after saving or discarding your note. Returned reports can be revised and
resubmitted. Each submission and decision remains in history. Review permissions
follow the current reporting relationship.

In **Daily reports**, search by employee name or code and use **From**,
**Through** and the status menu. Both date boundaries are included and either
can be left open. Counts cover all matching reports, and each page shows up to
25. **Clear report filters** resets search, date and status and keeps the
employee selection.

In **Daily reports → Remote report follow-ups**, choose a work date and load the
list of completed remote shifts with a missing, draft or returned report. The
list uses the work date and attendance method saved at check-in, including hybrid
remote days and overnight shifts. Managers see their current team and HR sees
the current staff they can review. The employee filter narrows the list.

Report approval and task acceptance do not change punches, absence, overtime,
performance ratings or pay. Employees can check out without submitting a report.
Task availability never blocks legitimate attendance.
