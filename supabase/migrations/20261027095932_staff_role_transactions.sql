-- Atomic multi-role assignment contract. Runtime stays disabled until module scopes are verified.
begin;
-- Prior immutable events retain unknown status; new decisions capture both states.
alter table hris_private.staff_role_events add column previous_active boolean,add column active boolean;
create table hris_private.staff_role_runtime_policy(
 company_id uuid primary key references public.companies(id)on delete restrict,
 runtime_ready boolean not null default false
);
alter table hris_private.staff_role_runtime_policy enable row level security;
revoke all on hris_private.staff_role_runtime_policy from public,anon,authenticated,service_role;
insert into hris_private.staff_role_runtime_policy(company_id)select id from public.companies;
create function hris_private.compatible_staff_base_role(p_roles text[])returns text
language plpgsql immutable strict security invoker set search_path=''as $$
declare r text[]:=hris_private.normalize_staff_roles(p_roles);
begin
 if 'administrator'=any(r)then return'administrator';end if;
 if r@>array['hr_generalist','recruiter','learning_performance_officer','payroll_officer']::text[]then return'hr_generalist';end if;
 if 'manager'=any(r)then return'manager';end if;
 return'employee';
end;$$;

create table hris_private.staff_role_change_receipts(
 id uuid primary key,
 company_id uuid not null,
 user_id uuid not null,
 actor_id uuid not null,
 payload jsonb not null,
 receipt jsonb not null,
 created_at timestamptz not null default now(),
 foreign key(company_id,user_id)references public.company_members(company_id,user_id)on delete restrict,
 foreign key(company_id,actor_id)references public.company_members(company_id,user_id)on delete restrict
);
create index staff_role_receipts_subject_idx on hris_private.staff_role_change_receipts(company_id,user_id,created_at desc);
create index staff_role_receipts_actor_idx on hris_private.staff_role_change_receipts(company_id,actor_id);
alter table hris_private.staff_role_change_receipts enable row level security;
revoke all on hris_private.staff_role_change_receipts from public,anon,authenticated,service_role;
create trigger staff_role_receipts_immutable before update or delete on hris_private.staff_role_change_receipts
 for each row execute function hris_private.keep_staff_role_event();

create or replace function hris_private.staff_role_context()returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;a hris_private.staff_role_assignments;r text[];ready boolean;
begin
 select cm.*into m from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id
  where cm.user_id=(select auth.uid())and cm.active and e.status='active';
 if m.user_id is null then raise exception 'Current active employee membership is required.'using errcode='42501';end if;
 select *into a from hris_private.staff_role_assignments where company_id=m.company_id and user_id=m.user_id;
 r:=hris_private.legacy_staff_roles(m.role);
 if a.user_id is not null and a.base_role=m.role and hris_private.compatible_staff_base_role(a.roles)=m.role then r:=a.roles;end if;
 select coalesce((select runtime_ready from hris_private.staff_role_runtime_policy where company_id=m.company_id),false)into ready;
 return jsonb_build_object('schema_version',1,'company_id',m.company_id,'user_id',m.user_id,'employee_id',m.employee_id,
 'base_role',m.role,'revision',coalesce(a.revision,1),'runtime_ready',ready,'roles',r);
end;$$;

create or replace function hris_private.staff_role_admin_context(p_include_disabled boolean)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;accounts jsonb;n integer;ready boolean;
begin
 select cm.*into m from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id
 where cm.user_id=(select auth.uid())and cm.active and cm.role='administrator'and e.status='active';
 if m.user_id is null then raise exception 'Only a current Administrator reads company staff-role assignments.'using errcode='42501';end if;
 if p_include_disabled is null then raise exception 'Choose whether to include disabled accounts.'using errcode='23514';end if;
 select count(*)into n from public.company_members where company_id=m.company_id and(active or p_include_disabled);
 if n>1000 then raise exception 'Staff-role history exceeds this bounded view; narrow scope support is required.'using errcode='23514';end if;
 select coalesce(jsonb_agg(jsonb_build_object('user_id',cm.user_id,'employee_id',cm.employee_id,
  'employee_code',e.employee_code,'full_name',e.full_name,'active',cm.active,'base_role',cm.role,
  'member_updated_at',cm.updated_at,'revision',coalesce(a.revision,1),
  'roles',case when a.base_role=cm.role and hris_private.compatible_staff_base_role(a.roles)=cm.role
   then a.roles else hris_private.legacy_staff_roles(cm.role)end)order by e.employee_code,cm.user_id),'[]'::jsonb)into accounts
 from public.company_members cm join public.employees e on e.id=cm.employee_id and e.company_id=cm.company_id
 left join hris_private.staff_role_assignments a on a.company_id=cm.company_id and a.user_id=cm.user_id
 where cm.company_id=m.company_id and(cm.active or p_include_disabled);
 select coalesce((select runtime_ready from hris_private.staff_role_runtime_policy where company_id=m.company_id),false)into ready;
 return jsonb_build_object('schema_version',1,'company_id',m.company_id,'user_id',m.user_id,'runtime_ready',ready,'account_count',n,'accounts',accounts);
