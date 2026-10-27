-- Two-phase Auth reservation with one atomic employee/contact/membership/role commit.
-- Keep the existing staff runtime gate FALSE until every specialist boundary is verified.
begin;
alter table hris_private.staff_role_events drop constraint staff_role_events_kind_check;
alter table hris_private.staff_role_events add constraint staff_role_events_kind_check
 check(kind in('legacy_backfill','assignment_changed','account_provisioned'));

create table hris_private.staff_provision_intents(
 id uuid primary key,
 company_id uuid not null references public.companies(id)on delete restrict,
 actor_id uuid not null references auth.users(id)on delete restrict,
 user_id uuid not null,
 auth_preexisting boolean not null default false,
 email text not null check(email=lower(btrim(email))and length(email)between 3 and 254),
 payload jsonb not null check(jsonb_typeof(payload)='object'and pg_column_size(payload)<=24576),
 supersedes uuid unique references hris_private.staff_provision_intents(id)on delete restrict,
 state text not null default'reserved'check(state in('reserved','committed','superseded','cancelled')),
 receipt jsonb,
 created_at timestamptz not null default clock_timestamp(),
 closed_at timestamptz,
 check((state='reserved'and closed_at is null and receipt is null)
  or(state='committed'and closed_at is not null and receipt is not null)
  or(state in('superseded','cancelled')and closed_at is not null and receipt is null))
);
create unique index staff_provision_open_email_idx on hris_private.staff_provision_intents(company_id,email)where state='reserved';
create unique index staff_provision_open_user_idx on hris_private.staff_provision_intents(company_id,user_id)where state='reserved';
create index staff_provision_actor_idx on hris_private.staff_provision_intents(company_id,actor_id,created_at);
create table hris_private.staff_provision_events(
 id uuid primary key default gen_random_uuid(),
 intent_id uuid not null references hris_private.staff_provision_intents(id)on delete restrict,
 company_id uuid not null references public.companies(id)on delete restrict,
 actor_id uuid not null references auth.users(id)on delete restrict,
 kind text not null check(kind in('reserved','committed','superseded','cancelled')),
 reason text not null check(length(btrim(reason))between 1 and 500),
 created_at timestamptz not null default clock_timestamp(),
 unique(intent_id,kind)
);
create index staff_provision_events_actor_idx on hris_private.staff_provision_events(company_id,actor_id,created_at);
alter table hris_private.staff_provision_intents enable row level security;
alter table hris_private.staff_provision_events enable row level security;
revoke all on hris_private.staff_provision_intents,hris_private.staff_provision_events from public,anon,authenticated,service_role;
create trigger staff_provision_events_immutable before update or delete on hris_private.staff_provision_events
 for each row execute function hris_private.keep_staff_role_event();
create function hris_private.keep_staff_provision_source()returns trigger
language plpgsql security invoker set search_path=''as $$
begin
 if tg_op='DELETE'then raise exception 'Provisioning history is retained.'using errcode='23514';end if;
 if(row(new.id,new.company_id,new.actor_id,new.user_id,new.auth_preexisting,new.email,new.payload,new.supersedes,new.created_at)
  is distinct from row(old.id,old.company_id,old.actor_id,old.user_id,old.auth_preexisting,old.email,old.payload,old.supersedes,old.created_at))
  or old.state<>'reserved'or new.state not in('committed','superseded','cancelled')then
  raise exception 'Provisioning sources and closed decisions are immutable.'using errcode='23514';end if;
 return new;
end;$$;
create trigger staff_provision_source_immutable before update or delete on hris_private.staff_provision_intents
 for each row execute function hris_private.keep_staff_provision_source();

-- Only checked entry points are granted. Internal helpers remain unavailable to API roles.
create function hris_private.staff_provision_admin(p_ready boolean)returns public.company_members
language plpgsql security definer set search_path=''as $$
declare m public.company_members;
begin
 select cm.*into m from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id
  where cm.user_id=(select auth.uid())and cm.active and cm.role='administrator'and e.status='active';
 if m.user_id is null then raise exception 'Current Administrator access is required.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 select cm.*into m from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id
  where cm.user_id=(select auth.uid())and cm.active and cm.role='administrator'and e.status='active';
 if m.user_id is null then raise exception 'Your Administrator access changed.'using errcode='42501';end if;
 if p_ready and not coalesce((select runtime_ready from hris_private.staff_role_runtime_policy where company_id=m.company_id),false)then
  raise exception 'Account provisioning awaits the verified protected workspace rollout.'using errcode='42501';end if;
 return m;
