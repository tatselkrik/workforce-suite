# Portal 0.1.7 - private local delivery

The repaired portal runs at **http://localhost:3032** on this PC, listening only
on `127.0.0.1`. The older 0.1.6 server on `127.0.0.1:3031` remains running so its
drafts and sessions are not discarded. Save those drafts before moving to 0.1.7.
The two hostnames keep their browser cookies separate; ports alone would not.

Application source: `8a68a6c045192f86828825e04c6cf2ddd3c74856`.
The source was clean at the frozen optimized build. Later documentation records
delivery without changing the application or this immutable package.

| Artifact | SHA-256 |
| --- | --- |
| Packaged `HRIS-Employee-Portal.exe` | `4fa7507f12797ff97c1747b5c00c68de1a287ff6143074b2275b4170c9a8267f` |
| Private 0.1.7 ZIP | `53f87ebe7fdf5be58b5d2f28ede2074a066af383dd67d0bf8ba28c497c9c9fed` |
| Source 0.1.7 ZIP | `4d03915ad09428829ab9221b8745bb2147c74ed0d1756e35a08147ec2b2f713a` |

The final executable serves all eight frozen web assets byte-for-byte. Its
43 real hosted browser checks pass, including narrow navigation/hover, own-only
Employee/Manager data, contact/emergency edits, photo/draft protection and
account/session/logout behavior. All 35 prior payroll/government record sets
and prior Auth users match. Synthetic accounts remain disabled/archived, their
sessions ended; no personal password or email was changed.

The independent fictional browser matrix passes 22 groups at six widths in both
themes, covering populated attendance/leave history, inbox and payslip layouts.
CSV and light/dark single-page PDFs retain exact amounts, Unicode employee name,
literal code and pay month. PDF text and rendered pages were checked. These
fixtures do not establish live payroll calculations or new approval writes.
See [the scoped verification](PORTAL-0.1.7-VERIFICATION.md).

The exact private package starts normally with its configured connection and
branding. Sign-in/current CSS load, anonymous private APIs return 401, and both
retained servers listen only on loopback. Installed desktop 0.1.60 and all its
top-level files remain byte-identical. There is no desktop installer/update,
schema/rate change, remote publication or public CI claim for this portal release.

The private/source ZIPs, updated guide, build manifest and delivery receipt are
copied to the existing local outputs folder with exact hash verification.
`BUILD.json` records the **pre-delivery package** state; the separate delivery
receipt establishes copied files and normal launch afterward. The generic guide
inside the frozen package retains dated historical review text; `START-HERE.txt`
and this record describe the 0.1.7 result. Frozen older artifacts are preserved.

## What to check

1. Open the new local address and sign in with your normal HRIS account. In this
   portal, staff roles still see only their own records.
2. At a narrow browser width, all five navigation tabs fit and the selected tab
   stays readable. The profile photo opens My profile.
3. Contact/emergency Save and Discard, and the guard for an unsaved draft.
4. Your own available attendance/leave history and notification inbox. Empty
   real accounts can legitimately have no requests, notices or issued slips.
5. For an available monthly payslip, Download CSV and Print / save PDF. The
   pay month and exact totals should remain on the printed statement.

Physical-phone behavior, actual email confirmation and the operating-system
printer dialog remain unverified/deferred. The configured company has no logo;
populated logo pixels were checked with a fictional fixture. The browser-only
portal milestone is complete within Kirk's narrowed scope. Public hosting,
external delivery, unresolved vendor connections and actual devices stay deferred.

Ignored exact evidence: `work/portal017-final-live/`,
`work/portal017-fixture-ready/`, `work/portal017-frozen-release.log`, and
`work/portal-deployment-0.1.7/{package-result,normal-start,delivery-result}.json`.
