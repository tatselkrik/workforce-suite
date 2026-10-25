# Desktop0.1.51 — delivered attendance request imports

**Installed and privately published**, frozen application source
`53d93f12cf910cdd9f0758fc928d2f7886af3836`. Keep0.1.50as rollback. The attendance
portion of roadmap6is complete; leave-request batch imports remain next.
Use [the review checklist](ATTENDANCE-IMPORTS-0.1.51-CHECKLIST.md).

- 534default Rust checks pass, including12new attendance-import cases. Seven
  explicit opt-ins are excluded from that count. Separate hosted DTO and real
  installer/recovery opt-ins pass. Compiler output has no warnings.
- 82new PostgreSQL checks and6,287migration, authorization, workflow and source
  preservation assertions pass. All67checked-in migrations replay from empty
  local Auth/Storage fixtures. Invalid offsets, leap seconds, reversed ranges,
  unknown/archived personnel, duplicates, stale sources and break bounds are tested.
- Five actual fictional native layouts were inspected: entry, compact dark row
  errors, protected preview, mixed approval history and uncertain submission.
  Clock rows align and retain complete timestamps. The compact table scrolls
  horizontally. A failed QA-only scroll offset capture was retained, then fixed.
- Guarded migration67preserved105prior public rowsets and all335existing functions.
  Real hosted Auth covers six roles/actors, pure previews, atomic pending batches,
  concurrent exact retries, actor/payload mismatch, independent Manager approval
  races, withdrawal retries, private tables, forged metadata and immediate
  revocation. All108pre-existing public rowsets and171prior Auth facts survived
  the hosted workflow test; only synthetic records were added/changed.
- Six actual protected hosted previews/receipts/history projections decode and
  validate through the native contracts. A committed receipt still validates
  after ordinary approval changes the attendance source.
- The normal optimized executable excludes QA hooks and company connection data.
  444source files and the frozen archive pass the deployment/privacy scan. Full
  free upstream Excel dependency notices are packaged.
- The exact signed updater passed isolated successful installation, preservation
  of company profile/key/logo/unrelated files, and failed-installer recovery.
  The first harness attempt omitted its executable inputs and failed before any
  installation; its log remains, and the corrected complete test passed.
- Actual installed executable/hash, BUILD/source version, Windows Apps entry and
  responsive normal launch/close match. Existing company files and signing/update
  identities were preserved, previous files backed up, and no unsaved work lost.
- Private publication verifies the source archive, all67required source migrations,
  authenticated download bytes/hash/signature, ordinary Employee/public denial
  and the0.1.51staff feed. Immutable previous releases remain.

The database used about31.6MB and Storage213.5MB before the6.8MBupdate upload;
the existing free-capacity guards passed without billing changes. Security review
found no new warning-level database issue. The new ledger tables intentionally
deny direct access and use protected RPCs, so the informational no-policy notice
is expected. The existing [leaked-password protection advisory](https://supabase.com/docs/guides/auth/password-security#password-strength-and-leaked-password-protection)
remains unresolved under the free-only boundary; no paid setting was enabled.

No remote CI, Windows Authenticode, Office UI, real signed-in native file-dialog/
button automation, current portal browser/phone/email, agency-validator acceptance
or physical hardware verification is claimed. Hosted/API tests and fictional
native capture are distinct evidence. The separate running local portal is preserved.

| Artifact | SHA256 |
| --- | --- |
| Installed executable | `59ba2b52cefe5ad8e76f3cac6695529615e803bbac279e62c50f9707af07bcb2` |
| Setup | `8f31ce4463329691736b27860e1d553db543e732767d4a71e6ee17ac1ea3a64d` |
| Signed updater | `7a598ba7fcc5c37615f95eb4f44542224cbb9d128b4bccf40032f9fbdfa77f22` |
| Frozen source ZIP | `bf22aa62698937851caf4945521c320ce1daebc8dc82a705df85f9ad0ce87c0e` |
