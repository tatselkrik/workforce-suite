# Desktop 0.1.55 — read-only BIR source preflight

**Native implementation verified; installation/private delivery pending.**
This increment adds local published-structure checks for unvalidated1604C
Schedule 1 plaintext files and private CSV/Excel diagnostic exports. It does not
generate an agency return from payroll or establish official validator acceptance.
See [the testing checklist](BIR-0.1.55-CHECKLIST.md) and
[the exact supported contract](BIR-FILE-PREFLIGHT.md).

## Meaningful verification

- **595 default Rust checks** pass, with 13 intentional opt-ins excluded.21 new default
  cases cover the 31-column monetary mapping, exact centavos/one-cent differences,
  invalid/unknown amounts, dates/codes/widths/record order, duplicate identifiers
  versus namesakes, byte hashes, bounds, physical lines, diagnostics caps and
  unchanged source files. Private view state follows current company/account/role.
- Real localhost HTTP requests exercise the production client: normal source
  inspection and CSV/Excel exports; revocation before read, after registration
  and immediately before file IO; stale source hashes; foreign registration
  envelopes; existing-file preservation; non-HR/unchecked-export denial before
  network or source access. Requests contain no selected-file content.
- One explicit fictional vector run and two actual hosted native role runs pass.
  Independent Python reads of 8 native Excel files / 13 data rows verify CSV parity,
  original source hashes, exact 31-control mapping, literal strings and absence
  of raw employee names/identifiers/compensation, formulas, external links or macros.
- Actual normal hosted HR/Admin sign-ins run the native check and both findings
  exporters. Manager/Employee denial, forged metadata denial and existing-token
  revocation pass. 35 prior payroll/government record sets and all preexisting Auth
  users are unchanged. Synthetic actors are disabled/archived with sessions ended.
- Nine fictional native layouts cover entry/summary/findings, expanded controls
  through PERA, compact size, dark 125 percent and Manager exclusion. Visual review
  caught and corrected a first-column vertical offset before delivery.

No SQL, rule pack or dependency changed. Schema 69 remains current; the full
database suite/replay was not repeated for this client-only change. Official
reference archives stayed untouched; no BIR installer/program was executed.

## Delivery boundary and limitations

The normal optimized binary, frozen source archive, existing-key signed updater,
exact installer/recovery, actual installed hash/BUILD/Windows Apps/normal launch
and private authenticated download are verified separately before delivery is
called complete. Current source is not yet that delivered release.

Actual signed-in native file-dialog interaction, Office UI, remote CI,
Authenticode and official BIR validator/filing acceptance are unverified.
Supported sources are ASCII, 2023 through the current company year, Schedule 1,
at most 2 MiB / 500 details. Other encodings/schedules/validated or sealed formats
fail explicitly. Employer matching compares current registered values only;
missing registration stays unresolved. No identity, eligibility, historical
registration, tax calculation,2316 certification or HRIS-payroll comparison claim.

The published C1 PERA column and one readable bundled method conflict. Following
the published layout is not proof of compiled-module behavior. All government
milestone 2 acceptance work remains partial; SSS remains internal review. The
portal and paid/external/hardware work retain their existing separate boundaries.
