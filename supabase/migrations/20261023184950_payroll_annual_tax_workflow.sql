-- Independently approved annual tax close. Old monthly/cutoff/13th slips remain immutable.
begin;
create table public.payroll_tax_reconciliations(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid not null,
 year integer not null check(year between 2026 and 2200),kind text not null check(kind in('year_end','termination')),
 as_of date not null,service_end date not null,title text not null check(char_length(btrim(title))between 1 and 120),
 employee_name text not null,employee_code text not null,department text not null,
 status text not null default 'draft'check(status in('draft','submitted','approved','settled','cancelled')),
 revision bigint not null default 0 check(revision>=0),created_by uuid not null references auth.users(id),submitted_by uuid references auth.users(id),approved_by uuid references auth.users(id),approved_at timestamptz,
 settled_on date,settlement_reference text not null default ''check(char_length(settlement_reference)<=500),rule_version text not null default 'PH-annual-tax-2026-10-24-v1',
 source_basis jsonb not null check(octet_length(source_basis::text)<=2097152),worksheet jsonb not null check(octet_length(worksheet::text)<=131072),totals jsonb not null,
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),unique(company_id,id),
 foreign key(company_id,employee_id)references public.employees(company_id,id),
 check(extract(year from as_of)=year and extract(year from service_end)=year and service_end<=as_of),
 check(kind='termination'or(service_end=make_date(year,12,31)and as_of=service_end)),
 check((status in('submitted','approved','settled')and submitted_by is not null)or(status not in('submitted','approved','settled')and submitted_by is null)),
 check((status in('approved','settled')and approved_by is not null and approved_at is not null and approved_by<>submitted_by)or(status not in('approved','settled')and approved_by is null and approved_at is null)),
 check((status='settled'and settled_on is not null and btrim(settlement_reference)<>'')or(status<>'settled'and settled_on is null and settlement_reference=''))
);
create index payroll_tax_reconciliations_employee_idx on public.payroll_tax_reconciliations(company_id,employee_id,year,status);
create index payroll_tax_reconciliations_creator_idx on public.payroll_tax_reconciliations(created_by);
create index payroll_tax_reconciliations_submitter_idx on public.payroll_tax_reconciliations(submitted_by);
create index payroll_tax_reconciliations_approver_idx on public.payroll_tax_reconciliations(approved_by);
create table public.payroll_tax_slips(
 id uuid primary key,company_id uuid not null,employee_id uuid not null,year integer not null,kind text not null,as_of date not null,service_end date not null,
 company_name text not null,employee_name text not null,employee_code text not null,department text not null,totals jsonb not null,
 approved_by uuid not null references auth.users(id),approved_at timestamptz not null,rule_version text not null,
 foreign key(company_id,id)references public.payroll_tax_reconciliations(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_tax_slips_employee_idx on public.payroll_tax_slips(company_id,employee_id,year);
create index payroll_tax_slips_approver_idx on public.payroll_tax_slips(approved_by);
create trigger payroll_tax_slips_immutable before update or delete on public.payroll_tax_slips for each row execute function hris_private.employment_history_immutable();
do $$declare t text;begin
 foreach t in array array['payroll_tax_reconciliations','payroll_tax_slips']loop
  execute format('alter table public.%I enable row level security',t);execute format('revoke all on public.%I from public,anon,authenticated',t);execute format('grant select on public.%I to authenticated',t);
 end loop;
end;$$;
create policy payroll_tax_reconciliations_hr_read on public.payroll_tax_reconciliations for select to authenticated using(hris_private.payroll_hr(company_id));
create policy payroll_tax_slips_read on public.payroll_tax_slips for select to authenticated using(hris_private.payroll_hr(company_id)or hris_private.payroll_own(company_id,employee_id));

create function hris_private.annual_tax_basis(cid uuid,eid uuid,yr integer,asof date,service_end date,exclude_id uuid)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare e public.employees;lo date;income jsonb;payments jsonb;previous jsonb;adjustment bigint;pending integer;pending_adjustments integer;ext bigint;other bigint;latest date;begin
 select * into e from public.employees where company_id=cid and id=eid;
 if e.id is null then raise exception 'Employee unavailable.'using errcode='42501';end if;
 lo:=greatest(make_date(yr,1,1),coalesce(e.joined_on,make_date(yr,1,1)));
 if lo>service_end then raise exception 'Employee joined after this service end.'using errcode='23514';end if;
 with raw as(
  select 'monthly:'||s.id::text id,s.month first,(s.month+interval '1month' - interval '1day')::date last,r.status,r.paid_on,r.revision,
   (s.item#>>'{totals,taxable}')::bigint taxable,(s.item#>>'{totals,withholding}')::bigint withheld,0::bigint gross,0::bigint exempt,0::bigint contribution,true coverage
  from public.payroll_slips s join public.payroll_runs r on r.company_id=s.company_id and r.id=s.run_id where s.company_id=cid and s.employee_id=eid
  union all
  select 'cutoff:'||s.id::text,s.starts_on,s.ends_on,r.status,r.paid_on,r.revision,(s.item#>>'{totals,taxable}')::bigint,(s.item#>>'{totals,withholding}')::bigint,0,0,0,true
  from public.payroll_cutoff_slips s join public.payroll_cutoff_runs r on r.company_id=s.company_id and r.id=s.run_id where s.company_id=cid and s.employee_id=eid
  union all
  select 'thirteenth:'||s.id::text,s.as_of,s.as_of,r.status,r.paid_on,r.revision,(s.item#>>'{totals,taxable_cents}')::bigint,(s.item#>>'{totals,withholding_cents}')::bigint,(s.item#>>'{totals,gross_cents}')::bigint,(s.item#>>'{totals,exempt_cents}')::bigint,
   (s.item#>>'{totals,sss_adjustment_cents}')::bigint+(s.item#>>'{totals,philhealth_adjustment_cents}')::bigint+(s.item#>>'{totals,pagibig_adjustment_cents}')::bigint,false
  from public.payroll_thirteenth_slips s join public.payroll_thirteenth_runs r on r.company_id=s.company_id and r.id=s.run_id where s.company_id=cid and s.employee_id=eid
 ),included as(select * from raw where last<=asof)
 select coalesce(jsonb_agg(jsonb_build_object('id',id,'starts_on',case when coverage and greatest(first,lo)<=least(last,service_end)then greatest(first,lo)end,'ends_on',case when coverage and greatest(first,lo)<=least(last,service_end)then least(last,service_end)end,'taxable_cents',taxable,'withholding_cents',withheld,'benefit_gross_cents',gross,'benefit_exempt_cents',exempt,'contribution_cents',contribution)order by id)filter(where status='paid'and extract(year from paid_on)=yr and paid_on<=asof),'[]'),
 coalesce(jsonb_agg(jsonb_build_object('id',id,'paid_on',paid_on,'status',status,'revision',revision)order by id),'[]'),count(*)filter(where status='approved')::integer
 into income,payments,pending from included;
 select coalesce(max((s.item#>>'{worksheet,external_thirteenth_cents}')::bigint),0),coalesce(max((s.item#>>'{worksheet,other_benefits_cents}')::bigint),0)into ext,other from public.payroll_thirteenth_slips s join public.payroll_thirteenth_runs r on r.company_id=s.company_id and r.id=s.run_id where s.company_id=cid and s.employee_id=eid and s.year=yr and s.as_of<=asof and r.status='paid'and extract(year from r.paid_on)=yr and r.paid_on<=asof;
 select coalesce(jsonb_agg(jsonb_build_object('id',p.id,'revision',p.revision,'status',p.status,'as_of',p.as_of,'adjustment_cents',(p.totals->>'adjustment_cents')::bigint,'settled_on',p.settled_on)order by p.id),'[]'),
 coalesce(sum((p.totals->>'adjustment_cents')::bigint)filter(where p.status='settled'),0)::bigint,count(*)filter(where p.status='approved')::integer,max(greatest(p.as_of,coalesce(p.settled_on,p.as_of)))
 into previous,adjustment,pending_adjustments,latest from public.payroll_tax_reconciliations p where p.company_id=cid and p.employee_id=eid and p.year=yr and p.id<>exclude_id and p.status in('approved','settled');
 if latest>asof then raise exception 'Use an as-of date at least as late as the earlier issued tax close.'using errcode='23514';end if;
 return jsonb_build_object('employee',jsonb_build_object('id',e.id,'full_name',e.full_name,'employee_code',e.employee_code,'department',e.department,'joined_on',e.joined_on,'status',e.status),'payments',payments,'previous',previous,
 'calculation_source',jsonb_build_object('starts_on',lo,'service_end',service_end,'as_of',asof,'income',income,'prior_adjustment_cents',adjustment,'pending_payments',pending,'pending_adjustments',pending_adjustments,'external_thirteenth_floor',ext,'other_benefits_floor',other));
end;$$;
create function hris_private.annual_tax_check(r public.payroll_tax_reconciliations)returns void language plpgsql stable security invoker set search_path=''as $$
declare fresh jsonb;c jsonb;begin
 fresh:=hris_private.annual_tax_basis(r.company_id,r.employee_id,r.year,r.as_of,r.service_end,r.id);
 if fresh is distinct from r.source_basis then raise exception 'Tax/payment sources changed. Refresh and review again.'using errcode='PT409';end if;
 c:=hris_private.annual_tax_ready(fresh->'calculation_source',r.worksheet);
 if c is distinct from r.totals then raise exception 'Annual tax totals do not reconcile.'using errcode='23514';end if;
end;$$;
create function hris_private.manage_payroll_tax(p_operation text,p_record jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;actor_label text;today date;zone text;rid uuid;eid uuid;expected bigint;why text;old_record jsonb;result jsonb;birth hris_private.payroll_birth;
 r public.payroll_tax_reconciliations;e public.employees;fresh jsonb;w jsonb;t jsonb;allowed text[];yr integer;kind text;asof date;service_end date;settled date;
begin
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Active HR or Administrator access is required.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));perform 1 from public.companies where id=m.company_id for update;
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist')for update;
 if m.user_id is null then raise exception 'Your payroll access changed. Refresh.'using errcode='42501';end if;
 select full_name into actor_label from public.employees where company_id=m.company_id and id=m.employee_id;
 select timezone,(now()at time zone timezone)::date into zone,today from public.companies where id=m.company_id;
 if p_operation is null or p_operation not in('create','save','refresh','submit','return','cancel','approve','settle')or jsonb_typeof(p_record)is distinct from 'object'or octet_length(p_record::text)>131072 then raise exception 'Use a supported bounded annual tax operation.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=hris_private.payroll_text(p_record,'reason',500,true);
 if rid is null or rid='00000000-0000-0000-0000-000000000000'then raise exception 'Annual tax record identity is required.'using errcode='23514';end if;
 allowed:=case p_operation when'create'then array['employee_id','year','kind','as_of','service_end','title']when'save'then array['worksheet']when'settle'then array['settled_on','settlement_reference']else array[]::text[]end;
 if p_record-(allowed||array['id','expected_revision','reason'])<>'{}'then raise exception 'Unsupported annual tax fields.'using errcode='23514';end if;
 if p_operation='create'then
  if expected is not null then raise exception 'New tax records must not have a revision.'using errcode='23514';end if;
  select * into birth from hris_private.payroll_birth where company_id=m.company_id and operation='tax_create'and record_id=rid;
  if birth.record_id is not null then
   if birth.actor_id<>m.user_id or birth.payload<>p_record then raise exception 'This tax creation identity was already used.'using errcode='23514';end if;
   select to_jsonb(x)into result from public.payroll_tax_reconciliations x where x.company_id=m.company_id and x.id=rid;return result;
  end if;
  yr:=(p_record->>'year')::integer;kind:=hris_private.payroll_text(p_record,'kind',20,true);asof:=hris_private.thirteenth_date(p_record,'as_of');service_end:=hris_private.thirteenth_date(p_record,'service_end');perform hris_private.annual_tax_dates(yr,kind,service_end,asof);
  eid:=(p_record->>'employee_id')::uuid;select * into e from public.employees where company_id=m.company_id and id=eid;
  if e.id is null then raise exception 'Employee unavailable.'using errcode='42501';end if;
  fresh:=hris_private.annual_tax_basis(m.company_id,eid,yr,asof,service_end,rid);w:=hris_private.annual_tax_empty();t:=hris_private.annual_tax_compute(fresh->'calculation_source',w);
  insert into public.payroll_tax_reconciliations(id,company_id,employee_id,year,kind,as_of,service_end,title,employee_name,employee_code,department,created_by,source_basis,worksheet,totals)
   values(rid,m.company_id,eid,yr,kind,asof,service_end,hris_private.payroll_text(p_record,'title',120,true),e.full_name,e.employee_code,e.department,m.user_id,fresh,w,t)returning * into r;
  insert into hris_private.payroll_birth(company_id,operation,record_id,actor_id,payload)values(m.company_id,'tax_create',rid,m.user_id,p_record);
 else
  select * into r from public.payroll_tax_reconciliations where company_id=m.company_id and id=rid for update;
  if r.id is null then raise exception 'Tax record unavailable.'using errcode='42501';end if;
  if expected is distinct from r.revision then raise exception 'Annual tax record changed. Refresh.'using errcode='PT409';end if;
  old_record:=to_jsonb(r);
  if p_operation in('save','refresh')then
   if r.status<>'draft'then raise exception 'Only draft tax worksheets may change.'using errcode='23514';end if;
   fresh:=hris_private.annual_tax_basis(m.company_id,r.employee_id,r.year,r.as_of,r.service_end,r.id);
   if p_operation='save'then
    if fresh is distinct from r.source_basis then raise exception 'Tax/payment sources changed. Refresh and review again.'using errcode='PT409';end if;w:=p_record->'worksheet';
   else
    w:=r.worksheet||'{"final_compensation_reviewed":false,"previous_employers_reviewed":false,"benefits_reviewed":false,"review_note":""}'::jsonb;
   end if;
   if p_operation='refresh'then begin t:=hris_private.annual_tax_compute(fresh->'calculation_source',w);exception when check_violation then w:=hris_private.annual_tax_empty();t:=hris_private.annual_tax_compute(fresh->'calculation_source',w);end;
   else t:=hris_private.annual_tax_compute(fresh->'calculation_source',w);end if;
   select * into e from public.employees where company_id=m.company_id and id=r.employee_id;
   update public.payroll_tax_reconciliations set source_basis=fresh,worksheet=w,totals=t,employee_name=e.full_name,employee_code=e.employee_code,department=e.department,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation in('submit','approve')then
   if r.year<>2026 or r.rule_version<>'PH-annual-tax-2026-10-24-v1'or r.as_of>today then raise exception 'Close completed2026payment facts only; future years need a reviewed rule pack.'using errcode='23514';end if;
   perform hris_private.annual_tax_check(r);
   if p_operation='submit'then
    if r.status<>'draft'then raise exception 'Submit a draft tax close.'using errcode='23514';end if;
    update public.payroll_tax_reconciliations set status='submitted',submitted_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
   else
    if m.role<>'administrator'or r.submitted_by=m.user_id then raise exception 'Another Administrator must approve the annual tax close.'using errcode='42501';end if;
    if r.status<>'submitted'then raise exception 'Submit the tax close before approval.'using errcode='23514';end if;
    update public.payroll_tax_reconciliations set status='approved',approved_by=m.user_id,approved_at=clock_timestamp(),revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
    insert into public.payroll_tax_slips(id,company_id,employee_id,year,kind,as_of,service_end,company_name,employee_name,employee_code,department,totals,approved_by,approved_at,rule_version)
     select r.id,r.company_id,r.employee_id,r.year,r.kind,r.as_of,r.service_end,c.name,r.employee_name,r.employee_code,r.department,r.totals,r.approved_by,r.approved_at,r.rule_version from public.companies c where c.id=m.company_id;
   end if;
  elsif p_operation='return'then
   if r.status<>'submitted'then raise exception 'Return a submitted tax close only.'using errcode='23514';end if;
   update public.payroll_tax_reconciliations set status='draft',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='cancel'then
   if r.status not in('draft','submitted')then raise exception 'Issued tax advice stays immutable.'using errcode='23514';end if;
   update public.payroll_tax_reconciliations set status='cancelled',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  else
   if m.role<>'administrator'then raise exception 'Only Administrators record tax settlements.'using errcode='42501';end if;
   settled:=hris_private.thirteenth_date(p_record,'settled_on');
   if r.status<>'approved'or settled<r.as_of or settled<(r.approved_at at time zone zone)::date or settled>today then raise exception 'Use an actual settlement date after approval and the payment as-of date.'using errcode='23514';end if;
   update public.payroll_tax_reconciliations set status='settled',settled_on=settled,settlement_reference=hris_private.payroll_text(p_record,'settlement_reference',500,true),revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  end if;
 end if;
 result:=to_jsonb(r);
 insert into public.payroll_events(company_id,record_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)values(m.company_id,rid,'tax_'||p_operation,why,m.user_id,actor_label,m.role,old_record,result);
 return result;
end;$$;
revoke all on function hris_private.annual_tax_basis(uuid,uuid,integer,date,date,uuid),hris_private.annual_tax_check(public.payroll_tax_reconciliations),hris_private.manage_payroll_tax(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_payroll_tax(text,jsonb)to authenticated;
create function public.manage_payroll_tax(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_payroll_tax(p_operation,p_record);$$;
revoke all on function public.manage_payroll_tax(text,jsonb)from public,anon,authenticated;
grant execute on function public.manage_payroll_tax(text,jsonb)to authenticated;
commit;
