# Local portal 0.1.7 browser fixes

## Changes

The mobile navigation now gives all five tabs an equal share of the available
width, including 320px screens. The selected tab retains readable white text on
teal while hovered. This resolves both findings from the
[0.1.6 browser review](PORTAL-BROWSER-20261027-REVIEW.md).

Browser-print review also found that the general print rule hid the payslip's
pay month. Printing now retains that month, uses a white page in either theme,
and uses standard printable numerals. Company, employee code/name, components,
exact amounts and the payment disclaimer remain. Navigation and action buttons
stay out of the printout. This does not change payroll calculations.

## Verification before packaging

- 48 Rust tests and 15 existing JavaScript checks pass. The deliberately ignored
  fictional preview-server test remains ignored; the normal executable contains
  no preview endpoint. Dependencies and server/Auth behavior are unchanged.
- The real Edge browser fixture passes 22 check groups against the actual web
  files. Six widths (320, 360, 390, 640, 1024 and 1440px) in light/dark mode cover
  tab containment, minimum 44px targets, hover contrast and keyboard focus.
- Fictional populated screens exercise breaks/checkout, correction submission,
  cancellation and decision history, partial overtime history, half-day leave
  estimates/filing/cancellation, payslip detail and notification read/reset links.
  These in-memory fixtures establish browser behavior, not hosted database writes
  or financial correctness. A fictional company logo is also rendered.
- An actual browser CSV download preserves the UTF-8 name, literal `000045`
  employee code and exact `22091.34` net. The print button dispatches to printing;
  light/dark Chromium PDFs each fit one A4 page. Independent PDF text checks retain
  month/name/code/amounts, exclude controls, and rendered pages were inspected.
  The operating system's printer dialog and a physical printer were not tested.
- The initial optimized binary passes 43 real hosted browser checks with new
  synthetic Employee/Manager accounts. Sign-in with Enter, own-record scope,
  contact/emergency Save/Discard, circular private photo, draft protection,
  rejected password mismatch, other-session logout and final sign-out pass.
  Cookies remain HttpOnly/SameSite Strict; browser storage stays empty. There
  are no uncaught script errors or unexpected external browser requests.
  The 35 prior payroll/government record sets and all prior Auth users match;
  synthetic accounts are retained disabled/archived with sessions ended.

The final print-only CSS update follows that first hosted run. Exact final-binary
regression, frozen-source/package checks and local delivery must be recorded in
the separate delivery record before this release is called delivered. No schema,
rate pack, desktop release or Auth confirmation setting changed. The database
suite was not rerun for this browser-only change.

## Evidence and limits

Ignored local evidence: `work/portal017-rust.log`, `work/portal017-js.log`,
`work/portal017-fixture-ready/` and `work/portal017-live-browser/`.
Earlier fixture attempts are preserved, including test-harness encoding/focus
failures and the print-month/dark-page findings; they are not passing evidence.
`tools/portal-browser.test.cjs` is the portable fictional browser runner and
requires available Playwright plus installed Edge. It serves only loopback and
does not use real credentials or Supabase.

The old 0.1.6 server and its possible drafts/sessions remain on `127.0.0.1:3031`.
The new package will use `localhost:3032`, bound to `127.0.0.1`. Using a different
hostname avoids replacing the old host-scoped session cookie; a different port
alone would not separate browser cookies. Neither address exposes the portal
to a phone, LAN or the public internet.

Physical-phone behavior and actual email confirmation remain separately
deferred. Actual company-logo pixels are not established by the hosted company,
which has no logo; the populated logo view is fictional. No external sending,
government software execution/filing, vendor connection, billing or public
publication occurs in this release.
