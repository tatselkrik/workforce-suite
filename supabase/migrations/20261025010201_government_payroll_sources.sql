-- Read-only review facts. No remittance, filing or original payroll data is changed.
begin;
create function hris_private.government_payroll_sources(p_month date,p_agency text)returns jsonb
language plpgsql stable security definer set search_path=''as $report$
declare m public.company_members;c public.companies;today date;facts jsonb;n integer;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active
  and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Current HR/Admin access is required for government payroll review.'using errcode='42501';end if;
 select *into strict c from public.companies where id=m.company_id;
 today:=(statement_timestamp()at time zone c.timezone)::date;
 if(p_month>=date'2025-01-01'and p_month<=today and extract(day from p_month)=1
 and p_agency in('bir','sss','philhealth','pagibig'))is not true then
  raise exception 'Choose a supported agency and elapsed/current calendar month.'using errcode='23514';end if;
 with regular as(
  select 'monthly'::text kind,s.id,s.company_id,s.employee_id,s.month starts_on,
   (s.month+interval'1month -1day')::date ends_on,s.item->'totals'totals,r.status
  from public.payroll_slips s join public.payroll_runs r on r.company_id=s.company_id and r.id=s.run_id
  where s.company_id=m.company_id
  union all
  select 'cutoff',s.id,s.company_id,s.employee_id,s.starts_on,s.ends_on,s.item->'totals',r.status
  from public.payroll_cutoff_slips s join public.payroll_cutoff_runs r on r.company_id=s.company_id and r.id=s.run_id
  where s.company_id=m.company_id
 ),normal as(
  select s.kind,s.id,s.company_id,s.employee_id,s.starts_on,s.ends_on,
   case when p_agency='bir'then null::date else date_trunc('month',s.starts_on)::date end contribution_month,
   p.paid_on received_on,case when p.paid_on>today then'dated_after_as_of'when p.id is not null then'received'when s.status='paid'then'receipt_missing'else'approved'end state,
   jsonb_build_object('employee_cents',s.totals->case p_agency when'bir'then'withholding'else p_agency||'_employee'end,
    'employer_cents',case when p_agency='bir'then'0'::jsonb else s.totals->(p_agency||'_employer')end,
    'ec_cents',case when p_agency='sss'then s.totals->'ec'else'0'::jsonb end)parts
  from regular s left join public.payroll_payment_receipts p on p.company_id=s.company_id
   and p.employee_id=s.employee_id and p.kind=s.kind and p.slip_id=s.id
  where (p_agency<>'bir'and date_trunc('month',s.starts_on)::date=p_month)
   or(p_agency='bir'and(date_trunc('month',p.paid_on)::date=p_month or(p.paid_on>today and date_trunc('month',s.ends_on)::date=p_month)
    or(p.id is null and(s.status='paid'or date_trunc('month',s.ends_on)::date=p_month))))
 ),benefit as(
  select 'thirteenth'::text kind,s.id,s.company_id,s.employee_id,make_date(s.year,1,1)starts_on,s.as_of ends_on,
   null::date contribution_month,p.paid_on received_on,
   case when p.paid_on>today then'dated_after_as_of'when p.id is not null then'received'when r.status='paid'then'receipt_missing'else'approved'end state,
   jsonb_build_object('employee_cents',s.item->'totals'->case p_agency when'bir'then'withholding_cents'else p_agency||'_adjustment_cents'end,
    'employer_cents',case when p_agency='bir'then'0'::jsonb else null::jsonb end,
    'ec_cents',case when p_agency='sss'then null::jsonb else'0'::jsonb end)parts
  from public.payroll_thirteenth_slips s join public.payroll_thirteenth_runs r on r.company_id=s.company_id and r.id=s.run_id
  left join public.payroll_payment_receipts p on p.company_id=s.company_id and p.employee_id=s.employee_id and p.kind='thirteenth'and p.slip_id=s.id
  where s.company_id=m.company_id and(p_agency<>'bir'or date_trunc('month',p.paid_on)::date=p_month or(p.paid_on>today and date_trunc('month',s.as_of)::date=p_month)
   or(p.id is null and(r.status='paid'or date_trunc('month',s.as_of)::date=p_month)))
 ),amendment as(
  select 'amendment'::text kind,s.id,s.company_id,s.employee_id,s.starts_on,s.ends_on,
   case when p_agency<>'bir'and s.target_kind in('monthly','cutoff')
    then date_trunc('month',s.starts_on)::date else null::date end contribution_month,
   r.paid_on received_on,case when s.purpose='review_only'then'review_only'when r.status='withdrawn'then'withdrawn'
    when r.paid_on>today then'dated_after_as_of'when r.status='paid'then'received'else'approved'end state,
   case when s.purpose='review_only'then jsonb_build_object('employee_cents',0,'employer_cents',0,'ec_cents',0)
    else jsonb_build_object('employee_cents',r.worksheet->case p_agency when'bir'then'withholding_cents'else p_agency||'_cents'end,
     'employer_cents',case when p_agency='bir'then'0'::jsonb else null::jsonb end,
     'ec_cents',case when p_agency='sss'then null::jsonb else'0'::jsonb end)end parts
  from public.payroll_amendment_slips s join public.payroll_amendment_cases r on r.company_id=s.company_id and r.id=s.id
  where s.company_id=m.company_id and(p_agency<>'bir'or date_trunc('month',r.paid_on)::date=p_month or(r.paid_on>today and date_trunc('month',s.cash_date)::date=p_month)
   or(r.status='approved'and date_trunc('month',s.cash_date)::date=p_month))
 ),final_adjustment as(
  select case when p_agency='bir'then'final_adjustment'else'final_contribution'end kind,
   s.id,s.company_id,s.employee_id,s.employment_start starts_on,s.service_end ends_on,null::date contribution_month,
   case when st.kind='settled'then st.recorded_on end received_on,
   case when r.status='withdrawn'then'withdrawn'when st.kind='settled'and st.recorded_on>today then'dated_after_as_of'when st.kind='settled'then'received'else'approved'end state,
   jsonb_build_object('employee_cents',case when p_agency='bir'then s.totals#>'{tax,adjustment_cents}'else r.worksheet->(p_agency||'_adjustment_cents')end,
    'employer_cents',case when p_agency='bir'then'0'::jsonb else null::jsonb end,
    'ec_cents',case when p_agency='sss'then null::jsonb else'0'::jsonb end)parts
  from public.payroll_final_slips s join public.payroll_final_cases r on r.company_id=s.company_id and r.id=s.id
  left join public.payroll_final_status st on st.company_id=s.company_id and st.id=s.id
  where s.company_id=m.company_id and(p_agency<>'bir'or(st.kind='settled'and(date_trunc('month',st.recorded_on)::date=p_month or(st.recorded_on>today and date_trunc('month',s.cash_date)::date=p_month)))
   or(r.status='approved'and date_trunc('month',s.cash_date)::date=p_month))
 ),annual_tax as(
  select 'annual_tax'::text kind,s.id,s.company_id,s.employee_id,least(make_date(s.year,1,1),s.service_end)starts_on,
   s.service_end ends_on,null::date contribution_month,r.settled_on received_on,
   case when r.settled_on>today then'dated_after_as_of'when r.status='settled'then'received'else'approved'end state,
   jsonb_build_object('employee_cents',s.totals->'adjustment_cents','employer_cents',0,'ec_cents',0)parts
  from public.payroll_tax_slips s join public.payroll_tax_reconciliations r on r.company_id=s.company_id and r.id=s.id
  where s.company_id=m.company_id and p_agency='bir'and(date_trunc('month',r.settled_on)::date=p_month or(r.settled_on>today and date_trunc('month',s.as_of)::date=p_month)
   or(r.status<>'settled'and date_trunc('month',s.as_of)::date=p_month))
 ),all_facts as(
  select *from normal union all select *from benefit union all select *from amendment
  union all select *from final_adjustment union all select *from annual_tax
 ),selected as(
  select jsonb_build_object('company_id',f.company_id,'employee_id',f.employee_id,'id',f.id,'agency',p_agency,
   'kind',f.kind,'state',f.state,'starts_on',f.starts_on,'ends_on',f.ends_on,'contribution_month',f.contribution_month,
   'received_on',f.received_on,'parts',f.parts)fact
  from all_facts f where p_agency='bir'or f.contribution_month is null or f.contribution_month=p_month limit 20001
 )select coalesce(jsonb_agg(fact order by fact->>'kind',fact->>'id'),'[]'),count(*)into facts,n from selected;
 if n>20000 then raise exception 'Too many government source facts; complete outstanding contribution allocations first. No totals were truncated.'using errcode='23514';end if;
 if(select count(distinct(x->>'kind',x->>'id',x->>'agency'))from jsonb_array_elements(facts)x)<>n then
  raise exception 'Duplicate government sources require review.'using errcode='23514';end if;
 return jsonb_build_object('version','government-payroll-sources-v1','company_id',m.company_id,
  'company_name',c.name,'generated_at',transaction_timestamp(),'as_of',today,'month',p_month,'agency',p_agency,'facts',facts);
end;$report$;
revoke all on function hris_private.government_payroll_sources(date,text)from public,anon,authenticated;
grant execute on function hris_private.government_payroll_sources(date,text)to authenticated;
create function public.government_payroll_sources(p_month date,p_agency text)returns jsonb
language sql stable security invoker set search_path=''as $$select hris_private.government_payroll_sources(p_month,p_agency);$$;
revoke all on function public.government_payroll_sources(date,text)from public,anon,authenticated;
grant execute on function public.government_payroll_sources(date,text)to authenticated;
commit;
