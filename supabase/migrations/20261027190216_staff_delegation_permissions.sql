-- Role-bound operational delegation. Existing grants/decisions remain immutable.
-- Saved legacy roles are mapped from their original grant, never today's account.
begin;
create function hris_private.staff_roles_for(p_company uuid,p_user uuid)returns text[]
language sql stable security definer set search_path=''as $$
 select coalesce((select case when coalesce(policy.runtime_ready,false)and a.user_id is not null
  and a.base_role=m.role and hris_private.compatible_staff_base_role(a.roles)=m.role then a.roles
  else hris_private.legacy_staff_roles(m.role)end
 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id and e.status='active'
 left join hris_private.staff_role_assignments a on a.company_id=m.company_id and a.user_id=m.user_id
 left join hris_private.staff_role_runtime_policy policy on policy.company_id=m.company_id
 where m.company_id=p_company and m.user_id=p_user and m.active),'{}'::text[])
$$;
create function hris_private.staff_delegation_substitute(p_scope text,p_roles text[],p_delegate_roles text[])returns boolean
language plpgsql immutable security invoker set search_path=''as $$
declare pc text[];dc text[];
begin
 if p_scope is null or p_scope not in('leave','attendance','overtime')or cardinality(p_roles)=0 or cardinality(p_delegate_roles)=0 then return false;end if;
 pc:=hris_private.staff_role_capabilities(p_roles);dc:=hris_private.staff_role_capabilities(p_delegate_roles);
 if not('desktop'=any(pc)and 'desktop'=any(dc))then return false;end if;
 if 'account_administration'=any(pc)then return 'account_administration'=any(dc);end if;
 if 'leave_administration'=any(pc)then return 'leave_administration'=any(dc);end if;
 if p_scope in('attendance','overtime')then
  return 'team_review'=any(pc)and('team_review'=any(dc)or 'attendance_administration'=any(dc));
 end if;
 -- Other staff can hold only a named leave-plan assignment; principal_allowed
 -- must still prove their original authority for this exact request and step.
 return true;
end;$$;
revoke all on function hris_private.staff_roles_for(uuid,uuid),hris_private.staff_delegation_substitute(text,text[],text[])from public,anon,authenticated,service_role;

create table hris_private.approval_delegation_role_bindings(
 delegation_id uuid primary key references public.approval_delegations(id)on delete restrict,
 principal_roles text[]not null check(principal_roles=hris_private.normalize_staff_roles(principal_roles)),
 delegate_roles text[]not null check(delegate_roles=hris_private.normalize_staff_roles(delegate_roles))
);
alter table hris_private.approval_delegation_role_bindings enable row level security;
revoke all on hris_private.approval_delegation_role_bindings from public,anon,authenticated,service_role;
insert into hris_private.approval_delegation_role_bindings
 select id,hris_private.legacy_staff_roles(principal_role),hris_private.legacy_staff_roles(delegate_role)from public.approval_delegations;
create trigger approval_delegation_role_bindings_immutable before update or delete on hris_private.approval_delegation_role_bindings
 for each row execute function hris_private.approval_immutable();
create function hris_private.bind_approval_delegation_roles()returns trigger
language plpgsql security definer set search_path=''as $$
declare pr text[];dr text[];
begin
 pr:=hris_private.staff_roles_for(new.company_id,new.principal_id);dr:=hris_private.staff_roles_for(new.company_id,new.delegate_id);
 if not hris_private.staff_delegation_substitute(new.scope,pr,dr)
  or hris_private.compatible_staff_base_role(pr)<>new.principal_role or hris_private.compatible_staff_base_role(dr)<>new.delegate_role then
  raise exception 'Current operational staff authority changed; review delegation.'using errcode='42501';end if;
 insert into hris_private.approval_delegation_role_bindings values(new.id,pr,dr);
 return new;
end;$$;
revoke all on function hris_private.bind_approval_delegation_roles()from public,anon,authenticated,service_role;
create trigger approval_delegations_bind_roles after insert on public.approval_delegations
 for each row execute function hris_private.bind_approval_delegation_roles();
-- These remain descriptive compatibility codes, not authority.
alter table public.approval_delegations drop constraint approval_delegations_principal_role_check;
alter table public.approval_delegations add constraint approval_delegations_principal_role_check check(principal_role in('administrator','hr_generalist','manager','employee'));
alter table public.approval_delegations drop constraint approval_delegations_delegate_role_check;
alter table public.approval_delegations add constraint approval_delegations_delegate_role_check check(delegate_role in('administrator','hr_generalist','manager','employee'));

