# Version 0.1.43 review checklist

The registration increment is installed and verified locally. See
[release evidence](GOVERNMENT-0.1.43-VERIFICATION.md). Use fictional employee
records for testing.

1. Open **Payroll → Government → Registration details** as HR/Admin. Switch
   between Company registration and Employee registration, then load a record.
2. Enter legal name parts explicitly. Check registered addresses and identifiers;
   unknown values stay blank and leading zeros remain. These fields must not
   rename Employees or the company logo/name in Settings.
3. Choose **Edit registration**. Add a supporting reference, reason and review
   attestation, then save. Reopen and check the retained registration history.
4. Edit a saved record again. Its revision should increase and earlier snapshots
   must remain. A stale form should require a refresh without losing its input.
5. Check that invalid dates, wrong identifier lengths, all-zero unassigned IDs
   and missing supporting review are rejected. A recorded main branch0000 is
   explicit; it must not fill a missing TIN.
6. Navigate while editing and verify unsaved-change protection. Inspect the
   form's final save area and history at your usual scale/theme.
7. Managers and ordinary Employees must not see or read these HR-private records,
   including the ordinary Employee's own registration. Sign out to clear data.
8. Existing government reconciliation and privacy-limited CSV must still work;
   private identifiers/references/notes must not enter that generic worksheet.

Next: exact agency file adapters after complete specification/validator checks.
Registration shape validation is not agency verification, filing or payment.
