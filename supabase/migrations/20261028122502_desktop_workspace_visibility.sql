-- Presentation configuration only. Domain authority and existing records are unchanged.
begin;
create function hris_private.normalize_desktop_modules(p_modules text[])returns text[]
language plpgsql immutable security invoker set search_path=''as $$
declare ordered text[]:=array['organization','development','hiring','exit_clearance','payroll','reports'];r text[];
begin
 if p_modules is null or coalesce(array_ndims(p_modules),1)<>1 or cardinality(p_modules)>6
 or array_position(p_modules,null)is not null or exists(select 1 from unnest(p_modules)x where not x=any(ordered))
 or cardinality(p_modules)<>(select count(distinct x)from unnest(p_modules)x)
 then raise exception 'Choose distinct supported optional desktop sections.'using errcode='23514';end if;
 select coalesce(array_agg(x order by array_position(ordered,x)),array[]::text[])into r from unnest(p_modules)x;
 return r;
end;$$;
create table hris_private.desktop_workspace_settings(
 company_id uuid primary key references public.companies(id)on delete restrict,
 revision bigint not null check(revision>0),
 enabled text[]not null check(enabled=hris_private.normalize_desktop_modules(enabled)),
 updated_at timestamptz not null default now()
);
create table hris_private.desktop_workspace_events(
 id uuid primary key,
 company_id uuid not null references public.companies(id)on delete restrict,
 actor_id uuid not null references auth.users(id)on delete restrict,
 payload jsonb not null,
 previous_enabled text[]not null,
 enabled text[]not null,
 revision bigint not null check(revision>0),
 receipt jsonb not null,
 created_at timestamptz not null default now(),
 unique(company_id,revision)
);
alter table hris_private.desktop_workspace_settings enable row level security;
alter table hris_private.desktop_workspace_events enable row level security;
revoke all on hris_private.desktop_workspace_settings,hris_private.desktop_workspace_events from public,anon,authenticated,service_role;

create function hris_private.keep_desktop_workspace_event()returns trigger
language plpgsql security invoker set search_path=''as $$begin
 raise exception 'Desktop visibility history is immutable.'using errcode='23514';
end;$$;
create trigger desktop_workspace_event_immutable before update or delete on hris_private.desktop_workspace_events
for each row execute function hris_private.keep_desktop_workspace_event();

create function hris_private.desktop_workspace_context()returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;s hris_private.desktop_workspace_settings;
begin
 select cm.*into m from public.company_members cm join public.employees e
 on e.company_id=cm.company_id and e.id=cm.employee_id
 where cm.user_id=(select auth.uid())and cm.active and e.status='active'
 and hris_private.has_staff_capability(cm.company_id,'desktop');
 if m.user_id is null then raise exception 'Current desktop staff access is required.'using errcode='42501';end if;
 select *into s from hris_private.desktop_workspace_settings where company_id=m.company_id;
 return jsonb_build_object('schema_version',1,'company_id',m.company_id,'user_id',m.user_id,
 'revision',coalesce(s.revision,0),'enabled',coalesce(s.enabled,array['organization','development','hiring','exit_clearance','payroll','reports']::text[]),
 'updated_at',s.updated_at);
end;$$;
create function hris_private.save_desktop_workspaces(p_change jsonb)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;s hris_private.desktop_workspace_settings;prior hris_private.desktop_workspace_events;
 request_id uuid;expected bigint;enabled text[];previous text[];rev bigint;at_time timestamptz;receipt jsonb;
