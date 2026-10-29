# Android device alerts — implementation contract

This is the remaining authorized free Android increment; it is not delivered until the protected pipeline and actual background reception pass. Existing Android0.1.0 remains the released fallback. Firebase only transports generic alerts; Supabase remains Auth/storage. No Analytics, paid Functions, billing upgrade, public portal or email provider is used.

## Ownership and privacy

Explicit enablement requires a current signed-in account, a real current Auth session and active protected company/employee membership. Registration uses possession of the SDK-generated opaque FCM address; there is no client-supplied employee/company/user target. Device rows, addresses, notice hashes, delivery jobs and retry receipts are private with deny-by-default RLS and no client table grants. Public wrappers are invokers; private helpers check current authority. Staff using Android still receive only their own employee updates.

A device alert grant is separate from a browser/Auth session. The existing Rust service revokes its local Auth session when the app closes; tying the grant's lifetime to that session would defeat background delivery. Closing retains the opted-in generic alert grant; explicit sign-out/disable switches off local reception immediately and removes the grant when online. Opening always needs current sign-in. Account deactivation, archiving, deletion or banning stops server delivery; stale grants expire after30days without authenticated confirmation. No HRIS password/token/employee cache is stored on disk. The Firebase SDK manages its own installation identifier/address; local app preferences hold only the alert opt-in boolean.

The trusted sender sends data-only FCM with the fixed inbox hint. There are no employee names, identifiers, salaries, leave details, case content or source-record keys in the transport payload. The Android receiver uses fixed generic text/private visibility and ignores supplied message text. The notification intent is immutable. Unknown/revoked recipients cannot obtain private details by opening an alert.

## Source and delivery

Reuse hris_private.portal_notification_rows and lifecycle_notification_rows with the employee surface and current company timezone. No Auth impersonation, broad staff queue or duplicated pay/leave rules. Seed current keys when the first device is enabled to avoid a historical alert flood. Later new unread keys are hashed/deduplicated; marking unread/resetting read history is not a new HR event. Push never marks an inbox item read or changes canonical HR/pay facts.

Delivery is a private queued wake-up, not a guaranteed receipt/read. Coalesce pending wakes per device; lease claims and actor/payload-bound test receipts prevent stale workers and ordinary retries from duplicating sends. Retry transient failures with bounded backoff; remove invalid addresses only for token-specific provider errors. A network failure after acceptance can cause a generic retry; a stable notification/collapse identifier limits duplicate display. Membership/grant checks occur at dispatch; an already-in-flight generic message cannot be recalled.

Use small batches, explicit record/history budgets (5000 current notice keys and10000 retained dedup keys per person), a global daily attempt cap and a15minute scheduled tick. The tick skips external invocation when no active work exists. Keep the worker credential in Supabase Vault and sender credential only in encrypted private deployment state/Supabase secrets. The Google service account has only the project-scoped FCM API role. No sender credential is shipped to a client.

## Verification gates

Local PostgreSQL role/ownership/session/revocation, baseline/unread/dedup, retry/lease, rate/budget and preservation checks; handler auth/payload/error/privacy tests; Rust identity/CSRF/projection checks; hosted synthetic own/cross-account cases; real SDK registration and background FCM notification/open-inbox behavior in the owned emulator. Release lint/signature/16KB/source/privacy and exact installation checks remain distinct from debug tests. Physical phone and email confirmation remain unverified until actually exercised.
