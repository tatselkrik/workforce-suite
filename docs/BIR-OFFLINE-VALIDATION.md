# BIR module verification

The suite's local preflight checks and the official Alphalist validation module
are different validation layers. Internal structure, declarations, source
comparison and monetary controls do not establish official acceptance.

## Preparing a validation case

Use an applicable official module and patch, the matching published schedule,
and fictional test data. Keep an unchanged original input and record its hash.
Run each case in a separate working copy so generated outputs with the same
filename cannot replace another case's report.

Record the module build and patch, exact input, result, errors and generated
report separately from Workforce's preflight result. A known baseline and a
deliberate invalid variant help distinguish successful acceptance from a file
that was never actually read.

## Interpreting results

Keep unexpected findings literal. Do not modify expected withholding, declarations
or source records simply to obtain a successful result. Reference methods, form
instructions and a validator result must each be identified precisely when they
disagree. Preserve original files and reports when investigating a discrepancy.

Official-module acceptance has not been established by the suite's source tests.
Even a successful fictional case does not establish real-company registration,
complete filing acceptance, official certificates or correctness for every tax case.
See [format references](GOVERNMENT-FILE-REVIEW.md),
[local preflight](BIR-FILE-PREFLIGHT.md) and
[declaration arithmetic](BIR-DECLARATION-ARITHMETIC.md).
