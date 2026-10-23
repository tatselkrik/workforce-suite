-- Preserve the deployed payroll migration; combine equivalent SELECT predicates.
begin;
drop policy payroll_slips_hr_read on public.payroll_slips;
drop policy payroll_slips_own_read on public.payroll_slips;
create policy payroll_slips_read on public.payroll_slips for select to authenticated
using(hris_private.payroll_hr(company_id)or hris_private.payroll_own(company_id,employee_id));
commit;