end;$$;

-- Keep the original owner/self/last-admin/person-link protections available internally.
do $copy_account$
declare source text;
begin
 select pg_get_functiondef('hris_private.save_company_account(uuid,text,text,boolean,uuid,uuid)'::regprocedure)into source;
 execute replace(source,'FUNCTION hris_private.save_company_account(','FUNCTION hris_private.save_company_account_legacy(');
end;$copy_account$;
revoke all on function hris_private.save_company_account_legacy(uuid,text,text,boolean,uuid,uuid)from public,anon,authenticated,service_role;
create or replace function hris_private.save_company_account(p_user_id uuid,p_email text,p_role text,p_active boolean,p_employee_id uuid,p_manager_user_id uuid)returns uuid
language plpgsql security definer set search_path=''as $$
declare m public.company_members;t public.company_members;a hris_private.staff_role_assignments;target uuid;ready boolean;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and role='administrator';
 select coalesce((select runtime_ready from hris_private.staff_role_runtime_policy where company_id=m.company_id),false)into ready;
 if ready then
  perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
  select coalesce(p_user_id,(select id from auth.users where lower(email)=lower(btrim(p_email))))into target;
  select *into t from public.company_members where user_id=target and company_id=m.company_id;
  select *into a from hris_private.staff_role_assignments where user_id=target and company_id=m.company_id;
  if((p_role='hr_generalist'and t.role is distinct from p_role)
   or(a.user_id is not null and a.base_role=t.role and a.roles<>hris_private.legacy_staff_roles(t.role)))
   and not(p_active is false and p_role=t.role)then
   raise exception 'Use the updated Account Manager to change specialist staff roles; existing access can still be disabled.'using errcode='42501';
  end if;
 end if;
 return hris_private.save_company_account_legacy(p_user_id,p_email,p_role,p_active,p_employee_id,p_manager_user_id);
end;$$;
revoke all on function hris_private.save_company_account(uuid,text,text,boolean,uuid,uuid)from public,anon,authenticated,service_role;
grant execute on function hris_private.save_company_account(uuid,text,text,boolean,uuid,uuid)to authenticated;

do $copy_access$
declare source text;
begin
 select pg_get_functiondef('hris_private.my_hris_access()'::regprocedure)into source;
 execute replace(source,'FUNCTION hris_private.my_hris_access(','FUNCTION hris_private.my_hris_access_legacy(');
end;$copy_access$;
revoke all on function hris_private.my_hris_access_legacy()from public,anon,authenticated,service_role;
create or replace function hris_private.my_hris_access()returns jsonb
language sql stable security definer set search_path=''as $$
 select hris_private.my_hris_access_legacy()||jsonb_build_object('staff_contract_version',1)
$$;
revoke all on function hris_private.my_hris_access()from public,anon,authenticated,service_role;
grant execute on function hris_private.my_hris_access()to authenticated;

create function hris_private.set_staff_roles(p_change jsonb)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;t public.company_members;e public.employees;a hris_private.staff_role_assignments;
 saved hris_private.staff_role_change_receipts;request_id uuid;target uuid;expected bigint;v_roles text[];
 base text;previous_roles text[];reason text;next_revision bigint;result jsonb;ready boolean;email text;desired_active boolean;
