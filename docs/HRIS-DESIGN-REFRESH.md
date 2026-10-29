# HRIS design refresh — approved30October2026

Kirk chose the recommendation from [the comparison board](design/hris-style-directions-2026-10-30.png): D Teal hybrid light, with C Focused charcoal as its dark theme. Apply to HRIS Desktop first. Native Rust/egui remains; no web frontend or new module. Shared code must preserve separate Payroll appearance/behavior and all existing releases. Android011/PCportal018 and sessions remain intact.

## Direction and boundaries

Use soft off-white content, pale teal navigation selection, rich restrained teal accents, dark ink typography, thin borders and moderate card rounding for light. Dark uses charcoal/navy surfaces, slightly lifted panels, off-white text and quiet teal/indigo emphasis. Company circular logo/name stays top left, current-role navigation remains scoped, Account Manager before Settings when allowed, circular profile anchored bottom left. Keep version/update notices and current theme preferences.

The board is conceptual: its500employee totals, chart and names are fictional. Restyle/reorganize actual existing scoped facts only; never add fake analytics or reinterpret missing attendance as absence. Preserve existing workflows, membership/RLS, approval/history, drafts, employment data and optional-module controls. No financial/rate/schema changes expected. No Payroll tab reintroduced in HRIS.

Verify isolated fictional native/headless renders at normal/compact widths and relevant scales in both themes. Check contrast, text/row alignment, navigation/profile reachability, forms/tables, dialogs, long labels, notices and sign-in. Use targeted existing tests and checks; do not control Kirk's PC windows/browser. No paid fonts/services or unnecessary dependencies. Preserve compact caches and existing files.

## Delivery

Work is starting from clean4e55faa. HRIS069, Payroll011, Android011 and portal018 are prior delivered baselines. New verified HRIS delivery must preserve connection/branding/keys/rollback and be actually installed with hash/BUILD/Windows Apps/normal-launch proof. Same private HRIS publication retains its exact guard. Do not overwrite immutable releases or discard drafts. Same reset heartbeat is active under the new scope; current checkpoint is authoritative for partial progress. Pause on verified completion, newer human pause or genuine handoff.


## Implemented candidate0.1.70

Centralized HRIS-only light/dark tokens, installed Windows Segoe UI typography with bundled egui fallback, native vector navigation icons, circular company/profile treatment, responsive toolbar and Overview cards/table. No font files or new runtime dependencies are redistributed. Separate Payroll retains its legacy full theme/spacing and financial behavior.

23fictional native views were actually reviewed, covering normal/compact widths,100/125percent, light/dark, employee edit, attendance/leave, settings/profile, hiring/development, cases/services/benefits, reports/leadership, Manager scope and sign-in. Capture sidecars verify actual theme/zoom. The Account Manager visual is its demo protected-fetch error state; editor/permissions have automated coverage.872default Rust checks across67suites pass with37intentional opt-ins ignored, including81focused UI checks. Exact frozen-source release/installer/install/publication checks follow separately; this candidate note is not delivery proof. See HRIS-0.1.70-REVIEW.md for Kirk's visual checklist.
