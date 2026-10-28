# Separate Payroll Desktop — delivered29October2026

Payroll Desktop0.1.0 and HRIS0.1.65 are installed on Kirk's PC. Both use the same
company Supabase and protected account responsibilities. Financial administration
now opens in Payroll Desktop; HRIS retains own published advice plus HR/company/
Account Manager work. Existing financial engines,24 source contracts,88 migrations,
issued slips, receipts, remittances, reports and histories remain intact.

Frozen source: `7b983b0b0ec59a7ca7eaa39e1702d5c525fb1043`. The preceding source
`00b214cf4177e352d6cd349590438bf939078091` passed the native/hosted/UI gates;
the sole subsequent source change corrects four NSIS dialog argument orders.
Rust inputs are identical. Exact final installer checks pass after that correction.

- 784 default native checks pass;31 explicit opt-ins excluded. New admission,
 worker, own-projection, profile, financial-action and128 sidebar geometry cases
 augment the existing financial regressions. No tax/rate rule was changed.
- 24 final fictional native views were actually inspected: all15 financial sections,
 Settings/Profile/sign-in, ordinary/compact/dark125 and HRIS own advice. Pre-auth
 sign-in is light. Rejected earlier horizontal-navigation captures are superseded.
- 28 hosted product/role combinations pass. Five financial combinations compare
 all24 source projections and ten protected reads. HRIS own-only advice, forged
 Auth metadata denial and existing-token revocation pass.115 prior public record
 sets and priorAuth facts are unchanged. One synthetic salary source is retained;
 its two QA actors are disabled/archived/session-free. Role runtime remains TRUE.
- Both normal configured builds launch/close. Payroll rejects the HRIS update
 entrypoint. Exact HRIS silent installation/company-file preservation and failed-
 setup recovery pass. Payroll isolated tests verify correct payload/preservation,
 reject HRIS/unknown namespaces and preserve actual registration.
- Actual installation verifies executable hashes, BUILD/source/app identifiers,
 Windows Apps versions and normal launch/close. Payroll shortcuts are separate;
 HRIS/company/logo/key/unknown files are preserved. No drafts were discarded.
- HRIS0.1.65 is signed with the existing Ed25519 identity and privately published
 to the existing authorized channel. Exact authenticated download/hash/signature,
 migration subset, staff feed, Employee/public denial and newer-release preservation
 pass within existing free capacity. Payroll remains LOCAL with no new channel/key.

## Exact installed artifacts

| Product | Executable SHA256 |
| --- | --- |
| HRIS0.1.65 | `c96bbd0e02b8a4cd8084c13b11b1d7cd78d415de810f4ad8fe060c4965596854` |
| Payroll0.1.0 | `6bb50d8fec75db7099525591663036247e1d5d27a96ccde484e8438612f95af6` |

See [testing checklist](PAYROLL-DESKTOP-0.1.0-CHECKLIST.md). Installed-user sign-in,
button clicks/drafts/manual scrolling are not claimed as automated. No publicCI,
Authenticode signature, new SQL/rate pack, agency acceptance, external connection,
transfer/filing, paid service or portal change is claimed. The separate portal0.1.7
is unchanged. Keep HRIS0.1.64 rollback and the local Payroll installer.

Evidence is retained privately under work/: payroll-desktop-native-full.log,
payroll-desktop-hosted-results.json, payroll-desktop-visual-review.json,
payroll-desktop-installer-results.json, payroll-desktop-hris-installer-test.log,
install-payroll100-actual/, install-payroll065-actual/ and
payroll065-publication-results.json. Frozen source/archive/packages are immutable.

Work and the SAME reset scheduler remain ACTIVE. Next, implement employee
relations, then employee services, expanded benefits administration and leadership
reporting. Earlier stop-after-Payroll notes are superseded by Kirk's latest request.
