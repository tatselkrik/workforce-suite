# Prototype verification — 2026-10-25

## Native government reconciliation — source0.1.42

-423default Rust tests pass. Three gated checks are separate: both government
 PostgreSQL-vector checks pass in an explicit18test run; the real installer and
 recovery check requires the exact candidate artifacts and is recorded with the
 packaged release evidence. An earlier blanket include-ignored invocation failed
 that installer check because its required environment was unset, not a tested
 application defect. No passing installer result is inferred from that run.
-3,785local PostgreSQL assertions and chronological replay of all58migrations
 pass. The additive reconciliation migration preserves86prior public record sets
 and243functions. Existing source-v1 stays unchanged. Explicit missing JSON
 attestations/source bases and fractional revisions are rejected.
-Actual hosted Auth/REST verifies HR preparation, retained issued employee amount,
 independent Administrator approval, all-editor denial, concurrent duplicate
 remittance prevention, idempotent voids/linked replacement, protected table/API
 denial and immediate revoked-membership denial. Six protected responses with114
 source rows and privacy-limited CSV validate through the native engine in memory.
 All24prior payroll record sets and earlier Auth users match. Synthetic payments
 end voided and synthetic accounts remain disabled/archived/session-free.
-Eleven fictional native views were rendered and inspected: overview, sources,
 allocation, approval, remittance, void, history, compact, dark125percent, HR and
 Manager denial. These prove native layout; actual signed-in native button clicks,
 agency-validator acceptance and current portal browser/phone visuals remain
 unverified. Portal packages0.1.4/0.1.5 are already delivered independently.
-New government Rust modules/examples pass their formatting check. The full
 repository formatting gate still reports differences in32existing files,
 confirmed against the preceding commit. That unrelated formatting debt is
 preserved. No remote CI result is claimed. Post-deployment advisors add no
 warning/error or unindexed FK finding; three older warnings remain.
-Versioned package/signature/installer/actual installed-state results are separate
 private release evidence. This increment implements reconciliation worksheets;
 legal identities and exact agency upload files remain queued. No return was
 filed and no money transferred. See [review checklist](docs/GOVERNMENT-0.1.42-CHECKLIST.md).

## Separate local employee portal — version0.1.2

-25 Rust portal tests, one CSV safety test, JavaScript syntax and3,275 local
 PostgreSQL assertions passed. New routes support own correction/overtime
 proposals, parent/break/candidate revisions, cancellation and redacted history.
 Company-local times reject ambiguous/missing DST instants; no reviewer routes
 are exposed. Existing desktop writers remain authoritative and unchanged.
-The read-only migration `20261024133750` brings hosted history to55versions and
 preserved all85 prior public record sets. Invoker/grant checks passed. Advisor
 comparison found no new warnings/errors or unindexed FKs; three older warnings
 and five older FK INFO remain. An initial advisor harness incorrectly expected
 zero total warnings; comparison with the retained baseline resolved that check.
-Seven actual hosted Auth/API proof groups passed: scoped context; validation,
 CSRF/revision and no early punch writes; independent Manager correction/history;
 saved-basis overtime with partial approval; missing-record proposal/cancellation;
 staff own-only access/no review route; revoked membership denying an existing
 cookie. Normal random credentials remained in memory. Synthetic accounts were
 disabled/archived/session-free, retaining their history. Earlier harness attempts
 were corrected for non-JSON422 responses and retained fixture-name uniqueness.
-Fictional desktop browser checks covered aligned forms, own history, dirty-draft
 navigation blocking and discard restoring date/overtime controls. A screenshot
 is retained privately. Phone-width verification is pending after the browser
 runtime failed to restart. Historical phone checks apply to prior increments.
-Normal portal build/package/startup and exact source/hash evidence are retained
 with its private artifacts. No public hosting, physical-phone, browser-print,
 remoteCI or Authenticode result is claimed. Desktop0.1.41 stays installed.
-The overall first milestone in `docs/REMAINING-ROADMAP.md` remains partial;
 notifications, pictures and account-security controls continue next.

## Linked payroll amendments and cross-year final pay — version0.1.41

-407 Rust tests and3,168 local PostgreSQL assertions passed. Fourteen checked-in
 amendment centavo vectors and five normalized report-v2 contracts cover native/
 server agreement without ignored-work prerequisites. Existing166 payroll rule
 vectors remain. Coverage includes independent creator/preparer/submitter
 exclusions, source/revision/current-role guards, rollback/retry, immutable
 originals, retained outside benefits, shared caps and bounded statutory refunds
 after annual/final settlements. Review-only discrepancies change no income,
 payment, debt or recovery facts.
-Three target-guarded migrations were applied, with54 hosted versions through
 `20261024112502`. All82 earlier public record sets were preserved apart from
 additive private default review columns. New table RLS, invoker wrappers,
 protected writer grants and denied private helpers were verified. Advisors
 returned no new WARN/ERROR or unindexed foreign keys; five prior FK INFO remain.
-Seventeen actual hosted Auth/REST/native proof groups passed using synthetic
 accounts with random RAM-only credentials. They exercised a paid original final
 statement, independently approved/paid linked earnings and additional13th pay,
 review-only discrepancy, actual2025 separation/2026 final settlement, exact
 retries, native/server centavos, fresh CSV export, own Manager/Employee scope,
 Employee desktop denial, Administrator self-approval denial, revoked membership
 and old consolidated-report update guards. All316 preexisting financial row
 hashes at the final probe's baseline remained unchanged. Test histories remain;
 the exact synthetic accounts were disabled/archived and their sessions removed.
 An earlier probe stopped on an incorrect field name in the test harness; the
 corrected complete probe passed without changing that server contract.
-Thirteen fictional native views were inspected, including editor/approval/
 payment, own Manager statements, review-only discrepancy, cross-year final pay,
 consolidated reporting and125% dark theme. Misaligned new statement fields
 found in the first render were fixed and recaptured. Both one-page PDF exports
 were rendered and visually inspected; CSV exports retain aggregate-only facts.
-Receipt-year review is stored in two private top-level case columns, preserving
 the legacy nested worksheet shape. Native report-v2 counts supplemental cash
 once; older consolidated reports fail with an update instruction when they
 would omit linked amendments. Source-only older reports remain available.
-The reviewed final-pay extension supports2025/2026 separation and2026 receipt
 from6January. It does not introduce2027 rules, invent current-year service gaps
 or move earlier-year withholding automatically. Current official source dates
 and financial limits are in `docs/PAYROLL-AMENDMENTS.md`.
-Exact normal build/startup, source/archive privacy, signed private package,
 updater download and actual installation evidence are retained separately with
 this version's private artifacts. No remote CI or Authenticode result is
 claimed. Signed-in native buttons were not automated; real Auth/API/native
 backend proof and fictional layout captures are distinct evidence.

Kirk requested stopping after finishing and installing this version. The
usage-reset scheduler is paused; no subsequent portal/roadmap work starts.

## Consolidated payroll reporting — version0.1.40

-398 normal Rust tests and2,964 local PostgreSQL assertions pass. Seven new
 accounting/scope/date/CSV tests include17 actual SQL projections; two transport
 tests cover one scoped report RPC and revocation before CSV creation. The5000/
5001boundary is tested without silently truncating financial totals.
-One additive read-only migration through20261024072104 is deployed:51versions.
 All82 earlier public record sets matched. STABLE single-statement readers,
 invoker public wrapper, protected current HR/Admin access and denied raw/private/
 anonymous access passed. No newWARN/ERROR or unindexedFK;5 prior INFO remain.
-Twelve actual hosted nativeRust/Auth/REST proof groups pass. All five financial
 sources and12date/source filters match native row projections and summaries;
 employee/empty filters, actual fresh CSV save and private-field exclusion pass.
 Manager/Employee access, anonymous access, invalid filters and a disabled HR's
 stale token are denied. All21 existing financial tables matched before/after.
 Synthetic accounts remain disabled/archived/session-free; no financial writes.
-Seven fictional native views and their exact CSV were inspected: approval and
 actual payment/settlement filters, source figures, detail/cash/offset rows, empty
 results, Manager absence and dark125 layout. Final key/value rows top-align
 wrapped labels. Normal configured startup, source/privacy, exact installer,
 signing and private publication are recorded in the versioned release evidence.
-The report separates source offsets from combined final cash and keeps annual
 tax advice/settled adjustments separate from payroll cash. Outstanding/reserved
 balances are current at generation for the selected records, not a reconstructed
 historical close. No salary, tax, payment, filing or remittance is created.
-Wage reviews from39 are included. Catalog verified24October2026, bounded through
31December2026; unsupported dates/coverage remain unresolved. No automatic arrears.
-The two authorized milestones are complete on private publication. Portal remains
 paused. No remoteCI, Authenticode, signed-in native-button or other-PC claim.
 The open installed app is preserved; the latest signed package is delivered.

## Historical regional wage compliance — version0.1.39

-389 normal Rust tests and2,823 local PostgreSQL assertions pass.39 new manual
 wage boundary vectors agree exactly between Rust and SQL;166 total payroll
 centavo vectors are retained. Independent creator/preparer exclusion, effective
 dates, workplace freshness, immutable history, RLS, older monthly/cutoff guards,
 changed-fact resubmission and audit rollback are covered.
-Two additive migrations through20261024061626 are deployed:50 hosted versions.
 All79 earlier public record sets matched before/after. Protected SELECT-only
 grants, invoker wrapper/view, private engine and immutable guards were verified.
 No newWARN/ERROR or missing-FK-index findings;5 unrelated prior INFO remain.
-Thirteen actual hosted nativeRust/Auth/REST proof groups pass. They include
 independent classification approval, exact create retry, Rust/server result
 parity, compact source privacy, legacy monthly advisory snapshots, changed-fact
 approval rollback, Manager private-read exclusion, Employee desktop rejection,
 stale revisions and unchanged earlier issued/received financial records.
 Live company enforcement stayed unchanged/advisory. Strict monthly AND cutoff
 gates were verified in local PostgreSQL rather than toggling the user's setting.
 Synthetic accounts retain history disabled/archived with zero Auth sessions.
-Eleven fictional native captures inspected, including date-field alignment,
 classification save footer, approval confirmation, catalog/history, Admin
 enforcement, Manager absence and dark125percent layout. Normal release startup
 and normal close passed in an isolated configured folder. The normal executable
 excludes QA controls and carries no company connection or credentials.
-The catalog has69 dated bands/40categories across17 wage boards, verified
24October2026. Unsupported dates stay unresolved. No salary/tax status changes,
 automatic arrears, actual payments or government filings occur. Portal paused.
 Regional wage and expanded reporting are the only resumed milestones.

Current exact packaging/signature/installer/private-feed evidence is recorded
separately; no remoteCI, Authenticode, signed-in native-button or other-PC claim.
The user's installed38 remains untouched while39 is prepared.

## Historical final-pay reviews and settlement — version0.1.38

-378 normal Rust tests and2,483 local PostgreSQL assertions pass, including28
 new Rust/PostgreSQL final-pay centavo vectors and99 earlier payroll vectors.
 Cases cover calendar service and minima, eligibility/tax treatment, shared-cap
 floors, original withholding, prospective tax, source freshness, exact retries,
 independent approval, archived recipients, unpaid withdrawal and audit rollback.
-Two additive migrations are deployed through20261024001000:48 hosted versions.
 All76 earlier public record sets matched across deployment. RLS/SELECT-only
 grants, invoker wrapper, private helpers/source view, immutable history and
 source-receipt binding/reservation guards passed. No newWARN/ERROR or unindexed
 foreign keys; five unrelated earlier FK-index INFO findings remain.
-Fifteen actual hosted nativeRust/Auth/REST proof groups pass. Independently
 approved final salary and earned13th sources, Rust/server centavo agreement,
 combined settlement, exact retry, reserved-source and HR denials, unpaid
 withdrawal, sibling-outstanding/remaining-batch behavior and annual income/tax
 credit once were tested. Manager own statements/PDF/CSV exclude private evidence;
 ordinary Employee desktop admission and anonymous/direct API access are denied.
 The REST follow-up compared all19 unrelated HR sets and existing financial facts
 before/after; original source slips were also compared in the native workflow.
 Synthetic actors were retained disabled/archived with no Auth sessions. An
 initial harness403 expectation was corrected to the actor-bound409 replay
 contract; only remaining REST checks were repeated, without new payroll writes.
-Ten unique fictional native views and a one-page statementPDF were inspected,
 including worksheet top/footer, approval, unconfirmed settlement, withdrawn and
 settled statements, own-only Manager view and dark125percent layout. The normal
 release build excludes QA controls. Its isolated configured startup/normal close
 and source/privacy evidence are recorded in versioned packaging records.
-Exact installer/update preservation, failed-setup recovery, existing-key signing,
 private feed/download/access proofs and installed state are recorded separately
 after packaging. The current user's open app is preserved. No signed-in native
 button automation, publicGit/CI, Authenticode, other-PC, actual transfer,
 government filing or official2316 generation is claimed.
