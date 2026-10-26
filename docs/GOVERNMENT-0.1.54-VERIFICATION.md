# Desktop0.1.54 — government Excel review exports

**Installed and privately published on26October2026.** Frozen application source:
`15c490f5a58777d6676dfe261c5cbc469de64ecd`, branch `codex/employee-contacts`.
Keep0.1.53 as rollback. No migration, payroll-rate change or new dependency.

In **Payroll → Government → Reconciliation**, choose CSV or Excel for either
fresh review or agency preparation. Both load current protected source data,
recompute validated totals and render before the final access check/new-file
write. Excel preserves identifiers, formula-like names and exact decimal PHP
amounts as literal text. Unknowns, excluded sources, individual receipt dates,
negative adjustments and selected-month split/voided remittance allocations
retain their meanings. These are internal review worksheets; editing a workbook
does not recalculate totals or change HRIS. See [the testing checklist](GOVERNMENT-0.1.54-CHECKLIST.md).

## Verification completed

- **574default Rust checks**,11intentional opt-ins excluded, zero Rust warnings.
  Fourteen new cases cover native text/precision/scope, missing registrations,
  unknown/postdated/negative/voided sources, recomputed cached summaries, malformed
  wire/field scopes, format bounds, file preservation and account-bound selection.
- Actual localhost HTTP requests exercise both real client export adapters with
  CSV/Excel, successful saves and late403revocation blocking file creation.
  Manager/Employee client guards reject before any projection request. These
  local fixtures are distinct from the hosted Auth proofs below.
- Two explicit native vector runs render **24workbooks**, including16from eight
  unchanged raw hosted HR/Admin projections for all four agencies. Independent
  Python inspection checks **280data rows**, CSV parity, original wire identities,
  selected-agency fields, exact integer-centavo sums and receipt allocations,
  literal string cell types, frozen/filterable headers and no formulas/macros/
  external links. Eight fictional workbooks include leading-zero and Unicode/
  formula-like names; no Office installation or external conversion is used.
- Actual hosted normal sign-ins verify HR/Admin positive access, Manager/Employee
  denial, metadata forgery denial and immediate existing-token revocation.
  **35prior payroll/government record sets and all preexisting Auth users remain
  unchanged.** Synthetic accounts are disabled/archived, sessions ended and
  history retained. No financial or registration record was changed for this test.
- **Five actual fictional native layouts** were inspected: CSV, Excel, compact,
  dark125percent and Manager exclusion.467source files pass the privacy review;
  unchanged upstream copyright/license notices are preserved. The normal optimized
  binary excludes QA controls and the company connection values.
- The exact signed updater passes the existing real silent-installer/recovery
  opt-in, preserving company profiles, branding, verification keys and unrelated
  files in isolated installations. Actual PC installation at10:41:30UTC verifies
  executable hash, BUILD version/source, Windows Apps registration, responsive
  normal launch and normal close. No unsaved work was discarded.
- The existing DPAPI/Ed25519 identity is unchanged. The authorized private channel
  returns the exact package size/hash/signature; public/ordinary Employee access
  is denied, latest staff feed is0.1.54 and all69source migrations are present.
  The installation receipt's earlier `private_update_published:false` is its
  installation-stage value; the later publication receipt confirms publication.

## Artifact hashes

| Artifact | SHA256 |
| --- | --- |
| Installed executable | `20ab5643bf5fdb72fc3194b42f15907fc7fe0af91de6937ba03b76c036fbfb09` |
| Full setup | `ec08af29ffa37b6aeb9145436c3b260ca48facfbe769f8855bfde686f9959671` |
| Signed update package | `2ed81654ac0de2a431d50b4542586395f664ed657616db8b3aea736c24ab86d3` |
| Frozen source archive | `e346da0a18c9a08cf2b144037526f618ece91bf904ee5d6907be5dac301d4873` |

## Limits and retained evidence

Schema69is unchanged; the full database suite/migration replay was **not rerun**
for this client-only increment. Exact-source migration presence and fresh hosted
permissions/preservation were checked. The existing leaked-password-protection
warning remains; no Auth/billing/plan setting changed. Current project storage was
234,054,818bytes before the6,959,872byte update; database32,741,043bytes. This is
project headroom evidence, not an organization-wide egress/billing assertion.

Actual signed-in native file-dialog/button interactions, Microsoft Office UI,
current portal browser/phone/email, remoteCI and Authenticode were not verified.
No BIR/SSS/EPRS/Pag-IBIG acceptance, real filing, transfer or vendor connection is
claimed. The government-file milestone remains partial. Kirk's downloaded BIR
module is queued for further inspection; the SSS draft remains internal review.

Compact private evidence is retained under `work/government054-*`,
`work/government-native-0.1.54`, `work/install-government054-actual`, the immutable
company-deployment package and private release manifest. Source/actual hosted
captures are not included in generic source or public artifacts.
