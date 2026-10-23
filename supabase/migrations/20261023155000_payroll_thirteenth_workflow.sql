-- Independently approved current-employer annual-benefit payouts.
begin;
create table public.payroll_thirteenth_runs(
 id uuid primary key,company_id uuid not null references public.companies(id),
 year integer not null check(year between 2026 and 2200),as_of date not null check(extract(year from as_of)=year),
 pay_date date not null,title text not null check(char_length(btrim(title))between 1 and 120),
 status text not null default 'draft'check(status in('draft','submitted','approved','paid','cancelled')),
 revision bigint not null default 0 check(revision>=0),created_by uuid not null references auth.users(id),submitted_by uuid references auth.users(id),approved_by uuid references auth.users(id),
 paid_on date,payment_reference text not null default ''check(char_length(payment_reference)<=500),rule_version text not null default 'PH-13th-2026-10-23-v1',
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),unique(company_id,id),
 check(pay_date>=as_of and pay_date<=make_date(year+1,1,31)),
 check((status in('approved','paid')and approved_by is not null and submitted_by is not null and approved_by<>submitted_by)or(status not in('approved','paid')and approved_by is null)),
 check((status='paid'and paid_on is not null and btrim(payment_reference)<>'')or(status<>'paid'and paid_on is null and payment_reference=''))
);
create index payroll_thirteenth_runs_creator_idx on public.payroll_thirteenth_runs(created_by);
create index payroll_thirteenth_runs_submitter_idx on public.payroll_thirteenth_runs(submitted_by);
create index payroll_thirteenth_runs_approver_idx on public.payroll_thirteenth_runs(approved_by);
create table public.payroll_thirteenth_items(
 id uuid primary key,company_id uuid not null,run_id uuid not null,employee_id uuid not null,
 employee_name text not null,employee_code text not null,department text not null,
 source_basis jsonb not null check(octet_length(source_basis::text)<=2097152),worksheet jsonb not null,totals jsonb not null,
 revision bigint not null default 0 check(revision>=0),unique(company_id,id),unique(company_id,run_id,employee_id),
 foreign key(company_id,run_id)references public.payroll_thirteenth_runs(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_thirteenth_items_employee_idx on public.payroll_thirteenth_items(company_id,employee_id);
create table public.payroll_thirteenth_slips(
 id uuid primary key,company_id uuid not null,run_id uuid not null,employee_id uuid not null,
 year integer not null,as_of date not null,pay_date date not null,company_name text not null,item jsonb not null,
 approved_by uuid not null references auth.users(id),approved_at timestamptz not null default clock_timestamp(),rule_version text not null,
 foreign key(company_id,run_id)references public.payroll_thirteenth_runs(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_thirteenth_slips_employee_idx on public.payroll_thirteenth_slips(company_id,employee_id,year);
create index payroll_thirteenth_slips_run_idx on public.payroll_thirteenth_slips(company_id,run_id);
create index payroll_thirteenth_slips_approver_idx on public.payroll_thirteenth_slips(approved_by);
create trigger payroll_thirteenth_slips_immutable before update or delete on public.payroll_thirteenth_slips for each row execute function hris_private.employment_history_immutable();
do $$declare t text;begin
 foreach t in array array['payroll_thirteenth_runs','payroll_thirteenth_items','payroll_thirteenth_slips']loop
  execute format('alter table public.%I enable row level security',t);execute format('revoke all on public.%I from public,anon,authenticated',t);execute format('grant select on public.%I to authenticated',t);
 end loop;
 foreach t in array array['payroll_thirteenth_runs','payroll_thirteenth_items']loop execute format('create policy %I on public.%I for select to authenticated using(hris_private.payroll_hr(company_id))',t||'_hr_read',t);end loop;
end;$$;
create policy payroll_thirteenth_slips_read on public.payroll_thirteenth_slips for select to authenticated using(hris_private.payroll_hr(company_id)or hris_private.payroll_own(company_id,employee_id));

create function hris_private.thirteenth_empty()returns jsonb language sql immutable security invoker set search_path=''as $$
 select '{"openings":[],"classification":"","eligibility_note":"","served_one_month":false,"basic_adjustment_cents":0,"adjustment_note":"","policy_topup_cents":0,"policy_note":"","external_thirteenth_cents":0,"other_benefits_cents":0,"opening_note":"","requested_gross_cents":null,"withholding_cents":0,"tax_reviewed":false,"tax_note":"","sss_adjustment_cents":0,"philhealth_adjustment_cents":0,"pagibig_adjustment_cents":0,"contributions_reviewed":false,"contribution_note":"","reviewed":false,"review_note":""}'::jsonb;
$$;
create function hris_private.thirteenth_seed(s jsonb)returns jsonb language sql immutable security invoker set search_path=''as $$
 select hris_private.thirteenth_empty()||jsonb_build_object('external_thirteenth_cents',s->'external_thirteenth_floor','other_benefits_cents',s->'other_benefits_floor','opening_note',case when(s->>'external_thirteenth_floor')::bigint>0 or(s->>'other_benefits_floor')::bigint>0 then 'Previously approved external payment and benefit facts retained. Review the original evidence.'else ''end);
$$;
create function hris_private.thirteenth_basis(cid uuid,eid uuid,yr integer,asof date)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare e public.employees;lo date;ps jsonb;prior jsonb;ext bigint;other bigint;latest date;begin
 select * into e from public.employees where company_id=cid and id=eid;
 if e.id is null then raise exception 'Employee unavailable.'using errcode='42501';end if;
 if yr not between 2026 and 2200 or extract(year from asof)<>yr then raise exception 'Use an as-of date in the benefit year.'using errcode='23514';end if;
 lo:=greatest(make_date(yr,1,1),coalesce(e.joined_on,make_date(yr,1,1)));
 if lo>asof then raise exception 'Employee joined after this benefit coverage.'using errcode='23514';end if;
 select coalesce(jsonb_agg(x.p order by x.p->>'starts_on',x.p->>'id'),'[]')into ps from(
  select jsonb_build_object('id',s.id,'starts_on',greatest(s.month,lo),'ends_on',(s.month+interval '1month -1day')::date,'basic_cents',(s.item#>>'{totals,basic}')::bigint)p
   from public.payroll_slips s where s.company_id=cid and s.employee_id=eid and extract(year from s.month)=yr and(s.month+interval '1month -1day')::date<=asof
  union all
  select jsonb_build_object('id',s.id,'starts_on',greatest(s.starts_on,lo),'ends_on',s.ends_on,'basic_cents',(s.item#>>'{totals,basic}')::bigint)p
   from public.payroll_cutoff_slips s where s.company_id=cid and s.employee_id=eid and extract(year from s.month)=yr and s.ends_on<=asof
 )x;
 select coalesce(jsonb_agg(jsonb_build_object('id',s.id,'gross_cents',(s.item#>>'{totals,gross_cents}')::bigint,'exempt_cents',(s.item#>>'{totals,exempt_cents}')::bigint)order by s.id),'[]'),coalesce(max((s.item#>>'{worksheet,external_thirteenth_cents}')::bigint),0),coalesce(max((s.item#>>'{worksheet,other_benefits_cents}')::bigint),0),max(s.as_of)
 into prior,ext,other,latest from public.payroll_thirteenth_slips s where s.company_id=cid and s.employee_id=eid and s.year=yr;
 if latest>asof then raise exception 'A later annual-benefit payout is already issued; reconcile through that coverage or later.'using errcode='23514';end if;
 return jsonb_build_object('employee',jsonb_build_object('id',e.id,'full_name',e.full_name,'employee_code',e.employee_code,'department',e.department,'joined_on',e.joined_on,'status',e.status),
  'calculation_source',jsonb_build_object('starts_on',lo,'as_of',asof,'periods',ps,'prior',prior,'external_thirteenth_floor',ext,'other_benefits_floor',other));
end;$$;
create function hris_private.thirteenth_check_item(i public.payroll_thirteenth_items,r public.payroll_thirteenth_runs)returns void language plpgsql stable security invoker set search_path=''as $$
declare fresh jsonb;t jsonb;begin
 fresh:=hris_private.thirteenth_basis(i.company_id,i.employee_id,r.year,r.as_of);
 if fresh is distinct from i.source_basis then raise exception 'Annual benefit sources changed. Refresh and review again.'using errcode='PT409';end if;
 t:=hris_private.thirteenth_ready(fresh->'calculation_source',i.worksheet);
 if t is distinct from i.totals then raise exception 'Annual benefit calculation does not reconcile.'using errcode='23514';end if;
end;$$;
create function hris_private.thirteenth_published_item(i public.payroll_thirteenth_items)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare j jsonb:=to_jsonb(i)-'source_basis';k text;begin
 foreach k in array array['eligibility_note','adjustment_note','policy_note','opening_note','tax_note','contribution_note','review_note']loop j:=jsonb_set(j,array['worksheet',k],'""');end loop;
 j:=jsonb_set(j,'{worksheet,openings}',coalesce((select jsonb_agg(o||jsonb_build_object('reference','')order by o->>'starts_on')from jsonb_array_elements(i.worksheet->'openings')o),'[]'));
 return j;
end;$$;

create function hris_private.manage_payroll_thirteenth(p_operation text,p_record jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;actor_label text;today date;rid uuid;expected bigint;why text;old_record jsonb;result jsonb;birth hris_private.payroll_birth;
 r public.payroll_thirteenth_runs;i public.payroll_thirteenth_items;fresh jsonb;w jsonb;t jsonb;creating boolean:=false;allowed text[];begin
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Active HR or Administrator access is required.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));perform 1 from public.companies where id=m.company_id for update;
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist')for update;
 if m.user_id is null then raise exception 'Your payroll access changed. Refresh.'using errcode='42501';end if;
 select full_name into actor_label from public.employees where company_id=m.company_id and id=m.employee_id;select(now()at time zone timezone)::date into today from public.companies where id=m.company_id;
 if p_operation is null or p_operation not in('run_create','run_update','item_add','item_save','item_refresh','item_remove','run_submit','run_return','run_cancel','run_approve','run_paid')or jsonb_typeof(p_record)is distinct from 'object'or octet_length(p_record::text)>131072 then raise exception 'Use a supported bounded annual benefit request.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=hris_private.payroll_text(p_record,'reason',500,true);
 if rid is null or rid='00000000-0000-0000-0000-000000000000'then raise exception 'An annual benefit record identity is required.'using errcode='23514';end if;
 allowed:=case p_operation when'run_create'then array['year','as_of','pay_date','title']when'run_update'then array['pay_date','title']when'item_add'then array['run_id','employee_id']when'item_save'then array['worksheet']when'run_submit'then array['scope_reviewed']when'run_paid'then array['paid_on','payment_reference']else array[]::text[]end;
 if p_record-(allowed||array['id','expected_revision','reason'])<>'{}'::jsonb then raise exception 'Unsupported annual benefit fields.'using errcode='23514';end if;
 if expected is null and p_operation in('run_create','item_add')then
  select * into birth from hris_private.payroll_birth where company_id=m.company_id and operation='thirteenth_'||p_operation and record_id=rid;
  if birth.record_id is not null then
   if birth.actor_id<>m.user_id or birth.payload<>p_record then raise exception 'This annual benefit record ID already exists.'using errcode='PT409';end if;
   if p_operation='run_create'then select to_jsonb(x)into result from public.payroll_thirteenth_runs x where id=rid and company_id=m.company_id;else select to_jsonb(x)into result from public.payroll_thirteenth_items x where id=rid and company_id=m.company_id;end if;
   if result is null then raise exception 'The prior record was removed. Refresh.'using errcode='PT409';end if;return result;
  end if;creating:=true;
 end if;
 if p_operation='run_create'then
  if not creating then raise exception 'Use a new run ID.'using errcode='23514';end if;
  insert into public.payroll_thirteenth_runs(id,company_id,year,as_of,pay_date,title,created_by)values(rid,m.company_id,(p_record->>'year')::integer,hris_private.thirteenth_date(p_record,'as_of'),(p_record->>'pay_date')::date,hris_private.payroll_text(p_record,'title',120,true),m.user_id)returning * into r;result:=to_jsonb(r);
 elsif p_operation in('item_add','item_save','item_refresh','item_remove')then
  if p_operation='item_add'then select * into r from public.payroll_thirteenth_runs where company_id=m.company_id and id=(p_record->>'run_id')::uuid for update;
  else select * into i from public.payroll_thirteenth_items where company_id=m.company_id and id=rid for update;
   if i.id is null then raise exception 'Annual benefit worksheet unavailable.'using errcode='42501';end if;
   if i.revision is distinct from expected then raise exception 'Annual benefit worksheet changed. Refresh.'using errcode='PT409';end if;
   select * into r from public.payroll_thirteenth_runs where company_id=m.company_id and id=i.run_id for update;old_record:=to_jsonb(i);
  end if;
  if r.id is null then raise exception 'Annual benefit run unavailable.'using errcode='42501';end if;
  if r.status<>'draft'then raise exception 'Only draft annual benefit worksheets can change. Issued payroll stays immutable.'using errcode='23514';end if;
  if p_operation='item_remove'then delete from public.payroll_thirteenth_items where id=i.id;result:=old_record;
  else
   fresh:=hris_private.thirteenth_basis(m.company_id,case when p_operation='item_add'then(p_record->>'employee_id')::uuid else i.employee_id end,r.year,r.as_of);
   if p_operation='item_save'and fresh is distinct from i.source_basis then raise exception 'Sources changed. Refresh source and review again.'using errcode='PT409';end if;
   w:=case when p_operation='item_add'then hris_private.thirteenth_seed(fresh->'calculation_source')when p_operation='item_save'then p_record->'worksheet'else i.worksheet||jsonb_build_object('reviewed',false,'review_note','','tax_reviewed',false,'contributions_reviewed',false)end;
   if p_operation='item_refresh'then w:=w||jsonb_build_object('external_thirteenth_cents',greatest((w->>'external_thirteenth_cents')::bigint,(fresh#>>'{calculation_source,external_thirteenth_floor}')::bigint),'other_benefits_cents',greatest((w->>'other_benefits_cents')::bigint,(fresh#>>'{calculation_source,other_benefits_floor}')::bigint));end if;
   begin t:=hris_private.thirteenth_compute(fresh->'calculation_source',w);
   exception when check_violation then
    if p_operation<>'item_refresh'then raise;end if;
    w:=hris_private.thirteenth_seed(fresh->'calculation_source');t:=hris_private.thirteenth_compute(fresh->'calculation_source',w);
   end;
   if p_operation='item_add'then
    insert into public.payroll_thirteenth_items(id,company_id,run_id,employee_id,employee_name,employee_code,department,source_basis,worksheet,totals)values(rid,m.company_id,r.id,(fresh#>>'{employee,id}')::uuid,fresh#>>'{employee,full_name}',fresh#>>'{employee,employee_code}',coalesce(fresh#>>'{employee,department}',''),fresh,w,t)returning * into i;
   else
    update public.payroll_thirteenth_items set employee_name=fresh#>>'{employee,full_name}',employee_code=fresh#>>'{employee,employee_code}',department=coalesce(fresh#>>'{employee,department}',''),source_basis=fresh,worksheet=w,totals=t,revision=revision+1 where id=rid returning * into i;
   end if;result:=to_jsonb(i);
  end if;
  update public.payroll_thirteenth_runs set revision=revision+1,updated_at=clock_timestamp()where id=r.id;
 else
  select * into r from public.payroll_thirteenth_runs where company_id=m.company_id and id=rid for update;
  if r.id is null then raise exception 'Annual benefit run unavailable.'using errcode='42501';end if;
  if r.revision is distinct from expected then raise exception 'Annual benefit run changed. Refresh.'using errcode='PT409';end if;old_record:=to_jsonb(r);
  if p_operation in('run_submit','run_approve')then
   if r.as_of>today or r.year<>2026 then raise exception 'Approve earned coverage only; a new year needs refreshed statutory review.'using errcode='23514';end if;
   if not exists(select 1 from public.payroll_thirteenth_items where run_id=r.id)then raise exception 'Add at least one reviewed worksheet.'using errcode='23514';end if;
   for i in select * from public.payroll_thirteenth_items where company_id=m.company_id and run_id=r.id order by id loop perform hris_private.thirteenth_check_item(i,r);end loop;
  end if;
  if p_operation='run_update'then
   if r.status<>'draft'then raise exception 'Only a draft annual benefit title/pay date can change.'using errcode='23514';end if;
   update public.payroll_thirteenth_runs set title=hris_private.payroll_text(p_record,'title',120,true),pay_date=(p_record->>'pay_date')::date,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='run_submit'then
   if r.status<>'draft'or(p_record->>'scope_reviewed')::boolean is not true then raise exception 'Confirm included/omitted employees before submitting a draft.'using errcode='23514';end if;
   update public.payroll_thirteenth_runs set status='submitted',submitted_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='run_return'then
   if r.status<>'submitted'then raise exception 'Only submitted annual benefit pay can return to draft.'using errcode='23514';end if;
   update public.payroll_thirteenth_runs set status='draft',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='run_cancel'then
   if r.status not in('draft','submitted')then raise exception 'Issued annual benefit pay cannot be cancelled or overwritten.'using errcode='23514';end if;
   update public.payroll_thirteenth_runs set status='cancelled',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='run_approve'then
   if m.role<>'administrator'or r.submitted_by=m.user_id then raise exception 'Another Administrator must approve annual benefit payroll.'using errcode='42501';end if;
   if r.status<>'submitted'then raise exception 'Submit this annual benefit before approval.'using errcode='23514';end if;
   insert into public.payroll_thirteenth_slips(id,company_id,run_id,employee_id,year,as_of,pay_date,company_name,item,approved_by,rule_version)
    select pi.id,pi.company_id,pi.run_id,pi.employee_id,r.year,r.as_of,r.pay_date,c.name,hris_private.thirteenth_published_item(pi),m.user_id,r.rule_version from public.payroll_thirteenth_items pi join public.companies c on c.id=pi.company_id where pi.company_id=m.company_id and pi.run_id=r.id;
   update public.payroll_thirteenth_runs set status='approved',approved_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  else
   if m.role<>'administrator'then raise exception 'Only Administrators record payments.'using errcode='42501';end if;
   if r.status<>'approved'or(p_record->>'paid_on')is null or(p_record->>'paid_on')::date>today or(p_record->>'paid_on')::date<r.as_of then raise exception 'Record an actual payment date after this annual benefit ends.'using errcode='23514';end if;
   update public.payroll_thirteenth_runs set status='paid',paid_on=(p_record->>'paid_on')::date,payment_reference=hris_private.payroll_text(p_record,'payment_reference',500,true),revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  end if;result:=to_jsonb(r);
 end if;
 if creating then insert into hris_private.payroll_birth(company_id,operation,record_id,actor_id,payload)values(m.company_id,'thirteenth_'||p_operation,rid,m.user_id,p_record);end if;
 insert into public.payroll_events(company_id,record_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)values(m.company_id,rid,'thirteenth_'||p_operation,why,m.user_id,actor_label,m.role,old_record,result);
 return result;
end;$$;

revoke all on function hris_private.thirteenth_empty(),hris_private.thirteenth_seed(jsonb),hris_private.thirteenth_basis(uuid,uuid,integer,date),hris_private.thirteenth_check_item(public.payroll_thirteenth_items,public.payroll_thirteenth_runs),hris_private.thirteenth_published_item(public.payroll_thirteenth_items),hris_private.manage_payroll_thirteenth(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_payroll_thirteenth(text,jsonb)to authenticated;
create function public.manage_payroll_thirteenth(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_payroll_thirteenth(p_operation,p_record);$$;
revoke all on function public.manage_payroll_thirteenth(text,jsonb)from public,anon,authenticated;
grant execute on function public.manage_payroll_thirteenth(text,jsonb)to authenticated;
commit;
