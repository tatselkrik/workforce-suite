# Leave request CSV and Excel imports

Desktop0.1.52implements the remaining request-import portion of roadmap6. Its
versioned verification/checklist records delivery status and testing boundaries.

HR/Admin prepare1–100rows with employee_code, leave_type_code, starts_on, ends_on,
day_part (full, am, pm), reason and optional private reference. Use supported
same-year ISO dates or date-only Excel cells. A half-day uses one date. One file
may cover at most3,660combined calendar dates. Keep employee/type codes as Text.
Invalid sources, overlaps and insufficient balances block the entire batch.

Local preview uses the normal native leave engine in source-row order. The
protected server preview rehearses the actual normal submit_leave engine in a
subtransaction, with the company lock retained. Each successful preceding row
reserves leave for later rows during that rehearsal. Its temporary requests and
events are always rolled back before a preview is returned. This endpoint is
not a read-replica/read-only SQL operation; it makes no durable bookings, import
ledger entries, external messages or employee-identity changes. This uses the
documented [PostgreSQL exception-block rollback behavior](https://www.postgresql.org/docs/17/plpgsql-control-structures.html#PLPGSQL-ERROR-TRAPPING)
and is checked against complete before/after table snapshots.

The source fingerprint binds the full payload, current protected actor/company,
policy/versions, entitlements, credits, openings, existing requests, eligible
personnel and rule identities. Generated rehearsal UUIDs/times are excluded.
Posting checks the same rehearsal and then submits all rows through the normal
engine atomically. The actual charge dates/units, policy revision, type name and
approval plan must match the protected preview. No payroll or issued slip changes.

Existing HR-assisted leave authority remains: normal self-review restrictions,
configured chains, delegation, future-dated approval rules and cancellation apply.
Attendance imports' separate creator exclusion is not added to leave. Review or
cancel under Leave; the import history reflects the normal request status.

An immutable private batch/item ledger records the actual HR importer and source
rows. Same-actor, complete-payload retries return the original receipt even after
review/cancellation. Other actors or changed payloads cannot reuse its identity.
Private import notes/references stay outside ordinary own/team leave records.
History has explicit500batch/5,000item limits without silent truncation.

Native drafts participate in unsaved-work protection, retain exact uncertain
submissions and clear when protected access changes. Error CSV/Excel exports
preserve literal source text, source row numbers and existing destination files,
and recheck current HR access after rendering. No Office subscription, file upload,
paid service, public portal or device connection is involved.
