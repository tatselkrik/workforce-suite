# Protected staff provisioning

Administrator account setup preserves one employee identity, exact retry receipts
and immutable access history across the database and Auth services.

## Atomic business commit and recoverable Auth identity

Auth is a separate service, so its creation is not one SQL transaction with HR
records. A current active Administrator first reserves an immutable, credential-free
request with a server-generated Auth UUID. Successful reservation rehearses normal
employee/contact constraints inside an always-rolled-back subtransaction. It changes
no employee, contact, membership or employment-history record.

The native-only account-admin operation create_staff_v2 checks the caller through
Auth and protected membership. The caller JWT reserves/commits; only the hosted
handler uses its server credential for GET/POST of the one reserved Auth UUID.
It creates an unconfirmed identity with an initial password held only in memory.
It never assigns permissions through metadata, resets an existing password, lists
all Auth users, or deletes an identity after uncertain delivery.

One protected SQL commit then saves employment, private contacts, membership,
explicit roles, immutable role/access events and the actor/full-payload receipt.
An audit failure rolls back all business writes. An uncommitted Auth identity has
no employee membership or HRIS access. Exact retries recover one reserved identity
and one frozen receipt. Replayed completion does not repeat Auth creation or send
confirmation mail. Administrators use the existing explicit Send confirmation action.

Current official Auth supports a supplied AdminUserAttributes.id. Source review:
https://supabase.com/docs/reference/javascript/auth-admin-createuser
https://github.com/supabase/auth-js/blob/master/src/lib/types.ts
https://github.com/supabase/auth/blob/master/internal/api/admin.go

## Source and recovery guards

For an existing employee without an account, the server returns one same-statement
employment/contact source and an opaque SHA256 fingerprint. Reservation/commit
compare that source under the existing company and employee/archive locks. An
existing membership, cross-company person, stale source, unsupported role, forged
actor, revoked Administrator or disabled rollout is rejected.

A stale attempt can be explicitly replaced by a new linked request after review.
The replacement keeps the same actor, company, email, employee identity and reserved
Auth UUID. Old payloads/history stay immutable; a closed predecessor has one successor.
Reasoned cancellation retains its history and any unconfirmed Auth identity. Recovery
of the current actor's pending requests never returns credentials. There is an
explicit30-open-attempt quota, with bounded complete views and no silent truncation.

Private tables have RLS and no API/service-role table grants. Only the checked RPC
entry points are granted to authenticated; internal helpers remain ungranted. These
functions require fresh protected Administrator membership, not a decoded role claim.
Workforce Employee retains its own-record scope.

## Native integration and existing sign-in identities

The gated native form now supports new employees and adding access to an existing
employee while preserving employment/contact edits. Four internal sections reuse
the existing employment/contact controls. Saved-source changes require explicit
review. Pending setup can be resumed or checked; cancellation has a frozen reason.
Credentials stay in the form/worker only and are cleared after submission. The
normal unsaved-work guard includes account setup. Existing account access uses the
same protected multi-role editor instead of the old four-role selector.

Reservation may adopt an unassigned Auth identity by the reviewed email. It never
relinks an established membership, changes its password/confirmation/metadata, or
recreates a deleted pre-existing identity. This also recovers an unconfirmed identity
after a cancelled attempt without adding a separate employee-linking setup step.
The operator sees that an existing sign-in identity keeps its original password.

The handler requires a protected compatibility preflight
before an old create operation can contact privileged Auth. Ready installations
require the new credential-free-intent contract. Failed or uncertain old saves
retain the Auth identity for reviewed recovery; no compensating Auth deletion is
performed. Original business ownership/employee identity guards remain. These
guards preserve prior account identities and immutable history during compatible upgrades.
