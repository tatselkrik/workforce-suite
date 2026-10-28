-- HR-reviewed benefits only. No canonical, monetary, provider or portal side effects.
begin;
create function hris_private.benefit_keys(v jsonb, allowed text[]) returns boolean
language sql immutable security invoker set search_path='' as $$
 select case when jsonb_typeof(v)='object' then
 (select count(*)=cardinality(allowed) and coalesce(bool_and(k=any(allowed)),false) from jsonb_object_keys(v)k)
 else false end
$$;
create function hris_private.benefit_uuid(v jsonb) returns boolean
language plpgsql immutable security invoker set search_path='' as $$
begin return coalesce(jsonb_typeof(v)='string' and (v#>>'{}')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid,false);
exception when invalid_text_representation then return false;end;$$;
create function hris_private.benefit_date(v jsonb) returns boolean
language plpgsql immutable security invoker set search_path='' as $$
declare t text:=v#>>'{}'; d date;
begin
 if jsonb_typeof(v) is distinct from 'string' or t!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$' then return false;end if;
 d:=t::date;return d between date'1900-01-01' and date'2100-12-31' and to_char(d,'YYYY-MM-DD')=t;
exception when invalid_datetime_format or datetime_field_overflow then return false;end;$$;
create function hris_private.benefit_plan_valid(v jsonb) returns boolean
language sql immutable security invoker set search_path='' as $$
 select coalesce(hris_private.benefit_keys(v,array['code','name','category','description','eligibility_guidance','dependent_limit','available'])
 and jsonb_typeof(v->'code')='string' and v->>'code'~'^[A-Z0-9_-]{2,32}$'
 and jsonb_typeof(v->'name')='string' and hris_private.service_text(v->>'name',3,160)
 and v->>'category' in('health_insurance','wellness','retirement','other')
 and jsonb_typeof(v->'description')='string' and hris_private.service_text(v->>'description',3,2000)
 and jsonb_typeof(v->'eligibility_guidance')='string' and hris_private.service_text(v->>'eligibility_guidance',3,2000)
 and jsonb_typeof(v->'dependent_limit')='number' and v->>'dependent_limit'~'^[0-9]{1,2}$'
 and (v->>'dependent_limit')::integer between 0 and 25 and jsonb_typeof(v->'available')='boolean',false)
$$;
create function hris_private.benefit_employee(e public.employees) returns jsonb
language sql immutable security invoker set search_path='' as $$
 select jsonb_build_object('id',e.id,'company_id',e.company_id,'employee_code',e.employee_code,'full_name',e.full_name,'department',e.department,'job_title',e.job_title,'joined_on',e.joined_on,'employment_type_id',e.employment_type_id,'work_location_id',e.work_location_id,'active',e.status='active')
$$;
create function hris_private.benefit_enrollment_valid(v jsonb, company uuid) returns boolean
language plpgsql immutable security invoker set search_path='' as $$
declare s jsonb; d jsonb; a date; b date; da date; de date; seen uuid[]:='{}'; dep uuid;
begin
 if not hris_private.benefit_keys(v,array['plan_id','plan_revision','plan_snapshot','employee_source','eligibility','eligibility_basis','status','coverage_from','coverage_to','dependents'])
 or not hris_private.benefit_uuid(v->'plan_id') or jsonb_typeof(v->'plan_revision') is distinct from 'number'
 or coalesce(v->>'plan_revision','')!~'^[0-9]{1,5}$' or (v->>'plan_revision')::bigint not between 1 and 10000
 or not hris_private.benefit_plan_valid(v->'plan_snapshot') or jsonb_typeof(v->'dependents') is distinct from 'array'
 or v->>'eligibility' is null or v->>'eligibility' not in('pending','eligible','ineligible')
 or jsonb_typeof(v->'eligibility_basis') is distinct from 'string' or not hris_private.service_text(v->>'eligibility_basis',3,2000)
 or v->>'status' is null or v->>'status' not in('pending','active','declined','ended')
 or not hris_private.benefit_date(v->'coverage_from') or (v->'coverage_to'<>'null'::jsonb and not hris_private.benefit_date(v->'coverage_to')) then return false;end if;
 s:=v->'employee_source';
 if not hris_private.benefit_keys(s,array['id','company_id','employee_code','full_name','department','job_title','joined_on','employment_type_id','work_location_id','active'])
 or not hris_private.benefit_uuid(s->'id') or not hris_private.benefit_uuid(s->'company_id') or (s->>'company_id')::uuid<>company
 or jsonb_typeof(s->'employee_code') is distinct from 'string' or not hris_private.service_text(s->>'employee_code',1,32)
 or jsonb_typeof(s->'full_name') is distinct from 'string' or not hris_private.service_text(s->>'full_name',1,200)
 or jsonb_typeof(s->'department') is distinct from 'string' or not hris_private.service_text(s->>'department',0,100)
 or jsonb_typeof(s->'job_title') is distinct from 'string' or not hris_private.service_text(s->>'job_title',0,100)
 or (s->'joined_on'<>'null'::jsonb and not hris_private.benefit_date(s->'joined_on'))
 or (s->'employment_type_id'<>'null'::jsonb and not hris_private.benefit_uuid(s->'employment_type_id'))
 or (s->'work_location_id'<>'null'::jsonb and not hris_private.benefit_uuid(s->'work_location_id'))
 or jsonb_typeof(s->'active') is distinct from 'boolean' then return false;end if;
 a:=(v->>'coverage_from')::date;b:=(v->>'coverage_to')::date;
 if (b is not null and b<a) or (v->>'status'='active' and v->>'eligibility'<>'eligible')
 or (v->>'status'='declined' and v->>'eligibility'='eligible') or (v->>'status'='ended' and b is null)
 or jsonb_array_length(v->'dependents')>(v#>>'{plan_snapshot,dependent_limit}')::integer then return false;end if;
 for d in select value from jsonb_array_elements(v->'dependents')loop
 if not hris_private.benefit_keys(d,array['id','full_name','relationship','birth_date','coverage_from','coverage_to'])
 or not hris_private.benefit_uuid(d->'id') or jsonb_typeof(d->'full_name') is distinct from 'string' or not hris_private.service_text(d->>'full_name',1,200)
 or d->>'relationship' is null or d->>'relationship' not in('spouse','child','parent','sibling','other')
 or not hris_private.benefit_date(d->'coverage_from') or (d->'coverage_to'<>'null'::jsonb and not hris_private.benefit_date(d->'coverage_to'))
 or (d->'birth_date'<>'null'::jsonb and not hris_private.benefit_date(d->'birth_date')) then return false;end if;
 dep:=(d->>'id')::uuid;da:=(d->>'coverage_from')::date;de:=(d->>'coverage_to')::date;
 if dep=any(seen) or da<a or (de is not null and de<da) or (b is not null and (da>b or de is null or de>b))
 or (d->>'birth_date')::date>da then return false;end if;
 seen:=array_append(seen,dep);
 end loop;return true;
end;$$;

create table hris_private.benefit_records(
 id uuid primary key, company_id uuid not null references public.companies(id) on delete restrict,
 kind text not null check(kind in('plan','enrollment')), revision bigint not null check(revision between 1 and 10000),
 data jsonb not null, employee_id uuid references public.employees(id) on delete restrict,
 plan_id uuid references hris_private.benefit_records(id) on delete restrict,
 created_at timestamptz not null, updated_at timestamptz not null check(updated_at>=created_at),
 check((kind='plan' and employee_id is null and plan_id is null and hris_private.benefit_plan_valid(data))
 or (kind='enrollment' and employee_id is not null and plan_id is not null and hris_private.benefit_enrollment_valid(data,company_id)))
);
create unique index benefit_plan_code on hris_private.benefit_records(company_id,(data->>'code'))where kind='plan';
create index benefit_records_company on hris_private.benefit_records(company_id,kind,id);
create index benefit_records_employee on hris_private.benefit_records(employee_id,plan_id);
create index benefit_records_plan on hris_private.benefit_records(plan_id);
create table hris_private.benefit_events(
 id uuid primary key, company_id uuid not null references public.companies(id) on delete restrict,
 subject_id uuid not null references hris_private.benefit_records(id) on delete restrict,
 actor_id uuid not null references auth.users(id) on delete restrict,
 actor_name text not null check(hris_private.service_text(actor_name,1,200)),
 revision bigint not null check(revision between 1 and 10000),body text not null check(hris_private.service_text(body,3,2000)),
 previous_state jsonb,current_state jsonb not null,payload jsonb not null,receipt jsonb not null,
 created_at timestamptz not null,unique(subject_id,revision)
);
create index benefit_events_company on hris_private.benefit_events(company_id,subject_id,revision);
create index benefit_events_actor on hris_private.benefit_events(actor_id);
alter table hris_private.benefit_records enable row level security;
alter table hris_private.benefit_events enable row level security;
revoke all on hris_private.benefit_records,hris_private.benefit_events from public,anon,authenticated,service_role;
create function hris_private.keep_benefit_history()returns trigger language plpgsql security invoker set search_path=''as $$
begin raise exception 'Benefit records and history are retained.'using errcode='23514';end;$$;
create trigger benefit_events_immutable before update or delete on hris_private.benefit_events for each row execute function hris_private.keep_benefit_history();
create trigger benefit_records_retained before delete on hris_private.benefit_records for each row execute function hris_private.keep_benefit_history();

create function hris_private.benefit_member()returns public.company_members
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;
begin
 select cm.* into m from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id and e.status='active'
 where cm.user_id=(select auth.uid())and cm.active and hris_private.has_staff_capability(cm.company_id,'desktop')and hris_private.has_staff_capability(cm.company_id,'personnel');
 if m.user_id is null or m.user_id='00000000-0000-0000-0000-000000000000'::uuid then raise exception 'Current HR benefits access is required.'using errcode='42501';end if;return m;
end;$$;
create function hris_private.benefit_record(r hris_private.benefit_records)returns jsonb
language sql immutable security invoker set search_path=''as $$
 select jsonb_build_object('id',r.id,'company_id',r.company_id,'revision',r.revision,'created_at',r.created_at,'updated_at',r.updated_at,'data',r.data)
$$;
create function hris_private.benefit_context()returns jsonb
language plpgsql volatile security definer set search_path=''as $$
declare m public.company_members;result jsonb;plans jsonb;enrollments jsonb;events jsonb;people jsonb;today date;
begin
 m:=hris_private.benefit_member();
 if(select count(*)from public.employees where company_id=m.company_id)>10000
 or(select count(*)from hris_private.benefit_records where company_id=m.company_id and kind='plan')>500
 or(select count(*)from hris_private.benefit_records where company_id=m.company_id and kind='enrollment')>5000
 or(select count(*)from hris_private.benefit_events where company_id=m.company_id)>20000 then
 raise exception 'Benefits history exceeds its supported budget. Nothing was truncated.'using errcode='54000';end if;
 select(clock_timestamp()at time zone c.timezone)::date into today from public.companies c where id=m.company_id;
 select coalesce(jsonb_agg(hris_private.benefit_employee(e)order by e.employee_code,e.id),'[]')into people from public.employees e where e.company_id=m.company_id;
 select coalesce(jsonb_agg(hris_private.benefit_record(r)order by r.data->>'code',r.id),'[]')into plans from hris_private.benefit_records r where r.company_id=m.company_id and r.kind='plan';
 select coalesce(jsonb_agg(hris_private.benefit_record(r)order by r.updated_at desc,r.id),'[]')into enrollments from hris_private.benefit_records r where r.company_id=m.company_id and r.kind='enrollment';
 select coalesce(jsonb_agg(jsonb_build_object('id',e.id,'company_id',e.company_id,'actor_id',e.actor_id,'actor_name',e.actor_name,'body',e.body,'created_at',e.created_at,'previous',e.previous_state,'current',e.current_state)order by e.subject_id,e.revision),'[]')into events from hris_private.benefit_events e where e.company_id=m.company_id;
 result:=jsonb_build_object('schema_version',1,'company_id',m.company_id,'user_id',m.user_id,'today',today,'employees',people,'plans',plans,'enrollments',enrollments,'events',events);
 if octet_length(result::text)>8*1024*1024 then raise exception 'Benefits history exceeds its byte budget. Nothing was truncated.'using errcode='54000';end if;
 return result;
end;$$;

create function hris_private.save_benefit(p_change jsonb)returns jsonb
language plpgsql volatile security definer set search_path=''as $$
<<benefit_change>>
declare m public.company_members;r hris_private.benefit_records; prior hris_private.benefit_events; p hris_private.benefit_records;
 key_id uuid;subject uuid;kind text;v_data jsonb;expected bigint;old_state jsonb;current_state jsonb;previous_data jsonb;result jsonb;
 employee public.employees;employee_id uuid;plan_id uuid;plan_revision bigint;at_time timestamptz;today date;actor_name text;next_status text;old_status text;
begin
 m:=hris_private.benefit_member();perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));m:=hris_private.benefit_member();
 if p_change is null or pg_column_size(p_change)>24576 or not hris_private.benefit_keys(p_change,array['id','company_id','actor_id','subject_id','expected_revision','body','change'])
 or not hris_private.benefit_uuid(p_change->'id')or not hris_private.benefit_uuid(p_change->'subject_id')
 or(p_change->>'company_id')::uuid is distinct from m.company_id or(p_change->>'actor_id')::uuid is distinct from m.user_id
 or jsonb_typeof(p_change->'body')is distinct from 'string'or not hris_private.service_text(p_change->>'body',3,2000)
 or jsonb_typeof(p_change->'expected_revision')is distinct from 'number'or coalesce(p_change->>'expected_revision','')!~'^[0-9]{1,5}$'
 or not hris_private.benefit_keys(p_change->'change',array['kind','data'])then raise exception 'Use the complete current benefits request.'using errcode='23514';end if;
 key_id:=(p_change->>'id')::uuid;subject:=(p_change->>'subject_id')::uuid;expected:=(p_change->>'expected_revision')::bigint;kind:=p_change#>>'{change,kind}';v_data:=p_change#>'{change,data}';
 if expected not between 0 and 9999 or kind is null or kind not in('plan','enrollment')then raise exception 'Review benefits kind and revision.'using errcode='23514';end if;
 select * into r from hris_private.benefit_records where id=subject for update;
 if r.id is not null and(r.company_id<>m.company_id or r.kind<>kind)then raise exception 'Benefits record is outside current scope.'using errcode='42501';end if;
 select * into prior from hris_private.benefit_events where id=key_id;
 if prior.id is not null then
 if prior.actor_id<>m.user_id or prior.company_id<>m.company_id or prior.payload<>p_change or r.id is null then raise exception 'Benefits retry identity belongs to another payload.'using errcode='23514';end if;
 return prior.receipt;end if;
 if coalesce(r.revision,0)<>expected then raise exception 'Benefits record changed; refresh and review.'using errcode='23514';end if;
 if r.id is not null and r.data=v_data then raise exception 'Change the saved benefits record before submitting.'using errcode='23514';end if;
 at_time:=clock_timestamp();select(at_time at time zone c.timezone)::date into today from public.companies c where id=m.company_id;
 old_state:=case when r.id is null then null else jsonb_build_object('kind',r.kind,'record',hris_private.benefit_record(r))end;
 previous_data:=r.data;
 if kind='plan'then
 if not hris_private.benefit_plan_valid(v_data)then raise exception 'Review benefit plan details and eligibility guidance.'using errcode='23514';end if;
 else
 if not hris_private.benefit_enrollment_valid(v_data,m.company_id)then raise exception 'Review eligibility, dependents and coverage dates.'using errcode='23514';end if;
 employee_id:=(v_data#>>'{employee_source,id}')::uuid;plan_id:=(v_data->>'plan_id')::uuid;plan_revision:=(v_data->>'plan_revision')::bigint;next_status:=v_data->>'status';old_status:=previous_data->>'status';
 select * into employee from public.employees where id=employee_id and company_id=m.company_id for share;
 if employee.id is null then raise exception 'Employee is outside current company.'using errcode='42501';end if;
 if previous_data is not null then
 if (previous_data#>>'{employee_source,id}')::uuid<>employee_id or(previous_data->>'plan_id')::uuid<>plan_id
 or not(case old_status when 'pending'then next_status in('pending','active','declined')when 'active'then next_status in('active','ended')when 'declined'then next_status in('pending','declined')else false end)then raise exception 'Retain enrollment identity and terminal history.'using errcode='23514';end if;
 elsif next_status='ended'then raise exception 'Choose existing active coverage to end.'using errcode='23514';end if;
 if next_status='ended'then
 if previous_data->'plan_revision'<>v_data->'plan_revision'or previous_data->'plan_snapshot'<>v_data->'plan_snapshot'
 or previous_data->'employee_source'<>v_data->'employee_source'or previous_data->'coverage_from'<>v_data->'coverage_from'
 or previous_data->'eligibility'<>v_data->'eligibility'or previous_data->'eligibility_basis'<>v_data->'eligibility_basis'then raise exception 'End coverage against its original reviewed source.'using errcode='23514';end if;
 else
 select x.* into p from hris_private.benefit_records x where x.id=benefit_change.plan_id and x.company_id=m.company_id and x.kind='plan'for share;
 if p.id is null or p.revision<>plan_revision or p.data<>v_data->'plan_snapshot'or not(p.data->>'available')::boolean
 or hris_private.benefit_employee(employee)<>v_data->'employee_source'or(next_status='active'and employee.status<>'active')then raise exception 'Plan or employment changed; review current eligibility.'using errcode='23514';end if;
 end if;
 if next_status='ended'and (
 (previous_data->>'coverage_to'is not null and(v_data->>'coverage_to')::date>(previous_data->>'coverage_to')::date)
 or jsonb_array_length(previous_data->'dependents')<>jsonb_array_length(v_data->'dependents')
 or exists(select 1 from jsonb_array_elements(previous_data->'dependents')old_d where not exists(
 select 1 from jsonb_array_elements(v_data->'dependents')new_d where new_d-'coverage_to'=old_d-'coverage_to'
 and(new_d->>'coverage_to')::date=least(coalesce((old_d->>'coverage_to')::date,(v_data->>'coverage_to')::date),(v_data->>'coverage_to')::date))))then
 raise exception 'End the existing dependents at their original or earlier parent end date; do not rewrite them.'using errcode='23514';end if;
 if exists(select 1 from jsonb_array_elements(v_data->'dependents')d where(d->>'birth_date')::date>today)then raise exception 'Recorded birth dates cannot be future dates.'using errcode='23514';end if;
 if next_status<>'declined'and exists(select 1 from hris_private.benefit_records x where x.company_id=m.company_id and x.kind='enrollment'and x.employee_id=benefit_change.employee_id and x.plan_id=benefit_change.plan_id and x.id<>subject and x.data->>'status'<>'declined'
 and(x.data->>'coverage_from')::date<=coalesce((v_data->>'coverage_to')::date,date'9999-12-31')and(v_data->>'coverage_from')::date<=coalesce((x.data->>'coverage_to')::date,date'9999-12-31'))then raise exception 'This person already has overlapping coverage or a pending enrollment.'using errcode='23514';end if;
 end if;
 if r.id is null then
 insert into hris_private.benefit_records(id,company_id,kind,revision,data,employee_id,plan_id,created_at,updated_at)values(subject,m.company_id,kind,1,v_data,employee_id,plan_id,at_time,at_time)returning *into r;
 else
 update hris_private.benefit_records set revision=expected+1,data=benefit_change.v_data,updated_at=at_time where id=subject returning *into r;
 end if;
 current_state:=jsonb_build_object('kind',kind,'record',hris_private.benefit_record(r));
 select e.full_name into strict actor_name from public.employees e where e.company_id=m.company_id and e.id=m.employee_id;
 result:=jsonb_build_object('schema_version',1,'id',key_id,'company_id',m.company_id,'actor_id',m.user_id,'subject_id',subject,'kind',kind,'revision',r.revision,'applied_at',at_time);
 insert into hris_private.benefit_events(id,company_id,subject_id,actor_id,actor_name,revision,body,previous_state,current_state,payload,receipt,created_at)values(key_id,m.company_id,subject,m.user_id,actor_name,r.revision,p_change->>'body',old_state,current_state,p_change,result,at_time);
 -- Enforce complete context budgets inside the same transaction; overflow rolls everything back.
 perform hris_private.benefit_context();return result;
end;$$;
create function public.employee_benefit_context()returns jsonb language sql volatile security invoker set search_path=''as $$select hris_private.benefit_context()$$;
create function public.save_employee_benefit(p_change jsonb)returns jsonb language sql volatile security invoker set search_path=''as $$select hris_private.save_benefit(p_change)$$;
revoke all on function hris_private.benefit_keys(jsonb,text[]),hris_private.benefit_uuid(jsonb),hris_private.benefit_date(jsonb),hris_private.benefit_plan_valid(jsonb),hris_private.benefit_employee(public.employees),hris_private.benefit_enrollment_valid(jsonb,uuid),hris_private.keep_benefit_history(),hris_private.benefit_member(),hris_private.benefit_record(hris_private.benefit_records),hris_private.benefit_context(),hris_private.save_benefit(jsonb),public.employee_benefit_context(),public.save_employee_benefit(jsonb)from public,anon,authenticated,service_role;
grant execute on function hris_private.benefit_context(),hris_private.save_benefit(jsonb),public.employee_benefit_context(),public.save_employee_benefit(jsonb)to authenticated;
notify pgrst,'reload schema';
commit;