do $delegation_permissions$
declare spec record;definition text;source text;matches integer;
begin
 for spec in select *from(values
 ('hris_private.approval_match(uuid,text,uuid,uuid,jsonb)',$old$m.role<>'employee'$old$,$new$'desktop'=any(hris_private.staff_capabilities_for(m.company_id,m.user_id))$new$,1),
 ('hris_private.delegated_request_read(text,uuid)',$old$m.role<>'employee'$old$,$new$'desktop'=any(hris_private.staff_capabilities_for(m.company_id,m.user_id))$new$,1),
 ('hris_private.save_approval_delegation(jsonb)',$old$cm.role<>'employee'$old$,$new$'desktop'=any(hris_private.staff_capabilities_for(cm.company_id,cm.user_id))$new$,2),
 ('hris_private.save_approval_delegation(jsonb)',$old$(p.role='administrator'and d.role<>'administrator')or(p.role='hr_generalist'and d.role not in('administrator','hr_generalist'))$old$,$new$ not hris_private.staff_delegation_substitute(chosen_scope,hris_private.staff_roles_for(company,p.user_id),hris_private.staff_roles_for(company,d.user_id))$new$,1),
 ('hris_private.approval_grant_current(uuid)',$old$and not exists(select 1 from public.approval_delegation_revocations r where r.delegation_id=d.id)$old$,$new$and exists(select 1 from hris_private.approval_delegation_role_bindings b where b.delegation_id=d.id
  and b.principal_roles=hris_private.staff_roles_for(d.company_id,d.principal_id)
  and b.delegate_roles=hris_private.staff_roles_for(d.company_id,d.delegate_id)
  and hris_private.staff_delegation_substitute(d.scope,b.principal_roles,b.delegate_roles))
 and not exists(select 1 from public.approval_delegation_revocations r where r.delegation_id=d.id)$new$,1)
 )v(signature,needle,replacement,expected)
 loop
  source:=pg_get_functiondef(spec.signature::regprocedure);matches:=(length(source)-length(replace(source,spec.needle,'')))/length(spec.needle);
  if matches<>spec.expected then raise exception 'Unexpected delegation predicate in %: expected %, found %',spec.signature,spec.expected,matches;end if;
  definition:=replace(source,spec.needle,spec.replacement);execute definition;
 end loop;
end;$delegation_permissions$;

-- Keep the installed v1 DTO intact. New clients request a separately versioned
-- projection with saved role bindings and protected current staff candidates.
create function hris_private.approval_delegation_staff_context()returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare c jsonb;company uuid;staff jsonb;grants jsonb;
begin
 c:=hris_private.approval_delegation_context();company:=(c->>'company_id')::uuid;
 select coalesce(jsonb_agg(g.value||jsonb_build_object('principal_staff_roles',b.principal_roles,'delegate_staff_roles',b.delegate_roles)order by(g.value->>'company_revision')::bigint),'[]'::jsonb)
 into grants from jsonb_array_elements(c->'grants')g(value)join hris_private.approval_delegation_role_bindings b on b.delegation_id=(g.value->>'id')::uuid;
 if jsonb_array_length(grants)<>jsonb_array_length(c->'grants')then raise exception 'Delegation role history is incomplete; no partial result returned.'using errcode='23514';end if;
 staff:='[]';
 if hris_private.has_staff_capability(company,'account_administration')then
  select coalesce(jsonb_agg(jsonb_build_object('user_id',cm.user_id,'employee_id',cm.employee_id,'name',e.full_name,'role',cm.role,'staff_roles',hris_private.staff_roles_for(company,cm.user_id))order by e.full_name,cm.user_id),'[]'::jsonb)
  into staff from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id and e.status='active'
  where cm.company_id=company and cm.active and 'desktop'=any(hris_private.staff_capabilities_for(company,cm.user_id));
  if jsonb_array_length(staff)>1000 then raise exception 'Delegation staff directory exceeds the supported limit; no partial result returned.'using errcode='23514';end if;
 end if;
 return c||jsonb_build_object('version','approval-delegation-v2','grants',grants,'staff',staff);
end;$$;
create function public.approval_delegation_staff_context()returns jsonb language sql security invoker set search_path=''as $$select hris_private.approval_delegation_staff_context();$$;
revoke all on function hris_private.approval_delegation_staff_context(),public.approval_delegation_staff_context()from public,anon,authenticated;
grant execute on function hris_private.approval_delegation_staff_context(),public.approval_delegation_staff_context()to authenticated;
notify pgrst,'reload schema';
commit;
