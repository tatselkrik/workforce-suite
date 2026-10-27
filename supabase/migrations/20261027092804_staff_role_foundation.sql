-- Additive staff-role contract. No existing role gate or client is changed here.
-- Specialist activation follows only after versioned native/server module guards.
begin;

create function hris_private.normalize_staff_roles(p_roles text[]) returns text[]
language plpgsql immutable security invoker set search_path='' as $$
declare v_roles text[]; v_count integer;
begin
 if p_roles is null or cardinality(p_roles)<1 or cardinality(p_roles)>6
    or array_ndims(p_roles)<>1 or array_position(p_roles,null) is not null then
  raise exception 'Choose 1–6 staff roles, or Employee alone.' using errcode='23514';
 end if;
 if exists(select 1 from unnest(p_roles)r where r not in
  ('administrator','hr_generalist','recruiter','learning_performance_officer','payroll_officer','manager','employee'))then
  raise exception 'Choose a supported staff role.' using errcode='23514';
 end if;
 select count(distinct r)into v_count from unnest(p_roles)r;
 if v_count<>cardinality(p_roles)or(cardinality(p_roles)>1 and 'employee'=any(p_roles))then
  raise exception 'Roles must be distinct; Employee is a portal-only assignment.'using errcode='23514';
 end if;
 select array_agg(r order by array_position(array['administrator','hr_generalist','recruiter',
  'learning_performance_officer','payroll_officer','manager','employee']::text[],r))
 into v_roles from unnest(p_roles)r;
 return v_roles;
end;$$;

create function hris_private.legacy_staff_roles(p_role text)returns text[]
language plpgsql immutable security invoker set search_path='' as $$
begin
 case p_role
 when 'administrator'then return array['administrator']::text[];
 when 'hr_generalist'then return array['hr_generalist','recruiter','learning_performance_officer','payroll_officer']::text[];
 when 'manager'then return array['manager']::text[];
 when 'employee'then return array['employee']::text[];
 else raise exception 'Unsupported legacy role.'using errcode='23514';
 end case;
end;$$;

create function hris_private.staff_role_capabilities(p_roles text[])returns text[]
language plpgsql immutable security invoker set search_path='' as $$
declare r text[]:=hris_private.normalize_staff_roles(p_roles); c text[]:=array['own_records']::text[];
begin
 if 'administrator'=any(r)then return array['own_records','desktop','personnel','attendance_administration',
  'leave_administration','private_hr_notes','personnel_documents','general_hr_reports','recruiting',
  'development_administration','payroll_preparation','government_preparation','payroll_reports',
  'team_review','account_administration','company_configuration','payroll_approval','payment_recording']::text[];end if;
 if not('employee'=any(r))then c:=c||array['desktop']::text[];end if;
 if 'hr_generalist'=any(r)then c:=c||array['personnel','attendance_administration','leave_administration',
  'private_hr_notes','personnel_documents','general_hr_reports']::text[];end if;
 if 'recruiter'=any(r)then c:=c||array['recruiting']::text[];end if;
 if 'learning_performance_officer'=any(r)then c:=c||array['development_administration']::text[];end if;
 if 'payroll_officer'=any(r)then c:=c||array['payroll_preparation','government_preparation','payroll_reports']::text[];end if;
 if 'manager'=any(r)then c:=c||array['team_review']::text[];end if;
 return c;
end;$$;

create table hris_private.staff_role_assignments(
 company_id uuid not null,
 user_id uuid not null,
 roles text[]not null check(roles=hris_private.normalize_staff_roles(roles)),
 revision bigint not null default 1 check(revision>0),
 base_role text not null check(base_role in('administrator','hr_generalist','manager','employee')),
 created_at timestamptz not null default now(),
 updated_at timestamptz not null default now(),
 primary key(company_id,user_id),
 foreign key(company_id,user_id)references public.company_members(company_id,user_id)on delete restrict
);
create table hris_private.staff_role_events(
 id uuid primary key default gen_random_uuid(),
 company_id uuid not null,
 user_id uuid not null,
 actor_id uuid references auth.users(id)on delete restrict,
 kind text not null check(kind in('legacy_backfill','assignment_changed')),
 previous_roles text[],
 roles text[]not null check(roles=hris_private.normalize_staff_roles(roles)),
 revision bigint not null check(revision>0),
 reason text not null check(length(btrim(reason))between 1 and 500),
 created_at timestamptz not null default now(),
 foreign key(company_id,user_id)references hris_private.staff_role_assignments(company_id,user_id)on delete restrict,
 check(kind='legacy_backfill'or actor_id is not null),
 unique(company_id,user_id,revision)
);
create index staff_role_events_actor_idx on hris_private.staff_role_events(actor_id);