end;$$;
create function hris_private.staff_person_envelope(p_company uuid,p_person uuid)returns jsonb
language sql stable security invoker set search_path=''as $$
 with source as(select to_jsonb(e)employee,to_jsonb(c)contacts from public.employees e left join public.employee_contacts c
  on c.company_id=e.company_id and c.employee_id=e.id where e.company_id=p_company and e.id=p_person)
 select jsonb_build_object('schema_version',1,'employee_id',p_person,
  'source_hash',encode(sha256(convert_to(jsonb_build_object('employee',employee,'contacts',contacts)::text,'UTF8')),'hex'),
  'employee',employee-'created_at'-'updated_at','contacts',contacts-'id')from source
$$;
create function hris_private.staff_employee_source(p_employee uuid)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;source jsonb;
begin
 m:=hris_private.staff_provision_admin(false);
 source:=hris_private.staff_person_envelope(m.company_id,p_employee);
 if source is null then raise exception 'Choose an employee in this company.'using errcode='42501';end if;
 return source||jsonb_build_object('company_id',m.company_id,'actor_id',m.user_id,
  'runtime_ready',coalesce((select runtime_ready from hris_private.staff_role_runtime_policy where company_id=m.company_id),false));
end;$$;

-- Rehearsal uses normal constraints/triggers inside an always-rolled-back subtransaction.
create function hris_private.check_staff_provision_payload(p_company uuid,p_payload jsonb,p_rehearse boolean)returns text[]
language plpgsql security invoker set search_path=''as $$
declare r text[];person uuid;source jsonb;verified boolean:=false;employee jsonb;contacts jsonb;mail text;reason text;
begin
 if p_payload is null or jsonb_typeof(p_payload)<>'object'or pg_column_size(p_payload)>24576
  or exists(select 1 from jsonb_object_keys(p_payload)k where k not in('id','email','roles','active','reason','employee','contacts','expected_source','supersedes'))then
  raise exception 'Use the supported credential-free provisioning request.'using errcode='23514';end if;
 if(p_payload->>'id')::uuid is null or(p_payload->>'id')::uuid='00000000-0000-0000-0000-000000000000'::uuid
  or jsonb_typeof(p_payload->'active')is distinct from'boolean'or jsonb_typeof(p_payload->'email')is distinct from'string'
  or jsonb_typeof(p_payload->'reason')is distinct from'string'then raise exception 'A valid identity, email, status and reason are required.'using errcode='23514';end if;
 mail:=lower(btrim(p_payload->>'email'));reason:=p_payload->>'reason';
 if length(mail)not between 3 and 254 or mail!~'^[^[:space:]@]+@[^[:space:]@]+\.[^[:space:]@]+$'
  or reason<>btrim(reason)or length(reason)not between 1 and 500 or reason~'[[:cntrl:]]'then
  raise exception 'Enter a valid email and a reason of 1–500 characters.'using errcode='23514';end if;
 if jsonb_typeof(p_payload->'roles')is distinct from'array'then raise exception 'Choose staff responsibilities.'using errcode='23514';end if;
 if jsonb_array_length(p_payload->'roles')not between 1 and 6 or exists(select 1 from jsonb_array_elements(p_payload->'roles')v where jsonb_typeof(v)<>'string')then
  raise exception 'Choose supported staff responsibilities.'using errcode='23514';end if;
 select hris_private.normalize_staff_roles(array_agg(value#>>'{}'))into r from jsonb_array_elements(p_payload->'roles');
 employee:=p_payload->'employee';contacts:=p_payload->'contacts';
 if jsonb_typeof(employee)is distinct from'object'or jsonb_typeof(contacts)is distinct from'object'then
  raise exception 'Employment and contact details are required.'using errcode='23514';end if;
 if exists(select 1 from jsonb_object_keys(employee)k where k not in('id','company_id','employee_code','full_name','department','job_title','joined_on','annual_leave_days','status','reports_to','employment_type_id','work_location_id'))
  or exists(select 1 from jsonb_object_keys(contacts)k where k not in('company_id','employee_id','phone','personal_email','address','emergency_name','emergency_relationship','emergency_phone','emergency_alternate_phone','updated_at'))then
  raise exception 'Use the supported employment and contact fields.'using errcode='23514';end if;
 if(employee->>'company_id')::uuid is distinct from p_company or(contacts->>'company_id')::uuid is distinct from p_company then
  raise exception 'Use the current company employee and contact details.'using errcode='42501';end if;
 person:=(employee->>'id')::uuid;
 if(contacts->>'employee_id')::uuid is not null and(contacts->>'employee_id')::uuid is distinct from person
  and(contacts->>'employee_id')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid then
  raise exception 'Contacts belong to another employee.'using errcode='23514';end if;
 if(employee->>'status')is distinct from'active'and(r<>array['employee']::text[]or(p_payload->>'active')::boolean)then
  raise exception 'Restore the employee before assigning staff access.'using errcode='23514';end if;
 if person is null then
  if p_payload->>'expected_source'is not null then raise exception 'A new person has no saved source.'using errcode='23514';end if;
 else
  -- Follow employee-save row ordering before taking the archive/account advisory lock.
  perform 1 from public.employees where company_id=p_company and id=person for update;
  perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(person::text,0));
  perform 1 from public.employee_contacts where company_id=p_company and employee_id=person for update;
  source:=hris_private.staff_person_envelope(p_company,person);
  if source is null then raise exception 'Choose an employee in this company.'using errcode='42501';end if;
  if source->>'source_hash'is distinct from(p_payload->>'expected_source')then
   raise exception 'Employment or contacts changed; refresh and review before provisioning.'using errcode='23514';end if;
  if exists(select 1 from public.company_members where company_id=p_company and employee_id=person)then
   raise exception 'This employee already has an account. Edit its saved responsibilities.'using errcode='23505';end if;
  if(p_payload->>'active')::boolean and hris_private.compatible_staff_base_role(r)='employee'
   and exists(select 1 from public.employees where company_id=p_company and reports_to=person and status='active')then
   raise exception 'Keep a Manager responsibility or reassign direct reports first.'using errcode='23514';end if;
 end if;
 if p_rehearse then
  begin
   perform public.save_employee_record(employee,contacts);verified:=true;
   raise exception using errcode='ZX001',message='Internal rolled-back provisioning rehearsal.';
  exception when sqlstate'ZX001'then if not verified then raise;end if;
  end;
 end if;
 return r;
end;$$;
create function hris_private.staff_provision_reply(p_intent hris_private.staff_provision_intents)returns jsonb
language sql immutable security invoker set search_path=''as $$
 select jsonb_build_object('schema_version',1,'id',p_intent.id,'company_id',p_intent.company_id,'actor_id',p_intent.actor_id,
  'user_id',p_intent.user_id,'auth_preexisting',p_intent.auth_preexisting,'email',p_intent.email,'state',p_intent.state,'receipt',p_intent.receipt)
$$;
create function hris_private.begin_staff_provision(p_request jsonb)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;saved hris_private.staff_provision_intents;prior hris_private.staff_provision_intents;
 request_id uuid;parent uuid;target uuid;mail text;r text[];n integer;preexisting boolean:=false;
begin
 m:=hris_private.staff_provision_admin(true);
 if p_request is null or jsonb_typeof(p_request)<>'object'or pg_column_size(p_request)>24576 then
  raise exception 'Use the supported provisioning request.'using errcode='23514';end if;
 request_id:=(p_request->>'id')::uuid;
 if request_id is null or request_id='00000000-0000-0000-0000-000000000000'::uuid then raise exception 'A request identity is required.'using errcode='23514';end if;
 select *into saved from hris_private.staff_provision_intents where id=request_id for update;
 if saved.id is not null then
  if saved.company_id<>m.company_id or saved.actor_id<>m.user_id or saved.payload<>p_request then
   raise exception 'This request belongs to another actor or payload.'using errcode='42501';end if;
  if saved.state not in('reserved','committed')then raise exception 'This provisioning attempt is closed.'using errcode='23514';end if;
  return hris_private.staff_provision_reply(saved);
 end if;
 r:=hris_private.check_staff_provision_payload(m.company_id,p_request,true);
 mail:=lower(btrim(p_request->>'email'));parent:=(p_request->>'supersedes')::uuid;
 if parent is not null then
  select *into prior from hris_private.staff_provision_intents where id=parent for update;
  if prior.id is null or prior.company_id<>m.company_id or prior.actor_id<>m.user_id then
   raise exception 'Choose your own prior provisioning attempt.'using errcode='42501';end if;
  if prior.state not in('reserved','cancelled')or prior.email<>mail
   or prior.payload->'employee'->>'id'is distinct from(p_request->'employee'->>'id')then
   raise exception 'Keep the same provisional account and employee when reviewing a replacement attempt.'using errcode='23514';end if;
  target:=prior.user_id;preexisting:=prior.auth_preexisting;
  if prior.state='reserved'then
   update hris_private.staff_provision_intents set state='superseded',closed_at=clock_timestamp()where id=parent;
   insert into hris_private.staff_provision_events(intent_id,company_id,actor_id,kind,reason)
    values(parent,m.company_id,m.user_id,'superseded',p_request->>'reason');
  end if;
 else
  select id into target from auth.users where lower(email)=mail;
  preexisting:=target is not null;
  if target is null then target:=gen_random_uuid();end if;
 end if;
 if exists(select 1 from public.company_members where user_id=target)then
  raise exception 'This sign-in identity already has an employee. Edit its existing access.'using errcode='23505';end if;
 select count(*)into n from hris_private.staff_provision_intents where company_id=m.company_id and actor_id=m.user_id and state='reserved';
 if n>=30 then raise exception 'Review or close earlier pending account attempts before creating another.'using errcode='54000';end if;
 insert into hris_private.staff_provision_intents(id,company_id,actor_id,user_id,auth_preexisting,email,payload,supersedes)
  values(request_id,m.company_id,m.user_id,target,preexisting,mail,p_request,parent)returning *into saved;
 insert into hris_private.staff_provision_events(intent_id,company_id,actor_id,kind,reason)
  values(request_id,m.company_id,m.user_id,'reserved',p_request->>'reason');
 return hris_private.staff_provision_reply(saved);
end;$$;
create function hris_private.commit_staff_provision(p_request jsonb)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;saved hris_private.staff_provision_intents;member public.company_members;
 r text[];person uuid;base text;result jsonb;
begin
 m:=hris_private.staff_provision_admin(true);
 if p_request is null or jsonb_typeof(p_request)<>'object'or pg_column_size(p_request)>24576 then raise exception 'Use the supported provisioning request.'using errcode='23514';end if;
 select *into saved from hris_private.staff_provision_intents where id=(p_request->>'id')::uuid for update;
 if saved.id is null or saved.company_id<>m.company_id or saved.actor_id<>m.user_id or saved.payload<>p_request then
  raise exception 'Use the original actor-bound provisioning request.'using errcode='42501';end if;
 if saved.state='committed'then return saved.receipt;end if;
 if saved.state<>'reserved'then raise exception 'This provisioning attempt is closed.'using errcode='23514';end if;
 if exists(select 1 from public.company_members where user_id=saved.user_id)then raise exception 'The reserved account already has a membership.'using errcode='23514';end if;
 if not exists(select 1 from auth.users where id=saved.user_id and lower(email)=saved.email)then
  raise exception 'The reserved Auth identity is not available with its original email.'using errcode='23514';end if;
 r:=hris_private.check_staff_provision_payload(m.company_id,p_request,false);
 person:=public.save_employee_record(p_request->'employee',p_request->'contacts');
 base:=hris_private.compatible_staff_base_role(r);
 perform hris_private.save_company_account_legacy(saved.user_id,saved.email,base,(p_request->>'active')::boolean,person,null);
 insert into hris_private.staff_role_assignments(company_id,user_id,roles,base_role)values(m.company_id,saved.user_id,r,base);
 insert into hris_private.staff_role_events(id,company_id,user_id,actor_id,kind,roles,revision,reason,active)
  values(saved.id,m.company_id,saved.user_id,m.user_id,'account_provisioned',r,1,p_request->>'reason',(p_request->>'active')::boolean);
 select *into member from public.company_members where company_id=m.company_id and user_id=saved.user_id;
 result:=jsonb_build_object('schema_version',1,'id',saved.id,'company_id',m.company_id,'actor_id',m.user_id,
  'user_id',saved.user_id,'employee_id',person,'active',member.active,'roles',r,'revision',1,'base_role',base,'member_updated_at',member.updated_at);
 update hris_private.staff_provision_intents set state='committed',receipt=result,closed_at=clock_timestamp()where id=saved.id;
 insert into hris_private.staff_provision_events(intent_id,company_id,actor_id,kind,reason)
  values(saved.id,m.company_id,m.user_id,'committed',p_request->>'reason');
 return result;
end;$$;
create function hris_private.pending_staff_provisions()returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;n integer;entries jsonb;
begin
 m:=hris_private.staff_provision_admin(false);
 select count(*)into n from hris_private.staff_provision_intents where company_id=m.company_id and actor_id=m.user_id and state='reserved';
 if n>100 then raise exception 'Pending account history exceeds the supported view. Nothing was omitted.'using errcode='54000';end if;
 select coalesce(jsonb_agg(jsonb_build_object('intent',hris_private.staff_provision_reply(i),'request',i.payload,'created_at',i.created_at)order by i.created_at,i.id),'[]'::jsonb)
  into entries from hris_private.staff_provision_intents i where company_id=m.company_id and actor_id=m.user_id and state='reserved';
 return jsonb_build_object('schema_version',1,'company_id',m.company_id,'actor_id',m.user_id,'entry_count',n,'entries',entries);
end;$$;
create function hris_private.staff_provision_status(p_id uuid)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;saved hris_private.staff_provision_intents;
begin
 m:=hris_private.staff_provision_admin(false);
 select *into saved from hris_private.staff_provision_intents where id=p_id and company_id=m.company_id and actor_id=m.user_id;
 if saved.id is null then return null;end if;
 return hris_private.staff_provision_reply(saved);
end;$$;
create function hris_private.cancel_staff_provision(p_id uuid,p_reason text)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;saved hris_private.staff_provision_intents;
begin
 m:=hris_private.staff_provision_admin(false);
 if p_reason is null or p_reason<>btrim(p_reason)or length(p_reason)not between 1 and 500 or p_reason~'[[:cntrl:]]'then
  raise exception 'Enter a cancellation reason of 1–500 characters.'using errcode='23514';end if;
 select *into saved from hris_private.staff_provision_intents where id=p_id for update;
 if saved.id is null or saved.company_id<>m.company_id or saved.actor_id<>m.user_id then raise exception 'Choose your own pending account attempt.'using errcode='42501';end if;
 if saved.state='cancelled'and exists(select 1 from hris_private.staff_provision_events where intent_id=p_id and kind='cancelled'and reason=p_reason)then
  return jsonb_build_object('id',p_id,'state','cancelled');end if;
 if saved.state<>'reserved'then raise exception 'This attempt is already closed.'using errcode='23514';end if;
 update hris_private.staff_provision_intents set state='cancelled',closed_at=clock_timestamp()where id=p_id;
 insert into hris_private.staff_provision_events(intent_id,company_id,actor_id,kind,reason)values(p_id,m.company_id,m.user_id,'cancelled',p_reason);
 return jsonb_build_object('id',p_id,'state','cancelled');
end;$$;

create function public.staff_employee_source(p_employee uuid)returns jsonb language sql security invoker set search_path=''as $$select hris_private.staff_employee_source(p_employee)$$;
create function public.begin_staff_provision(p_request jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.begin_staff_provision(p_request)$$;
create function public.commit_staff_provision(p_request jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.commit_staff_provision(p_request)$$;
create function public.pending_staff_provisions()returns jsonb language sql security invoker set search_path=''as $$select hris_private.pending_staff_provisions()$$;
create function public.staff_provision_status(p_id uuid)returns jsonb language sql security invoker set search_path=''as $$select hris_private.staff_provision_status(p_id)$$;
create function public.cancel_staff_provision(p_id uuid,p_reason text)returns jsonb language sql security invoker set search_path=''as $$select hris_private.cancel_staff_provision(p_id,p_reason)$$;
revoke all on function hris_private.keep_staff_provision_source(),hris_private.staff_provision_admin(boolean),hris_private.staff_person_envelope(uuid,uuid),
 hris_private.check_staff_provision_payload(uuid,jsonb,boolean),hris_private.staff_provision_reply(hris_private.staff_provision_intents)from public,anon,authenticated,service_role;
revoke all on function hris_private.staff_employee_source(uuid),hris_private.begin_staff_provision(jsonb),hris_private.commit_staff_provision(jsonb),hris_private.pending_staff_provisions(),hris_private.cancel_staff_provision(uuid,text),hris_private.staff_provision_status(uuid),
 public.staff_employee_source(uuid),public.begin_staff_provision(jsonb),public.commit_staff_provision(jsonb),public.pending_staff_provisions(),public.cancel_staff_provision(uuid,text),public.staff_provision_status(uuid)from public,anon,authenticated,service_role;
grant execute on function hris_private.staff_employee_source(uuid),hris_private.begin_staff_provision(jsonb),hris_private.commit_staff_provision(jsonb),hris_private.pending_staff_provisions(),hris_private.cancel_staff_provision(uuid,text),hris_private.staff_provision_status(uuid),
 public.staff_employee_source(uuid),public.begin_staff_provision(jsonb),public.commit_staff_provision(jsonb),public.pending_staff_provisions(),public.cancel_staff_provision(uuid,text),public.staff_provision_status(uuid)to authenticated;
comment on table hris_private.staff_provision_intents is 'Credential-free actor-bound Auth reservations. An unconfirmed Auth identity has no employee access until atomic commit; uncertain delivery never authorizes deleting it.';
commit;