# Desktop 0.1.56 — standalone minimum-wage file checks

Installed/private delivery is verified in [the evidence note](BIR-0.1.56-VERIFICATION.md).

1. Open **Payroll → Government → Reconciliation → BIR → Check BIR 1604C file**.
   Choose an unvalidated ASCII file containing only Schedule 2: H1604C,
   59-column D2 details and a 45-column C2 control. Schedule 1 remains supported.
2. Confirm the detected schedule, source hash, year and detail count. Even when
   all columns/totals agree, **MWE declarations remain unverified review items**.
   This check never determines minimum-wage eligibility or a legal wage floor.
3. Expand **Exact file control totals**, scroll to the bottom, and check PERA
   and **Statutory basic net of contributions/dues**. The 40 monetary controls
   retain exact centavos. The days/year factor must not appear as money.
4. A blank amount shows an unavailable detail total; a one-cent control
   difference remains visible. A blank, fractional or oversized factor is a
   shape error. A correctly shaped factor remains an unverified declaration.
5. Save findings as CSV and Excel to new files. Recheck after source changes.
   Names, identifiers and compensation values must stay out of findings.
6. A mixed D1/D2 file is refused whole-file, rather than partially passed.
   Existing source/export files and company registration must stay unchanged.
7. Check the compact and dark views; Manager/Employee access remains denied.

Use fictional samples. Do not change company registration to match a fictional
employer or submit these samples to BIR. Official module acceptance, tax/wage
calculations, 2316 certification, HRIS payroll comparison and filing are separate.
