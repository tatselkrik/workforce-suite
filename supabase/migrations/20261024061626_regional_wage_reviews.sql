-- Additive employee wage reviews and compatibility guards for installed clients.
begin;
create table public.payroll_wage_classifications(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid not null,
 starts_on date not null,ends_on date not null,coverage text not null check(coverage in('covered','exempt')),series_key text not null,
 workplace text not null check(char_length(btrim(workplace))between 1 and 240),evidence text not null check(char_length(btrim(evidence))between 1 and 1000),note text not null check(char_length(btrim(note))between 1 and 1000),
 employee_basis jsonb not null check(jsonb_typeof(employee_basis)='object'),status text not null default'draft'check(status in('draft','approved','retired','cancelled')),revision bigint not null default 0 check(revision>=0),
 created_by uuid not null references auth.users(id),prepared_by uuid not null references auth.users(id),approved_by uuid references auth.users(id),approved_at timestamptz,
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),unique(company_id,id),
 foreign key(company_id,employee_id)references public.employees(company_id,id),
 check(starts_on<=ends_on and extract(year from starts_on)=2026 and extract(year from ends_on)=2026),
 check((status in('approved','retired')and approved_by is not null and approved_at is not null and approved_by<>created_by and approved_by<>prepared_by)or(status in('draft','cancelled')and approved_by is null and approved_at is null)),
 check((coverage='covered'and btrim(series_key)<>'')or(coverage='exempt'and series_key='')),
 exclude using gist(company_id with =,employee_id with =,daterange(starts_on,ends_on,'[]')with &&)where(status='approved')
);
create index payroll_wage_classifications_employee_idx on public.payroll_wage_classifications(company_id,employee_id,starts_on,ends_on);
create index payroll_wage_classifications_creator_idx on public.payroll_wage_classifications(created_by);
create index payroll_wage_classifications_preparer_idx on public.payroll_wage_classifications(prepared_by);
create index payroll_wage_classifications_approver_idx on public.payroll_wage_classifications(approved_by);
create table public.payroll_wage_settings(
 id uuid primary key references public.companies(id),company_id uuid not null unique references public.companies(id),enforced boolean not null,revision bigint not null check(revision>0),updated_by uuid not null references auth.users(id),updated_at timestamptz not null default clock_timestamp(),check(id=company_id)
);
create index payroll_wage_settings_actor_idx on public.payroll_wage_settings(updated_by);
create table public.payroll_wage_checks(
 id uuid primary key default gen_random_uuid(),company_id uuid not null references public.companies(id),run_id uuid not null,kind text not null check(kind in('monthly','cutoff')),stage text not null check(stage in('submitted','approved')),run_revision bigint not null,enforced boolean not null,assessment jsonb not null check(octet_length(assessment::text)<=16777216),created_by uuid not null references auth.users(id),recorded_at timestamptz not null default clock_timestamp(),unique(company_id,kind,run_id,run_revision,stage)
);
create index payroll_wage_checks_run_idx on public.payroll_wage_checks(company_id,kind,run_id,run_revision);
create index payroll_wage_checks_actor_idx on public.payroll_wage_checks(created_by);
do $$declare t text;begin foreach t in array array['payroll_wage_classifications','payroll_wage_settings','payroll_wage_checks']loop
 execute format('alter table public.%I enable row level security',t);execute format('revoke all on public.%I from public,anon,authenticated',t);execute format('grant select on public.%I to authenticated',t);
 execute format('create policy wage_hr_read on public.%I for select to authenticated using(hris_private.payroll_hr(company_id))',t);
end loop;end;$$;
-- Routine desktop refresh reads compact results, not repeated private source bundles.
create view public.payroll_wage_check_index with(security_invoker=true)as
 select c.id,c.company_id,c.run_id,c.kind,c.stage,c.run_revision,c.enforced,c.recorded_at,
 jsonb_build_object('version',c.assessment->'version','enforced',c.enforced,'settings_revision',c.assessment->'settings_revision','employees',coalesce((select jsonb_agg(x-'source'order by x->>'employee_id')from jsonb_array_elements(c.assessment->'employees')x),'[]'))assessment
 from public.payroll_wage_checks c;
revoke all on public.payroll_wage_check_index from public,anon,authenticated;
grant select on public.payroll_wage_check_index to authenticated;
create trigger payroll_wage_checks_immutable before update or delete on public.payroll_wage_checks for each row execute function hris_private.employment_history_immutable();
create function hris_private.wage_classification_immutable()returns trigger language plpgsql security invoker set search_path=''as $fn$
begin
 if tg_op='DELETE'or old.status in('retired','cancelled')then raise exception 'Retain wage review history.'using errcode='42501';end if;
 if old.status='approved'and(new.status<>'retired'or(to_jsonb(new)-array['status','revision','updated_at'])is distinct from(to_jsonb(old)-array['status','revision','updated_at']))then raise exception 'Approved classification facts remain immutable; retire and replace with reviewed intervals.'using errcode='42501';end if;
 return new;
