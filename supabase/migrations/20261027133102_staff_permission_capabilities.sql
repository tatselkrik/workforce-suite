-- Capability kernel only. No existing policy/RPC is broadened and the gate stays FALSE.
begin;
create function hris_private.current_staff_capabilities(p_company uuid)returns text[]
language sql stable security definer set search_path=''as $$
 select coalesce((select hris_private.staff_role_capabilities(
  case when coalesce(policy.runtime_ready,false)and a.user_id is not null and a.base_role=m.role
   and hris_private.compatible_staff_base_role(a.roles)=m.role then a.roles else hris_private.legacy_staff_roles(m.role)end)
 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id and e.status='active'
 left join hris_private.staff_role_assignments a on a.company_id=m.company_id and a.user_id=m.user_id
 left join hris_private.staff_role_runtime_policy policy on policy.company_id=m.company_id
 where m.company_id=p_company and m.user_id=(select auth.uid())and m.active),'{}'::text[])
$$;
create function hris_private.has_staff_capability(p_company uuid,p_capability text)returns boolean
language sql stable security invoker set search_path=''as $$
 select coalesce(p_capability=any(hris_private.current_staff_capabilities(p_company)),false)
$$;
revoke all on function hris_private.current_staff_capabilities(uuid),hris_private.has_staff_capability(uuid,text)from public,anon,authenticated,service_role;
grant execute on function hris_private.current_staff_capabilities(uuid),hris_private.has_staff_capability(uuid,text)to authenticated;
comment on function hris_private.current_staff_capabilities(uuid)is 'Current protected membership only; matches the bound native context and preserves legacy rights while rollout is disabled. No user metadata or caller-selected identity.';
comment on function hris_private.has_staff_capability(uuid,text)is 'A module permission is necessary, not sufficient: retain per-person scope, source freshness, independent approvals and private-field projection guards.';
commit;