# Government registration 0.1.43 — local delivery

Version **0.1.43** is installed on Kirk's PC. Its verified release source is
`72717aebd4269a9ef402925f3a6d6dc02e091d38`. Open **Payroll → Government →
Registration details** and use the [review checklist](GOVERNMENT-0.1.43-CHECKLIST.md).
The overall government milestone remains partial: agency file adapters follow.

## Verification

- 428 default Rust checks, three explicitly gated PostgreSQL-vector checks and
  the real installer/recovery test pass: 432 distinct checks. The new transport
  tests cover access revoked during fetch and an identical save retry after a
  failed refresh. Unknowns remain blank and incomplete audit chains are rejected.
- 3,974 PostgreSQL assertions and fresh chronological replay of all 59 migrations
  pass. The additive deployment preserves 90 prior public record sets and 257
  existing functions, including the installed reconciliation contract.
- Actual hosted Auth/REST checks cover HR/Admin changes, missing/false review,
  unknown fields, exact operation retries after a later edit, stale revisions,
  retained before/after history, direct-write denial, forged role metadata and
  existing-token revocation. Ordinary Employees cannot read their own private
  registration. Three fresh snapshots pass native validation entirely in memory.
  All 24 prior payroll sets, earlier Auth users and employer registration facts
  match. No fake employer identity was written. Synthetic employee records are
  retained with accounts disabled/archived and sessions removed.
- Ten fictional native layouts were inspected: company/person, edit/empty values,
  save area, history, compact layout, dark theme at 125 percent and Manager denial.
  The normal configured release launches responsively and closes normally.
- Source and source archive privacy checks pass. The production binary excludes
  screenshot/preview code and contains no deployment profile. The existing DPAPI
  signing identity is preserved; Rust verifies the Ed25519 updater signature and
  exact installer. No private key was written or packaged. Authenticode remains
  unsigned. The real updater test checks every checksum-listed payload and
  recovery from a failed installer in isolated installations.
- Actual installation exits 0. All delivered payload hashes, executable, BUILD
  version/source and Windows Apps registration match. Connection, branding,
  verification key and unrelated files are preserved, with a complete 0.1.42
  rollback backup. The installed normal binary launches and closes normally;
  no unsaved work was discarded.
- Advisory review finds no new warnings or unindexed foreign keys. The three
  earlier warnings and five earlier foreign-key findings remain documented in
  [the government source review](PAYROLL-GOVERNMENT.md). New unused-index notices
  are informational; necessary indexes were retained. Full formatting still has
  32 preexisting files with differences; no new file-format failure was introduced.

## Limits and artifact evidence

Live signed-in button interactions, current browser/phone views, agency identity
verification and government validator acceptance were not automated. No return
was filed and no money was transferred. No remote CI or public release is claimed.
Identifier checks validate supported shapes, not agency registration/checksums.
The employer Pag-IBIG format remains conservatively bounded until its current
primary document can be inspected without bypassing the browser challenge.

Executable SHA256:
`c328f185e3a6cf6f704c99a89d8cb301674ee4fa0aba50f14847840b76c5920f`.
Signed updater SHA256:
`c3ec83cf4de378c3925376f11ba3a3e9b90b01fc5fa14a96f08e74a849d3730d`.
Source archive SHA256:
`85d08dbd8133fc071d4d24985d2924ef33ef5d5ab765b545ac45523ddf8ad978`.

Private proof is retained under `work/government043-*`,
`work/government-registration-*`, `work/company-deployment-0.1.43` and
`work/install-government043-actual/installation-results.json`.

Automatic approval review rejected new private-channel publication because earlier
trusted upload authorization covered 0.1.41. A human authorization question is
pending; neither 0.1.42 nor 0.1.43 was uploaded. Local installation is complete
independently. Do not retry or bypass this publication boundary without a trusted
human answer.