end;$fn$;
create trigger wage_classification_immutable before update or delete on public.payroll_wage_classifications for each row execute function hris_private.wage_classification_immutable();
create function hris_private.wage_employee_basis(cid uuid,eid uuid)returns jsonb language sql stable security invoker set search_path=''as $fn$
 select jsonb_build_object('work_location_id',work_location_id,'employment_type_id',employment_type_id)from public.employees where company_id=cid and id=eid;
$fn$;
create function hris_private.wage_run_assessment(cid uuid,rid uuid,which text)returns jsonb language plpgsql stable security invoker set search_path=''as $fn$
declare lo date;hi date;x record;src jsonb;cls jsonb;rows jsonb:='[]';settings public.payroll_wage_settings;count_rows integer:=0;begin
 select *into settings from public.payroll_wage_settings where company_id=cid;
 if which='monthly'then select month,(month+interval'1month -1day')::date into lo,hi from public.payroll_runs where company_id=cid and id=rid;
 elsif which='cutoff'then select starts_on,ends_on into lo,hi from public.payroll_cutoff_runs where company_id=cid and id=rid;
 else raise exception 'Choose monthly/cutoff wage checks.'using errcode='23514';end if;
 if lo is null then raise exception 'Payroll run unavailable.'using errcode='42501';end if;
 for x in
  select i.employee_id,i.employee_name,i.employee_code,i.profile,e.joined_on from public.payroll_items i join public.employees e on e.company_id=i.company_id and e.id=i.employee_id where which='monthly'and i.company_id=cid and i.run_id=rid
  union all select i.employee_id,i.employee_name,i.employee_code,i.profile,e.joined_on from public.payroll_cutoff_items i join public.employees e on e.company_id=i.company_id and e.id=i.employee_id where which='cutoff'and i.company_id=cid and i.run_id=rid
  order by employee_id
 loop
  count_rows:=count_rows+1;if count_rows>500 then raise exception 'At most500 wage-reviewed recipients.'using errcode='23514';end if;
  select coalesce(jsonb_agg(to_jsonb(c)order by c.id),'[]')into cls from public.payroll_wage_classifications c where c.company_id=cid and c.employee_id=x.employee_id and c.status='approved'and c.starts_on<=hi and c.ends_on>=greatest(lo,coalesce(x.joined_on,lo));
  src:=jsonb_build_object('company_id',cid,'employee_id',x.employee_id,'pay',jsonb_build_object('basis',x.profile#>'{pay,basis}','rate_cents',x.profile#>'{pay,rate_cents}','annual_divisor',x.profile#>'{pay,annual_divisor}'),'employee_basis',hris_private.wage_employee_basis(cid,x.employee_id),'starts_on',greatest(lo,coalesce(x.joined_on,lo)),'ends_on',hi,'classifications',cls);
  rows:=rows||jsonb_build_array(jsonb_build_object('employee_id',x.employee_id,'employee_code',x.employee_code,'employee_name',x.employee_name,'source',src,'result',hris_private.regional_wage_compute(src)));
 end loop;
 return jsonb_build_object('version',hris_private.regional_wage_catalog()->'version','enforced',coalesce(settings.enforced,false),'settings_revision',coalesce(settings.revision,0),'employees',rows);
end;$fn$;
create function hris_private.wage_payroll_guard()returns trigger language plpgsql security definer set search_path=''as $fn$
declare m public.company_members;which text;assessment jsonb;previous jsonb;enforced boolean;begin
 if new.status not in('submitted','approved')or new.status=old.status then return new;end if;
 select *into m from public.company_members where user_id=(select auth.uid())and company_id=new.company_id and active and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Current HR/Admin payroll access required.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(new.company_id::text,7));perform 1 from public.companies where id=new.company_id for update;
 select *into m from public.company_members where user_id=(select auth.uid())and company_id=new.company_id and active and employee_id is not null and role in('administrator','hr_generalist')for update;
 if m.user_id is null or(new.status='approved'and m.role<>'administrator')then raise exception 'Your payroll approval access changed.'using errcode='42501';end if;
 which:=case tg_table_name when'payroll_runs'then'monthly'else'cutoff'end;
 assessment:=hris_private.wage_run_assessment(new.company_id,new.id,which);enforced:=(assessment->>'enforced')::boolean;
 if enforced and exists(select 1 from jsonb_array_elements(assessment->'employees')x where(x#>>'{result,blocking_days}')::integer<>0)then raise exception 'Resolve regional wage flags for every included employee before submitting/approving payroll.'using errcode='23514';end if;
 if new.status='approved'then
  select c.assessment into previous from public.payroll_wage_checks c where c.company_id=new.company_id and c.kind=which and c.run_id=new.id and c.stage='submitted'order by c.run_revision desc limit 1;
  if previous is null and enforced then raise exception 'Return and resubmit this older payroll to record current wage checks.'using errcode='PT409';end if;
  if previous is not null and previous is distinct from assessment then raise exception 'Wage facts changed after submission. Return and resubmit payroll.'using errcode='PT409';end if;
 end if;
 insert into public.payroll_wage_checks(company_id,run_id,kind,stage,run_revision,enforced,assessment,created_by)values(new.company_id,new.id,which,new.status,new.revision,enforced,assessment,m.user_id);
 return new;
end;$fn$;
create trigger payroll_monthly_wage_guard before update of status on public.payroll_runs for each row execute function hris_private.wage_payroll_guard();
create trigger payroll_cutoff_wage_guard before update of status on public.payroll_cutoff_runs for each row execute function hris_private.wage_payroll_guard();
create function hris_private.manage_payroll_wage(p_operation text,p_record jsonb)returns jsonb language plpgsql security definer set search_path=''as $fn$
declare m public.company_members;c public.payroll_wage_classifications;settings public.payroll_wage_settings;birth hris_private.payroll_birth;rid uuid;eid uuid;expected bigint;why text;allowed text[];old_record jsonb;result jsonb;basis jsonb;test jsonb;actor_label text;begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Current HR/Admin access required.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));perform 1 from public.companies where id=m.company_id for update;
 select *into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist')for update;
 if m.user_id is null then raise exception 'Your wage-review access changed.'using errcode='42501';end if;
 if p_operation is null or p_operation not in('create','save','approve','retire','cancel','settings')or jsonb_typeof(p_record)is distinct from'object'or octet_length(p_record::text)>16384 then raise exception 'Use a supported bounded wage-review operation.'using errcode='23514';end if;
 if p_record->>'catalog_version'is distinct from hris_private.regional_wage_catalog()->>'version'then raise exception 'Update the wage catalog before changing a review.'using errcode='PT409';end if;
 if p_operation in('approve','retire','settings')and m.role<>'administrator'then raise exception 'Only Administrators approve/retire wage reviews or configure enforcement.'using errcode='42501';end if;
 allowed:=case p_operation when'create'then array['employee_id','starts_on','ends_on','coverage','series_key','workplace','evidence','note']when'save'then array['starts_on','ends_on','coverage','series_key','workplace','evidence','note']when'settings'then array['enforced']else array[]::text[]end;
 if p_record-(allowed||array['id','expected_revision','reason','catalog_version'])<>'{}'then raise exception 'Unsupported wage fields.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=hris_private.payroll_text(p_record,'reason',500,true);
 if rid is null or rid='00000000-0000-0000-0000-000000000000'then raise exception 'Wage identity required.'using errcode='23514';end if;
 select full_name into actor_label from public.employees where company_id=m.company_id and id=m.employee_id;
 if p_operation='settings'then
  if rid<>m.company_id or jsonb_typeof(p_record->'enforced')is distinct from'boolean'then raise exception 'Choose this company and an enforcement state.'using errcode='23514';end if;
  select *into settings from public.payroll_wage_settings where id=rid for update;
  if expected is null or expected is distinct from coalesce(settings.revision,0)then raise exception 'Wage settings changed. Refresh.'using errcode='PT409';end if;
  old_record:=to_jsonb(settings);
  insert into public.payroll_wage_settings(id,company_id,enforced,revision,updated_by)values(rid,rid,(p_record->>'enforced')::boolean,1,m.user_id)
  on conflict(id)do update set enforced=excluded.enforced,revision=public.payroll_wage_settings.revision+1,updated_by=m.user_id,updated_at=clock_timestamp()returning to_jsonb(payroll_wage_settings.*)into result;
 else
  if p_operation='create'then
   select *into birth from hris_private.payroll_birth where company_id=m.company_id and operation='wage_create'and record_id=rid;
   if birth.record_id is not null then
    if birth.actor_id<>m.user_id or birth.payload<>p_record then raise exception 'Wage creation identity already used.'using errcode='PT409';end if;
    select to_jsonb(x)into result from public.payroll_wage_classifications x where x.company_id=m.company_id and x.id=rid;return result;
   end if;
   if expected is not null then raise exception 'New wage reviews have no revision.'using errcode='23514';end if;
   eid:=(p_record->>'employee_id')::uuid;basis:=hris_private.wage_employee_basis(m.company_id,eid);
   if basis is null then raise exception 'Employee unavailable.'using errcode='42501';end if;
   insert into public.payroll_wage_classifications(id,company_id,employee_id,starts_on,ends_on,coverage,series_key,workplace,evidence,note,employee_basis,created_by,prepared_by)
   values(rid,m.company_id,eid,(p_record->>'starts_on')::date,(p_record->>'ends_on')::date,hris_private.payroll_text(p_record,'coverage',20,true),hris_private.payroll_text(p_record,'series_key',100,false),hris_private.payroll_text(p_record,'workplace',240,true),hris_private.payroll_text(p_record,'evidence',1000,true),hris_private.payroll_text(p_record,'note',1000,true),basis,m.user_id,m.user_id)returning *into c;
   insert into hris_private.payroll_birth(company_id,operation,record_id,actor_id,payload)values(m.company_id,'wage_create',rid,m.user_id,p_record);
  else
   select *into c from public.payroll_wage_classifications where company_id=m.company_id and id=rid for update;
   if c.id is null then raise exception 'Wage review unavailable.'using errcode='42501';end if;
   if expected is null or expected<>c.revision then raise exception 'Wage review changed. Refresh.'using errcode='PT409';end if;old_record:=to_jsonb(c);
   if p_operation='save'then
    if c.status<>'draft'then raise exception 'Only draft wage reviews may change.'using errcode='23514';end if;
    update public.payroll_wage_classifications set starts_on=(p_record->>'starts_on')::date,ends_on=(p_record->>'ends_on')::date,coverage=hris_private.payroll_text(p_record,'coverage',20,true),series_key=hris_private.payroll_text(p_record,'series_key',100,false),workplace=hris_private.payroll_text(p_record,'workplace',240,true),evidence=hris_private.payroll_text(p_record,'evidence',1000,true),note=hris_private.payroll_text(p_record,'note',1000,true),employee_basis=hris_private.wage_employee_basis(m.company_id,c.employee_id),prepared_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into c;
   elsif p_operation='approve'then
    if c.status<>'draft'or m.user_id in(c.created_by,c.prepared_by)then raise exception 'A different Administrator must approve this wage classification.'using errcode='42501';end if;
    if c.employee_basis is distinct from hris_private.wage_employee_basis(m.company_id,c.employee_id)then raise exception 'Workplace/employment changed. Save a fresh draft review.'using errcode='PT409';end if;
    update public.payroll_wage_classifications set status='approved',approved_by=m.user_id,approved_at=clock_timestamp(),revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into c;
    test:=hris_private.regional_wage_compute(jsonb_build_object('company_id',c.company_id,'employee_id',c.employee_id,'pay',jsonb_build_object('basis','monthly','rate_cents',10000000000,'annual_divisor',365),'employee_basis',c.employee_basis,'starts_on',c.starts_on,'ends_on',c.ends_on,'classifications',jsonb_build_array(to_jsonb(c))));
    if(test->>'blocking_days')::integer<>0 then raise exception 'Some dates have no verified rate. Use supported dates or review coverage.'using errcode='23514';end if;
   elsif p_operation='retire'then
    if c.status<>'approved'then raise exception 'Only approved wage classifications retire.'using errcode='23514';end if;
    update public.payroll_wage_classifications set status='retired',revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into c;
   else
    if c.status<>'draft'then raise exception 'Only draft wage reviews cancel.'using errcode='23514';end if;
    update public.payroll_wage_classifications set status='cancelled',revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into c;
   end if;
  end if;
  if c.coverage='covered'and not exists(select 1 from jsonb_array_elements(hris_private.regional_wage_catalog()->'rates')x where x->>'series_key'=c.series_key)then raise exception 'Choose a verified regional category.'using errcode='23514';end if;
  result:=to_jsonb(c);
 end if;
 insert into public.payroll_events(company_id,record_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)values(m.company_id,rid,'wage_'||p_operation,why,m.user_id,actor_label,m.role,old_record,result);
 return result;
end;$fn$;
revoke all on function hris_private.wage_classification_immutable(),hris_private.wage_employee_basis(uuid,uuid),hris_private.wage_run_assessment(uuid,uuid,text),hris_private.wage_payroll_guard(),hris_private.manage_payroll_wage(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_payroll_wage(text,jsonb)to authenticated;
create function public.manage_payroll_wage(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_payroll_wage(p_operation,p_record);$$;
revoke all on function public.manage_payroll_wage(text,jsonb)from public,anon,authenticated;
grant execute on function public.manage_payroll_wage(text,jsonb)to authenticated;
commit;
