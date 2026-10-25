# Agency preparation 0.1.44 — delivered

Version **0.1.44** is installed and published to the existing private update
channel. Release source: `bb5949a37574cc40fd462210cc227609612bed74`.
Use **Payroll → Government → Export agency preparation CSV** and the
[review checklist](GOVERNMENT-0.1.44-CHECKLIST.md).

## Meaningful verification

- 434 default Rust checks, four explicit PostgreSQL-vector checks and the exact
  real-installer/recovery check pass: 439 distinct checks. The SSS reference
  renderer matches twelve independent fictional month vectors, retains namesakes
  and rejects malformed/overflowing inputs. Preparation checks cover agency field
  privacy, foreign/duplicate identities, formula protection, unresolved data and
  revocation before file creation.
- 4,110 PostgreSQL assertions and fresh replay of all 60 migrations pass. The new
  read-only preparation projection preserves 92 prior public record sets and 264
  existing functions. No table, wage rate, issued payroll, receipt or registration
  value is changed by exporting.
- Hosted Auth/REST verifies current HR/Admin access, active Manager/Employee
  denial, forged role metadata and existing-token revocation. Four agency responses
  and 47,437 bytes of internal CSV are validated natively in memory. A retained
  synthetic person's SSS/PhilHealth/Pag-IBIG identifiers are correctly selected;
  January BIR excludes that person because its source set does not include them.
- All 24 prior payroll record sets, earlier Auth users and employer registration
  match. Synthetic identity evidence stays on an already archived test employee;
  temporary accounts end disabled/archived/session-free. No fake employer identity
  or actual payment was created for these checks.
- Four fictional native layouts were inspected: normal, compact, dark at 125
  percent and Manager denial. The new controls/text wrap within their content
  area. Normal configured startup/close and source/archive privacy checks pass.
  The binary excludes QA helpers and contains no packaged connection profile.
- The existing DPAPI signing identity and installed verification key are retained.
  Rust verifies the exact Ed25519 manifest/package. The real updater checks every
  checksum-listed payload and failed-installer recovery. Actual installation exits
  0; installed hashes, BUILD/source, Windows Apps and normal launch/close match.
  Company files, branding, keys, unrelated files and a full 0.1.43 rollback are
  preserved. No unsaved work was discarded. Authenticode remains unsigned.
- Advisory counts remain unchanged: three earlier warnings and five earlier
  unindexed foreign keys; no new finding in those categories. Full formatting has
  32 preexisting files with differences and no newly introduced file failure.

## Private publication and limits

Kirk explicitly authorized 0.1.42 and future verified releases to the existing
private HRIS channel on 25 October 2026. The earlier approval-review publication
block is superseded by that direct permission. Versions 0.1.42, 0.1.43 and 0.1.44
are published through the normal signature-verifying endpoint. Their frozen
archives, signed artifacts and required migration subsets are checked independently
of later HEAD. Authenticated downloads match size/SHA256 and pass Rust signature
verification. Public and ordinary-Employee download/feed access is denied; the
Manager staff feed serves 0.1.44. No artifacts were rewritten or downgraded.

Executable SHA256:
`b59b699b2dd034d8155a7730c5edc18fec16635b9b05771f3fa6c61265615e58`.
Updater SHA256:
`e0560b849dc47a0c2a35d850c8a1829f3788599168d838964d8510ce6ce00ca5`.
Source archive SHA256:
`957c099b78c3f63cd6796ab0afb270b32b7eb232ab8fff2f4ffffa7d9be84959`.

Proof remains in ignored `work/government044-*`, `work/government-preparation-*`,
`work/government-held-publication-results.json` and
`work/install-government044-actual/installation-results.json`.

Live signed-in button interactions and current browser/phone visuals were not
automated. Preparation CSV is an internal worksheet, not an agency upload/return.
The SSS reference encoder has no product download control yet. Broader encoding,
reserved-slot semantics, agency declarations and validator acceptance remain
queued; the full government milestone stays partial. No return was filed, money
transferred or portal publicly hosted. No remote CI/public release is claimed.
