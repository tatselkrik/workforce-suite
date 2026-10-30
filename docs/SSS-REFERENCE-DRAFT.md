# SSS reference draft

In Workforce Payroll, select **Payroll → Government → Prepare SSS R3 reference
draft** after generating an SSS month. It uses the existing protected preparation
projection; no new schema, contribution rates or issued payroll values change.


Preparation requires complete reconciled contributions, known employer/EC shares,
legal/assigned SSS registrations, and one active full-month receipt matching the
known total. Split/multi-month receipts and zero/no-earnings people need separately
reviewed handling. A TR/SBR exceeding the reference's ten-character field is
blocked; it is never silently shortened or substituted for another identifier.

Employee identifiers, contribution amounts and receipt facts are fixed from the
fresh source. File name representations, middle initials and Normal/New hire/
Separated declarations are explicit HR inputs. Same-month event dates, printable
ASCII and fixed field widths are enforced without truncation/transliteration.
Namesakes remain separate. These declarations do not alter canonical employee,
registration, separation or payroll records.

A fingerprint binds the form to company/month/as-of date, effective source facts,
registration revisions/fields and the immutable selected receipt. A volatile
response timestamp alone does not invalidate it. Every export reloads the single
protected projection, compares the source binding, renders exact cents, rechecks
current protected membership, then exclusively creates a new `.txt` file. Existing
files are preserved. Missing/stale/revoked data blocks file creation.

Checking sources retains unsaved names/declarations; replacing the form requires
the explicit discard/load action. Edits reset the review attestation. Company,
account or protected-role changes clear private forms. Navigation/update guards
treat an open form as unsaved work.

This is a reference-format draft for HR review. Twelve fictional month vectors
match inspected DTO/string methods of the currently downloadable official R3
package. Current My.SSS acceptance, reserved-slot semantics, broader encoding,
quarterly/split-payment cases and no-earnings declarations remain unverified.
No legacy rate validator, agency login, submission or money transfer is invoked.
For the distinction between internal preparation and agency acceptance, see
[format review](GOVERNMENT-FILE-REVIEW.md).
