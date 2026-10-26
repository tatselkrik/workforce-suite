# BIR 7.4 offline test preparation — 27 October 2026

**Prepared; official application NOT RUN.** Desktop 0.1.58 remains the installed,
privately published release. This investigation adds no app release, SQL, rate
pack, filing, company registration or declaration. The official module/downloads
are already supplied; do not request them again.

## What is ready

The supplied NSIS payload was extracted as inert files, without running the
installer or modifying registry/security settings. The separate ignored
`work/bir074-local-validation` folder contains 436 members / 36,210,061 bytes,
including the validation executable and bundled VFP runtimes. All member sizes
and SHA256 values were verified after preparation. Stock DBF headers were checked
for bounded record layouts; their presence does not certify table contents or
current rules. No HRIS data was copied into this folder.

| Supplied/prepared item | SHA256 |
| --- | --- |
| Supplied installer payload | `6fb71a5a6ab541acc6094c6ab0b81871a2fe89b30d82fd3dc845bb1f8be00ef9` |
| Prepared `Alphalist Validation.exe` | `eaf082508178f2bd8f675ce7904f398d0bbb5e6d7b712553159cb464e5ed58d8` |
| Original `val-1604c.scx` | `46b85b156c1120001968ff2bc4837e739647b600adc04a25f7db1f8ba866c172` |
| Original `val-1604c.sct` | `ef4db55728dd4b883fdc1449fdeb834988b6a82b9b7eb9ae73e737107fa551fd` |

Windows signature inspection reports the prepared executable as **unsigned**.
Its generic PowerShell status message was not a launch test or an observed Windows
launch refusal. Neither government executable nor installer has been executed.
The native Windows controller failed initialization with kernel-assets OS error3;
no window control followed. Do not repeat unchanged initialization attempts.

## Static findings and limits

Bounded SCX/SCT decoding found 26 records. Memo blocks are one byte, consistent
with the documented format; memo headers use big-endian lengths/offset metadata.
See the [Microsoft-origin VFP memo format reference](https://www.vfphelp.com/help/_5WN12PC0N.htm).
Form record2 separately contains 90,067 readable METHODS bytes and 42,403 OBJCODE
bytes. Readable code is not proof of the instructions executed by the compiled app.

The readable Validate handler calls `val1604`, then `val1604CF`. In the extracted
record2 METHODS text, line67 initializes `control_on` to false; line1478 gates
control checks on that flag. No direct reenablement was found in that method.
This warrants a runtime probe; it does **not** prove a compiled-validator defect.
Unresolved compiled/global behavior and helper calls must remain explicit.

The earlier published/readable-method conflict remains: published C1 includes
PERA in column36, whereas a readable excess check starts there; published C2
includes PERA/net-basic in44/45, whereas its excess check starts at44. See
[the published-layout preflight contract](BIR-FILE-PREFLIGHT.md). Preserve the
native published columns; do not remove controls to imitate uncertain old code.

Readable handlers clear the module's local validation table, create reports,
create/mark/encrypt validated output and may rename selected inputs. A conditional
FTP path also exists. Its `DATA/LOVAL/JRCSVR.DBF` marker is absent in this prepared
stock workspace; this is not a complete network/security audit. Use disposable
fictional copies only and do not initiate any submission/network operation.

## Fictional probe matrix

Ten files are prepared under `work/bir059-runtime-probes`, each with a recorded
SHA256 and `official_result: NOT_RUN`. The frozen 0.1.58 native parser was executed
against all ten, and every original source hash was checked afterward.

| Cases | Change | Observed native result | Official runtime |
| --- | --- | --- | --- |
| `s1-baseline`, `s2-baseline` | Published standalone layout; zero money | Zero errors; S2 retains MWE basis review | NOT_RUN |
| `s1-control-mismatch`, `s2-control-mismatch` | First money control1.00 vs detail0.00 | Rejected: control mismatch | NOT_RUN |
| `s1-extra-control-column`, `s2-extra-control-column` | One extra control field | Rejected: column count | NOT_RUN |
| Four `mixed-*` cases | Details-first/schedule-block ordering; global/per-schedule sequence | Whole-file mixed schedules unsupported | NOT_RUN |

The synthetic baselines are **candidates**, not known official-valid fixtures.
If their identity/region/employment/zero-amount declarations fail official checks,
resolve the baseline first; unrelated errors cannot establish control behavior.
Later paired tests should include nonzero PERA/net-basic, missing controls,
boundary cents and wrong totals. Mixed ordering must not be inferred from a
single successful run or from a validator that ignores controls.

## First manual check

This is a local validation check, not a return. No actual employer/employee
identifiers, HRIS files, credentials, filing or payment are needed.

1. Open `work/bir074-local-validation/Alphalist Validation.exe` from its folder.
   If Windows presents a security barrier or required-component error, stop and
   record the message. Do not change security settings or run an installer.
2. In the validation module use fictional withholding-agent TIN `012345678`,
   branch `0000`, year `2026`, then choose **1604C**. The supplied 7.0 job aid
   describes Browse → Validate File → report directory; current 7.4 screen
   behavior is unverified.
3. Browse to the **operator copy** below, click Validate File, and choose that
   case's `Reports` folder. Retain the generated report even if validation fails.
   Close any report window, then repeat for the second baseline. Do not submit,
   upload, email, standardize filenames or overwrite another case's report.

Operator copies, with separate empty report folders:

- `work/bir059-runtime-inputs/s1-baseline/0123456780000123120261604C.DAT`
- `work/bir059-runtime-inputs/s2-baseline/0123456780000123120261604C.DAT`

Keep `work/bir059-runtime-probes` pristine. All ten cases have disposable copies
in `work/bir059-runtime-inputs`, but start with these two only. The app's generated
`eAlpha`/encrypted outputs may share a filename; preserve each distinct report
and original source hash before comparing cases. Do not treat generated output
as a file approved for filing.

## Evidence and next step

Ignored evidence: `bir059-form-structure`, `bir059-runtime-probes`,
`bir059-runtime-preparation-audit.json`, and the per-member
`bir074-local-validation/HRIS-PREPARATION.json`. The audit verified 436 members,
45 DBF headers, ten disposable-copy hashes and ten saved native outcomes.
The two baseline reports are the next input. Record official build/patch,
unchanged input hash, exact error text and outcome separately from the native
result. A fictional offline success still does not establish agency assignment,
2316 correctness, complete filing acceptance or current tax/rate rules.
