# Employee portal browser review —27October2026

Reviewed the verified private **portal0.1.6** package using bundled Playwright and
the already installed Microsoft Edge156.0.4314.42 in a separate ephemeral
headless browser. These are actual browser renders/interactions, unlike the
older JavaScript VM tests. The interactive computer-use runtimes still fail
before initialization with kernel-assets error3; the headless test succeeded.

No portal was running at the start. The existing package was hash-checked and
started on loopback3031; it remains available locally after review. Existing
browser profiles were not used. No desktop release, portal application code,
deployment, Auth configuration or schema was changed.

## Confirmed findings

1. **At320px the fixed bottom navigation clips My profile.** The five buttons
   each have a70px minimum width, plus navigation padding, exceeding the available
   width. This reproduces across Home, Attendance, Leave, Payslips, My profile
   and Notifications. The fixed child extends outside the viewport even though
   document scroll width remains320px. Source: `employee-portal/web/app.css`,
   the mobile navigation rule on line10. Wider390px rendering fits.
2. **The selected tab loses contrast on hover at mobile widths.** The selected
   button retains white text, while the later mobile hover rule changes its
   background to pale mint. Actual computed colors were white255/255/255 over
   mint230/245/241. This is visible after a click/hover in the browser, including
   Home and Leave. Source: the same mobile hover rule, combined with the earlier
   selected-tab text rule. Physical-phone touch hover behavior was not tested.

These are unresolved application findings. No CSS fix or new portal release was
made during this review. The final run deliberately remains **not all passed**;
six narrow-layout cases represent the first underlying issue, not six distinct
bugs. The second issue was confirmed by screenshot and computed styles.

## Browser checks completed

The final run records36successful checks and17screenshots, with the complete
functional sequence reaching its final assertion. Checked desktop1440px,
mobile-width390px and320px, and dark-mode profile views. Actual synthetic
Employee and Manager sessions use the normal local portal sign-in.

- Enter submits sign-in; password fields clear afterward.
- Minimal own employee data and five own-record navigation entries load for both
  roles. Read-only employment fields remain outside the contact form.
- Contact/emergency Save and Discard work. Unsaved contact/photo drafts prevent
  leaving the profile page.
- A synthetic transparent PNG saves through the actual private photo path and
  renders in the circular profile/sidebar. The sidebar image opens My profile.
- Mismatched new passwords are rejected; attempted-password fields clear, and
  explicit account-draft discard works. No password or sign-in email was changed.
- Sign out other sessions closes a second own portal cookie while retaining the
  current session. Normal sign-out clears private rendered fields and the photo.
- Home, Attendance, Leave, Payslips, Profile and the inbox render. The leave form
  shows the normal balance/charge preview. No uncaught JavaScript errors or
  unexpected external browser requests were observed.
- Session cookies are HttpOnly/SameSite Strict; local/session storage stay empty.

The target-guarded synthetic runner preserved all35prior payroll/government table
sets and prior Auth users. Test accounts remain retained but disabled/archived,
with upstream sessions ended. Real employees, passwords and payroll were not
changed; credentials were kept in child-process memory, not saved to evidence.

## Limits and evidence

Physical-phone behavior and actual email confirmation remain unverified. The
company had no logo to display, so its current name/initials fallback was checked;
this does not establish a populated company-logo visual. New synthetic accounts
had no issued slips, filed requests or notifications: their empty states were
viewed, but populated detail/history/card layouts and browser print/download
were not exercised here. Clocking, breaks and leave/correction/overtime writes
were not submitted in this browser review. Earlier API checks are separate proof.

Ignored evidence: `work/portal-browser-20261027/run-3/browser-results.json`,
`live-results.json` and the17PNGs. The earlier harness-shape failure and the
first320px failure remain preserved in prior attempt folders. The final runner
continued to collect other observations while keeping its overall failure state;
it did not skip findings or substitute an old proof. The portal milestone stays
partial pending the confirmed UI fixes and remaining recorded coverage.