-First pack: same-year2026 service end/cash dates from6January2026. Original source
 nets are discharged inside combined cash and statutory offsets; they are not
 another full-net cash payment. Cross-year settlements and settled amendments
 remain queued. See [final-pay rules](docs/PAYROLL-FINAL-PAY.md) and the
 [review checklist](docs/FINAL-PAY-REVIEW-CHECKLIST.md).
-Kirk requested a stop after this milestone. Automatic continuation and portal
 work remain paused; no next roadmap item has started.

## Individual payroll payments — version0.1.37

-361normal Rust tests and2,258local PostgreSQL assertions pass. Receipt cases cover
 per-person/full issued amounts, exact retries, stale concurrent revisions,
 remaining-batch compatibility, annual cash-date attribution, protected roles,
 immutable sources and atomic rollback after an audit failure.
-The additive receipt migration is deployed through `20261023202500`:
46hosted versions. All75existing public record sets matched across deployment.
 Prior paid-run dates/nets were backfilled without rewriting those rows or slips.
 RLS/SELECT-only grants, public invoker wrapper, private source view, binding and
 immutable triggers and the three old batch-client triggers passed checks.
 No newWARN/ERROR or unindexed-FK findings; five unrelated FK-index INFO findings
 already existed and remain outside this change.
-Twelve actual hosted nativeRust/Auth/REST proof groups pass: independently
 approved source payroll, per-person recording, sibling-outstanding behavior,
 identical retries and stale rejection, later-batch preservation, annual receipt
 dates, private HR evidence, HR write denial, Employee desktop denial, direct
 REST403/409/anonymous denials, preservation and cleanup.19unrelated record sets,
 all earlier published monthly/cutoff/13th/tax slips and all earlier receipts
 matched. All six synthetic actors were restored, disabled, archived and session-free.
-Eight unique fictional native views were inspected, including pending/partial
 batches, saved history, unconfirmed/confirmed forms, footer and dark125percent
 layout. Monthly/cutoff list/detail rendering now also filters own company/person
 even if stale HR data remains in a view; corrected Manager preview shows one own
 slip and no Payments tab. Invalid intermediate preview is excluded from delivery.
-Normal release startup/source privacy, exact installer/signing/private publication
 and installation state are recorded in the versioned release evidence after
 packaging. No signed-in native-button, publicGit/CI, Authenticode, other-PC,
 transfer, filing or remittance claim. Final-pay entitlements, prospective last-pay
 annualization and final settlement remain a separate in-progress milestone.
 See [individual payments](docs/PAYROLL-PAYMENTS.md) and its review checklist.
 Portal work remains paused.

## Annual compensation tax — version0.1.36

-358normalRust tests passed after annual-tax/backend/transport/native draft wiring.
 After the saved-evidence contrast and PDF-header fixes, the3annual workflow/
 centavo/export tests and existingPDFencoding test passed again.2,173local
 PostgreSQL assertions pass, including29new SQL/Rust vectors (99combined payroll,
 cutoff,13th and annual vectors), tax brackets/refunds, successive prior employers,
 shared-cap reclassification, paid-source receipt years, cutoff/13th amounts,
 incomplete openings, review/revision/conflicting-approval guards, private scopes,
 immutable slips and transactional audit failure.
-Three additive migrations are deployed through `20261023191233`:45hosted
 versions.73existingpublic record sets were preserved across the engine/workflow
 deployment and75across the covering-index deployment. Two newRLS/SELECT-only
 tables, invoker wrapper and protected/private helpers passed grants checks.
 Advisors show0newannualWARN/ERROR or annual FK-index findings; unrelated INFO findings remain.
-Eleven actual hostedAuth/nativeRust/REST proof groups pass: guarded creation
 retries, centavo refunds/prior-employer/shared-cap calculation, Administrator
 self-approval denial/independent approval, concurrent-source rejection/new reviews,
 earlier settled credits, immutable aggregate receipts, Managerown-only records,
 freshCSV/PDF exports, settlement recording, Employee desktop denial and
 REST409/403/anonymous denials.19unrelated record sets and all existing monthly,
 cutoff and13th slips matched across verification. Six synthetic roles/dates were
 restored, memberships disabled, employees archived and Auth sessions removed.
 Earlier fixture-harness cleanup required repair; isolated recipient accounts
 were then created through normal account provisioning and the full checks passed.
-16unique fictional native views and one corrected one-pagePDF were inspected,
 including readable saved evidence, missing-balance blockers, refund/deficiency,
 ownManageradvice and dark125percent layout. Normal configured release startup
 was responsive and closed normally. Current normalexeSHA256:
 `fdb9ef1ca2524e9d732acb1b6203369edeb17d9900ad1a0cac4a3c6838357c1b`.
 The normal binary excludes QA controls and embedded company connection values.
-Exact installer/recovery, signing, private publication and actual installation
 are recorded in the versioned release evidence after packaging. No publicGit/CI,
 Authenticode, actual signed-in-native-button, other-PC, transfer, government
 filing, remittance or official2316 claim is made. This module issues closing
 advice from recorded payments, not an automatic unpaid-last-pay adjustment.
 See [annual tax](docs/PAYROLL-ANNUAL-TAX.md) and its review checklist. Portalpaused.

## 13th-month pay — version0.1.35

-353Rust tests passed after native workflow/export/draft preservation wiring.
 2,060local PostgreSQL assertions passed;18new SQL/Rust vectors plus52earlier
 payroll/cutoff vectors agree in exact centavos. Tests cover earned coverage,
 missing/overlapping openings, prior payments, shared cap, independent approval,
 source revisions/concurrent installments, redaction, immutable slips, disabled
 scopes and transactional audit failure. Exact-installer testing follows packaging.
-Two additive migrations are deployed through `20261023155000`:42hosted
 versions. All70existing public record sets matched immediately across deployment.
 Three new RLS tables have read-only client grants; public invoker routes call
 protected writers and eight private helpers. No new annual-benefit WARN/ERROR
 remains in advisors; informational unused-index suggestions are retained.
-Eleven actual hostedAuth/nativeRust/REST proof groups passed: creation retries,
 SQL/Rust amounts, actual Administrator self-approval denial and independent
 approval, concurrent-source rejection and three new reviews, immutable/private
 slips, Manager own-only scopes, fresh CSV/PDF exports, payment recording,
 Employee desktop denial and REST409/403/anonymous rejection.19unrelated record
 sets and all existing monthly/cutoff slips matched. Synthetic roles were restored,
 access disabled, employee records archived and Auth sessions removed.
-Fourteen fictional native screens and a rendered one-page benefit slip were
 inspected, including dark125%, missing basis, saved approver evidence and Manager
 own slip. The final QA-only fixture preparation followed the normal suite.
 These are not actual signed-in native-button automation. Normal configured
 startup, source privacy, exact installer/recovery, Ed25519 signing, private
 feed/download and installation have separate revision/hash evidence after packaging.
-Entitlement and shared annual benefit exemption are automatic. Withholding and
 contribution adjustments are explicitly reviewed inputs, including zero.
 Automatic annualization/prior-employer reconciliation follows separately.
 No transfer, government filing, remittance, publicCI, Authenticode, other-PC or
 physical-phone verification is claimed. Employee portal remains paused.

## Semi-monthly payroll cutoffs — version 0.1.34

- 337 Rust tests and 1,983 local PostgreSQL assertions passed after the final
  overtime safeguard. 18 cutoff vectors plus 34 legacy statutory/premium vectors
  compare exact SQL/Rust centavos. Tests cover monthly residual cents/caps,
  semi-monthly tax boundaries, contributions on cutoff2, proration, source and
  revision guards, independent approval, overlap prevention and audit rollback.
- Three additive migrations are deployed through version `20261023150310`;
  40 hosted migration versions match local history. Across the final migration,
  all 70 existing public record sets retained their immediate counts/digests.
  Three new tables have RLS/read-only client grants, public invoker wrappers and
  protected writers/helpers. No new cutoff WARN/ERROR remains in advisors;
  informational unused-index suggestions are retained for the new scoped tables.
- Eleven actual hosted Auth/Rust/REST proof groups passed again after the final
  safeguard: creation retry and calculation parity, stale-source rejection and
  refresh/review, first-before-second reconciliation, actual Administrator
  self-approval denial and independent approval, own Manager slips/private draft
  denial, redaction/immutability, refreshed-scope CSV/PDF exports, payment recording,
  Employee desktop denial and REST409/403/anonymous checks. Seventeen unrelated
  record sets and all existing monthly slips matched. Synthetic roles were
  restored, access disabled, employees archived and Auth sessions removed.
- Ten fictional native screens were inspected across aligned normal/light,
  dark125%, worksheet/decision/detail and own-slip previews. A rendered one-page
  synthetic PDF fits with aligned amounts and exact period/pay date. Final labels
  and private HR review display have a subsequent release capture. These are not
  actual signed-in native form/button automation.
- Normal configured startup, clean source/privacy, exact installer/recovery,
  signing, private feed/download and installation checks are recorded separately
  with exact revision/artifact hashes after packaging. Earlier test counts alone
  do not establish those gates. No public CI, Authenticode, real bank transfer,
  government filing, remittance, other-PC or physical-phone claim is made.
- Rule references were checked on23October2026. The fixed halves/second-cutoff
  contribution timing are stated prototype policies. Actual company pay schedules
  still need review. Monthly-register annual exports exclude cutoff records;
  cutoff reconciliation identifies included halves and incomplete months. The
  paused employee portal has not added cutoff slips. Actual13th-month payout,
  year-end/prior-employer adjustment and final pay are subsequent milestones.

## Philippine payroll and benefits — version 0.1.33

- 327 Rust tests passed, with the prepared-installer integration test intentionally
  ignored until packaging. 1859 local PostgreSQL assertions passed after all three
  new migrations; 34 statutory and premium vectors match exact SQL/Rust centavos.
  Local PostgreSQL tests simulate Auth/Storage metadata. Final label refinements,
  normal build/startup and the exact packaged installer have separate release
  evidence; they are not implied by the earlier test counts.
- Migrations `20261023082335`, `20261023090105` and `20261023092104` are applied; 36 hosted versions
  match local history. Immediate before/after digests of 23 existing HR table
  sets matched. Six public tables have RLS and read-only client grants, with a
  protected writer, guarded revisions/source facts, actor-bound creation receipts
  and immutable slips/events. The second migration folds equivalent payslip
  predicates into one policy. No new payroll security WARN, multiple-policy WARN
  or missing-FK finding remains; prior advisories remain. Private birth rows have
  intentional default-deny RLS/no client grants.
- Final review corrected the true-monthly unworked double-regular-holiday case:
  add the second eligible day's pay above the day already included in salary.
  Hosted calculation and new-run v2 default were checked; all six payroll table
  counts/digests matched across this function/default migration. Issued pre-release
  synthetic v1 slips were retained. Actual Auth workflow tests preceded this
  isolated correction; the full local suite and hosted function checks cover v2.
- Actual Auth/Rust/REST tests passed creation retries, server/Rust amount parity,
  stale-source HTTP409, refresh/review, independent Administrator approval,
  self-approval denial, Manager-only own issued payslips, and private-note/evidence
  redaction. Anonymous/direct writes, forged account metadata and unauthorized
  mutations were denied. Disabled memberships lost own-payslip access using
  unexpired JWTs. CSV/PDF exports and payment recording preserved issued slips.
  Eleven unrelated core record sets matched before/after. Test sessions were
  revoked; synthetic identities are retained for audit and disabled after release
  verification. No real password changed or money moved.
- Fifteen fictional native payroll previews were inspected at compact size,
  light/dark themes and 100/125% scale. A one-page fictional PDF was rendered and
  checked for alignment, exact totals and supported Philippine Latin characters.
  Unsupported PDF glyphs fail explicitly with UTF-8 CSV available. No signed-in
  native-button automation or government filing was performed.
- Official-source rule dates and payroll limits are in [PAYROLL.md](docs/PAYROLL.md).
  The first module is a monthly reconciliation register, not a payment-frequency
  authorization. Missing attendance never causes an automatic absence deduction.
  Annual adjustment, prior employers, regional minimum wages, fringe-benefit tax,
  actual 13th-month payouts, final pay and bank/government connections remain
  separate work. Windows packaging, signature, update publication and installation
  state are recorded in the versioned private release report. No Authenticode,
  remote CI, other-PC or actual concurrent-client timing claim is made.

## Installer metadata correction - version 0.1.32