begin
 select cm.*into m from public.company_members cm join public.employees person
 on person.company_id=cm.company_id and person.id=cm.employee_id
 where cm.user_id=(select auth.uid())and cm.active and cm.role='administrator'and person.status='active';
 if m.user_id is null then raise exception 'Only a current Administrator assigns staff roles.'using errcode='42501';end if;
 if p_change is null or jsonb_typeof(p_change)<>'object'or pg_column_size(p_change)>16384
 or exists(select 1 from jsonb_object_keys(p_change)k where k not in('id','user_id','expected_revision','expected_base_role','expected_member_updated_at','roles','active','reason'))
 then raise exception 'Use the supported staff-role request.'using errcode='23514';end if;
 if jsonb_typeof(p_change->'active')is distinct from'boolean'then
  raise exception 'Choose the reviewed sign-in access status.'using errcode='23514';end if;
 desired_active:=(p_change->>'active')::boolean;
 request_id:=(p_change->>'id')::uuid;target:=(p_change->>'user_id')::uuid;expected:=(p_change->>'expected_revision')::bigint;
 if request_id is null or request_id='00000000-0000-0000-0000-000000000000'::uuid
 or target is null or target='00000000-0000-0000-0000-000000000000'::uuid or expected is null or expected<1
 then raise exception 'A request identity, target and current revision are required.'using errcode='23514';end if;
 if target=m.user_id then raise exception 'Another Administrator must change your roles.'using errcode='23514';end if;
 if jsonb_typeof(p_change->'roles')is distinct from'array'or jsonb_array_length(p_change->'roles')not between 1 and 6
 or exists(select 1 from jsonb_array_elements(p_change->'roles')v where jsonb_typeof(v)<>'string')then
 raise exception 'Choose supported staff roles.'using errcode='23514';end if;
 select hris_private.normalize_staff_roles(array_agg(value#>>'{}'))into v_roles from jsonb_array_elements(p_change->'roles');
 reason:=p_change->>'reason';
 if reason is null or reason<>btrim(reason)or length(reason)not between 1 and 500 or reason~'[[:cntrl:]]'then
  raise exception 'Enter a staff-role change reason of1–500characters without control characters.'using errcode='23514';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 if not exists(select 1 from public.company_members cm join public.employees person on person.company_id=cm.company_id and person.id=cm.employee_id
  where cm.company_id=m.company_id and cm.user_id=m.user_id and cm.active and cm.role='administrator'and person.status='active')then
  raise exception 'Your Administrator access changed.'using errcode='42501';end if;
 select coalesce((select runtime_ready from hris_private.staff_role_runtime_policy where company_id=m.company_id),false)into ready;
 if not ready then raise exception 'Staff-role editing remains disabled until protected workspace rollout is verified.'using errcode='42501';end if;
 select *into saved from hris_private.staff_role_change_receipts where id=request_id;
 if saved.id is not null then
  if saved.company_id<>m.company_id or saved.actor_id<>m.user_id or saved.user_id<>target or saved.payload<>p_change then
   raise exception 'This request identity belongs to another actor or payload.'using errcode='42501';end if;
  return saved.receipt;
 end if;
 select *into t from public.company_members where user_id=target and company_id=m.company_id for update;
 if t.user_id is null or t.employee_id is null then raise exception 'Choose an existing employee account in this company.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(t.employee_id::text,0));
 select *into e from public.employees where id=t.employee_id and company_id=m.company_id;
 if e.id is null or(e.status<>'active'and v_roles<>array['employee']::text[])then
  raise exception 'Restore the employee before assigning a staff role.'using errcode='23514';end if;
 select *into a from hris_private.staff_role_assignments where company_id=m.company_id and user_id=target for update;
 if coalesce(a.revision,1)<>expected or t.role is distinct from(p_change->>'expected_base_role')
 or t.updated_at is distinct from(p_change->>'expected_member_updated_at')::timestamptz then
  raise exception 'Account roles changed; refresh and review before saving.'using errcode='23514';end if;
 base:=hris_private.compatible_staff_base_role(v_roles);
 if target=(select owner_id from public.companies where id=m.company_id)and base<>'administrator'then
  raise exception 'The installation Administrator must retain Administrator access.'using errcode='23514';end if;
 previous_roles:=case when a.user_id is not null and a.base_role=t.role
  and hris_private.compatible_staff_base_role(a.roles)=t.role then a.roles else hris_private.legacy_staff_roles(t.role)end;
 if a.user_id is null then
  insert into hris_private.staff_role_assignments(company_id,user_id,roles,base_role)values(m.company_id,target,previous_roles,t.role);
  insert into hris_private.staff_role_events(company_id,user_id,actor_id,kind,roles,revision,reason,active)
   values(m.company_id,target,m.user_id,'legacy_backfill',previous_roles,1,'Captured protected membership before its first explicit role assignment.',t.active);
 end if;
 if expected=9223372036854775807 then raise exception 'Staff-role revision limit reached.'using errcode='23514';end if;
 select au.email into email from auth.users au where au.id=target;
 perform hris_private.save_company_account_legacy(target,email,base,desired_active,t.employee_id,null);
 next_revision:=expected+1;
 update hris_private.staff_role_assignments set roles=v_roles,revision=next_revision,base_role=base,updated_at=now()
  where company_id=m.company_id and user_id=target;
 insert into hris_private.staff_role_events(id,company_id,user_id,actor_id,kind,previous_roles,roles,revision,reason,previous_active,active)
  values(request_id,m.company_id,target,m.user_id,'assignment_changed',previous_roles,v_roles,next_revision,reason,t.active,desired_active);
 select *into t from public.company_members where company_id=m.company_id and user_id=target;
 result:=jsonb_build_object('schema_version',1,'id',request_id,'company_id',m.company_id,'actor_id',m.user_id,
  'user_id',target,'employee_id',t.employee_id,'active',t.active,'roles',v_roles,'revision',next_revision,'base_role',base,'member_updated_at',t.updated_at);
 insert into hris_private.staff_role_change_receipts(id,company_id,user_id,actor_id,payload,receipt)
  values(request_id,m.company_id,target,m.user_id,p_change,result);
 return result;
end;$$;
create function public.set_staff_roles(p_change jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.set_staff_roles(p_change)$$;
revoke all on function hris_private.compatible_staff_base_role(text[])from public,anon,authenticated,service_role;
revoke all on function hris_private.set_staff_roles(jsonb),public.set_staff_roles(jsonb)from public,anon,authenticated,service_role;
grant execute on function hris_private.set_staff_roles(jsonb),public.set_staff_roles(jsonb)to authenticated;
comment on table hris_private.staff_role_runtime_policy is 'Deployment gate only. Keep false until all role-specific native/RLS/RPC/storage scopes are verified.';
commit;