begin
 select cm.*into m from public.company_members cm join public.employees e
 on e.company_id=cm.company_id and e.id=cm.employee_id
 where cm.user_id=(select auth.uid())and cm.active and cm.role='administrator'and e.status='active'
 and hris_private.has_staff_capability(cm.company_id,'company_configuration');
 if m.user_id is null then raise exception 'Only a current Administrator configures desktop sections.'using errcode='42501';end if;
 if p_change is null or jsonb_typeof(p_change)<>'object'or pg_column_size(p_change)>4096
 or exists(select 1 from jsonb_object_keys(p_change)k where k not in('id','company_id','actor_id','expected_revision','enabled'))
 or(p_change->>'company_id')::uuid is distinct from m.company_id
 or(p_change->>'actor_id')::uuid is distinct from m.user_id
 or jsonb_typeof(p_change->'enabled')is distinct from'array'
 or exists(select 1 from jsonb_array_elements(p_change->'enabled')v where jsonb_typeof(v)<>'string')
 then raise exception 'Use the current supported desktop visibility request.'using errcode='23514';end if;
 request_id:=(p_change->>'id')::uuid;expected:=(p_change->>'expected_revision')::bigint;
 if request_id is null or request_id='00000000-0000-0000-0000-000000000000'::uuid or expected is null or expected<0
 then raise exception 'A request identity and current revision are required.'using errcode='23514';end if;
 select hris_private.normalize_desktop_modules(coalesce(array_agg(v#>>'{}'),array[]::text[]))into enabled
 from jsonb_array_elements(p_change->'enabled')v;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 if not exists(select 1 from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id
 where cm.company_id=m.company_id and cm.user_id=m.user_id and cm.active and cm.role='administrator'and e.status='active'
 and hris_private.has_staff_capability(cm.company_id,'company_configuration'))then
 raise exception 'Your Administrator access changed.'using errcode='42501';end if;
 select *into prior from hris_private.desktop_workspace_events where id=request_id;
 if prior.id is not null then
  if prior.company_id<>m.company_id or prior.actor_id<>m.user_id or prior.payload<>p_change then
   raise exception 'This request identity belongs to another actor or payload.'using errcode='42501';end if;
  return prior.receipt;
 end if;
 select *into s from hris_private.desktop_workspace_settings where company_id=m.company_id for update;
 if expected<>coalesce(s.revision,0)then raise exception 'Desktop sections changed. Refresh and review the current settings.'using errcode='23514';end if;
 previous:=coalesce(s.enabled,array['organization','development','hiring','exit_clearance','payroll','reports']::text[]);
 rev:=coalesce(s.revision,0)+1;at_time:=clock_timestamp();
 insert into hris_private.desktop_workspace_settings(company_id,revision,enabled,updated_at)
 values(m.company_id,rev,enabled,at_time)on conflict(company_id)do update
 set revision=excluded.revision,enabled=excluded.enabled,updated_at=excluded.updated_at;
 receipt:=jsonb_build_object('schema_version',1,'id',request_id,'company_id',m.company_id,'actor_id',m.user_id,
 'revision',rev,'enabled',enabled,'applied_at',at_time);
 insert into hris_private.desktop_workspace_events(id,company_id,actor_id,payload,previous_enabled,enabled,revision,receipt,created_at)
 values(request_id,m.company_id,m.user_id,p_change,previous,enabled,rev,receipt,at_time);
 return receipt;
end;$$;

create function public.desktop_workspace_context()returns jsonb language sql stable security invoker set search_path=''
as $$select hris_private.desktop_workspace_context()$$;
create function public.save_desktop_workspaces(p_change jsonb)returns jsonb language sql security invoker set search_path=''
as $$select hris_private.save_desktop_workspaces(p_change)$$;
revoke all on function hris_private.normalize_desktop_modules(text[]),hris_private.keep_desktop_workspace_event(),
 hris_private.desktop_workspace_context(),hris_private.save_desktop_workspaces(jsonb),
 public.desktop_workspace_context(),public.save_desktop_workspaces(jsonb)from public,anon,authenticated,service_role;
grant execute on function hris_private.desktop_workspace_context(),hris_private.save_desktop_workspaces(jsonb),
 public.desktop_workspace_context(),public.save_desktop_workspaces(jsonb)to authenticated;
comment on table hris_private.desktop_workspace_settings is 'Optional native navigation only; never grants/revokes data authority or removes source records.';
notify pgrst,'reload schema';
commit;
