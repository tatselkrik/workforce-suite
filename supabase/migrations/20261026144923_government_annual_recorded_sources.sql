-- Read-only annual BIR recorded sources, not an expected return or tax formula.
-- Reuse the existing protected facts; never mutate slips, receipts or reviews.
begin;
create function hris_private.government_annual_ledger(p_year integer) returns jsonb
language plpgsql stable security invoker set search_path='' as $$
declare first_month date;last_month date;seed jsonb;cid uuid;entries jsonb;
 labels jsonb;observations integer;distinct_sources integer;conflicts integer;
begin
 if (p_year between 2025 and 2200) is not true then
  raise exception 'Choose a supported elapsed/current year from 2025 onward.' using errcode='23514';
 end if;
 first_month:=make_date(p_year,1,1);
 -- This call checks current protected HR/Admin membership and the company date.
 seed:=hris_private.government_payroll_sources(first_month,'bir');
 cid:=(seed->>'company_id')::uuid;
 last_month:=least(make_date(p_year,12,1),date_trunc('month',(seed->>'as_of')::date)::date);
 with months as materialized(
  select first_month as observed_month,seed wire
  union all
  select d::date,hris_private.government_payroll_sources(d::date,'bir')
  from pg_catalog.generate_series(first_month+interval'1 month',last_month,interval'1 month')d
 ),raw as materialized(
  select m.observed_month,f.fact from months m
  cross join lateral pg_catalog.jsonb_array_elements(m.wire->'facts')f(fact)
  limit 40001
 ),grouped as materialized(
  select fact->>'kind'kind,fact->>'id'id,min(fact::text)::jsonb fact,
   count(distinct fact) variants,jsonb_agg(distinct observed_month order by observed_month) observed_months
  from raw group by fact->>'kind',fact->>'id'
 ),bounded as materialized(select *from grouped order by kind,id limit 20001)
 select coalesce(jsonb_agg(jsonb_build_object('fact',fact,'observed_months',observed_months)order by kind,id),'[]'),
  (select count(*)from raw),count(*),(select count(*)from grouped where variants<>1)
 into entries,observations,distinct_sources,conflicts from bounded;
 if observations>40000 or distinct_sources>20000 then
  raise exception 'Annual recorded sources exceed the review limit. No totals were truncated.' using errcode='23514';
 end if;
 if conflicts<>0 then
  raise exception 'Conflicting annual source observations require review. No totals were returned.' using errcode='23514';
 end if;
 select coalesce(jsonb_agg(jsonb_build_object('employee_id',e.id,'employee_code',e.employee_code,
  'employee_name',e.full_name)order by e.id),'[]') into labels
 from public.employees e where e.company_id=cid and e.id in(
  select (j->'fact'->>'employee_id')::uuid from pg_catalog.jsonb_array_elements(entries)j);
 if jsonb_array_length(labels)<>(select count(distinct j->'fact'->>'employee_id')
  from pg_catalog.jsonb_array_elements(entries)j) then
  raise exception 'Annual sources lack their protected employee labels.' using errcode='23514';
 end if;
 return jsonb_build_object('version','government-annual-ledger-v1','company_id',cid,
  'company_name',seed->'company_name','generated_at',seed->'generated_at','as_of',seed->'as_of',
  'year',p_year,'through_month',last_month,'entries',entries,'employees',labels);
end;$$;
revoke all on function hris_private.government_annual_ledger(integer) from public,anon,authenticated;
grant execute on function hris_private.government_annual_ledger(integer) to authenticated;
create function public.government_annual_ledger(p_year integer) returns jsonb
language sql stable security invoker set search_path='' as $$select hris_private.government_annual_ledger(p_year);$$;
revoke all on function public.government_annual_ledger(integer) from public,anon,authenticated;
grant execute on function public.government_annual_ledger(integer) to authenticated;
commit;
