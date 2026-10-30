# Government registration records

Payroll Officers and Administrators open **Payroll → Government → Registration details**, then the single
company record or a selected employee. Legal name parts, registered address and
assigned identifiers are recorded explicitly from supporting documents. They
never rename employment records, change company branding, create payroll
coverage, rewrite a payslip or confirm registration with an agency. Archived
employees retain their records. Managers and ordinary Employees cannot read
these private records, including through their own-record employee-app permissions.

Unknown fields remain blank. Spaces/dashes in identifiers are normalized while
leading zeros are retained. Shape checks cover BIR TIN/branch/RDO/region, SSS,
PhilHealth and employee Pag-IBIG MID. These are neither agency lookup nor checksum
validation. A main-branch code0000 must be entered explicitly; it is not a default
TIN. Unassigned all-zero identifiers are rejected. The employer Pag-IBIG number
accepts 1–20 digits as an internal identifier shape check. This is not certification of the agency upload format.
Birth dates must be valid and no later than the company-local date. The agency
sex code and employer category describe the supporting document; the category
does not switch this prototype's private-sector payroll rule pack.

Saving requires supporting reference, reason and an explicit review attestation.
The server checks current protected membership, company scope and exact revision.
Each operation has an actor-bound identity: an identical retry returns its
original receipt, even after a later edit, without creating a duplicate or
restoring old details. Reusing that identity for another payload is rejected.
Direct table writes are denied even to Administrator clients. Creation identity
and immutable before/after events are retained. Excessive history fails visibly
rather than silently truncating an audit trail. Private references, notes,
identifiers and actor identities do not enter the generic reconciliation CSV.

The desktop rechecks access before and after requests. A revoked response is
discarded; a failed refresh after a save retains the same operation for a safe
retry. Changing targets or navigating away cannot silently discard an edit.
Company, employee and editing views have separate scroll contexts.

Primary sources and export limitations remain in
[PAYROLL-GOVERNMENT.md](PAYROLL-GOVERNMENT.md). The BIR annual continuation,
SSS contribution list and PhilHealth RF1/ER1/PMRF were inspected for explicit
identity fields. The [2026-0050 PhilHealth advisory](https://www.philhealth.gov.ph/advisories/2026/PA2026-0050_v1.pdf)
requires approved EPRS4.9 access for the additional-payment process from1October2026;
an old RF1 layout alone is not proof of current upload acceptance. No agency
account access, employee-data upload, return filing or payment is included.
