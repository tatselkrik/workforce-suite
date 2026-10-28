# Separate Payroll Desktop — local implementation in progress

Kirk authorized this milestone after the28October usage reset. Keep one repository,
shared Rust payroll engine/adapters/screens, and the same dedicated company Supabase.
This extraction changes application boundaries; it does not copy or migrate payroll
records, change rates, reset roles, recalculate issued advice, or rewrite history.
The prior HRIS0.1.64 release remains delivered and immutable until a new verified
candidate is actually installed. Implementation is not a delivered milestone.

## Native products

- HRIS Desktop retains `io.github.tatselkrik.hrisdesktop` and its private update
  identity. The new candidate retains own published advice while financial
  administration moves to Payroll. An explicit Open Payroll Desktop button uses
  only the verified local installation; no path is taken from company data.
- Payroll Desktop is the separate `payroll-desktop/` workspace package, version
  0.1.0, identifier `io.github.tatselkrik.payrolldesktop`. It shares the root Rust
  library and dependency lock. Its company profile is packaged separately from
  authentication; tokens remain in process memory. Missing installed configuration
  must fail closed. An unconfigured source build can use the labeled fictional demo.
- Payroll accepts current Administrator/Payroll Officer responsibilities (including
  combined duties), with the existing independent financial approval/payment
  exclusions. Ordinary Employees remain portal-only; other staff responsibilities
  alone do not admit a person to financial administration.

## Preserved functional parity

All fifteen existing shared screens remain the implementation, with financial
sections in the separate app sidebar: monthly registers,
salaries/benefits, payroll holidays, payslips/PDF, consolidated reports, statutory
reference rules, cutoffs,13th-month pay, annual tax, individual payments, final pay,
regional wage checks, linked amendments, government review/remittances/BIR drafts,
and local data exchange. CSV/Excel/PDF handling, source/revision/retry controls,
immutable slips/receipts and deferred agency-acceptance labels remain intact.
No current financial rule is altered by extraction.

A product-bound loader fetches payroll, the employee directory needed for salary/
register selection, own contacts/employment labels, company branding and validated
personal notices. It does not load employee files, hiring/exit cases, appraisals,
training records, HR policy or account administration. Existing protected server
memberships/RLS/RPC contracts remain the data authority. The worker rejects
non-Payroll operations even if a caller bypasses the menu. HRIS's new candidate
loads only the current employee's issued payroll advice/status, including for
Administrators; it cannot prepare payroll or generate financial administration
exports through its worker. Unchanged generic library clients preserve the older
full snapshot contract for compatibility and shared integration tests.

Own financial notifications retain their existing keys and read receipts. Payroll
filters the shared protected feed and never resets unrelated HR read history.
Profile/security/session/display controls use the same own-account implementation.
Company/personnel/access settings remain in HRIS.

## Delivery gates

Verify both products, old-client compatibility and complete financial screen/action/
export parity before replacing the installed HRIS financial screens. Verify own-
only projection and current role denial/revocation, local fictional UI, exact
installer preservation/recovery and actual hash/BUILD/Windows Apps/normal launch.
Keep the existing HRIS signed channel separate. No new Payroll publication channel
or signing identity is authorized by the older HRIS-only publication guard;
complete independent local delivery without configuring a new external channel.

No paid service, purchase, public portal exposure, government software execution,
real filing, transfer, external employee communication or hardware work is included.
After this milestone is delivered, continue the newly authorized four HR modules
in root AGENTS.md, one verified increment at a time; use the same reset scheduler.


HRIS's optional My payslips control governs the personal HRIS section; it does not
revoke Payroll responsibility or disable the separate Payroll app. Account profile,
password/security/session and display preference changes apply to the same person
across the two apps. Each app signs in independently and keeps its tokens in memory.
The local Payroll installer is required on each PC that uses the separate app; the
HRIS private update package does not silently create a new Payroll installation.
