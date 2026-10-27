-- Versioned operational staff names/roles only; no Auth emails or account IDs.
begin;
create function hris_private.staff_workflow_people()returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;people jsonb;n integer;
begin
 select cm.*into m from public.company_members cm join public.employees actor
 on actor.company_id=cm.company_id and actor.id=cm.employee_id and actor.status='active'
 where cm.user_id=(select auth.uid())and cm.active;
 if m.user_id is null or not hris_private.has_staff_capability(m.company_id,'desktop')then
  raise exception 'Current staff access is required for workflow assignments.'using errcode='42501';end if;
 if not coalesce((select runtime_ready from hris_private.staff_role_runtime_policy where company_id=m.company_id),false)then
  raise exception 'The protected staff directory is not enabled.'using errcode='42501';end if;
 with eligible as(
  select e.id employee_id,e.full_name,cm.role,
   case when a.user_id is not null and a.base_role=cm.role and hris_private.compatible_staff_base_role(a.roles)=cm.role then a.roles else hris_private.legacy_staff_roles(cm.role)end roles
  from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id and e.status='active'
  left join hris_private.staff_role_assignments a on a.company_id=cm.company_id and a.user_id=cm.user_id
  where cm.company_id=m.company_id and cm.active and 'desktop'=any(hris_private.staff_capabilities_for(cm.company_id,cm.user_id))
  order by e.full_name,e.id limit 1001
 )select coalesce(jsonb_agg(to_jsonb(person)order by person.full_name,person.employee_id),'[]'),count(*)into people,n from eligible person;
 if n>1000 then raise exception 'Staff assignment directory exceeds its supported budget; no staff were silently omitted.'using errcode='23514';end if;
 return jsonb_build_object('schema_version',1,'company_id',m.company_id,'user_id',m.user_id,'runtime_ready',true,'person_count',n,'people',people);
end;$$;
create function public.staff_workflow_people()returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.staff_workflow_people()$$;
revoke all on function hris_private.staff_workflow_people(),public.staff_workflow_people()from public,anon,authenticated,service_role;
grant execute on function hris_private.staff_workflow_people(),public.staff_workflow_people()to authenticated;
commit;
