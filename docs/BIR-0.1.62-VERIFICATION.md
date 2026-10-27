# Desktop0.1.62 — BIR declaration builder verification

**Installed and privately published.** Frozen source
`cc95725d1d6037f139694d94497ed993153f89c4`; keep0.1.61rollback. The
[delivery record](BIR-0.1.62-DELIVERY.md) contains exact package, installer/recovery,
installed-state and private-channel proofs. This adds local published-layout DAT draft
assembly from explicit declarations, not automatic payroll classifications,
tax computation, official validation or submission. Its
[contract](BIR-DECLARATION-BUILDER.md) records the reviewed official sources.

## Implemented scope

- HR/Admin-only Schedule1/2 blank CSV/native Excel declaration templates.
- Local bounded parsing, strict unchanged header order, exact identifiers and
  explicit reviewed amounts. No blank-to-zero, transliteration or truncation.
- Registered employer header, deterministic sequences, exact monetary controls
  and published filename; existing complete-file/subtotal checks must pass.
- Native preview with declared controls and explicit review before DAT saving.
- Re-read source/current protected registration; bind actor/company/year/schedule,
  revision and complete source/output hashes. Recheck access before new-file IO.
- Existing files and sources preserved; no database/schema/rate/dependency change.

## Evidence completed so far

The final full-source run passes679default Rust checks, with28intentional opt-ins
excluded and no compiler warnings.
The22new core/native-HTTP/UI checks cover both layouts, all amount boundaries,
CSV/Excel parity, identifiers, dates/codes, blank values, overflow, duplicate
identities/namesakes,500/501rows, explicit review, current roles, changed source/
employer registration, revocation and no-overwrite. The final run includes the
layout correction; its two targeted UI checks/rebuild also passed.

The explicit fictional artifact opt-in ran one test. Independent Python Decimal,
CSV and OpenPyXL/ZIP reading verifies four DAT drafts, two identical CSV/Excel
output pairs,71published control positions and eight blank templates. Literal
IDs, row order, declared amounts, centavos, ASCII/no-BOM/CRLF, source/output hashes
and declared controls match. Workbook inputs are literal cells without formulas,
external links, macros, hidden/merged data or numeric identifier coercion.

Eight initial native captures were inspected. A centered control-label issue was
fixed; three subsequent normal/compact/dark125table captures are inspected and
aligned. Seven unaffected views plus those three corrected views cover empty/
preview, both schedules, review/save controls and Manager exclusion. This is
fictional native rendering, not interactive sign-in/file-dialog evidence.

Actual normal hosted HR/Admin native template requests and incomplete-company
registration guards pass, as do Manager/Employee denial, forged metadata and
existing-token revocation. All35prior payroll/government sets and prior Auth
users remain unchanged. Synthetic actors remain retained disabled/archived with
sessions ended. All72migrations remain unchanged; no source fields were uploaded.

The configured employer's BIR registration is incomplete. Its builder correctly
blocks DAT preparation; **hosted positive assembly against complete configured
employer facts remains unverified**. Positive assembly is verified with fictional
protected HTTP responses. No actual registration was fabricated or changed.

The initial hosted attempt compared two separately rendered Excel ZIP files,
whose creation timestamps differed. The test now compares the literal template
schema; the final guarded runs pass. Initial proof/log/tree are preserved under
`work/bir062-live-initial`, with cleanup confirmed after the first attempt.

## Evidence and delivery gate

- `work/bir062-{rust,builder-all-targeted,grid-targeted,vectors}.log`
- `work/bir062-independent-artifacts.json`, `work/bir062-vectors/`
- `work/bir-native-0.1.62/`, `work/bir-native-0.1.62-gridfix/`
- `work/builder062-native-live-{administrator,hr_generalist}/`
- `work/builder062-native-live-results.json`

The normal optimized candidate starts responsively and its isolated configured
sign-in window closes normally. Executable SHA256:
`eb6ce7379d9d25c0ccf3ed66a46e34bf3769a851f3daccffd3a5426fbcf47825`.
See `work/bir062-normal-startup.json` and `work/bir062-rust-final.log`.

Source/archive privacy, same-key signature, exact real-installer/recovery, actual
PC installation/hash/BUILD/Windows Apps/normal launch and private downloaded
size/hash/signature/access checks pass; see the separate delivery record. No
remote-CI or Authenticode success is claimed.
No full SQL suite/replay was needed for this native-only change.

Official7.4 runtime acceptance remains NOT_RUN. Mixed schedules, non-ASCII names,
embedded delimiters, historical/certificate/classification verification and
agency upload acceptance remain separate limitations. No government executable,
filing, money transfer, external communication, subscription or hardware is used.
