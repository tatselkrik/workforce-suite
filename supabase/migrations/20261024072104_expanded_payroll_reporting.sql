-- Read-only single-snapshot payroll reporting. No financial rows are changed.
begin;
create function hris_private.payroll_report_rows(cid uuid)returns setof jsonb
 language sql stable security invoker set search_path=''as $rows$
 with regular as(
 select 'monthly'::text kind,s.id,s.company_id,s.employee_id,s.run_id,s.item->>'employee_code'employee_code,s.item->>'employee_name'employee_name,s.item->>'department'department,s.month period_start,(s.month+interval'1month -1day')::date period_end,s.approved_at,r.status,s.rule_version,(s.item#>>'{totals,basic}')::bigint basic,(s.item#>>'{totals,gross}')::bigint gross,(s.item#>>'{totals,sss_employee}')::bigint+(s.item#>>'{totals,philhealth_employee}')::bigint+(s.item#>>'{totals,pagibig_employee}')::bigint ee,(s.item#>>'{totals,sss_employer}')::bigint+(s.item#>>'{totals,philhealth_employer}')::bigint+(s.item#>>'{totals,pagibig_employer}')::bigint+(s.item#>>'{totals,ec}')::bigint er,(s.item#>>'{totals,withholding}')::bigint wht,(s.item#>>'{totals,net}')::bigint net from public.payroll_slips s join public.payroll_runs r on r.company_id=s.company_id and r.id=s.run_id where s.company_id=cid
 union all
 select 'cutoff'::text kind,s.id,s.company_id,s.employee_id,s.run_id,s.item->>'employee_code'employee_code,s.item->>'employee_name'employee_name,s.item->>'department'department,s.starts_on period_start,s.ends_on period_end,s.approved_at,r.status,s.rule_version,(s.item#>>'{totals,basic}')::bigint basic,(s.item#>>'{totals,gross}')::bigint gross,(s.item#>>'{totals,sss_employee}')::bigint+(s.item#>>'{totals,philhealth_employee}')::bigint+(s.item#>>'{totals,pagibig_employee}')::bigint ee,(s.item#>>'{totals,sss_employer}')::bigint+(s.item#>>'{totals,philhealth_employer}')::bigint+(s.item#>>'{totals,pagibig_employer}')::bigint+(s.item#>>'{totals,ec}')::bigint er,(s.item#>>'{totals,withholding}')::bigint wht,(s.item#>>'{totals,net}')::bigint net from public.payroll_cutoff_slips s join public.payroll_cutoff_runs r on r.company_id=s.company_id and r.id=s.run_id where s.company_id=cid
 union all
 select 'thirteenth'::text kind,s.id,s.company_id,s.employee_id,s.run_id,s.item->>'employee_code'employee_code,s.item->>'employee_name'employee_name,s.item->>'department'department,make_date(s.year,1,1) period_start,s.as_of period_end,s.approved_at,r.status,s.rule_version,null::bigint basic,(s.item#>>'{totals,gross_cents}')::bigint gross,(s.item#>>'{totals,sss_adjustment_cents}')::bigint+(s.item#>>'{totals,philhealth_adjustment_cents}')::bigint+(s.item#>>'{totals,pagibig_adjustment_cents}')::bigint ee,null::bigint er,(s.item#>>'{totals,withholding_cents}')::bigint wht,(s.item#>>'{totals,net_cents}')::bigint net from public.payroll_thirteenth_slips s join public.payroll_thirteenth_runs r on r.company_id=s.company_id and r.id=s.run_id where s.company_id=cid
 )
 select jsonb_build_object('id',s.id,'company_id',s.company_id,'employee_id',s.employee_id,'kind',s.kind,'employee_code',s.employee_code,'employee_name',s.employee_name,'department',s.department,'period_start',s.period_start,'period_end',s.period_end,'issued_on',(s.approved_at at time zone c.timezone)::date,'received_on',p.paid_on,'status',s.status,'payment_state',case when p.id is not null then case when p.origin='final_settlement'then'final_offset'else'received'end when s.status='paid'then'receipt_missing'when fr.id is not null then'reserved'else'unpaid'end,'basic_cents',s.basic,'gross_cents',s.gross,'employee_contributions_cents',s.ee,'employer_contributions_cents',s.er,'withholding_cents',s.wht,'net_cents',s.net,'contained_source_net_cents',null,'cash_cents',case when p.id is not null and p.origin<>'final_settlement'then p.amount_cents else 0 end,'offset_cents',case when p.origin='final_settlement'then p.amount_cents else 0 end,'tax_adjustment_cents',null,'outstanding_cents',case when p.id is not null then 0 when s.status='paid'then null else s.net end,'reserved_cents',case when p.id is null and s.status<>'paid'and fr.id is not null then s.net else 0 end,'reserved_case_id',case when p.origin='final_settlement'then p.final_case_id else fr.id end,'rule_version',s.rule_version)
 from regular s join public.companies c on c.id=s.company_id
 left join public.payroll_payment_receipts p on p.company_id=s.company_id and p.kind=s.kind and p.slip_id=s.id and p.employee_id=s.employee_id
 left join lateral(select x.id from public.payroll_final_cases x where x.company_id=s.company_id and x.employee_id=s.employee_id and x.status='approved'
 and exists(select 1 from jsonb_array_elements(x.source_basis->'selected')z where z->>'kind'=s.kind and z->>'slip_id'=s.id::text))fr on true
 union all
 select jsonb_build_object('id',s.id,'company_id',s.company_id,'employee_id',s.employee_id,'kind','annual_tax','employee_code',s.employee_code,'employee_name',s.employee_name,'department',s.department,'period_start',make_date(s.year,1,1),'period_end',s.service_end,'issued_on',(s.approved_at at time zone c.timezone)::date,'received_on',r.settled_on,'status',r.status,'payment_state',case when r.status='settled'then'settled_adjustment'else'advice_only'end,'basic_cents',null,'gross_cents',null,'employee_contributions_cents',null,'employer_contributions_cents',null,'withholding_cents',null,'net_cents',null,'contained_source_net_cents',null,'cash_cents',0,'offset_cents',0,'tax_adjustment_cents',(s.totals->>'adjustment_cents')::bigint,'outstanding_cents',null,'reserved_cents',0,'reserved_case_id',null,'rule_version',s.rule_version)
 from public.payroll_tax_slips s join public.payroll_tax_reconciliations r on r.company_id=s.company_id and r.id=s.id
 join public.companies c on c.id=s.company_id where s.company_id=cid
 union all
 select jsonb_build_object('id',s.id,'company_id',s.company_id,'employee_id',s.employee_id,'kind','final','employee_code',s.employee_code,'employee_name',s.employee_name,'department',s.department,'period_start',s.employment_start,'period_end',s.service_end,'issued_on',(s.approved_at at time zone c.timezone)::date,'received_on',case when st.kind='settled'then st.recorded_on end,'status',r.status,'payment_state',r.status,'basic_cents',null,'gross_cents',null,'employee_contributions_cents',null,'employer_contributions_cents',null,'withholding_cents',null,'net_cents',(s.totals->>'net_cents')::bigint,'contained_source_net_cents',(s.totals->>'source_net_cents')::bigint,'cash_cents',case when st.kind='settled'then st.net_cents else 0 end,'offset_cents',0,'tax_adjustment_cents',null,'outstanding_cents',null,'reserved_cents',0,'reserved_case_id',null,'rule_version',s.rule_version)
 from public.payroll_final_slips s join public.payroll_final_cases r on r.company_id=s.company_id and r.id=s.id
 join public.companies c on c.id=s.company_id left join public.payroll_final_status st on st.company_id=s.company_id and st.id=s.id where s.company_id=cid;
$rows$;
revoke all on function hris_private.payroll_report_rows(uuid)from public,anon,authenticated;
create function hris_private.payroll_report(p_filter jsonb)returns jsonb
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
 and axis in('issued','received')and kind in('all','monthly','cutoff','thirteenth','annual_tax','final')and char_length(term)<=80 and term!~'[[:cntrl:]]')is not true then raise exception 'Use at most366days and a supported report source/date basis.'using errcode='23514';end if;
 if eid is not null and not exists(select 1 from public.employees where company_id=m.company_id and id=eid)then raise exception 'Employee outside this company.'using errcode='42501';end if;
 select coalesce(jsonb_agg(x.r order by x.on_date desc,x.r->>'employee_code',x.r->>'kind',x.r->>'id'),'[]'),count(*)into rows,n
 from(select r,case when axis='received'then(r->>'received_on')::date else(r->>'issued_on')::date end on_date
 from hris_private.payroll_report_rows(m.company_id)r
 where(case when axis='received'then(r->>'received_on')::date else(r->>'issued_on')::date end)between lo and hi
 and(kind='all'or r->>'kind'=kind)and(eid is null or r->>'employee_id'=eid::text)
 and(term=''or position(lower(term)in lower((r->>'employee_code')||' '||(r->>'employee_name')||' '||(r->>'department')))>0)
 limit 5001)x;
 if n>5000 then raise exception 'More than5000 rows. Narrow dates, person or source; totals were not truncated.'using errcode='23514';end if;
 if(select count(distinct(x->>'kind',x->>'id'))from jsonb_array_elements(rows)x)<>n then raise exception 'Duplicate financial source/reservation facts require review.'using errcode='23514';end if;
 return jsonb_build_object('version','payroll-report-v1','company_id',m.company_id,'company_name',(select name from public.companies where id=m.company_id),'generated_at',transaction_timestamp(),'filter',p_filter,'rows',rows);
end;$report$;
revoke all on function hris_private.payroll_report(jsonb)from public,anon,authenticated;
grant execute on function hris_private.payroll_report(jsonb)to authenticated;
create function public.payroll_report(p_filter jsonb)returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.payroll_report(p_filter);$$;
revoke all on function public.payroll_report(jsonb)from public,anon,authenticated;
grant execute on function public.payroll_report(jsonb)to authenticated;
commit;
