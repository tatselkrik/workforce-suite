-- HR-private registration facts; never changes canonical employment or issued pay.
begin;
create table public.government_registration_records(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid,
 revision bigint not null default 0 check(revision>=0),details jsonb not null check(octet_length(details::text)<=16384),
 reference text not null,note text not null,created_by uuid not null references auth.users(id),
 updated_by uuid not null references auth.users(id),created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create unique index government_registration_company_idx on public.government_registration_records(company_id)where employee_id is null;
create unique index government_registration_employee_idx on public.government_registration_records(company_id,employee_id)where employee_id is not null;
create index government_registration_creator_idx on public.government_registration_records(created_by);
create index government_registration_updater_idx on public.government_registration_records(updated_by);
create table public.government_registration_events(
 id uuid primary key,company_id uuid not null,record_id uuid not null,revision bigint not null,
 actor_id uuid not null references auth.users(id),actor_name text not null,reason text not null,
 before_record jsonb,after_record jsonb not null,recorded_at timestamptz not null default clock_timestamp(),
 unique(company_id,record_id,revision),foreign key(company_id,record_id)references public.government_registration_records(company_id,id)
);
create index government_registration_event_actor_idx on public.government_registration_events(actor_id);
alter table public.government_registration_records enable row level security;
alter table public.government_registration_events enable row level security;
revoke all on public.government_registration_records,public.government_registration_events from public,anon,authenticated;
grant select on public.government_registration_records,public.government_registration_events to authenticated;
create policy government_registration_hr on public.government_registration_records for select to authenticated using(hris_private.payroll_hr(company_id));
create policy government_registration_event_hr on public.government_registration_events for select to authenticated using(hris_private.payroll_hr(company_id));
create trigger government_registration_events_immutable before update or delete on public.government_registration_events for each row execute function hris_private.employment_history_immutable();
create function hris_private.government_registration_guard()returns trigger language plpgsql security invoker set search_path=''as $$
begin
 if tg_op='DELETE'then raise exception 'Registration history is retained; correct its recorded fields.'using errcode='42501';end if;
 if(new.id,new.company_id,new.employee_id,new.created_by,new.created_at)is distinct from(old.id,old.company_id,old.employee_id,old.created_by,old.created_at)or new.revision<>old.revision+1 then
  raise exception 'Registration identity/creator is pinned; use its current revision.'using errcode='42501';end if;return new;
end;$$;
revoke all on function hris_private.government_registration_guard()from public,anon,authenticated;
create trigger government_registration_guard before update or delete on public.government_registration_records for each row execute function hris_private.government_registration_guard();

create function hris_private.government_registration_fields(person boolean)returns text[]language sql immutable security invoker set search_path=''as $$
 select array['address_line1','address_line2','city','province','postal_code','country','bir_tin','bir_branch','bir_rdo','sss_number','philhealth_number','pagibig_number']::text[]
 ||case when person then array['last_name','first_name','middle_name','name_suffix','birth_date','sex','nationality','bir_region']else array['registered_name','telephone','email','employer_type']end;
$$;
revoke all on function hris_private.government_registration_fields(boolean)from public,anon,authenticated;
create function hris_private.government_registration_details(j jsonb,person boolean,today date)returns jsonb
language plpgsql immutable security invoker set search_path=''as $$
declare keys text[]:=hris_private.government_registration_fields(person);k text;v text;lim integer;digits integer;d date;result jsonb:='{}';begin
 if jsonb_typeof(j)is distinct from'object'then raise exception 'Explicit registration fields are required.'using errcode='23514';end if;
 if j-keys<>'{}'::jsonb then raise exception 'Use supported registration fields for this scope.'using errcode='23514';end if;
 for k in select unnest(keys)loop
  if j?k and jsonb_typeof(j->k)is distinct from'string'then raise exception 'Registration fields must be text; leave unknown values blank.'using errcode='23514';end if;
  v:=btrim(coalesce(j->>k,''));lim:=case when k in('address_line1','address_line2')then 300 when k in('registered_name','email')then 254 else 100 end;
  if length(v)>lim or v~'[[:cntrl:]]'then raise exception 'Registration text is too long or contains control characters.'using errcode='23514';end if;
  if k in('bir_tin','bir_branch','bir_rdo','bir_region','sss_number','philhealth_number','pagibig_number')and v<>''then
   if v!~'^[0-9 -]+$'then raise exception 'Copy identifier digits, preserving leading zeroes; spaces/dashes are accepted.'using errcode='23514';end if;
   v:=replace(replace(v,' ',''),'-','');digits:=case k when'bir_tin'then 9 when'bir_branch'then 4 when'bir_rdo'then 3 when'bir_region'then 4 when'sss_number'then 10 when'philhealth_number'then 12 when'pagibig_number'then case when person then 12 else null end end;
   if v=''or(digits is not null and length(v)<>digits)or(digits is null and length(v)not between 1 and 20)then raise exception 'Check identifier length against the registration document.'using errcode='23514';end if;
   if k in('bir_tin','sss_number','philhealth_number','pagibig_number')and v~'^0+$'then raise exception 'Leave an unassigned identifier blank; do not save zero placeholders as assigned IDs.'using errcode='23514';end if;
  elsif k='birth_date'and v<>''then
   if v!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'then raise exception 'Use YYYY-MM-DD for recorded birth dates.'using errcode='23514';end if;
   begin d:=v::date;exception when invalid_datetime_format or datetime_field_overflow then raise exception 'Use a valid birth date.'using errcode='23514';end;
   if d<date'1900-01-01'or d>today then raise exception 'The recorded birth date must be between1900 and today.'using errcode='23514';end if;
  elsif k='sex'and v not in('','M','F')then raise exception 'Use the recorded agency sex code M/F, or leave it unknown.'using errcode='23514';
  elsif k='country'and v<>''then v:=upper(v);if v!~'^[A-Z]{2}$'then raise exception 'Use a two-letter country code.'using errcode='23514';end if;
  elsif k='employer_type'and v not in('','private','government','household')then raise exception 'Choose the employer category recorded with the agency.'using errcode='23514';
  elsif k='email'and v<>''and v!~'^[^ @]+@[^ @]+\.[^ @]+$'then raise exception 'Check the registered contact email.'using errcode='23514';end if;
  result:=result||jsonb_build_object(k,v);
 end loop;return result;
end;$$;
revoke all on function hris_private.government_registration_details(jsonb,boolean,date)from public,anon,authenticated;

create function hris_private.save_government_registration(p_record jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;r public.government_registration_records;old_row jsonb;birth hris_private.payroll_birth;
 eid uuid;rid uuid;opid uuid;expected bigint;today date;why text;label text;saved bigint;begin
 m:=hris_private.government_actor();
 perform hris_private.final_pay_object(p_record,array['id','operation_id','employee_id','expected_revision','details','reference','note','reason','reviewed']);
 if octet_length(p_record::text)>32768 or not(p_record?'employee_id')or not(p_record?'expected_revision')then raise exception 'Use a bounded, explicit registration target/revision.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;opid:=(p_record->>'operation_id')::uuid;eid:=(p_record->>'employee_id')::uuid;
 if rid is null or opid is null or rid='00000000-0000-0000-0000-000000000000'or opid='00000000-0000-0000-0000-000000000000'then raise exception 'Record and operation identities are required.'using errcode='23514';end if;
 if eid is not null and not exists(select 1 from public.employees where company_id=m.company_id and id=eid)then raise exception 'Employee outside your company.'using errcode='42501';end if;
 why:=hris_private.payroll_text(p_record,'reason',2000,true);
 select *into birth from hris_private.payroll_birth where company_id=m.company_id and operation='government_registration_save'and record_id=opid;
 if birth.record_id is not null then
  if birth.actor_id<>m.user_id or birth.payload<>p_record then raise exception 'This registration operation identity was already used.'using errcode='PT409';end if;
  select e.revision into strict saved from public.government_registration_events e where e.company_id=m.company_id and e.id=opid;
  return jsonb_build_object('operation_id',opid,'record_id',rid,'saved_revision',saved);
 end if;
 if p_record->'reviewed'is distinct from'true'::jsonb then raise exception 'Review the registration document and entered facts before saving.'using errcode='23514';end if;
 select(now()at time zone c.timezone)::date into today from public.companies c where c.id=m.company_id;
 select e.full_name into label from public.employees e where e.company_id=m.company_id and e.id=m.employee_id;
 select *into r from public.government_registration_records where company_id=m.company_id and employee_id is not distinct from eid for update;
 if p_record->'expected_revision'='null'::jsonb then
  if r.id is not null then raise exception 'Registration already exists; refresh its current version.'using errcode='PT409';end if;
  r.id:=rid;r.company_id:=m.company_id;r.employee_id:=eid;r.revision:=0;r.created_by:=m.user_id;r.created_at:=clock_timestamp();
 else
  expected:=hris_private.government_money(p_record,'expected_revision');
  if r.id is null or r.id<>rid or r.revision<>expected then raise exception 'Registration changed. Refresh; unsaved entries remain yours.'using errcode='PT409';end if;
  old_row:=to_jsonb(r);r.revision:=r.revision+1;
 end if;
 r.details:=hris_private.government_registration_details(p_record->'details',eid is not null,today);
 r.reference:=hris_private.payroll_text(p_record,'reference',2000,true);r.note:=hris_private.payroll_text(p_record,'note',2000,false);r.updated_by:=m.user_id;r.updated_at:=clock_timestamp();
 if old_row is null then insert into public.government_registration_records select(r).*;
 else update public.government_registration_records set revision=r.revision,details=r.details,reference=r.reference,note=r.note,updated_by=r.updated_by,updated_at=r.updated_at where company_id=m.company_id and id=rid;end if;
 insert into public.government_registration_events(id,company_id,record_id,revision,actor_id,actor_name,reason,before_record,after_record)values(opid,m.company_id,rid,r.revision,m.user_id,label,why,old_row,to_jsonb(r));
 insert into hris_private.payroll_birth(company_id,operation,record_id,actor_id,payload)values(m.company_id,'government_registration_save',opid,m.user_id,p_record);
 return jsonb_build_object('operation_id',opid,'record_id',rid,'saved_revision',r.revision);
end;$$;
revoke all on function hris_private.save_government_registration(jsonb)from public,anon,authenticated;
grant execute on function hris_private.save_government_registration(jsonb)to authenticated;
create function public.save_government_registration(p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.save_government_registration(p_record);$$;
revoke all on function public.save_government_registration(jsonb)from public,anon,authenticated;
grant execute on function public.save_government_registration(jsonb)to authenticated;

create function hris_private.government_registration(p_employee_id uuid default null)returns jsonb language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;r public.government_registration_records;history jsonb;n integer;begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Current HR/Admin access required.'using errcode='42501';end if;
 if p_employee_id is not null and not exists(select 1 from public.employees where company_id=m.company_id and id=p_employee_id)then raise exception 'Employee outside your company.'using errcode='42501';end if;
 select *into r from public.government_registration_records where company_id=m.company_id and employee_id is not distinct from p_employee_id;
 select coalesce(jsonb_agg(to_jsonb(e)order by e.revision),'[]'),count(*)into history,n from(select q.*from public.government_registration_events q where q.company_id=m.company_id and q.record_id=r.id limit 1001)e;
 if n>1000 then raise exception 'Too much registration history for this view; no history was silently truncated.'using errcode='23514';end if;
 return jsonb_build_object('version','government-registration-v1','company_id',m.company_id,'employee_id',p_employee_id,'record',case when r.id is null then null else to_jsonb(r)end,'history',history);
end;$$;
revoke all on function hris_private.government_registration(uuid)from public,anon,authenticated;
grant execute on function hris_private.government_registration(uuid)to authenticated;
create function public.government_registration(p_employee_id uuid default null)returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.government_registration(p_employee_id);$$;
revoke all on function public.government_registration(uuid)from public,anon,authenticated;
grant execute on function public.government_registration(uuid)to authenticated;
commit;
