# Career development plans

Candidate for 0.1.31; migration is not deployed until verification succeeds.

Employees > Development holds shared career plans and milestones. Target role,
department, skills and coaching notes describe aspirations. They never change
employment, salary, reporting, attendance, leave, goals or learning records.
Coaching notes, progress, reasons and history are visible to the employee,
current reporting Manager and HR. Use the separate private appraisal notes for
HR-only context. This module adds no exit blocker or notification contract.

HR/Admin and a current reporting Manager can manage another employee's plan.
Staff can record progress on their own open milestones; another eligible staff
member must define and review them. Only independent HR can waive a milestone.
Ordinary employees have future-portal own reads and no desktop/staff-RPC access.
Manager scope follows current Reports to. HR retains archived employee records
and may cancel a plan; archived employees cannot start, edit or resume plans.

A plan has a start and optional target date, target role/department, skills,
shared notes and at most 50 milestones. Each milestone has actions, an optional
due date within the plan horizon, a required flag and shared progress. An
independent review records done with a factual date/result, or HR records a
reasoned waiver. Optional context references one same-employee goal, training
or certificate. There is no automatic completion from a referenced record.
Private 201 document identifiers, keys and certificate reference numbers never
enter the shared reference summary.

Completing a plan requires at least one done milestone, no open required
milestones, a factual date on/before company today and on/after all completion
dates, and a summary. The server snapshots milestone facts and limited related
metadata. Later source changes never rewrite the closed facts. Closed plans
are read-only until a reasoned reopen; previous completion snapshots remain in
immutable history. Current completion fields clear atomically on reopening.

The server enforces current protected membership/reporting, company write
serialization, exact revisions, strict payloads, actor-bound creation retries
and atomic immutable audit. All schema changes are additive. Local PostgreSQL
checks simulate Auth and do not prove hosted Auth, native clicks or concurrent
transaction timing; those checks are recorded separately when performed.
