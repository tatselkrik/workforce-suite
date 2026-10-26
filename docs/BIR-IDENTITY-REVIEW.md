# Current BIR file identity review

This local HR/Admin review matches a structurally consistent standalone 1604C
Schedule 1 or 2 file to **current registered TIN/branch pairs**. It does not match
by names, certify historical identity/agency assignment, compute expected tax,
compare 2316 certificates, establish required file coverage or validate filing.
It never changes the file, employee, registration, payroll or remittance records.

## Server projection

`government_bir_identities(year)` returns the protected annual ledger and a
bounded current/archived employee roster. Each person has only the current
code/name label and optional BIR registration ID, company/employee binding,
revision, TIN and branch. The employer binding contains the same selected BIR
fields. Other agencies, legal/contact fields, references, notes and registration
history are omitted. Unknown/unassigned fields remain blank.

Both new wrappers are security invoker with an empty search path. The existing
annual source guard requires current active HR/Admin membership; existing
employee/registration SELECT grants and RLS remain intact. No new table, broad
permission or security-definer function is introduced. Reject more than 20,000
roster records without truncation. Native decoding independently checks the
version, company, supported year, canonical bindings/revisions, unique people/
codes/registration IDs and exact same-statement annual source labels.

## Local matching and exports

The source stays local. Only the selected year enters the RPC. Reuse the existing
bounded ASCII preflight, exact 31/40 money controls and physical line numbers.
Require a known matching company BIR registration, the selected year and no
structure/control errors before identity review. Existing MWE/encoding review
items remain unresolved. Mixed schedules and unsupported payroll-source years
remain outside this review.

Match a complete non-placeholder TIN plus four-digit branch to exactly one
current person. Namesakes remain separate. Multiple current registrations using
one pair produce **ambiguous**; no match produces **unmatched**. Neither selects
an employee, guesses by name or substitutes a zero count. A unique match shows
its current label, distinct observed annual source count and actual receipt
source count. These are record counts, not compensation or tax comparisons.

Report current receipt-source employees not uniquely matched in the file as a
coverage review count. This does not prove a missing legally required declaration.
Archived records remain available for historical review; the roster total is not
active headcount or historical registration certification.

Fresh CSV/Excel exports reread the unchanged source SHA256 and fetch current
registrations again. Recheck current access before the RPC and after rendering,
immediately before new-file IO. Never overwrite an existing destination. The
internal identity worksheet contains source hash/year/snapshot, physical line,
review status, matched current code/name and source/candidate counts. It omits
raw tax identifiers, file-declared names, compensation and HR evidence. Literal
Excel text and existing CSV formula-prefix protection preserve codes/Unicode.
The original minimal BIR diagnostic export retains its separate privacy contract.

## Verification and deployment limits

The current prototype employer registration is incomplete. Hosted native tests
verify the real protected projection and that missing setup prevents exports;
they do not seed or alter employer facts to obtain a positive. Full configured-
company file matching remains a manual/live verification limit. Fictional model,
real local HTTP and independent native Excel cases cover positive matching,
namesakes, ambiguity, absent sources, incomplete registration and MWE review.

Hosted QA uses only a short-lived synthetic session against the installed
profile, pinned to the exact private project authorization and linked target.
Reject redirects. Use a fixed project-local evidence folder and persist only
aggregate proof, never hosted identity snapshots. Environment variables must not
select an arbitrary network destination or output directory.

Actual official-module/filing acceptance remains unverified. The supplied BIR7.4
module/readable-method conflicts and published PERA controls retain their existing
status in [BIR-FILE-PREFLIGHT.md](BIR-FILE-PREFLIGHT.md).
