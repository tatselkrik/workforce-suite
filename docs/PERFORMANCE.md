# Goals and appraisals

Implemented in version 0.1.29 with migration `20261022231643`.

The entry is Employees > Development, with Goals and Appraisals.
HR/Admin assign company cycles and reviewers. Managers assign goals to current
direct reports, report their own progress and assess only assigned current
reports. Ordinary Employee accounts remain portal-only; database own-goal and
own-published-review reads prepare for that separate application. They cannot
invoke the desktop staff mutation RPC.

HR notes and their events are separate from published review data. Reassignment
resets unfinished assessments and prevents a replacement Manager from reading
the previous reviewer's private draft/history. HR retains the audit. Current
membership, assignment and reporting checks apply on every mutation. Nobody
assesses, assigns, publishes or cancels their own appraisal through HR powers.
Another HR/Admin must manage it. HR can assess/publish another employee where
assigned; this is not a configurable multi-reviewer approval chain.

Goal progress is 0–100, with an explicit completion status after 100% is
recorded. Appraisals use a prototype overall rating of 1–5 plus assessment,
strengths and development feedback. A company rubric is later configuration.
Submission saves up to 100 overlapping, non-cancelled goal snapshots. Later
progress changes do not rewrite published facts. HR can return a submitted
review to draft; published reviews are immutable. Corrections use a follow-up
cycle. Cycle labels/dates freeze when reviews are assigned, and cycles close
only after unfinished reviews are resolved.

An exit-readiness projection adds unfinished reviews about/assigned to a
leaving employee. A protected late guard also rejects finalization for older
clients, rolling back membership deactivation and employee archiving together.
The deployed exit migration is unchanged; its replacement readiness function
and new trigger live in this performance migration. Performance inbox messages,
360 feedback and career records remain separate work. Training/certifications
are available through the separate [learning module](LEARNING.md).

Verification evidence is recorded in VERIFICATION.md and the separate versioned
release report. Local PostgreSQL tests simulate Auth; actual Auth/RLS checks and
native screenshots are verified separately. Performance inbox messages, 360
feedback and career records remain future modules.
