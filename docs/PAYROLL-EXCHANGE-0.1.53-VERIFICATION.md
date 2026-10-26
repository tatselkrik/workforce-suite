# HRIS Desktop0.1.53 — local exchange verification

The implementation is locally verified. Guarded deployment, live checks,
packaging, actual installation and private publication are the remaining delivery
gates at this source-freeze stage. Installed0.1.52 is retained until they pass.

-560default native checks pass;9explicit opt-ins are excluded from that count.
  These include14exchange cases and5UI draft/recovery/privacy cases. No warnings.
-215new PostgreSQL exchange assertions and7,429combined schema/workflow/
  preservation assertions pass, with fresh69migration replay. Existing functions
  and prior records are preserved. The additive migration is
  `20261026025110_payroll_exchange.sql`; no payroll calculation function is changed.
-The native opt-in validates two actual local PostgreSQL captures:41complete
  records across all six source kinds, and a1,000-row escaping/bounds fixture.
  An independent Python CSV reader verifies four frozen/preview snapshots and
  1,123complete rows, including headers, controls, unknown blanks, centavos and hashes.
-Seven fictional native layouts were inspected, including compact dark
  reconciliation, immutable export history, interrupted submission and Manager
  exclusion. Subsequent draft-guard refinements have focused native checks.
-Source privacy review covers deployment-profile values, secret patterns and
  private addresses. No new dependency, paid provider or external connection is added.

The wire checks verify native report/CSV/hash/receipt bindings; they do not certify
vendor acceptance or accounting posting. Local boundary generators are restored
exactly and provide no new Philippine financial-rule evidence. One legacy fixture
has no saved employee labels; unfiltered exchange now fails explicitly rather
than replacing its identity or dropping it. The explicit employee-code scope
retains all six complete source types.

Earlier evidence includes an incomplete access-capture fixture, a temporary
Windows QA-executable lock and a corrected legacy-label SQL alias. Their failed
logs are retained in ignored work evidence. None was a hosted payroll mutation.

No remote CI/Authenticode, actual signed-in file-dialog/Office UI, vendor delivery,
agency validator, current portal/phone/email or hardware verification is claimed.
Delivery evidence will record the exact frozen source, artifact hashes, guarded
remote preservation, real installer/recovery, installed BUILD/Apps/launch and
private authenticated download/signature checks when those operations complete.
