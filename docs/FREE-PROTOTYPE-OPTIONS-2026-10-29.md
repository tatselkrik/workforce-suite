# Free prototype options and official reference routes —29 October 2026

Kirk asked for public government-format research, whether another accounting system is needed, Android employee-app/push feasibility, and emulator/email-test costs. Biometrics must remain visible and disabled. This note records research and local capability checks, not an Android implementation or an external provider connection. The completed acceptance pass remains documented separately in ACCEPTANCE-2026-10-29.md; its scheduler remains deleted.

## Government references

| Agency | Official starting point | Verified result and remaining limit |
| --- | --- | --- |
| BIR | [Downloadables](https://www.bir.gov.ph/Downloadables); [RMC15-2025](https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2015-2025.pdf) | Official page lists Alphalist; circular identifies7.4 and attached structures/naming annexes. Supplied local7.4/reference material is already prepared under work/bir074-local-validation and docs/BIR-OFFLINE-VALIDATION.md. Do not download it again merely for this research. Actual official-runtime/agency acceptance remains unverified. |
| SSS | [Forms/electronic applications](https://www.sss.gov.ph/download-forms-and-electronic-applications/); [2026 Citizen's Charter, first edition](https://www.sss.gov.ph/wp-content/uploads/2026/07/2026-SSS-Citizens-Charter-1st-Edition-External-Services.pdf) | Public page still lists R3 File Generator2023. Current charter PDF pages22–24 (zero-based21–23) describes registered Employer My.SSS access, PRN → Contributions → Prepare Collection List and current e-CCL editing. This does not establish a current bulk-upload schema or offline AMS-CCL validator. Do not treat the older2023 editor instructions as a verified current contract. No authenticated template was retrieved. |
| PhilHealth | [Employer payment/reporting procedures](https://www.philhealth.gov.ph/partners/employers/pay_procedures.php) | Current page directs employers to EPRS through authorized representatives. A current public upload-column specification was not located in this bounded review. Old RF1 paper forms/manuals do not establish current EPRS compatibility. |
| Pag-IBIG | [MCRF encoding instructions](https://www.pagibigfund.gov.ph/document/pdf/dlforms/providentrelated/Instructions%20in%20Encoding%20of%20MCRF.pdf); [eSRS guide](https://www.pagibigfund.gov.ph/document/pdf/payments/ESRS.pdf) | Both ordinary public requests returned the site's browser/reCAPTCHA challenge again. No challenge was bypassed, current workbook recovered or accepted file claimed. |

No government software was executed, employer account accessed, employee data uploaded or filing/payment performed. These findings locate official routes; they do not complete current-format/validator acceptance.

## Accounting and Android

The prototype already has separate Payroll Desktop using the same company backend. Another accounting/payroll product is optional. Actual external integration can remain deferred until a future company supplies a selected system; the delivered local exchange foundations remain available.

An Android employee client can reuse current own-record Supabase contracts and replace browser-based employee access after feature parity is verified. This is a new client implementation; the browser portal has not been converted or removed. A direct backend client would not require the PC portal server to remain running. Preserve native staff versus own-employee permissions, separate application identity and company deployment rules.

[Firebase pricing](https://firebase.google.com/pricing) lists Cloud Messaging as no-cost. [FCM's server requirements](https://firebase.google.com/docs/cloud-messaging/server-environment) still require a trusted sender and protected credentials/device routing. [Android setup](https://firebase.google.com/docs/cloud-messaging/android/get-started) requires runtime notification permission on Android13+ and supports Google-API emulator images. Installing an Android app alone does not configure background push. Paid Firebase services/Blaze are not required merely for FCM; no provider project, credential or sender was configured here.

## Email can be free within limits

[Supabase Auth SMTP documentation](https://supabase.com/docs/guides/auth/auth-smtp) currently limits its built-in sender to project-team addresses and two messages/hour, best effort. That can support limited authorized own-inbox prototype checks; it is not general employee notification delivery. [Supabase's June2026 free-tier change](https://supabase.com/changelog/46599-changes-to-email-template-customisation-on-free-tier) confirms custom SMTP remains usable on Free; default-template customization is restricted for new free projects without custom SMTP.

[Resend Free](https://resend.com/pricing) currently offers3,000transactional emails/month and100/day, with SMTP support. [Its domain rules](https://resend.com/docs/dashboard/domains/introduction) require a domain the sender owns and verifies for normal sending. The email plan can be free while a new domain would cost money; no domain purchase is authorized. An approved existing SMTP account is another possible route. Do not imply a free quota is unlimited, enable billing or configure external sending without a resolved sender/target. Actual inbox receipt/link completion is separate from emulator testing or local mail capture.

## Emulator capability actually checked

The existing Android SDK, emulator executable, adb and Android36/Google Play x86_64 system image are installed. `emulator -list-avds` returns Pixel_7. `emulator -accel-check` returns0 and reports usable WHPX. `adb devices -l` showed no connected device; its daemon started as part of that inventory command. C: had about131.79GiB free during the check. No emulator was booted and no Android app/browser/push/email UI was tested in this turn. These are capability checks, not physical-phone evidence. No new SDK image or large build cache was downloaded.
