# Desktop 0.1.56 — standalone Schedule 2 preflight

**Native implementation verified; installation/private delivery pending.**
This client-only extension shares the local hash, permission and diagnostic-
export guards between published Schedule 1 and standalone Schedule 2. It adds
no SQL, payroll calculation, wage rate, dependency or paid service.

- **601 default Rust checks** pass, with 14 intentional opt-ins excluded and no
  warnings. Six new default cases cover published D2/C2 boundary mappings,
  PERA/net-basic controls, factor exclusion/shape, unknown amounts, exact one-cent
  differences, date/code positions, whole-file mixed-schedule denial and real HTTP
  client CSV/Excel handling. Existing Schedule 1/access/revocation cases pass.
- One explicit MWE native vector run and two actual hosted native role runs pass.
  Independent Python checks of eight findings workbooks / 21 data rows verify
  original hashes, all 40 control mappings, CSV parity, literal strings and no
  raw employee names/identifiers/amounts, formulas, links or macros.
- Actual hosted normal HR/Admin access, Manager/Employee/forged-metadata denial
  and existing-token revocation pass. All 35 prior payroll/government sets and
  prior Auth users remain unchanged; synthetic actors are disabled/archived with
  sessions ended. Sources remain local and unchanged.

The published Schedule 2 layout is 59 D2 columns / 45 C2 columns, with 40 monetary
controls. The days/year factor is not money. MWE eligibility, reported statutory
wages and the factor basis always remain unverified review items. No wage-floor,
statutory-deduction or tax calculation is performed.

Ten actual fictional native views were inspected: Schedule 1 summary/controls,
MWE summary/findings/control top/bottom, compact findings/controls, dark125percent
and Manager exclusion. The exact normal release/archive/signature/installer/
recovery, actual installation and private-channel delivery are recorded upon
completion. Neither code nor package existence alone establishes those checks.

Mixed schedules, other encodings and sealed/validated files remain unsupported.
Current registration matching is not identity/historical certification. Official
BIR compiled-module/filing acceptance remains unverified. A supplied readable
method conflicts with published C1 PERA and C2 PERA/net-basic controls; static
inspection is not executed-validator evidence. No government program has run.
Schema 69 remains unchanged; full database tests/replay are not repeated for
this local extension. Government milestone 2 remains partial. Actual signed-in
native file-dialog interaction, Office UI, remote CI and Authenticode remain
unverified; portal, paid/external-provider and hardware boundaries remain.
