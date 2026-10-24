-- Versioned reporting keeps the original read contract available to older installations.
begin;
create function hris_private.payroll_report_rows_v2(cid uuid)returns setof jsonb
 language sql stable security invoker set search_path=''as $rows$
 select case when r->>'kind'='annual_tax'and(r->>'period_start')::date>(r->>'period_end')::date then jsonb_set(r,'{period_start}',r->'period_end')else r end from hris_private.payroll_report_rows(cid)r
 union all
 select jsonb_build_object('id',s.id,'company_id',s.company_id,'employee_id',s.employee_id,'kind','amendment','employee_code',s.employee_code,'employee_name',s.employee_name,'department',s.department,'period_start',s.starts_on,'period_end',s.ends_on,'issued_on',(s.approved_at at time zone c.timezone)::date,'received_on',r.paid_on,'status',r.status,'payment_state',r.status,
 'basic_cents',s.totals->'basic_cents','gross_cents',s.totals->'gross_cents','employee_contributions_cents',s.totals->'contribution_cents','employer_contributions_cents',null,'withholding_cents',s.totals->'withholding_cents','net_cents',s.totals->'net_cents','contained_source_net_cents',null,'cash_cents',case when r.status='paid'then(s.totals->>'net_cents')::bigint else 0 end,'offset_cents',0,'tax_adjustment_cents',null,
 'outstanding_cents',case when s.purpose='review_only'then null when r.status='approved'then(s.totals->>'net_cents')::bigint else 0 end,'reserved_cents',0,'reserved_case_id',null,'rule_version',s.rule_version,'linked_kind',s.target_kind,'linked_id',s.target_id)
 ||case when s.purpose='review_only'then jsonb_build_object('review_amount_cents',s.totals->'review_amount_cents')else'{}'end
 from public.payroll_amendment_slips s join public.payroll_amendment_cases r on r.company_id=s.company_id and r.id=s.id join public.companies c on c.id=s.company_id where s.company_id=cid;
$rows$;
revoke all on function hris_private.payroll_report_rows_v2(uuid)from public,anon,authenticated;
create function hris_private.payroll_report_v2(p_filter jsonb)returns jsonb
 language plpgsql stable security definer set search_path=''as $report$
declare m public.company_members;lo date;hi date;axis text;kind text;eid uuid;term text;rows jsonb;n integer;begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Current HR/Admin access required for company payroll reports.'using errcode='42501';end if;
 if jsonb_typeof(p_filter)is distinct from'object'or octet_length(p_filter::text)>2048
 or(p_filter?&array['starts_on','ends_on','axis','kind','employee_id','search'])is not true
 or p_filter-array['starts_on','ends_on','axis','kind','employee_id','search']<>'{}'then raise exception 'Use the bounded payroll report filter.'using errcode='23514';end if;
 if(jsonb_typeof(p_filter->'starts_on')='string'and jsonb_typeof(p_filter->'ends_on')='string'and jsonb_typeof(p_filter->'axis')='string'and jsonb_typeof(p_filter->'kind')='string'and jsonb_typeof(p_filter->'search')='string'and jsonb_typeof(p_filter->'employee_id')in('null','string'))is not true then raise exception 'Invalid payroll report filter values.'using errcode='23514';end if;
 lo:=(p_filter->>'starts_on')::date;hi:=(p_filter->>'ends_on')::date;axis:=p_filter->>'axis';kind:=p_filter->>'kind';eid:=(p_filter->>'employee_id')::uuid;term:=btrim(p_filter->>'search');
 if(lo between date'2026-01-01'and date'2200-12-31'and hi between lo and date'2200-12-31'and hi-lo<=365
 and axis in('issued','received')and kind in('all','monthly','cutoff','thirteenth','annual_tax','final','amendment')and char_length(term)<=80 and term!~'[[:cntrl:]]')is not true then raise exception 'Use at most366days and a supported report source/date basis.'using errcode='23514';end if;
 if eid is not null and not exists(select 1 from public.employees where company_id=m.company_id and id=eid)then raise exception 'Employee outside this company.'using errcode='42501';end if;
 select coalesce(jsonb_agg(x.r order by x.on_date desc,x.r->>'employee_code',x.r->>'kind',x.r->>'id'),'[]'),count(*)into rows,n
 from(select r,case when axis='received'then(r->>'received_on')::date else(r->>'issued_on')::date end on_date
 from hris_private.payroll_report_rows_v2(m.company_id)r
 where(case when axis='received'then(r->>'received_on')::date else(r->>'issued_on')::date end)between lo and hi
 and(kind='all'or r->>'kind'=kind)and(eid is null or r->>'employee_id'=eid::text)
 and(term=''or position(lower(term)in lower((r->>'employee_code')||' '||(r->>'employee_name')||' '||(r->>'department')))>0)
 limit 5001)x;
 if n>5000 then raise exception 'More than5000 rows. Narrow dates, person or source; totals were not truncated.'using errcode='23514';end if;
 if(select count(distinct(x->>'kind',x->>'id'))from jsonb_array_elements(rows)x)<>n then raise exception 'Duplicate financial source/reservation facts require review.'using errcode='23514';end if;
 return jsonb_build_object('version','payroll-report-v2','company_id',m.company_id,'company_name',(select name from public.companies where id=m.company_id),'generated_at',transaction_timestamp(),'filter',p_filter,'rows',rows);
