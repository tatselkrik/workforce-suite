# Free-only roadmap boundaries

Kirk requires HRIS to remain completely free for now (25 October 2026).
Do not enable billing, upgrade plans, authorize overages, buy subscriptions or
purchase hardware/services. Existing free-tier limits remain hard boundaries.

| Scope | Current position |
| --- | --- |
| Native HRIS and local portal | Continue with existing local/free dependencies. Preserve running portal sessions/drafts. |
| In-app payroll/appraisal/training reminders | Continue; no mail/push provider or subscription is required. |
| Government files | Continue supported public-specification/offline work. Current agency acceptance is unverified; no real filing or payment is authorized. |
| Portal current browser/phone visuals | Pending: both supported UI runtimes again fail kernel-assets initialization. Earlier API/code checks are not current visual proof. |
| Actual email confirmation/delivery | Pending an approved no-charge sender and controlled destinations; existing hosted synthetic requests were rejected. Do not weaken confirmation rules. |
| Paid delivery/hosting/accounting/device services | On hold wherever a selected feature requires a paid plan/license. Independent free exchange foundations may continue. |
| Subscription-gated security add-ons | Keep paid-only add-ons disabled/on hold; existing access controls, RLS, validation and session protections continue independently. |
| Physical devices | Needs an identified already-available device/protocol and physical verification. New hardware purchases are on hold; biometric control remains disabled. |

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
