-- Read-only issued pay components beside existing receipt-year facts.
-- No tax mapping, current salary projection, receipt or source mutation.
begin;
create function hris_private.government_annual_pay_details(p_year integer) returns jsonb
language plpgsql stable security invoker set search_path='' as $$
declare ledger jsonb;cid uuid;details jsonb;
begin
 -- Reuse the existing current HR/Admin guard, bounds and immutable source set.
 ledger:=hris_private.government_annual_ledger(p_year);cid:=(ledger->>'company_id')::uuid;
 with entries as materialized(
  select j->'fact' f from pg_catalog.jsonb_array_elements(ledger->'entries')j
 ),sources as(
  select e.f,case e.f->>'kind'
   when'monthly'then m.id when'cutoff'then c.id when'thirteenth'then t.id
   when'amendment'then a.id when'final_adjustment'then x.id when'annual_tax'then y.id end source_id,
   case e.f->>'kind'when'monthly'then m.item->'totals'when'cutoff'then c.item->'totals'
   when'thirteenth'then t.item->'totals'when'amendment'then a.totals
   when'final_adjustment'then x.totals else null end totals
  from entries e
  left join public.payroll_slips m on e.f->>'kind'='monthly'and m.id=(e.f->>'id')::uuid
   and m.company_id=cid and m.employee_id=(e.f->>'employee_id')::uuid
  left join public.payroll_cutoff_slips c on e.f->>'kind'='cutoff'and c.id=(e.f->>'id')::uuid
   and c.company_id=cid and c.employee_id=(e.f->>'employee_id')::uuid
  left join public.payroll_thirteenth_slips t on e.f->>'kind'='thirteenth'and t.id=(e.f->>'id')::uuid
   and t.company_id=cid and t.employee_id=(e.f->>'employee_id')::uuid
  left join public.payroll_amendment_slips a on e.f->>'kind'='amendment'and a.id=(e.f->>'id')::uuid
   and a.company_id=cid and a.employee_id=(e.f->>'employee_id')::uuid
  left join public.payroll_final_slips x on e.f->>'kind'='final_adjustment'and x.id=(e.f->>'id')::uuid
   and x.company_id=cid and x.employee_id=(e.f->>'employee_id')::uuid
  left join public.payroll_tax_slips y on e.f->>'kind'='annual_tax'and y.id=(e.f->>'id')::uuid
   and y.company_id=cid and y.employee_id=(e.f->>'employee_id')::uuid
 ),selected as(
  select f,source_id,totals,case
   when f->>'state'='review_only'or f->>'kind'='annual_tax'then array[]::text[]
   when f->>'kind'in('monthly','cutoff')then array['basic','paid_leave','unworked_holiday','premium',
    'overtime','night','allowances','exempt_allowances','reimbursement','gross','taxable',
    'sss_employee','philhealth_employee','pagibig_employee']
   when f->>'kind'='thirteenth'then array['gross_cents','exempt_cents','taxable_cents',
    'sss_adjustment_cents','philhealth_adjustment_cents','pagibig_adjustment_cents']
   when f->>'kind'='amendment'then array['basic_cents','gross_cents','taxable_cents','contribution_cents']
   when f->>'kind'='final_adjustment'then array['vacation_gross_cents','vacation_exempt_cents',
    'vacation_shared_benefit_cents','separation_cents','retirement_cents','taxable_extras_cents',
    'shared_extras_cents','exempt_extras_cents','deposit_returns_cents','additional_contributions_cents','extras_gross_cents']
   else array[]::text[] end keys from sources
 )select coalesce(jsonb_agg(jsonb_build_object('company_id',cid,'employee_id',f->'employee_id',
  'kind',f->'kind','id',f->'id','source_found',source_id is not null,'values',
  coalesce((select jsonb_object_agg(k,totals->k)from unnest(keys)k),'{}'))
  order by f->>'kind',f->>'id'),'[]')into details from selected;
 return jsonb_build_object('version','government-annual-pay-v1','ledger',ledger,'details',details);
end;$$;
revoke all on function hris_private.government_annual_pay_details(integer)from public,anon,authenticated;
grant execute on function hris_private.government_annual_pay_details(integer)to authenticated;
create function public.government_annual_pay_details(p_year integer)returns jsonb
language sql stable security invoker set search_path=''as $$select hris_private.government_annual_pay_details(p_year);$$;
revoke all on function public.government_annual_pay_details(integer)from public,anon,authenticated;
grant execute on function public.government_annual_pay_details(integer)to authenticated;
commit;