end;$report$;
revoke all on function hris_private.payroll_report_v2(jsonb)from public,anon,authenticated;
grant execute on function hris_private.payroll_report_v2(jsonb)to authenticated;
create function public.payroll_report_v2(p_filter jsonb)returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.payroll_report_v2(p_filter);$$;
revoke all on function public.payroll_report_v2(jsonb)from public,anon,authenticated;
grant execute on function public.payroll_report_v2(jsonb)to authenticated;
-- Older report clients cannot represent linked supplements. Refuse incomplete totals,
-- while retaining their original monthly/cutoff/benefit and issued-slip contracts.
alter function hris_private.payroll_report(jsonb)rename to payroll_report_before_amendments;
create function hris_private.payroll_report(p_filter jsonb)returns jsonb language plpgsql stable security definer set search_path=''as $$
declare b jsonb;cid uuid;axis text;lo date;hi date;eid uuid;term text;begin
 b:=hris_private.payroll_report_before_amendments(p_filter);cid:=(b->>'company_id')::uuid;
 if exists(select 1 from jsonb_array_elements(b->'rows')r where(r->>'period_start')::date>(r->>'period_end')::date)then raise exception 'Update HRIS Desktop to review cross-year annual-tax reporting.'using errcode='PT409';end if;
 if p_filter->>'kind'='all'then
  axis:=p_filter->>'axis';lo:=(p_filter->>'starts_on')::date;hi:=(p_filter->>'ends_on')::date;eid:=(p_filter->>'employee_id')::uuid;term:=btrim(p_filter->>'search');
  if exists(select 1 from hris_private.payroll_report_rows_v2(cid)r where r->>'kind'='amendment'and(case when axis='received'then(r->>'received_on')::date else(r->>'issued_on')::date end)between lo and hi and(eid is null or r->>'employee_id'=eid::text)and(term=''or position(lower(term)in lower((r->>'employee_code')||' '||(r->>'employee_name')||' '||(r->>'department')))>0))then raise exception 'Update HRIS Desktop: this consolidated report includes linked amendments that the older report contract cannot represent.'using errcode='PT409';end if;
 end if;return b;
end;$$;
revoke all on function hris_private.payroll_report_before_amendments(jsonb),hris_private.payroll_report(jsonb)from public,anon,authenticated;
grant execute on function hris_private.payroll_report(jsonb)to authenticated;
create or replace function public.payroll_report(p_filter jsonb)returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.payroll_report(p_filter);$$;
commit;
