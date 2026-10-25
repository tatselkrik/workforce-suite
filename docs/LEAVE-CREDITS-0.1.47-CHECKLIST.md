# Desktop0.1.47 — credit adjustment checks

1. Open Leave > Credit adjustments. Select an employee and a finite leave type.
   Administrators can record adjustments; HR can review private history. Managers
   see scoped own/team credit facts and cannot post.
2. Enter a signed number of days, such as1,0.5or-0.5, today or a future date
   through next year, and a reason. This is a delta, not the total remaining
   balance or an opening-balance import. Future credit is not available early.
3. Post a synthetic adjustment. Its reason, reference, actor and date remain in
   history for HR/Admin. Own/Manager projections omit those private fields.
4. Prepare a full reversal with a reason. The original remains, only one reversal
   is accepted, and a reversal cannot be backdated into an earlier balance year.
5. Try a deduction greater than availability or against reserved leave. It must
   fail without saving an entry or advancing credit history. Later-year leave
   blocks a reduction that could change its carryover basis.
6. Refresh while a draft is open. Keep draft fields and the request identity until
   the matching saved entry is observed. A retry cannot post the same credit
   twice. Access changes clear private drafts.
7. The separate existing local portal's protected balance view includes applied
   credits. Older native clients keep sign-in/update access; they must update
   before submitting a leave preview that omits recorded credit deltas.

Policy effective dates and opening-balance imports remain queued in roadmap4.
This increment changes no payroll, cash payment, filed leave charges or approval
plans. It requires no paid provider and sends no external message.
