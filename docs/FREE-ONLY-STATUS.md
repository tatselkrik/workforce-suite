# Free-only roadmap boundaries

Kirk requires HRIS to remain completely free for now (25 October 2026).
Do not enable billing, upgrade plans, authorize overages, buy subscriptions or
purchase hardware/services. Existing free-tier limits remain hard boundaries.

| Scope | Current position |
| --- | --- |
| Native HRIS and local portal | Continue with existing local/free dependencies. Preserve running portal sessions/drafts. |
| In-app payroll/appraisal/training reminders | Continue; no mail/push provider or subscription is required. |
| Government files | Continue supported public-specification/offline work. Current agency acceptance is unverified; no real filing or payment is authorized. |
| Portal browser visuals | Current priority. Use existing no-charge tools, including mobile-width checks where possible. Supported UI runtimes previously failed initialization; API/code checks are not visual proof. Physical-phone testing is deferred and does not block browser completion. |
| Actual account email confirmation | Deferred until an approved no-charge setup is available; do not spend or weaken existing confirmation rules. Unverified confirmation does not block browser-only completion. |
| Company notification delivery | Portal inbox only for now; this display scope is already delivered. External company email and background browser/OS push are deferred, including otherwise-free provider setup. Existing native alerts continue. |
| Paid delivery/hosting/accounting/device services | On hold wherever a selected feature requires a paid plan/license. Independent free exchange foundations may continue. |
| Subscription-gated security add-ons | Keep paid-only add-ons disabled/on hold; existing access controls, RLS, validation and session protections continue independently. |
| Physical devices | Kirk has no biometric hardware. Keep the section visible and Connect device disabled. Actual device/driver/protocol work and purchases are deferred until hardware is supplied and scope resumed. |

Supabase documents that its default SMTP only sends to organization-team
addresses and requires a configured SMTP sender for general user delivery.
This is a delivery limitation, not proof that a paid subscription is necessary;
an approved suitable free option may be configured later. See
[official SMTP guidance](https://supabase.com/docs/guides/auth/auth-smtp).

SSS describes employer e-CL/PRN through My.SSS; PhilHealth requires EPRS for
remittance reporting. Current account-specific acceptance cannot be inferred
from an internal file or a legacy generator. Public documentation remains the
source for the current investigation; no registered-company login, upload or
browser-challenge bypass is authorized. See
[SSS](https://www.sss.gov.ph/pay-contribution),
[PhilHealth](https://www.philhealth.gov.ph/partners/employers/pay_procedures.php)
and [the file review](GOVERNMENT-FILE-REVIEW.md).

Continue independent authorized free work while a limited scope is pending;
do not mark partial milestones complete. The current order/status remains in
[REMAINING-ROADMAP.md](REMAINING-ROADMAP.md).