alter table hris_private.staff_role_assignments enable row level security;
alter table hris_private.staff_role_events enable row level security;
revoke all on hris_private.staff_role_assignments,hris_private.staff_role_events from public,anon,authenticated,service_role;

insert into hris_private.staff_role_assignments(company_id,user_id,roles,base_role)
 select company_id,user_id,hris_private.legacy_staff_roles(role),role from public.company_members;
insert into hris_private.staff_role_events(company_id,user_id,kind,roles,revision,reason)
 select company_id,user_id,'legacy_backfill',roles,revision,
 'Initial compatibility mapping from protected membership; no existing access changed.'
 from hris_private.staff_role_assignments;

create function hris_private.keep_staff_role_event()returns trigger
language plpgsql security invoker set search_path='' as $$
begin raise exception 'Staff-role history is immutable.'using errcode='23514';end;$$;
create trigger staff_role_events_immutable before update or delete on hris_private.staff_role_events
 for each row execute function hris_private.keep_staff_role_event();

create function hris_private.staff_role_context()returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare m public.company_members; person public.employees; assignment hris_private.staff_role_assignments;
 roles text[]; rev bigint:=1;
begin
 select cm.*into m from public.company_members cm join public.employees e
  on e.company_id=cm.company_id and e.id=cm.employee_id
  where cm.user_id=(select auth.uid())and cm.active and e.status='active';
 if m.user_id is null then raise exception 'Current active employee membership is required.'using errcode='42501';end if;
 select *into assignment from hris_private.staff_role_assignments
  where company_id=m.company_id and user_id=m.user_id;
 roles:=hris_private.legacy_staff_roles(m.role);
 if assignment.user_id is not null then
  rev:=assignment.revision;
  if assignment.base_role=m.role then roles:=assignment.roles;end if;
 end if;
 return jsonb_build_object('schema_version',1,'company_id',m.company_id,'user_id',m.user_id,
  'employee_id',m.employee_id,'base_role',m.role,'revision',rev,'runtime_ready',false,'roles',roles);
end;$$;

create function hris_private.staff_role_admin_context(p_include_disabled boolean)returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare m public.company_members; accounts jsonb; n integer;
begin
 select cm.*into m from public.company_members cm join public.employees e
  on e.company_id=cm.company_id and e.id=cm.employee_id
  where cm.user_id=(select auth.uid())and cm.active and cm.role='administrator'and e.status='active';
 if m.user_id is null then raise exception 'Only a current Administrator reads company staff-role assignments.'using errcode='42501';end if;
 if p_include_disabled is null then raise exception 'Choose whether to include disabled accounts.'using errcode='23514';end if;
 select count(*)into n from public.company_members where company_id=m.company_id and(active or p_include_disabled);
 if n>1000 then raise exception 'Staff-role history exceeds this bounded view; narrow scope support is required.'using errcode='23514';end if;
 select coalesce(jsonb_agg(jsonb_build_object('user_id',cm.user_id,'employee_id',cm.employee_id,
  'employee_code',e.employee_code,'full_name',e.full_name,'active',cm.active,'base_role',cm.role,
  'revision',coalesce(a.revision,1),'roles',case when a.base_role=cm.role then a.roles else hris_private.legacy_staff_roles(cm.role)end)
  order by e.employee_code,cm.user_id),'[]'::jsonb)into accounts
  from public.company_members cm join public.employees e on e.id=cm.employee_id and e.company_id=cm.company_id
  left join hris_private.staff_role_assignments a on a.company_id=cm.company_id and a.user_id=cm.user_id
  where cm.company_id=m.company_id and(cm.active or p_include_disabled);
 return jsonb_build_object('schema_version',1,'company_id',m.company_id,'user_id',m.user_id,
  'runtime_ready',false,'accounts',accounts);
end;$$;

create function public.staff_role_context()returns jsonb
language sql stable security invoker set search_path='' as $$select hris_private.staff_role_context()$$;
create function public.staff_role_admin_context(p_include_disabled boolean default false)returns jsonb
language sql stable security invoker set search_path='' as $$select hris_private.staff_role_admin_context(p_include_disabled)$$;

revoke all on function hris_private.normalize_staff_roles(text[]),hris_private.legacy_staff_roles(text),
 hris_private.staff_role_capabilities(text[]),hris_private.keep_staff_role_event()from public,anon,authenticated,service_role;
revoke all on function hris_private.staff_role_context(),hris_private.staff_role_admin_context(boolean),
 public.staff_role_context(),public.staff_role_admin_context(boolean)from public,anon,authenticated,service_role;
grant execute on function hris_private.staff_role_context(),hris_private.staff_role_admin_context(boolean),
 public.staff_role_context(),public.staff_role_admin_context(boolean)to authenticated;

comment on function public.staff_role_context()is 'Versioned protected role metadata. runtime_ready=false until native and module authorization rollout is verified; not a new grant.';
commit;
