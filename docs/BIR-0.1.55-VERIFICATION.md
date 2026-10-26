# Desktop 0.1.55 — read-only BIR source preflight

**Installed and privately published.** Frozen source `d7a2b6bc6d2e72c3f5cb11cc023256a991c22b47`; previous0.1.54 is retained for rollback.
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

The normal optimized configured sign-in launched and closed normally. The exact
existing-key Ed25519 updater passes real silent update and failed-installer
recovery checks. Actual PC installation at 2026-10-26T12:13:21.7259709Z verifies
the exact executable hash, BUILD/source, Windows Apps registration and normal
launch/close. Company connection/branding, verification keys and unrelated files
are preserved; no unsaved work was discarded.

The authorized existing private channel returns the exact signed update; size,
hash and Rust signature verification pass. Employee/public download is denied,
the staff feed selects0.1.55 and all69required frozen-source migrations are
present. No SQL/schema deployment was repeated. Existing Auth leaked-password
warning remains; no Auth/billing/plan setting changed. Project storage before
publication was241,014,690bytes, candidate7,007,676bytes, database32,773,811bytes.
These are project headroom checks, not organization-wide billing/egress claims.

| Artifact | SHA256 |
| --- | --- |
| Installed executable | `bd1053cc1989aefc2f6184bb5a5df3fe44c6df3a8f0607fa36b90628d9621d27` |
| Full installer | `7822e259bf73d27fbffbfe7b979dc94e67be0720850f8d1f83b821d300daea41` |
| Signed updater | `3b918b1d52bafe8d5bd665b0655b3a9420d79c3f8901291cb627885f72be6a8f` |
| Frozen source ZIP | `1dff6b11342ce503faf81b874236d2af8cd43a60014788cbb7309995c6629079` |

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