The 0.1.31 real installation check verified all7payload hashes, then caught
Windows Apps still reporting0.1.30. Installer resource and registration versions
were hard-coded. Published31artifacts are retained immutable;32supersedes them.
The installer now derives every version from its prepared BUILD.json, following
[NSIS searchparse](https://nsis.sourceforge.io/Reference/%21searchparse). The exact
installer/recovery test now checks the compiled setup's ProductVersion before
installation. Final32native/startup/artifact/installer/registration checks are
recorded in its versioned report. Career code/schema and its actualAuth31proofs
are unchanged; no extra roadmap module was started. The scheduler stays PAUSED.

## Career development plans - version 0.1.31

- 309 full Rust tests passed, followed by101final library/career/HTTP tests after
  native label refinements.1663local PostgreSQL assertions include107new career
  checks for independent/self/reporting scopes, bounded milestones, dates,
  metadata-only sameemployee references, immutable closure/reopen, no employment
  side effects, revisions/birth retries, archived retention, audit rollback and
  direct/anonymous/private-birth denials. LocalPG simulates Auth/Storage metadata.
- Migration `20261023035021` deployed;33local/hosted migration versions match.
  Immediate before/after23sets of existing HR/Auth digests matched. Three public
  tables use RLS/read-only grants, invoker public RPC/protected writer and immutable
  events. Private birth data has no client grants/defaultdeny RLS. Its no-policy
  INFO is intentional. No new security WARN or FK finding; existing advisories
  remain unchanged. No Edge Function, Storage pipeline or existing data rewrite.
- ActualAuth/Rust/REST verified actor-bound creation retries, own/currentManager
  progress, independent review, reporting-change revocation, HR-only waiver,
  completion snapshots, reasoned reopening and sameemployee context without
  changing source learning or exposing private certificate references. Raw self/
  Employee/directwrite/audit/anonymous/forgedmetadata denials and disabled valid
  JWTs passed. Unrelated employment/attendance/leave/files/hiring/performance/
  learning rows matched. Four synthetic actors remain disabled/archived with open
  plans cancelled; all verification sessions revoked, no real password changed.
-16fictional native views inspected: list/detail, light/dark125%, forms, progress,
  review/waiver/plan completion, saved closure, aligned history, Manager scope and
  existing learning tabs. A missing navigation glyph was replaced with plain text.
  Normal startup, source/binary privacy, exact installer, private publication and
  installation are recorded separately in the versioned release report. No
  signed-in native-button automation, actual parallel transaction timing,
  Authenticode, remoteCI, otherPC or running-parent update timing claim.
- See [the requested review checklist](docs/REVIEW-CHECKLIST-2026-10-23.md) for
  changes since Oct22at15:54local (versions19–32) and remaining roadmap. Development
  stops after this milestone; the automatic-resume scheduler is PAUSED.


## Training and certifications - version 0.1.30

- 295 Rust tests passed, followed by96targeted library/learning/HTTP tests after
  QA-only and confirmation-label refinements. One real installer test is run
  separately after the installer is prepared.
- 1556 local PostgreSQL assertions passed, including103learning checks for
  current-report/own-progress scope, HR-only completion, archive/validity retention,
  canonical private file bindings, strict revisions, actor-bound retries, immutable
  audit, disabled-session simulation and atomic audit rollback. This harness
  simulates Auth/Storage metadata and does not perform S3 uploads.
- Migration `20261023003106` deployed; all32local/hosted versions match exactly.
  Immediate before/after19sets of existing HR-table/Auth digests matched. Four
  public tables use RLS, invoker RPC/protected private writers and immutable
  history; direct client writes and anonymous reads/execution are revoked.
  Private retry rows have no client grants and intentional no-policy/defaultdeny
  RLS (INFO). No new security warning or foreign-key finding. No Edge Function
  change. Existing password-protection warning remains unchanged.
- Actual Admin/HR/Manager Auth through Rust/REST verified training creation/retry,
  scoped progress, reporting-change revocation, HR completion, certification
  revoke/archive/restore, HR-only private EMPTY binding records/events, own future
  portal reads, Employee desktop/RPC denial, direct/history/anonymous/forged Auth
  metadata denials and disabled valid-JWT reads. Unrelated employment, attendance,
  leave, files, onboarding, exits and performance records matched. Synthetic
  fixtures remain disabled/archived; all exact verification sessions revoked.
  No real password changed. No new S3 upload is claimed.
- 16 distinct fictional native views inspected, including normal/dark125%, forms,
  completion, private evidence, history, Manager scope and existing appraisal
  tabs. Certificate archive/revoke confirmations were recaptured with subject
  labels. Normal configured sign-in startup/close and source/binary privacy passed;
  QA controls and company connection values are excluded from the normal binary.
  Packaging/exact installer, private publication and installation are recorded
  separately in the versioned report. No signed-in native-button, actual parallel
  transaction timing, Authenticode, remote CI, other-PC or running-parent update
  timing claim.

## Goals and appraisals - version 0.1.29

- 283 Rust tests and 1453 local PostgreSQL assertions passed. Coverage includes
  bounded goal progress, safe creation retries, strict saved receipts, preserved
  drafts/refresh recovery, current reporting/assignment scopes, clean reassignment,
  separate HR notes, no self-appraisal decisions and immutable publication facts.
  Local tests simulate Auth and prove older exit finalization rolls back membership
  deactivation and archiving when unfinished appraisals block it.
- Migration `20261022231643` deployed. All31 local/hosted versions match exactly;
  immediate before/after digests match across14existing tables including Auth.
  Five new public tables use RLS, public invoker RPC/protected private writer,
  direct-write revocations and immutable history/publication triggers. Private
  original-retry rows have no client grants and default-deny RLS (intentional
  no-policy INFO). No new security warning or foreign-key finding; pre-existing
  leaked-password-protection warning remains. No Edge Function change.
- Actual Admin/HR/Manager Auth through Rust and REST verified goal creation/retry,
  progress, reporting/reviewer reassignment, draft privacy, assigned assessment,
  submission, HR publication and independent private notes. Employee own-goal/
  published-review reads worked while desktop sign-in was rejected. Direct writes,
  history deletion, anonymous/forged-metadata access and privileged mutations were
  denied; disabled unexpired JWTs lost all performance reads. Unrelated employee,
  attendance, leave, document, onboarding and exit records matched. Synthetic
  fixtures remain disabled/archived; exact verification sessions were revoked.
  An initial REST expectation incorrectly excluded own goal-history events; the
  corrected harness completed. No real password changed.
- 19 final fictional native previews inspected: goals/progress, forms, review
  cycles, assignments, assessments, submission/publication, private notes, history,
  Manager scopes and dark125%. Normal configured sign-in startup/close passed.
  Source/binary privacy, real account checks, exact installer/recovery, private
  signature/feed/download and installation are recorded separately at packaging.
- No signed-in native-button automation, real parallel-transaction timing,
  Authenticode, remote CI, other-PC or running-parent update timing claim.

## Exit and clearance - version 0.1.28

- 270 Rust tests passed, including exit lifecycle/permissions, original-ID retries,
  exact saved receipts, preserved drafts, and HTTP refresh recovery. Fixed older
  HTTP fixtures that assumed a fixed snapshot request count. One real-installer
  test is intentionally run separately after the installer is prepared.
- 1370 local PostgreSQL assertions passed. Coverage includes HR-private cases,
  assigned Manager task access, later named leave approvals and active named
  policies, required tasks, date/owner/self safeguards and late-audit rollback of
  both membership deactivation and employee archiving. Local Auth is simulated;
  real parallel-transaction timing is not claimed.
- Migration `20261022193159` deployed; all30 local/hosted versions match. Immediate
  before/after digests match for employees, memberships, attendance, leave,
  onboarding cases/tasks and Auth. New tables enforce RLS and revoke direct
  client writes. Public RPCs are invokers with protected private writers.
  Security and foreign-key findings match the previous baseline; new unused
  index informational findings were reviewed. No Edge Function change.
- Actual Admin/HR/Manager sessions through Rust and REST verified creation/retry,
  task assignment/completion/reopening, reassignment access removal, HR waivers,
  clearance and Admin finalization. Former assignees and disabled unexpired JWTs
  lost reads/writes. Employee/Manager private-case and readiness requests, forged
  metadata, direct writes and history deletion were denied. HR finalization was
  denied by the hosted server, not just the UI.
- Unrelated employee/attendance/leave/document/onboarding records matched after
  live testing. Synthetic records are retained disabled/archived and all exact
  verification sessions were revoked. An initial transient account-provisioning
  502 interrupted fixture setup; created fixtures were safely disabled/archived,
  and a fresh setup completed. No real account password changed.
- 12 final fictional native views inspected: private HR details, assigned Manager
  task and decision, forms, ready/finalized cases, retained history and dark 125%.
  Compact navigation now adjusts button padding and interaction height so Settings
  remains visible above Profile. Normal startup, binary/source privacy, exact
  installer recovery, private signature/publication and installation are recorded
  separately in the versioned report. No signed-in native button automation,
  Authenticode, remote CI, other-PC or running-parent update timing claim.

## Recruiting and onboarding - version 0.1.27

- 260 Rust tests and 1310 local PostgreSQL assertions passed, covering private
  attachments, safe retries, scheduling/overlap, feedback scope, approved offer
  transitions, exact amounts, atomic employee creation and required checklists.
  The PostgreSQL harness simulates Auth. Final UI wrapping changes are included
  in the final regression pass; sandbox-only socket/branding failures were
  resolved by running the existing local tests with appropriate access.
- Additive migrations `20261022180842` and `20261022183649` deployed; all29 local
  and hosted migration versions match. Existing HR data preserved. Public RPCs
  remain invokers with guarded private writers and active-membership RLS.
  Security/FK advisory findings unchanged; new unused-index and two overlapping
  read-policy informational findings were reviewed.
- Actual HR/Admin/Manager Auth through Rust and REST verified résumé PDF/DOCX
  upload/retry/original download/archive, interview reassignment/private feedback,
  Administrator offer approval, recorded acceptance and atomic hire/checklist
  retries. Managers can complete only assigned tasks and cannot load private
  applicants, files or offer/pay terms. Direct writes, stale revisions, forged
  metadata, anonymous and disabled valid-JWT access were denied on fresh reads.
- All live feature assertions completed. Initial fixture cleanup hit the
  existing active-direct-report archive guard; actual Admin cleanup resumed in
  dependency order. Fixtures retained disabled/archived; exact test sessions
  revoked. Unrelated HR and Administrator profile preserved; no real password
  changed. Previously authorized identical Storage URLs could return a cached
  response immediately after disabling the caller. Fresh query URLs denied it;
  desktop downloads now bypass that cache and uploads specify max-age0.
- 28 fictional native views inspected, including HR/Manager scopes, light/dark,
  125% display, forms, wrapped tabs and expanded offer/task/case history. Final
  long-value wrapping and status-label fixes were recaptured and reviewed.
- Normal binary/startup/privacy, exact installer/recovery, private Ed25519
  publication and local installation are recorded separately in the private
  versioned report. No signed-in native button automation, Authenticode,
  remote CI, other-PC or running-parent timing claim. No Edge Function change.

## Applicant pipeline - version 0.1.26

- 249 Rust tests and 1149 local PostgreSQL assertions passed. Coverage includes
  bounded fields/dates, permitted stages, required reasons, stable creation
  retries, stale revisions, archive/restore, atomic history failure and protected
  identity. HTTP tests verify company-scoped loads, failed-refresh retries and
  mismatched response rejection. Local PostgreSQL simulates Auth.
- Additive migration `20261022172521` deployed. All 27 hosted/local migration
  versions match. Previous HR/account/document/preference data preserved.
  Applicant tables have HR/Admin-only RLS; direct writes are revoked; the public
  invoker RPC uses a private guarded implementation. Existing security/FK
  findings unchanged; two new unused-index informational notices.
- Actual HR Auth verified create/edit/retry/stages/archive/restore/history and
  fresh-sign-in retention through the Rust adapter. Actual Admin/HR reads and
  Manager/portal exclusions were also checked through unfiltered REST. Direct
  writes, stale updates, crosscompany/anonymous access, forged user metadata
  and disabled HR with a still-valid JWT were denied. Unrelated HR data and
  Administrator profile preserved. Synthetic test accounts disabled, their
  employee records and application archived, all test sessions revoked.
- Twelve fictional native previews inspected across light/dark, 125% display,
  HR/Manager visibility, creation/detail/history and save/archive/restore
  controls. A measured headless regression verifies history alignment with
  multiline values across three widths. Save-response tests preserve drafts
  until a matching record arrives. No real password was changed.
- Normal binary/startup/privacy, exact installer/recovery and private signing,
  publication and installation are recorded in the private versioned report.
  No signed-in native button automation, Authenticode, remote CI, other-PC or
  running-parent updater timing claim. No Edge Function change.

## In-app notifications and reminders - version 0.1.25

- 236 Rust tests and1015 local PostgreSQL assertions passed. Coverage includes
  role/company/current-step scope, no self-review, own decision/local-date
  windows, document states/stages,24-hour overnight guard, bounded read keys,
  incremental merges and composite-key/RPC HTTP loading. Inbox rendering uses
  a cache invalidated by data/access changes and refreshed periodically.
- Additive migration `20261022145739` deployed; all26 hosted/local versions match.
  Prior HR counts and report layouts preserved. Own active desktop staff RLS,
  revoked direct mutation and invoker RPC/private guarded implementation
  verified. Security/FK findings unchanged; one informational unused-index
  notice for the new FK index. No Edge Function change.
- Actual Admin/Manager Auth verified HR correction/expiry alerts, own check-out
  and decision updates, Manager document exclusion, mark read/unread/reset,
  fresh-sign-in state and independent-session merge. Unfiltered private RLS,
  direct/crosscompany denial, JWT metadata exclusion and disabled valid-JWT
  access checked. Synthetic account disabled, employee/document archived,
  sessions revoked and unrelated HR/Admin profile/read markers preserved.
  Initial synthetic shift overlap correctly rejected; corrected dates used
  for final verification. No real password changed.
- Eleven final fictional native previews inspected across light/dark125%, roles,
  filters, read/unread, empty state and long wrapping. Normal startup/privacy,
  exact installer/recovery, signing/private publication and installation belong
  in the private version report. No signed-in native button automation, remote
  CI, Authenticode or other-PC claim is made.

## Workforce analysis and custom report columns — version 0.1.24

- 226 Rust tests and964 local PostgreSQL assertions passed. Current headcount,
  clipped daily/weekly/monthly recorded trends, saved calculation/overtime
  consistency, company/Manager scope, safe CSV and legacy filter defaults were
  verified. Actual headless pointer inputs verified column selection/order.
- Additive migration `20261022140158` stores personal layouts outside JWT
  metadata, with own active staff RLS and server revisions. All25 hosted/local
  migration versions match. Existing HR counts and legacy metadata preserved.
  Existing security/FK advisories unchanged; new FK index has an informational
  unused-index notice. No Edge Function change.
- Actual synthetic Manager Auth verified all headcount groupings, recorded
  trends, selected CSV order and fresh-sign-in views. Actual Admin/Manager
  tokens verified own-row privacy, no layout in JWT metadata, stale HTTP409,
  direct/crosscompany write denial and disabled access with an existing JWT.
  Real Admin preferences and unrelated HR records preserved; test account
  disabled, people archived and verification sessions revoked. Password unchanged.
- Twenty final fictional native previews inspected across light/dark,125%,
  all groupings/buckets, scoped roles and selected/empty/saved columns.
  Wrapped table cells and nested chooser sizing were corrected before final
  captures. No signed-in native button/file-chooser automation is claimed.
- Normal startup/privacy, exact installer/recovery, signing/private publication
  and installation are recorded separately in the private version report.
  No public push, remote CI, Authenticode or other-PC claim is made.

## Personal saved report views â€” version 0.1.23

- 209 Rust tests passed, including fixed/rolling date resolution, bounded and
  untrusted preferences, company/Manager scope checks, own-key Auth transport,
  matching-response editor completion and cached result/paging reset.
- Actual synthetic Manager Auth verified create/edit/delete, account refresh,
  fresh sign-in persistence, relative dates and scoped report generation.
  A forged employee preference granted no outside access. The real Admin's
  profile/preferences and unrelated HR data were preserved. The synthetic
  account was disabled, employee archived and verification sessions revoked.
- Eleven final fictional native previews inspected: saved/empty states,
  light/dark field alignment including125%, duplicate validation, HR/Manager,
  applied filters and scrolled report tables. The initial wrapped label was
  corrected before final previews. Native signed-in button automation was not
  performed; account actions were exercised through the actual Rust client.
- No schema or Edge Function changes. Existing required schema remains
  `20261022095123`; database suites were not rerun for preference-only changes.
  Normal startup/privacy, exact installers, signatures/private publication and
  local installation state are recorded separately in the private version
  report. No public push, remote CI, Authenticode or other-PC claim is made.

## Atomic employee CSV imports â€” version 0.1.22

- 200 Rust tests and917 local PostgreSQL assertions passed, including14 import
  workflows, uncertain-response retry HTTP coverage and55 database migration/
  import checks. Strict CSV/UTF8/BOM/quotes/multiline/source lines, defaults,
  duplicate/existing codes, contact rules, stale active references, batch limits,
  normalized server byte budget, safe error CSV and once-only history verified.
- Additive migration `20261022095123` deployed; all24 versions match. Existing
  counts preserved; security advisories unchanged/no new FK findings. Optional
  Docker catalog cache unavailable; actual hosted schema verified separately.
- Hosted rollback HR subjects verified protected identity/company/active status,
  actor/hash/links, atomic late-row rollback, retry/changed-retry behavior and
  Manager/disabled/anonymous/direct audit denials. An expected-code fixture was
  normalized to uppercase to match existing employee rules; fixtures rolled back.
- Actual Admin Auth imported two synthetic people/contact/emergency records,
  default leave and no accounts, one history each, protected audit and safe retry.
  Invalid/existing previews and changed retries denied; fresh sign-in retained
  batch/source rows. People archived, audit retained, unrelated data preserved
  and sessions revoked. HR/Manager database scopes used simulated hosted subjects.
- Eleven final fictional native previews inspected across ready/error/footer,
  expanded details,125%, history light/dark, HR and Manager-hidden controls.
  Native signed-in button/file-chooser automation was not performed. The final
  pure normalized-size guard was tested after these unchanged UI previews.
- Exact normal startup/privacy, source/archive, final installers/recovery,
  signing/private publication and installation state belong in the private
  version report. No Authenticode, remote CI, payroll or other-PC claim is made.

## Attendance/leave reports and CSV exports â€” version 0.1.21

- 185 Rust tests passed, including nine report workflows and three HTTP checks.
  They verify saved-rule time totals, partial/historical overtime, filed half-day
  charges clipped to the period, cancelled history, unknown totals, missing-day
  semantics, Manager scopes, bounds, duplicates, safe file creation, UTF-8/CSV
  quoting and formula-prefix protection. Export reloads records and rechecks
  access before file IO; revoked/changed scope creates no local file.
- Actual Admin and synthetic Manager Auth sessions verified attendance/summary/
  empty leave reports, 525 recorded gross minutes, UTF-8 export, own/direct-report
  scope, denied outside-employee export and fresh-sign-in persistence. Test user
  disabled, three synthetic people archived, unrelated data/rules preserved and
  verification sessions revoked. Export samples contain synthetic people only
  and remain private verification files.
- Fourteen final fictional native previews inspected across attendance, leave,
  summaries, employee/department filters, empty state, Manager/HR, paging,
  light/dark, 125% and right-side report columns. Nested-table height and wide
  column sizing were corrected after earlier captures; final previews supersede
  them. Signed-in native button/file-chooser automation is not claimed.
- No database migration, RLS/grant or Edge Function change. The prior 862 local
  PostgreSQL assertions remain historical evidence for unchanged schema; that
  database suite was not rerun for this report-only module. Normal build/startup,
  privacy, exact installers/recovery, signing/private publication and any local
  installation are recorded in the private version report. No remote CI,
  Authenticode, payroll or other-PC verification is claimed.

## Overtime approvals â€” version 0.1.20

- 173 Rust tests and 862 local PostgreSQL assertions passed: own submission,
  candidate caps, partial review/revoke, current reporting scopes, no self-review,
  immutable bases/events, cancellation/rejection and stale attendance/breaks.
  Fractional seconds floor per interval before aggregation, matching Rust.
- Migration `20261022082855` deployed without changing pre-existing record counts.
  Hosted rollback-only checks verified current Manager partial approval and
  revocation, actual-fact preservation, stale break conflicts, protected actors,
  RLS/invoker functions and unrelated/disabled/anonymous/direct-write denials.
  Existing security and older FK advisor findings are unchanged; no new FK lint.
- Actual synthetic Manager/requester and HR/reviewer Auth sessions verified
  submission, partial approval, revocation, rejection/cancellation, active-request
  server uniqueness and a server stale-break conflict. A punch correction made
  an existing approval historical. Fresh sign-in retained six requests/fourteen
  events. Unrelated records and saved rules were preserved. Synthetic identities
  are disabled, people archived, template inactive and test sessions revoked.
- Eighteen fictional native previews inspected at compact size, including normal
  and enlarged text, light/dark, request/review/history, partial authorization,
  revocation, stale pending/approved, Manager/HR and empty views. Fields, buttons
  and history cards align and remain accessible by scrolling.
- These checks do not automate signed-in Windows button interactions, payroll,
  other PCs or running-parent updater timing. Normal build/startup, exact final
  installers, signing/publication and installation are recorded in the private
  version report once completed. No remote CI or Authenticode claim is made.

## Attendance correction requests â€” version 0.1.19

- 166 Rust tests and 802 local PostgreSQL assertions passed for the implementation: seven new request workflows, a guarded RPC/history HTTP contract and 66 database/preservation/index checks after the existing 736. Coverage includes own submission, no premature attendance write, one pending request/date, Manager/HR review, no self-approval, immutable proposals/decisions, reason validation, stale parent/request revisions, current reporting scope, break-bound revalidation, cancellation/rejection, linked audit, rule retention, DST conversion and disabled/unassigned/anonymous denials.
- Migrations `20261022050704_attendance_correction_requests.sql` and `20261022081216_attendance_request_context_index.sql` are deployed. The second covers the private context FK reported by the advisor; existing security findings and five older unindexed-FK findings remain. Original counts matched when excluding newly labeled module fixtures. Existing attendance/corrections were not rewritten. Optional Docker catalog-cache refresh remained unavailable; actual hosted schema and behavior were independently verified.
- Rollback-only hosted PostgreSQL checks passed ordinary Employee own submission, Manager exact-proposal approval, private-proof/direct-write protection, linked true-actor correction audit, stale conflict rollback, rejection/cancellation, disabled/unrelated/anonymous scopes and request/event RLS. These subjects were simulated; all synthetic Auth/data changes rolled back.
- A real authenticated Rust probe provisioned separate labeled synthetic Manager/requester and HR/reviewer accounts. Only those synthetic accounts were confirmed for testing. Submission left attendance untouched; HR approval applied exactly once with linked audit; rejection/cancellation preserved actual times; a stale-view approval reached the hosted RPC and returned a conflict. Fresh sign-in retained four requests/eight events and saved shift/calculation snapshots. Unrelated company/leave/document/employee/attendance records stayed unchanged. Test accounts are disabled, people archived and template inactive; retained history is intentional. Verification sessions were revoked. Passwords were unchanged for existing accounts. Manager approval of a direct report uses hosted simulated roles, while actual Manager Auth tested own submission and HR Auth tested review.
- Seventeen final fictional native previews were inspected across list/history, existing/missing details, submission/error/footer, approve/reject/cancel forms, saved decision history, stale state, HR/Manager permissions, empty states, light/dark and 125% scaling. A Manager preview identity collision was fixed, and an unsupported arrow glyph replaced with plain text. Signed-in native button automation is not claimed.
- Single-step attendance review is independent of leave approval chains. Current membership/reporting scope is enforced on review; nobody reviews themselves. Approval reuses existing correction/overlap/break protections and preserves recorded rules. Direct HR correction remains available. Ordinary Employees remain portal-only. See [attendance requests](docs/ATTENDANCE-REQUESTS.md).

Normal optimized startup/privacy, exact installer/recovery, signed private publication and installation are recorded separately with private artifacts. No public push/remote CI, Authenticode, user Update-now automation or running-parent update-wait verification is claimed.

## Attendance time calculations â€” version 0.1.18

- 158 Rust tests and 736 local PostgreSQL assertions passed for the functional implementation, including nine calculation workflow tests and 55 new migration/permission/preservation assertions after the existing 681 checks. Coverage includes whole-minute grace, early departure, actual outside-shift unpaid intervals, optional pre-shift time, minimum/rounding, overnight/DST handling, unavailable/review states, snapshots, reasoned application, optimistic revisions and roles. The authenticated probe caught sorting a company-keyed policy table by a nonexistent `id`; the client now sorts by `company_id` and the HTTP contract test guards both policy tables. Final versioned tests/build and packaging are recorded with the private artifacts.
- Migration `20261022041651_attendance_calculations.sql` is deployed and matches hosted history. Employee, attendance, correction, leave, document, shift, assignment and break counts stayed unchanged through deployment. A neutral company policy was seeded; existing attendance was not backfilled. Policy and both history tables have RLS/read-only client grants. Public RPCs are security invokers; private handlers protect company/role/reason/revision checks and retained history. Existing security-advisor notices and unindexed-FK findings are unchanged; new unused-index INFO findings are expected. The optional CLI Docker catalog cache could not refresh, while actual hosted schema and behavior were verified independently.
- Rollback-only hosted PostgreSQL checks passed Administrator policy edits, HR application, Manager own/current-report summaries, future Employee own-record scope and unrelated/disabled/anonymous denials. Server-owned rules, UTC boundaries, actor/before/after history, immutable direct-write protection, stale policy/parent conflicts and preserved attendance/shift/break facts passed. These subjects are simulated transaction-locally; all synthetic Auth/data changes rolled back. Nonadministrator Auth tokens were not used.
- A real Administrator-authenticated Rust adapter probe saved/revised company rules, recorded labeled synthetic scheduled attendance and unpaid breaks, verified 12-minute late arrival and a 90-minute overtime candidate, applied revised rules with a reason, corrected checkout while retaining its snapshot, rejected stale policy/parent writes and verified fresh-sign-in persistence. Original company rule values were restored in a new retained policy version. Unrelated company/leave/employee/attendance/document/break/shift data stayed unchanged. The synthetic employee is archived and template inactive; audit remains intentionally. Successful probe sessions were revoked; passwords were unchanged.
- Eighteen final fictional native previews were inspected across list/details, policy form and Save/Cancel, company/record history, explicit application, missing rules/shift, off-day, approved leave, open attendance, HR/Manager scope, empty states, light/dark and 125% scaling. Attendance navigation now wraps at enlarged/narrow sizes. History displays planned local times in the saved time zone. Signed-in native button automation is not claimed.
- Undertime here means early departure. Overtime is a review candidate, not approved/payable time. Actual completed unpaid breaks alone reduce net attendance; required durations never automatically deduct. Approved leave uses current protected approved records and saved charge dates, without assuming half-day clock hours. Old records retain their saved rules; HR can explicitly apply current rules where saved shifts exist. See [attendance calculations](docs/ATTENDANCE-CALCULATIONS.md). Employee correction requests, overtime approval, payroll and the separate portal remain future work; biometric integration stays disabled.

Normal optimized startup/privacy, exact installer/recovery, signed private publication and installation are recorded separately with the versioned private artifacts. No public push, remote CI, Authenticode, user Update-now automation or running-parent update-wait verification is claimed. File recovery does not remove the database schema.

## Paid/unpaid attendance breaks â€” version 0.1.17

- 149 Rust tests and 681 local PostgreSQL assertions passed for the functional implementation, including eight new break workflow tests and 61 break migration/workflow checks. Coverage includes rules, saved classification, actual unpaid totals, no automatic deduction, own controls, checkout protection, bounds, overlap, retained correction/void history, legacy snapshots and optimistic revisions. The final versioned build and packaging checks are recorded separately with the artifacts.
- Migration `20261022035331_attendance_breaks.sql` is deployed and its version matches hosted history. Existing employee, attendance, correction, leave, document, template and assignment counts matched across deployment; both break tables initially contained no rows. New tables have RLS/explicit read-only client grants. The public RPC is a security invoker; its private handler checks protected membership, own/HR scope, parent-first locking, revisions, server time and saved classification. An attendance trigger rejects checkout during an open break and corrections outside recorded break bounds. Existing security-advisor notices are unchanged.
- The rollback-only hosted script passed HR/Manager/Employee/unrelated/disabled/anonymous scopes, actor/classification/time ownership, stale revisions, overlap/bounds, history retention, denied direct writes and open-break checkout protection. All synthetic PostgreSQL subjects/data rolled back. These are transaction-local simulated roles; nonadministrator Auth tokens were not used.
- A real authenticated Rust adapter check recorded paid/unpaid breaks, corrected, voided and replaced a break, verified totals and retained history, rejected stale break/attendance revisions on the server and verified persistence after fresh sign-in. Company settings, leave policies and unrelated employee/attendance/document/shift/break data stayed unchanged. The labeled synthetic employee is archived and its template inactive; audit rows remain intentionally. Verification sessions were revoked; account passwords were unchanged. Routine own Start/End uses local workflow tests and hosted simulated Employee scope, not the administrator's real attendance.
- Seventeen final fictional native previews were inspected across rule editing, aligned date/time fields and save controls, details, own open break, HR/Manager scope, history/void, empty/legacy state, daily entry point, light/dark and 125% scaling. A nested rule scroll area was removed after it cramped editing, and the Breaks entry point moved under the employee name. Signed-in native button automation is not claimed.
- Totals exclude actual completed unpaid break time only. Required durations do not create automatic deductions; paid breaks stay in elapsed time. These totals are not approved payable time. Lateness, undertime, overtime and payroll remain future work; the portal remains separate and biometric integration disabled. See [attendance breaks](docs/ATTENDANCE-BREAKS.md). File recovery does not undo the database checkout guard. Existing clients omitting break rules preserve them when editing templates; old attendance does not gain reconstructed rules.

Normal optimized startup/privacy, exact installer/recovery, signed private publication and local installation are recorded with the versioned private artifacts. No public push, remote CI, Authenticode, user Update-now button automation or running-parent update-wait verification is claimed.

## Shift schedules and employee assignments â€” version 0.1.16

- 141 Rust tests pass, including time-field fit/alignment, dated schedule selection, template/attendance snapshots, overnight attribution, revisions, hire dates, roles and the existing HR modules. 620 local PostgreSQL assertions pass: 476 legacy checks before upgrading, 74 leave checks and 70 shift/preservation checks. Compiler concurrency was reduced after this PC reported paging-file exhaustion; the final suite completed with incremental compilation disabled and two jobs.
- Migration `20261022030213_shift_schedules.sql` is deployed; existing employee, attendance, correction, leave and document counts matched across deployment. Historical attendance plans remain unset; earlier schedules were not invented. All three new tables have RLS/read-only client grants. Public RPCs remain security invokers; private handlers recheck protected HR membership, company locks and optimistic employee/template/time-zone versions. Existing security-advisor notices are unchanged.
- Rollback-only hosted PostgreSQL tests passed HR/Manager/Employee/unrelated/disabled/anonymous scope, stale writes, retained assignments and protected history. These use transaction-local simulated subjects; all synthetic Auth/data changes roll back. Nonadministrator Auth tokens were not used.
- A real authenticated Rust adapter probe created and edited labeled synthetic shifts, assigned historical and future schedules, replaced a same-date assignment without deleting history, recorded an after-midnight check-in, corrected it after an assignment change, and verified fresh-sign-in persistence. Attendance retained its original plan. Company settings, leave policies and unrelated records stayed unchanged. Synthetic templates are inactive and the synthetic employee archived; retained audit is intentional. Test sessions were revoked; account passwords were not changed.
- Thirteen final fictional native previews were inspected: shift lists, normal/125% aligned time fields, assignments and form/footer, template/assignment history, daily attendance, Manager read-only, HR editing, empty states and light/dark. The enlarged time-field overflow was fixed and protected by a geometry regression. Signed-in native button automation is not claimed.
- Break tracking, payable time, lateness/undertime/overtime, rotating/split shifts, swaps and publishing remain subsequent work. The employee portal remains separate and biometric integration remains disabled. See [shift schedules](docs/SHIFT-SCHEDULES.md). Individual schedules do not recalculate saved leave charges.

Normal optimized build/startup/privacy, exact source archive, installer/recovery, private update publication and installation evidence are recorded in each deployment's separate BUILD/delivery report. This source report does not claim remote CI, Authenticode, user Update-now button automation or running-parent update-wait verification.


## Chart alignment, leave policies and approval chains â€” version 0.1.15

- Fixed branch/leaf card alignment with an exact gutter and non-advancing expander placement. The paint regression checks equal peer origins and consistent indentation at normal/125%. Leave includes company workweeks/dated holidays, configurable paid/unpaid types, morning/afternoon/full-day requests, annual/monthly entitlement, capped expiring carryover, pending reservations, cancellation reasons and retained history. Administrators configure 1â€“5 ordered per-type reviewer/manager/HR/Admin/specific-staff steps. Final approval alone changes reserved days to used leave; self-approval, stale decisions and overlapping halves are rejected.
- All 132 Rust tests and 550 local PostgreSQL assertions passed. The database runner verifies 476 legacy checks before upgrading the retired leave-write contract, then 74 migration/new-workflow checks. Tests cover calendars, charges, scope, ordering, half-day overlap, reservations, carryover/expiry, unknown hire dates, immutable history, revisions, compatibility filtering and scalar RPC responses. Formatting/whitespace checks passed. Unchanged provisioning/publisher Node handlers were not rerun.
- Three checked-in migrations are deployed and match hosted history; required schema is 20261021210727. Existing original data digests matched across the first migration. Follow-ups prevent inferred accrual/carryover before hire/tracking and preserve older-client sign-in/update compatibility. New clients send a read-contract header; it grants no authorization. Public RPCs remain invokers; private handlers check protected membership and hold the company lock. New tables have RLS/explicit grants; direct client leave writes are retired. The final rollback-only hosted role checks passed Manager/HR/Admin/Employee/disabled/anonymous scope, ordered steps, stale conflicts and limited reviewer receipts, with all test Auth/data changes rolled back. These use simulated PostgreSQL subjects, not nonadministrator Auth tokens.
- Actual authenticated Rust checks passed calendar quote versus saved charge, reservations, AM/PM overlap, multistage review, stale policy/decision conflicts, cancellation/replay rejection, saved charge/plan retention after calendar edits and fresh sign-in persistence. Original company policy defaults were restored. Labeled archived fixture employees, cancelled requests and inactive type codes remain for future tests; inactive types are hidden by default. Unrelated employment/contact/document/company/branding/attendance/account records were unchanged. Verification sessions were revoked; passwords were unchanged. Real HTTP checks proved legacy/new client reads, compatible update feed and anonymous denial.
- All eighteen final fictional native previews were inspected: aligned chart peers/long values, requests, light/dark and 125% scale, calendars, types, aligned approval controls, review/history/cancellation, empty state, readonly HR/Manager summaries and request form at both scroll positions. Submit/Cancel stay outside scrolling fields. Normal optimized startup/closure and source/binary privacy checks passed; QA controls are absent from the normal executable. Signed-in native button automation is not claimed.
- Balances use the current policy/employee allowance and projected credits through the requested start date; this is not a posted credit or effective-date policy ledger. Unknown hire dates do not create accrued/carry credits. Delegation/escalation, cross-year single requests, opening-balance imports, manual credit adjustments and the employee portal remain separate work. See docs/LEAVE-POLICIES.md. No new security advisor warning appeared; the existing leaked-password-protection warning and intentional private-release-table informational notice remain. Exact installer/recovery, private signed publication and installed payload proofs are recorded with the versioned artifacts. No public push, remote CI, Windows Authenticode or other-machine validation is claimed.


## Organization chart â€” version 0.1.14

- Employees has Directory and Organization chart views. Cards use saved Reports to, show employment directory fields and open the existing record with Back to chart. Search, department/location filters, expand/collapse, HR-only archived inclusion and explicit unset/outside-view roots preserve reporting context without inventing managers. Managers remain limited to self and active direct reports; HR/Admin retain company scope. The chart grants no extra record access and contains no private contact, document or account-security fields.
- All 123 Rust regression tests passed, including seven new hierarchy/scope/filter tests and three UI input/paint tests. Tests cover deterministic forests, retained authorized ancestors, inactive/unset location choices, archives, cross-company/indirect-report denial, signed-out/Employee denial, malformed-cycle fallback and a 1,500-level iterative tree. Real headless input verifies card-open versus team-toggle actions. Paint checks require vertically stacked wrapped values, reserved row height and viewport bounds in deep filtered trees at normal/125% scale. Initial native previews caught inherited horizontal card layout and accumulated frame-border overflow; both were corrected and the regressions pass.
- A live helper using the actual Rust adapter created a labeled synthetic reporting chain, verified filtered ancestor context, changed Reports to, checked the rebuilt hierarchy, archived the fixtures in dependency order and confirmed persistence after a new sign-in. Existing employees, contacts, employment history, documents/events, attendance/leave, accounts and branding remained unchanged. The clearly labeled archived fixtures remain for repeat testing; verification sessions were revoked and credentials/profile were unchanged. This is actual HR/Admin adapter evidence; native role previews and controlled scope tests are not a new hosted Manager identity test.
- Seventeen final fictional native captures were inspected: normal/125% chart, filtered context, collapsed teams, archives, long light/dark values, deep hierarchy, empty/no-match, Manager/HR scopes, record return and cycle fallback. The normal feature-free build passed isolated configured startup/closure and source/binary privacy checks. Signed-in native form/button automation is not claimed; input actions are covered by the headless UI tests above.
- No database, RPC, grant, Edge Function or dependency change is needed. Existing schema `20261021164228` remains required. The database/hosted-role/Node-handler mutation suites from 0.1.13 were not rerun for this client-derived chart. Exact installer/recovery, signed private publication and local installation are recorded with the versioned private artifacts. No public push, remote CI, Windows Authenticode or other-machine validation is claimed.

## Private employee documents / digital 201 files â€” version 0.1.13

- HR/Admin can upload private PDF, DOCX, PNG or JPEG files up to 10 MiB inside saved employee records; label contracts/IDs/certificates/forms; keep optional dates/notes; search/filter; edit metadata; download original; archive/restore; and inspect protected document history. Documents save separately from employee/contact changes and survive employee archival. Expiry badges are reference indicators; notifications, e-signing and approval workflows remain future work.
- All 113 Rust tests and 476 local PostgreSQL assertions passed. Coverage includes validation, immutable original bytes/paths, actor/audit, revisions, roles, retention, checked downloads, pending retry/discard, HTTP contracts and database policies. DOCX tests cover Word package/content types/relationships/XML, CRC, expansion/depth limits, invalid/encrypted/corrupt archives, unsafe paths and known VBA/macro types. Local Auth/Storage are simulated. Unchanged provisioning/publisher handlers retain earlier 32-test evidence and were not rerun here. Sandbox restrictions initially blocked temporary branding-file replacement and local HTTP; the same suite passed outside the sandbox.
- Both additive document migrations are deployed and match hosted history; schema `20261021164228` adds DOCX. Existing document/event/managed-Storage metadata digests matched before/after that extension. Storage bytes are mutated through the Storage API only. No Edge Function redeployment or account-password/profile change was needed. Hosted rollback-only checks passed HR/Admin scope and Manager/Employee/disabled/unassigned/anonymous denials, protected actors, stale conflicts, immutable audit and pending/discard behavior. Two concurrent real metadata edits produced one save, one conflict and one event.
- Real Rust/private hosted Storage checks passed PDF/PNG/DOCX upload and byte-identical download, idempotent retry, metadata edits that retain originals, stale saves, archive/restore, archived employee/file retention and fresh sign-in persistence. HTTP checks verified actual size/MIME/owner finalization, absent-byte/invalid-MIME rejection, anonymous denial, overwrite/delete resistance and tombstone-before-cleanup recovery. Unrelated employee/contact/history/attendance/leave/company/logo data stayed unchanged. Labeled archived synthetic fixtures remain; verification sessions were revoked. The DOCX probe first assumed ascending API events; sorting by server sequence corrected that assertion and the live check passed.
- Seventeen final fictional native previews were inspected at normal/125% scale: list/empty/archived/pending, file/actions/history, light/dark upload/edit/error dialogs, DOCX selection/row and Manager restrictions. Scrollable bodies retain fixed save/cancel/close controls. These are native layout captures, not signed-in native button automation. The normal executable excludes QA controls. Final normal startup/privacy, exact installer/recovery, private signed update and installation evidence is recorded with the versioned private artifacts.
- Advisors found no new security warning; the pre-existing [leaked-password-protection setting](https://supabase.com/docs/guides/auth/password-security#password-strength-and-leaked-password-protection) and intentional private release-table informational notice remain. Pinned ZIP 9.0.5 and existing XML 0.41.0 inspect DOCX in memory without extraction/Office execution. This is not antivirus or full document compatibility validation. UTF-8 OOXML and stored/deflate entries are supported; DOC/DOCM/encrypted files are not. Storage backup/recovery remains separate from database/desktop recovery. No public Git push, remote CI, Windows Authenticode or other-machine verification is claimed.

## Employment history row alignment â€” version 0.1.12

- Field labels now use the same measured top-down cell layout as Saved value and Before/After cells. This fixes the vertical mismatch while retaining the height needed for wrapped values. The change is limited to history presentation; database, authorization, saved events, dependencies and deployment profiles are unchanged.
- The new actual paint-geometry regression reproduced the original Employee code misalignment before the fix. It now checks aligned headers and field/value text in starting snapshots and change entries at normal/125% scale, including long values. Both layout regressions and the complete 97-test Rust suite passed.
- Nine fictional native previews were inspected: snapshot headers/first rows, scrolled and bottom rows, Before/After changes, normal/125% scale and long values in light/dark themes. Labels align with the first line of their values; wrapped rows retain their height and Save/Cancel remain visible. Actual signed-in native form interaction is not claimed.
- No SQL or Edge Function changes were required. The database, hosted access and live employee mutation suites from 0.1.11 were not rerun for this presentation-only patch. Final feature-free build, privacy/startup, exact installer/recovery, signed private update and installation evidence are recorded separately with the versioned artifacts. No public push, remote CI, Windows Authenticode or other-machine verification is claimed.

## Automatic employment history â€” version 0.1.11

- HR Generalists and Administrators open the saved employee record's History tab for automatic creation/update events, saved actor/role/time and before/after values. Existing employees receive a Starting snapshot without invented earlier history. Ten Employment fields are tracked. Contact-only, role-only and unchanged saves add no event; archiving retains the timeline. Profile, Manager and ordinary Employee scopes do not gain HR-only history. Recorded time is the database save time; scheduled/effective-date employment actions remain future work.
- The additive migration `20261021125457_employment_history.sql` adds explicit employment-only snapshots, saved manager/type/location labels, server sequence/time, indexes, HR/Admin RLS and an immutable-event guard. The narrow recording helper is a private fixed-search-path trigger; no client audit writes or new public RPC are exposed. Protected active membership/canonical employee names supply actors; trusted operator/bootstrap writes without an HR actor are labeled System. History and combined employee/contact/staff saves share one transaction. Existing clients and provisioning are captured without a function redeployment or signature change.
- All 96 Rust tests, 387 local PostgreSQL assertions and 32 provisioning/publisher Node tests passed. Coverage includes baseline semantics, changed-field rendering, immutable labels/actors, no-op/contact noise, role/company scope, archived retention, automatic account identities, transport reload and combined rollback. A real paint-geometry regression checks that long wrapped department/title values reserve height and cannot overlap following rows or extend outside the viewport. Local Auth/Storage remain simulated; exact final Windows installer/recovery evidence is separate from the default suite.
- The migration is deployed and its filename matches hosted history. Hosted rollback-only checks passed protected actor/before-after, contact-only noise, failed-save rollback, HR reads/writes, Manager/Employee/unassigned/disabled/anonymous denial and history immutability. The first fixture omitted required blank department/title values; it was corrected and the transactional suite passed. No test actor remained. Advisors found no new security warning; the previously documented leaked-password-protection Auth warning remains.
- Real Rust adapter and authenticated HTTP checks passed creation/update/archive events, saved actor and labels, fresh-sign-in persistence, contact-only saves, older-client automatic capture, unchanged unrelated records/history/branding and direct/anonymous denials. Two concurrent employee edits both committed with an intact intermediate before/after chain; this module does not add optimistic employee conflict prevention. The labeled synthetic employee remains archived and test choices inactive. Verification sessions were revoked and account credentials/profile were unchanged.
- Seventeen final fictional native previews were inspected at normal/125%, light/dark, HR/Manager scopes, starting/creation/archive/empty timelines and scrolled long-value/snapshot rows. Initial previews caught zero-height wrapped cells overlapping later rows; measured vertical cells and the regression test fixed that defect. Final native/privacy/startup, immutable private publication and exact installed-artifact results are recorded with the versioned private artifacts. Actual signed-in native field/button automation, public push, remote CI, Windows Authenticode and other-machine rollout are not claimed. Contracts, attachments/digital 201 files and organization charts remain subsequent modules.

## Employment types and work locations â€” version 0.1.10

- Administrators manage company-owned employment types and work locations in Settings. HR Generalists and Administrators assign them in the shared Employee/Account Manager Employment editor; Profile mirrors both read-only. Employees provides assignment columns, search and filters, including Not set and inactive choices. Existing employees are left unclassified. Starter choices are editable labels rather than legal classifications.
- Choice names are trimmed, case-insensitively unique within each list and limited to 1â€“80 characters. Disabling preserves current assignments and filter visibility while preventing new assignments. Renaming updates the current displayed label; no employment-history module is implied. Revision checks reject stale choice edits with PT409. Unsaved choice forms block update/restart. Both existing attendance entry shortcuts are preserved.
- All 86 Rust tests, 340 local PostgreSQL assertions and 32 provisioning/publisher Node tests passed. New checks cover option validation, revisions, retained inactive values, filters, roles, RPC transport/reload, nullable assignments and atomic employee/contact/staff saves. The existing provisioning handler forwards the complete employee object; its regression fixture now includes both new IDs. Local Auth/Storage remain simulated. The Windows installer/recovery integration is separate from the default Rust run and recorded with the exact final artifacts.
- Additive migration `20261021121404_employment_types_and_locations.sql` was applied to the dedicated prototype, and its checked-in version matches hosted history. Protected dictionaries, indexed composite employee references and server-side membership/role checks enforce company boundaries. Choice configuration and employee validation share the company advisory lock. Older clients omitting the new keys retain assignments; explicit null clears them. No Edge Function redeployment was needed.
- Hosted rollback-only checks passed HR assignment/configuration separation, Manager read-only employment, unassigned denial, stale conflicts, legacy preservation and inactive retention/new-assignment denial. Live Rust and authenticated HTTP checks passed save/reload, current labels, archived fixture retention and unchanged unrelated records/branding. Two concurrent edits produced exactly one save and one PT409 conflict. Direct dictionary writes and anonymous configuration were denied. Clearly labeled inactive choices and an archived synthetic employee remain for repeat testing. Verification sessions were revoked; account passwords/profile were unchanged.
- Twelve final fictional native previews were inspected at normal/125% scale: Settings lists and Manager restrictions, add/inactive choice dialogs, new/inactive employee assignments, inactive filters and read-only Profile. Long selector labels truncate and long Profile values wrap; save controls remain visible. Signed-in native field/button automation is not claimed. Final normal-build/privacy/startup, signed private publication and exact installer/installed-startup results are recorded separately with the versioned artifacts. No public push, remote CI, Windows Authenticode or other-machine rollout is claimed. The previously documented leaked-password-protection Auth warning remains; no new security warning was found.

## Reasoned past attendance and correction history â€” version 0.1.9

- HR Generalists and Administrators can record missing past/current attendance and correct check-in/check-out with a required reason. The selected employee/work date stay fixed. Optional check-out dates support overnight shifts; open entries block another check-in. Routine Check in / Check out now retain database-server timestamps. Archived historical corrections preserve inactive status and history. No employee request/approval-chain module or employee deletion was added.
- Immutable HR-only history stores the verified actor/name/role, server save time, reason and full before/after records. Custom timestamps and revisions remain unavailable as direct client writes. The guarded private helper uses a short-lived protected transaction context; the write and audit are atomic. Revision checks and row locking reject stale edits, and the exclusion constraint arbitrates overlapping shifts. Both new migrations are additive; legacy server-timed clients remain compatible.
- All 78 Rust tests and 283 local PostgreSQL assertions passed. Tests cover required reasons, roles, archived/hire/future restrictions, fractional seconds, overnight/DST cases, duplicate/open/overlapping shifts, stale drafts after check-out, audit state and RPC transport/reload. The adapter's attendance-overlap message no longer describes a leave conflict. Storage/Auth in local tests are simulated. Unchanged provisioning/publisher Node handler code retains earlier 32-test evidence and was not rerun here.
- Both checked-in migrations were applied to the dedicated prototype. A hosted rollback-only suite verified past timestamps/audit, required reasons, stale writes, manager read/write denial and history immutability. Hosted security checks found no new security warning; leaked-password protection remains the previously documented Auth setting, and the release table intentionally uses no direct read policy. Existing/new indexes may remain unused on the small prototype.
- Live Rust adapter checks created clearly labeled synthetic attendance records, recorded and corrected past times, rejected stale writes, retained/corrected archived history, and verified unrelated employee/contact/company/logo data unchanged. Real concurrent authenticated HTTP saves produced exactly one success and one HTTP conflict without extra history; anonymous correction was denied. Synthetic fixtures remain archived for future testing. All verification sessions were revoked; no account password/profile change was made.
- The first live concurrency test caught an explicit 40001 response causing PostgREST 14 to retry indefinitely. The follow-up public invoker wrapper maps it to PT409 (HTTP 409). Only the identified retrying test connection was terminated, and no correction requests remained active afterward. The corrected live test requires the actual conflict message and passed. See [Supabase's documented retry behavior](https://supabase.com/docs/guides/troubleshooting/high-cpu-and-infinite-transaction-retries-when-using-custom-error-codes-in-rpc-functions-77326b).
- Nine final fictional native previews were inspected at normal/minimum/125% size: missing entry, existing correction, lower reason/save area, visible stale-conflict error, light/dark history and role-specific historical attendance. Save/Cancel remain outside the scroll area, conflict text stays visible, and history uses a supported text separator. Native signed-in field/button automation is not claimed. Final normal-build privacy/startup, exact installer/recovery, private publication and installed-startup evidence are recorded separately with the versioned artifacts. No public push, remote CI, Windows Authenticode or other-machine rollout is claimed.

## Attendance calendar, pre-login branding and compact company defaults â€” version 0.1.8

- Daily attendance replaces typed dates and daily arrows with a native calendar, direct month/year selection, recorded-day highlighting and Today. Future dates are disabled. The daily roster shows recorded attendance, approved weekday leave and no recorded check-in. Future hires are excluded; archived people with historical records remain visible. Missing records are not declared confirmed absence because shift/holiday rules and complete employment history are not implemented. Existing check-in/server-time and original-date overnight checkout rules remain enforced.
- Settings places company name/time zone/leave/save in the left column and shared logo controls in the right. The company name/logo now appear on sign-in. Only validated company branding is cached locally, scoped to the packaged connection; no account, employee, password, token or session fields are persisted. Successful sync refreshes it and logo removal clears it. A new device uses packaged/default branding until its first authenticated sync; another device's changes sync after sign-in. Private Storage/RLS remain unchanged.
- Account Manager help explains clearing **Sign-in access enabled** and saving to suspend access, **Show disabled** to find suspended staff, and **No staff role** to remove desktop admission while keeping the employee. The access switch appears before the role selector for visibility at 125%. Own/original Administrator protections remain unchanged.
- All 68 Rust tests passed. New checks cover real pointer selection of a calendar day, disabled future days, leap-year/month alignment, historical hire/archive/leave/role-scoped rosters, cache replacement/removal/deployment isolation, malformed/oversized cache rejection and branded minimum-size/125% form geometry. The enlarged geometry test initially caught overflow and the header was compacted before passing. Dependency versions remain unchanged. Formatting and the normal optimized Windows build passed. The separately invoked exact Windows installer/recovery test is recorded with the final private artifact evidence.
- Ten fictional native previews were captured and inspected, including normal/125% sign-in logos, company columns/save controls, light/dark calendar, past attendance counts/rows, and staff access/help. The compact calendar omits unused trailing weeks. Normal preconfigured startup loaded the local branding cache, reached a responsive sign-in window and closed normally in an isolated copy. Native signed-in field/button interactions are not claimed; the pointer tests run the actual egui controls headlessly.
- The real Rust adapter verified a temporary transparent logo upload, local branding persistence/reload without authentication, unchanged employee/contact data, and restoration of both original hosted and cached branding. No real company logo was present at the check; the synthetic image was removed. The verification session was revoked and original credentials/profile preserved.
- No schema, policy, hosted function or dependency change was needed. The 0.1.7 database/handler suites remain historical evidence for unchanged server behavior and were not rerun for these local UI/cache changes. The signed private release, installer hashes, installed-startup evidence and limitations belong in the versioned private report. No public push, remote CI, Windows Authenticode or other-machine/workplace rollout is claimed.

## Employee-first staff, own Profile, company logo and sign-in layout â€” version 0.1.7

- The sign-in version row no longer consumes the available height. Headless egui geometry checks verify the actual form at normal and minimum-size/125% equivalent viewports, including scroll-content height. Existing Enter-submit tests still pass. Native sign-in captures at both scales were inspected without the reported overflow.
- Account Manager now lists staff-role employees by name/code, hiding disabled records unless requested. It and Employees open the same Employment/Contact details/Emergency contact/Staff role & sign-in editor. The new invoker RPC saves employment, contacts and role atomically, preserving established identities. Profile mirrors all Employment fields read-only and edits only own private contact/emergency details. Photo/security/session controls remain; account metadata is never a permission source.
- Company defaults uploads/removes a shared private logo. PNG/JPEG size/dimension limits, normalized PNG/alpha preservation, full-image circular fit, revisioned memory cache and Admin-only changes are covered. A failed logo fetch falls back to initials without blocking HR records. Automatic update checks now run only after successful sign-in. Regression tests retain a verified available banner across later transient/up-to-date responses; sign-out clears it and unsaved edits block restart.
- All 62 Rust tests, 231 local PostgreSQL assertions and 32 provisioning/publisher Node tests passed. The Windows installer integration is excluded from the default Rust run and recorded separately with exact final-artifact evidence in the private distribution report. Formatting and the normal optimized Windows build passed. Normal startup was responsive and closed cleanly in an isolated demo copy; normal binary/source privacy checks exclude QA controls and private deployment data. Local Storage/Auth fixtures do not run hosted S3 or email services.
- Fifteen isolated native previews were captured and inspected: sign-in normal/125%, staff list/editor and new staff form/password actions, Profile Employment/contact/emergency including lower save actions, company logo/defaults in light/dark, update banner and manual check. They use fictional data and an explicitly fictional future update; they do not establish signed-in native clicks.
- Migration `20261021075507_employee_profile_and_company_logo.sql` was applied and account-admin v3 deployed to the dedicated prototype. desktop-release v1 remains compatible. The hosted rollback-only suite passed own-contact editing, protected employment/identity, atomic staff failure rollback, narrow supervisor-name projection, disabled/unassigned/anonymous denial and private logo configuration. Early fixture assumptions about uppercase employee codes and managed Storage indexes/delete protection were corrected. Hosted SQL now treats Storage object metadata as read-only; actual object mutations were verified through its API.
- The real Rust adapter passed own contact/emergency save and fresh-sign-in persistence without changing Employment or account details; original contact values were restored and rechecked through another sign-in. A synthetic transparent logo passed upload, private fresh-sign-in retrieval, removal and final absence. Combined provisioning created exactly one employee with full employment/contact/emergency fields and a disabled, unconfirmed staff account; an existing staff edit updated that same person. The labeled synthetic fixture is retained as authorized. Original administrator credentials, account details and company connection were unchanged. Test sessions were revoked. Private update feed compatibility passed.
- Hosted post-test counts were three Auth identities/memberships, four employees and three contact rows (including the restored blank own-contact row); one Administrator remains active. No company logo revision/object remains. Existing advisor notices remain historical rollout items. No public push, remote CI, other-machine test or full native signed-in form automation is claimed. The private release/installation report supplies publication hashes, installer/recovery and installed-startup evidence once completed.

## Unified employee records, automatic identities, themes and updates â€” version 0.1.6

- Employee names and the single Edit action open one record with Employment, Contact details and Emergency contact tabs. Save changes uses one invoker/RLS RPC transaction. Reports to is an employee relationship, including direct reports without accounts. Every company account receives a unique employee identity; existing Administrator access was preserved and backfilled. Unknown hire dates, departments and titles are not invented. Account roles remain protected membership data.
- Light, Dark and System themes join display scale and time format in Settings. The installed version appears at the top right. Updates are checked after sign-in and every 15 minutes. A compatible release offers Update now; pending edits block restart. A signed private manifest, bounded download and SHA-256 verification precede the copied local helper, per-user installer and sign-in restart. Company connection/key/logo files are preserved. The helper restores prior app files after installation failure; this does not undo a database migration.
- All 53 Rust tests passed; the Windows installer test is intentionally excluded from the default suite and was explicitly run separately. New checks cover automatic identities, atomic saves, reporting cycles, missing historical theme metadata, signed feed/schema validation and update tampering. The first sandboxed transport run was blocked by Windows loopback permissions; its escalated rerun passed. All 194 local PostgreSQL assertions and all 30 account/publisher handler tests passed. Local Auth/Storage fixtures do not represent hosted mail or S3 services.
- Both migrations were applied to the dedicated prototype with local versions aligned to hosted history. account-admin v2 and the configured desktop-release v1 were deployed. The rollback-only hosted refinement suite passed automatic identity creation/reuse, immutable employee links, atomic failure rollback, employee reporting without login, cycle/archive protection, HR/Manager/Employee contact scopes, disabled/unassigned/anonymous denial, private catalog/write restrictions and update schema compatibility. Its initial fixture omitted the company field; it was corrected before the passing run. All transactional actors/records were rolled back.
- Native previews were inspected for clickable directory/single Edit, unified employee tabs with fixed save controls, minimum size at 125%, light/dark settings, automatic account employees, update banner and Manager own-record read-only behavior. Preview data and the banner's future version are fictional. A native navigation recursion discovered during QA was fixed and all captures rerun. The normal optimized executable excludes QA preview controls. Normal startup/closure was also checked independently.
- The explicit Windows integration ran the real generic silent NSIS installer in isolated installations. It verified updated executable/metadata and preservation of company profile, verification key, branding and an unknown file. A signed deliberately invalid setup exercised restoration and relaunch of previous files. The test stopped only its own child processes. Parent-exit waiting was not simulated by this integration; it uses the already-exited-parent path. Final artifact hashes and the installed revision are recorded separately.
- The security advisor reported the existing disabled leaked-password protection warning and an informational no-policy notice on the intentionally private release catalog. Client catalog grants are revoked; only the trusted hosted publisher can write it and staff use a guarded metadata RPC. Performance notices concerned unused relationship/audit indexes in this small prototype; these indexes were retained. See [password protection](https://supabase.com/docs/guides/auth/password-security#password-strength-and-leaked-password-protection) and [the private-table notice](https://supabase.com/docs/guides/database/database-linter?lint=0008_rls_enabled_no_policy).
- Windows executables remain without an Authenticode certificate; update payloads use a separate Ed25519 signature. The private signing key is never packaged or committed. No public release/push, remote CI result, employee portal or automatic database deployment is claimed. Confirmation redirect/company mail configuration remains a rollout prerequisite. Live adapter, provisioning, private signed release/download and actual installation results are recorded in the versioned private report without retaining account credentials.

## Employee contacts â€” version 0.1.5

- Employees â†’ Contacts provides optional personal phone/email/home address and one emergency contact with relationship, primary phone and optional alternate phone. Both tabs save together. Administrator/HR edit all company contact records; other active members read only their own linked record. Manager team-directory scope does not reveal team private contacts. Ordinary Employee desktop admission remains blocked and the portal remains future work.
- All 45 Rust tests passed, including contact normalization, optional clearing, invalid field/partial emergency rejection, retention for archived employees, role restrictions and actual HTTP contracts for create/update/reload, company/employee filters and zero-row update errors. Formatting and the optimized normal Windows build passed. Direct dependency versions are unchanged.
- All 166 local PostgreSQL assertions and all 22 provisioning-handler regression tests passed. Contact assertions cover explicit grants/RLS, HR edits, own-only Manager/Employee reads, no client deletion or identity/timestamp reassignment, disabled/unassigned/anonymous denial, cross-record links, field validation and archived records. Local Auth/Storage are simulated; these are not concurrent-client or hosted mail tests.
- Additive migration `20261021033544_employee_contact_details.sql` is deployed to the dedicated prototype; its filename matches hosted history. No existing employee rows or account policies were rewritten. The rollback-only `tools/db-tests/hosted-contacts-smoke.sql` passed hosted validation, Administrator/HR writes, own-only Manager/Employee reads, membership revocation and outsider/anonymous denials. Its temporary role actors rolled back. One initial tool request expired before execution; current counts were checked and the retry succeeded.
- Native fictional QA captures were inspected for the directory, editable contact form, HR emergency tab at minimum 1000 Ã— 720 window/125% scale and Manager own read-only contacts. An initial hidden-footer problem was corrected with a scrollable body and fixed actions; delayed capture also avoids the opening animation. Save/Cancel and read-only Close are visible. These are native synthetic-layout checks, not signed-in native form interactions.
- A private probe compiled against the actual Rust adapter signed in to the dedicated prototype, changed only the labeled synthetic contact fixture, verified server reload/timestamp and persistence after sign-out/new sign-in, rejected an incomplete emergency contact and checked that account Profile/email were unchanged. Both probe sessions were revoked. Credentials stayed stdin/memory-only. This verifies the cloud adapter, not native authentication automation.
- The labeled synthetic employee `QA-CONTACT-001` and its fictional contact record remain for repeat testing, as authorized. Final hosted counts are one original Auth user/session/membership, one synthetic employee/contact, zero attendance/leave/access events and one active owner Administrator. The original credentials, deployment profile and plan were preserved. No test accounts were retained because this module did not require new real Auth identities.
- Security advisors report only the pre-existing [leaked-password-protection warning](https://supabase.com/docs/guides/auth/password-security#password-strength-and-leaked-password-protection); performance advisors have an informational [unused-index notice](https://supabase.com/docs/guides/database/database-linter?lint=0005_unused_index). No new contact-table security issue was reported. Company mail/confirmation landing setup remains a rollout item from 0.1.4.
- Installation, exact payload hashes and normal startup are recorded in the private 0.1.5 installation report. This remains an unsigned private Windows prototype, with no push, public release or remote CI claim. Employment history, documents/201 files and organization charts are not implemented by this update.

## Account Manager â€” version 0.1.4

- Administrators can create/link company accounts, assign four fixed roles, link employee records and reporting managers, enable/disable access and inspect recent access changes. Account Manager is above Settings and hidden from every other role. Manager desktop access is retained; ordinary Employee accounts are portal-only and their desktop sign-in is rejected before company HR records or private photos load. The employee portal itself is not implemented.
- All 40 Rust tests passed, including desktop role admission, manager access without administrator account reads, Employee rejection/session revocation, disabled account provisioning, own-leave review denial, signed-in HTTP contracts and worker cleanup on revoked access. Formatting and the optimized normal Windows build passed. No dependency versions changed.
- All 126 local PostgreSQL assertions passed against every migration. Tests cover HR/Manager/Employee record scopes, direct-report review without employee edit permission, self-approval denial, metadata tampering, disabled existing-token access, owner/self/last-admin protections, reporting cycles, active employee links, protected audit history and private photos. Auth, API roles and Storage metadata are simulated locally; these are not hosted S3 or concurrent-client tests.
- All 22 Node provisioning-handler tests passed. They exercise live-role verification boundaries, nonadministrator/forged bearer denial, server-credential separation, exact-new-identity cleanup, existing-account protection, pending mail delivery/retries, disabled creation and bounded input. Responses never return initial passwords, bearer tokens or server credentials. These tests use controlled upstream responses, not live Auth provisioning.
- Native QA captures were visually inspected for administrator account lists and creation form, HR Settings without company/account management, manager direct-report approval with own requests awaiting review, Employee desktop denial, minimum window size and 125% scale. Lower form actions and the disabled Biometric integration card were separately inspected. These are fictional QA data, not a deployed employee portal. Normal builds exclude the QA environment controls.
- The normal optimized executable opened a responsive, titled HRIS window and closed normally in an isolated fictional-demo copy. The initial input-idle probe attempted closing before the actual HRIS window appeared and timed out; the corrected probe waits for the HRIS title before checking startup/close. Installed payload/startup evidence belongs to the private installation report. Signed-in native Account Manager interactions are not claimed; the visual checks use fictional QA data and the live adapter checks below use actual network requests.
- After explicit deployment approval, migration `20261021021602_account_manager_roles.sql` was applied to the dedicated HRIS project and `account-admin` version 1 deployed active. Local migration history matches the hosted history. Hosted inspection confirmed the owner remains an active Administrator and all three public access/account RPCs are security invokers.
- The rollback-only `tools/db-tests/hosted-accounts-smoke.sql` passed on hosted PostgreSQL. It checks all four role scopes, direct-report review, own-leave denial, account/company-setting restrictions, metadata spoofing, reporting cycles, owner protection, active employee links, disabled-token denial and anonymous/unassigned access. Every fixture was rolled back. The fixture initially omitted the required employee allowance; it was corrected before the passing run.
- Actual HTTP checks passed administrator sign-in/access/account listing and anonymous function denial (401). The function created a disabled, unconfirmed QA account with its disabled status preserved. Sign-in before confirmation returned `email_not_confirmed`. The signup email arrived in the authorized mailbox; its verification returned HTTP 303 and the localhost redirect was not followed. After activation, the HR Generalist account could not list accounts, read access history, invoke provisioning or modify company settings.
- A nonowner member uploaded/read its own private test photo; the Administrator could not read that photo. Deactivation blocked existing-token access to the company and photo, and reactivation restored access. Photo removal was confirmed by Storage metadata and the actual Rust client's `Cache-Control: no-cache` reload: no photo and no warning. An auxiliary immediate HTTP deletion check without cache control was inconclusive. The release library's real Manager sign-in passed with one own employee record and no managed-account data. Real Employee sign-in was rejected by the desktop before HR loading.
- Guarded cleanup removed only the newly created QA identity, employee, access events and photo. Post-cleanup counts returned to one company, one Auth user, one pre-existing session, one owner membership, zero access events, zero employees/attendance/leave and zero profile-photo objects. Test sessions were revoked and the private probe terminated. The original administrator password, email, profile metadata and deployment connection were not changed.
- Security advisors reported only the existing [disabled leaked-password protection](https://supabase.com/docs/guides/auth/password-security#password-strength-and-leaked-password-protection) warning; the Free plan was preserved. Performance advisors reported informational [unused indexes](https://supabase.com/docs/guides/database/database-linter?lint=0005_unused_index) on the fresh membership/audit tables; required relationship/query indexes were retained.
- The Windows setup is private and unsigned. No push, public release, remote CI, automatic updater, full approval chains, employee portal or company production-readiness claim is included.
- The earlier 0.1.3 live Profile checks subsequently passed personal/photo/display updates, Enter sign-in, wrong-password rejection, password/email changes and restoration, and local/other/global session controls. Synthetic data was removed and the original account restored. These historical checks were not repeated for 0.1.4. Email confirmation's localhost redirect remains a recorded deployment issue; configure an appropriate confirmation destination before company rollout.

## Full Profile page and circular account photo â€” version 0.1.3

- The bottom-left circular photo/initials control opens Profile as a full main-content page, with the same navigation behavior as Settings. Personal details, Account & security, and Sessions group its account functions. Display scale and 12/24-hour time preferences belong in Settings.
- All 32 Rust tests passed. New checks cover avatar hit testing with/without a texture, bounded image decoding/cropping/circular alpha, personal/security validation, Auth metadata and private-photo round trips, failed uploads, incorrect-current-password rejection, fresh-session password/email updates, pending email confirmation, demo isolation, and global sign-out cleanup even when the server is unreachable. A final regression test checks that photo/security updates preserve unsaved personal/display edits while sign-out clears those drafts. The fixture listener explicitly resets accepted Windows sockets to blocking mode; initial fixture failures were corrected before the passing run.
- All 68 local PostgreSQL assertions passed, including photo read/write/delete ownership, cross-account and anonymous denial, unassigned-account denial, ownership/path transfer prevention and private bucket restrictions. Local Storage metadata is simulated; these assertions do not claim to run a hosted S3 service.
- The checked-in photo migration was applied to the dedicated HRIS project. Its local filename matches hosted version `20261020145925`. Read-only hosted checks confirmed the private PNG bucket, 2 MiB limit, Storage RLS and the four authenticated ownership policies. An actual anonymous HTTP photo-upload attempt was denied with HTTP 400; no authenticated account credential was used.
- Native screenshots were inspected at minimum window size for personal details, all Profile sections, the email/password area, Settings display preferences, enlarged display at 125%, circular photo rendering, and initials fallback. The decorative footer divider was corrected after the first inspection and avatar hit testing rechecked. Lower-page capture uses the supported scroll-to-bottom behavior; earlier blank QA captures were superseded. Synthetic QA assets are not deployment photos.
- The optimized Windows build completed successfully. Its exact source revision, executable/setup hashes, installation state and artifact evidence are recorded in the version's private distribution manifest/report. Actual account passwords, email and photos were not changed to test this update; live signed-in Profile actions, email delivery/confirmation and the native photo chooser still require user interaction. Previous live HR workflow checks below remain historical evidence for unchanged HR behavior.
- Supabase's security advisor reported one configuration warning: [leaked-password protection is disabled](https://supabase.com/docs/guides/auth/password-security#password-strength-and-leaked-password-protection). Supabase documents that feature as Pro-plan and above; the current Free plan was preserved. Current-password checks and normal server password-policy validation remain active. No privileged client key, persistent session, public photo URL, remote publication or unrelated hosted-project change was introduced.

## Company sidebar, profile and Enter-key login â€” version 0.1.2

- Native live workflows were verified in the user-signed-in 0.1.1 installation before rebuilding: employee creation/editing and server refresh, attendance check-in/check-out, annual leave request/approval and balance 15 to 14, and successful sign-out returning to an empty login screen. Hosted queries independently confirmed the saved records. Cleanup verified the exact synthetic employee and QA markers before deleting only that employee's attendance, leave and parent record. The database returned to one company and zero employees, attendance and leave requests.
- The sidebar now places a transparent company logo, or initials fallback, above the company name. It contains only the five navigation items followed by a bottom-left Profile menu. COMPANY/WORKSPACE headings and the old prototype footer were removed. Sign out moved from Settings into Profile; company Settings retains policy/default fields only.
- The optional deployment logo is bounded to PNG, 2 MiB and 2048 Ã— 2048 pixels. Tests confirm transparent and partially transparent pixels are preserved and invalid/oversized images are rejected. No actual company logo was supplied; QA used a synthetic alpha fixture that is not packaged.
- All 22 Rust tests passed, including two headless egui tests that send Enter events to the actual email/password widgets and confirm one submission without a newline and no resubmission while busy. This verifies keyboard handling without automating a Windows authentication dialog or sending synthetic credentials to a real server.
- Native screenshots were visually inspected for the sidebar, company Settings without account actions, Profile with a fictional account at minimum window size and a long company name, transparent-logo rendering, and sign-in. The account preview remains visibly labeled demo and never authenticates. One unsupported decorative arrow glyph was removed after inspection.
- Database schema, authorization policies and HR workflow rules are unchanged. Installation/build hashes and the exact source revision belong to this version's distribution manifest and update report. The revised installed UI requires a fresh local login after restart; native live workflows above were tested before the visual update, not falsely reported as retested afterward.

## Company deployment flow â€” version 0.1.1

- Company database selection is deployment configuration. The app reads only its packaged profile; remembered device settings and environment variables cannot select another project. Connection setup controls were removed from the interface.
- Configured installations start on HRIS account sign-in with no company records or fictional demo data loaded. Successful authentication opens Overview. Sign-out clears company records and returns to sign-in, including when remote session revocation fails.
- All 19 Rust tests passed: 14 domain/configuration checks, one deployment-profile test, two Auth/REST contract checks, and two worker access/sign-out regressions. Missing or malformed installed profiles block access instead of falling back to demo. Old device settings were present in the profile test and were ignored.
- Native QA screenshots were generated and visually inspected for configured sign-in, malformed profile, missing installed profile and the separate demo Settings screen. Forms fit the window; no URL/key/remember controls appear. Error states expose no HR modules or sample records. Screenshots used generic test profiles.
- The database schema, API authorization model and company data were not changed. The earlier 55 PostgreSQL assertions and actual live adapter checks below remain historical evidence for the unchanged database behavior; they were not repeated for this UI/deployment change.
- Build/installation hashes, the candidate's exact source revision and final installed-screen evidence are recorded in its distribution manifest and installation report. Interactive live sign-in remains a local user action; no automated UI authentication or privileged client key was introduced.

## Earlier Supabase connection update â€” version 0.1.0

- All 17 Rust tests passed: 14 HR workflow/configuration tests, two local Auth/REST contract tests, and one company-profile precedence/fail-closed test. Formatting was applied with the pinned-compatible Rust 1.95.0 toolchain. Tests use fictional identities and credentials.
- All 55 local PostgreSQL schema, permission, singleton-company, and workflow assertions passed against both checked-in migrations.
- Both migrations were applied to the dedicated HRIS project. Local filenames match the server-assigned migration history: `20261020111810_initial_hris.sql` and `20261020113112_single_company_installation.sql`. Files were originally generated with the Supabase CLI before their versions were aligned to hosted history.
- The hosted SQL smoke test exercised actual PostgreSQL RLS, owner access, unrelated-account denial, anonymous denial, company uniqueness, employee validation, server attendance timestamps, duplicate check-in rejection, leave approval and allowance limits. All fixture identities and HR records were rolled back; post-test counts confirmed zero fixture identities and zero company/employee records.
- The public Auth health endpoint returned HTTP 200. Unsigned-in REST reads returned HTTP 401 for each of the four HR tables. These requests used a modern publishable key without an administrative key.
- The security advisor reported no lints. The performance advisor reported four informational [unused-index notices](https://supabase.com/docs/guides/database/database-linter?lint=0005_unused_index) on the fresh database; required/query indexes were retained.
- The requested administrator was created through the normal Auth signup endpoint and its email was subsequently confirmed. Trusted bootstrap assigned the sole account to one placeholder company. No password or session was stored in source, settings, or deployment files.
- A live check then used the real Rust adapter with that account: password sign-in, employee creation and reload, attendance check-in/check-out, leave request/approval and annual-balance verification, refresh and sign-out all passed. The verification session was revoked. Cleanup checked the exact temporary employee's identity/QA labels and removed only its leave, attendance and employee records. Final hosted counts: one company, one confirmed account, zero employees, zero attendance and zero leave requests.
- The configured native startup screen was captured and visually inspected. Sign-in now appears before company defaults, with configured connection details collapsed. Email, password and sign-in controls fit in the visible window. This screenshot used a generic example profile rather than private deployment identifiers.

The hosted SQL tests use database roles and simulated request subjects; the additional real Rust-adapter check used an actual signed-in user's JWT and the public Data API. Interactive keyboard sign-in through the native UI, long-running token refresh, other workstations and hosted concurrent clients remain unverified. The headless adapter check establishes network/storage behavior, while the native screenshot check establishes layout.

## Earlier prototype checks

The initial prototype separately passed inspection of Overview, Employees, Attendance, Leave, Settings, Add employee and Request leave, plus a normal optimized Windows startup/close check. Its original 15 integration tests and 52 local SQL assertions were superseded by the current expanded suites above. The original screenshot helper returned transparent pixels; the supported egui screenshot callback is used instead. Restricted local networking required the HTTP fixture test to run with normal loopback access. A cache hard-link warning fell back to file copying; it did not fail compilation.

## Earlier installed Windows check â€” version 0.1.0

- A per-user NSIS 3.11 setup compiled without warnings and installed with exit code 0. It created desktop and Start menu shortcuts and a Windows Apps uninstall registration under `io.github.tatselkrik.hrisdesktop`.
- All six installed payload files matched the prepared distribution by SHA256. The installed app executable is unchanged from the previous verified candidate: `1b75e99680955f7965e4025e0f1eb4c38aeca2eb9bd2fabe761aaf22004250bb`.
- The Start menu shortcut launched the actual installed executable. Native computer-use inspection confirmed its Settings screen, configured profile and clearly labeled demo mode.
- Through the installed native UI, a fictional demo employee was created, found by code, and edited. Attendance check-in and check-out passed. A second daily check-in produced the expected rejection and did not create a duplicate.
- An annual leave request was submitted and approved through the native UI. The employee's balance changed from 15 to 14 days as expected.
- The installed app closed normally and reopened from its desktop shortcut. Demo headcount reset from nine to eight; searching for the synthetic employee returned no matches. The app was left at Settings with empty sign-in fields.
- The refreshed hosted baseline contained one company and zero employee, attendance or leave records. The native tests above stayed in the in-memory demo; no demo records were uploaded.
- The uninstall/reinstall cycle passed. The uninstaller removed its named files, shortcuts and Apps registration while retaining a synthetic unknown file in the installation folder. Reinstallation completed with exit code 0 and restored all six payload files with their original hashes, both shortcuts and registration. The installed app was reopened afterward.
- Interactive live sign-in and live UI persistence await a local sign-in by the user; actual Rust-adapter live persistence was previously verified as described above.

The current Windows candidate remains a private, unsigned prototype. Optimized assembly, startup, artifact hashes and source-revision evidence belong to each version's distribution manifest. No remote GitHub CI run, public release, signing, automatic update, other operating-system check or company production rollout has been verified. No unrelated hosted project was changed. The original 0.1.0 distribution manifest predates its installation checks; the preceding section records those additional checks without changing that older binary.
