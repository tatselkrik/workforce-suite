-- Administrator-only bounded account history. No Auth/email/credential fields,
-- raw legacy JSON or current-name guesses presented as historical identity.
begin;
create function hris_private.staff_account_history(p_user_id uuid)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare actor public.company_members;subject public.company_members;person public.employees;
 assignment hris_private.staff_role_assignments;r text[];role_count integer;access_count integer;role_events jsonb;access_events jsonb;
begin
 select m.*into actor from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id and e.status='active'
 where m.user_id=(select auth.uid())and m.active and m.role='administrator';
 if actor.user_id is null or not hris_private.has_staff_capability(actor.company_id,'account_administration')then raise exception 'Only a current Administrator reads account history.'using errcode='42501';end if;
 if p_user_id is null or p_user_id='00000000-0000-0000-0000-000000000000'then raise exception 'Choose an employee account.'using errcode='23514';end if;
 select *into subject from public.company_members where company_id=actor.company_id and user_id=p_user_id;
 select *into person from public.employees where company_id=actor.company_id and id=subject.employee_id;
 if subject.user_id is null or person.id is null then raise exception 'Account history is unavailable in this company.'using errcode='42501';end if;
 select *into assignment from hris_private.staff_role_assignments where company_id=actor.company_id and user_id=p_user_id;
 r:=case when assignment.base_role=subject.role and hris_private.compatible_staff_base_role(assignment.roles)=subject.role then assignment.roles else hris_private.legacy_staff_roles(subject.role)end;
 select count(*)into role_count from hris_private.staff_role_events where company_id=actor.company_id and user_id=p_user_id;
 select count(*)into access_count from public.account_access_events where company_id=actor.company_id and user_id=p_user_id;
 if role_count>1000 or access_count>1000 then raise exception 'This account history exceeds the supported complete view; no records were omitted.'using errcode='54000';end if;
 select coalesce(jsonb_agg(jsonb_build_object('id',e.id,'actor_id',e.actor_id,'actor_name',p.full_name,'kind',e.kind,
  'previous_roles',e.previous_roles,'roles',e.roles,'revision',e.revision,'reason',e.reason,'previous_active',e.previous_active,'active',e.active,'created_at',e.created_at)order by e.revision),'[]'::jsonb)
 into role_events from hris_private.staff_role_events e left join public.company_members m on m.company_id=e.company_id and m.user_id=e.actor_id
 left join public.employees p on p.company_id=m.company_id and p.id=m.employee_id where e.company_id=actor.company_id and e.user_id=p_user_id;
 select coalesce(jsonb_agg(jsonb_build_object('id',e.id,'actor_id',e.actor_id,'actor_name',p.full_name,'action',e.action,
  'previous_role',case when e.details->>'old_role'in('administrator','hr_generalist','manager','employee')then e.details->>'old_role'end,
  'role',case when e.details->>'role'in('administrator','hr_generalist','manager','employee')then e.details->>'role'end,
  'previous_active',case when jsonb_typeof(e.details->'old_active')='boolean'then(e.details->>'old_active')::boolean end,
  'active',case when jsonb_typeof(e.details->'active')='boolean'then(e.details->>'active')::boolean end,'created_at',e.created_at)order by e.created_at,e.id),'[]'::jsonb)
 into access_events from public.account_access_events e left join public.company_members m on m.company_id=e.company_id and m.user_id=e.actor_id
 left join public.employees p on p.company_id=m.company_id and p.id=m.employee_id where e.company_id=actor.company_id and e.user_id=p_user_id;
 return jsonb_build_object('schema_version',1,'company_id',actor.company_id,'actor_id',actor.user_id,'user_id',p_user_id,
  'employee_id',person.id,'employee_code',person.employee_code,'full_name',person.full_name,'revision',coalesce(assignment.revision,1),
  'current_roles',r,'active',subject.active,'member_updated_at',subject.updated_at,'role_event_count',role_count,'access_event_count',access_count,
  'role_events',role_events,'access_events',access_events);
end;$$;
create function public.staff_account_history(p_user_id uuid)returns jsonb language sql security invoker set search_path=''as $$select hris_private.staff_account_history(p_user_id);$$;
revoke all on function hris_private.staff_account_history(uuid),public.staff_account_history(uuid)from public,anon,authenticated,service_role;
grant execute on function hris_private.staff_account_history(uuid),public.staff_account_history(uuid)to authenticated;
notify pgrst,'reload schema';
commit;
