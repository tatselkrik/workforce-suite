-- Equivalent SELECT scope in one policy, avoiding duplicate per-row checks.
begin;
drop policy payroll_cutoff_slips_hr_read on public.payroll_cutoff_slips;
drop policy payroll_cutoff_slips_own_read on public.payroll_cutoff_slips;
create policy payroll_cutoff_slips_read on public.payroll_cutoff_slips for select to authenticated
using(hris_private.payroll_hr(company_id)or hris_private.payroll_own(company_id,employee_id));
commit;
