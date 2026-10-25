# Native Excel0.1.50 — candidate verification

Native implementation is verified; packaging, actual installation and private
publication are the remaining release gates. Installed0.1.49 remains the delivered
baseline until those finish. Full roadmap6 remains open for attendance/leave
request batch imports.

-522default Rust checks pass, six opt-ins remain ignored, zero compiler warnings.
  Sixteen new Excel behavior/security/integration checks cover native types,
  literal text, actual row numbers, existing protected opening batches, manager
  scope, file preservation and adversarial workbook refusal.
- An independent openpyxl3.1.5 reader verified eight fictional native workbooks:
  exact rows/columns/headers,693native dates,991numeric cells, literal identifiers/
  Unicode/multiline/formula-like text, freeze panes, filter/header styles and no
  formulas or structural warnings. This is not Microsoft Excel UI automation.
- Four actual fictional native layouts were inspected, including normal import
  controls, compact dark view, opening import and report Excel export controls.
  Normal scrolling is required for rows below the viewport. A final history-copy
  correction does not change the inspected layout.
- The two direct libraries and twelve new dependency packages are pinned. Full
  upstream free-license notices are retained in EXCEL-DEPENDENCY-NOTICES.md and
  must accompany the installation. No Office subscription/conversion service.
- No schema migration/product database mutation is introduced. Schema66 and its
  previously delivered0.1.49 hosted permission/workflow evidence remain the
  baseline; this increment does not claim a new hosted SQL workflow test.

Remaining gates: source/binary/archive privacy, normal release startup, exact
signed installer/recovery, installed hash/BUILD/Windows Apps/normal launch and
guarded private authenticated download/signature/hash/access verification.
No remote CI, Authenticode, actual signed-in native button/file-dialog automation,
current portal browser/phone/email success, agency acceptance or device claim.
