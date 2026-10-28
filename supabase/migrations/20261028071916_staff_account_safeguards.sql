-- Keep established employee identities and original access events. Specialists
-- use reviewed prospective roles internally, never a caller-set session bypass.
begin;
do $scoped_saver$
declare source text;needle text;
begin
 source:=pg_get_functiondef('hris_private.save_company_account_legacy(uuid,text,text,boolean,uuid,uuid)'::regprocedure);
 needle:='p_manager_user_id uuid)';
 if (length(source)-length(replace(source,needle,'')))/length(needle)<>1 then raise exception 'Review scoped account saver signature.';end if;
 source:=replace(source,'FUNCTION hris_private.save_company_account_legacy(','FUNCTION hris_private.save_company_account_scoped(');
 source:=replace(source,needle,'p_manager_user_id uuid, p_staff_roles text[])');
 needle:='if p_role is null or p_role not in';
 if (length(source)-length(replace(source,needle,'')))/length(needle)<>1 then raise exception 'Review scoped account role validation.';end if;
 source:=replace(source,needle,$new$p_staff_roles:=hris_private.normalize_staff_roles(p_staff_roles);
    if hris_private.compatible_staff_base_role(p_staff_roles)is distinct from p_role then
        raise exception 'The reviewed staff roles and compatibility role differ.'using errcode='23514';end if;
    if p_role is null or p_role not in$new$);
 needle:=$old$p_active and p_role='employee'$old$;
 if (length(source)-length(replace(source,needle,'')))/length(needle)<>1 then raise exception 'Review supervisor guard.';end if;
 source:=replace(source,needle,$new$p_active and not('desktop'=any(hris_private.staff_role_capabilities(p_staff_roles)))$new$);
 execute source;
end;$scoped_saver$;
revoke all on function hris_private.save_company_account_scoped(uuid,text,text,boolean,uuid,uuid,text[])from public,anon,authenticated,service_role;

-- The caller is a checked internal entry point or deployment, never an API user.
-- A baseline captures current facts; missing earlier status remains unknown.
create function hris_private.reconcile_staff_assignment(p_company uuid,p_user uuid,p_actor uuid,p_reason text)returns void
language plpgsql security definer set search_path=''as $$
declare m public.company_members;a hris_private.staff_role_assignments;r text[];last_active boolean;next_revision bigint;
begin
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(p_company::text,7));
 select *into m from public.company_members where company_id=p_company and user_id=p_user for update;
 if m.user_id is null then raise exception 'Protected employee membership is required.'using errcode='42501';end if;
 select *into a from hris_private.staff_role_assignments where company_id=p_company and user_id=p_user for update;
 r:=case when a.user_id is not null and a.base_role=m.role and hris_private.compatible_staff_base_role(a.roles)=m.role then a.roles else hris_private.legacy_staff_roles(m.role)end;
 select e.active into last_active from hris_private.staff_role_events e where e.company_id=p_company and e.user_id=p_user order by e.revision desc limit 1;
 if a.user_id is not null and a.base_role=m.role and a.roles=r and last_active is not distinct from m.active then return;end if;
 if a.revision=9223372036854775807 then raise exception 'Staff-role revision limit reached.'using errcode='23514';end if;
 next_revision:=coalesce(a.revision,0)+1;
 insert into hris_private.staff_role_assignments(company_id,user_id,roles,base_role,revision)values(p_company,p_user,r,m.role,next_revision)
 on conflict(company_id,user_id)do update set roles=excluded.roles,base_role=excluded.base_role,revision=excluded.revision,updated_at=clock_timestamp();
 insert into hris_private.staff_role_events(company_id,user_id,actor_id,kind,previous_roles,roles,revision,reason,previous_active,active)
 values(p_company,p_user,p_actor,case when p_actor is null or a.user_id is null then'legacy_backfill'else'assignment_changed'end,a.roles,r,next_revision,p_reason,last_active,m.active);
end;$$;
revoke all on function hris_private.reconcile_staff_assignment(uuid,uuid,uuid,text)from public,anon,authenticated,service_role;

create or replace function hris_private.save_company_account(p_user_id uuid,p_email text,p_role text,p_active boolean,p_employee_id uuid,p_manager_user_id uuid)returns uuid
language plpgsql security definer set search_path=''as $$
declare m public.company_members;t public.company_members;a hris_private.staff_role_assignments;target uuid;ready boolean;result uuid;
begin
 select cm.*into m from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id and e.status='active'
 where cm.user_id=(select auth.uid())and cm.active and cm.role='administrator';
 if m.user_id is null then raise exception 'Only a current active Administrator manages accounts.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 if not hris_private.has_staff_capability(m.company_id,'account_administration')then raise exception 'Your Administrator access changed.'using errcode='42501';end if;
 select coalesce((select runtime_ready from hris_private.staff_role_runtime_policy where company_id=m.company_id),false)into ready;
 target:=coalesce(p_user_id,(select id from auth.users where lower(email)=lower(btrim(p_email))));
 select *into t from public.company_members where user_id=target and company_id=m.company_id for update;
 select *into a from hris_private.staff_role_assignments where user_id=target and company_id=m.company_id for update;
 if ready then
  if t.user_id is null then raise exception 'Use the updated Account Manager to provision employee accounts.'using errcode='42501';end if;
  if((p_role='hr_generalist'and t.role is distinct from p_role)
   or(a.user_id is not null and a.base_role=t.role and a.roles<>hris_private.legacy_staff_roles(t.role)))
   and not(p_active is false and p_role=t.role)then
   raise exception 'Use the updated Account Manager for specialist roles; existing access can still be disabled.'using errcode='42501';end if;
 end if;
 result:=hris_private.save_company_account_legacy(p_user_id,p_email,p_role,p_active,p_employee_id,p_manager_user_id);
 perform hris_private.reconcile_staff_assignment(m.company_id,result,m.user_id,'Legacy Account Manager role/access change; original account access event retained.');
 return result;
end;$$;

-- Both callers already validate current Administrator, source, exact retry,
-- employee identity, owner/self/last-Admin and company/employee locks.
do $account_safeguards$
declare spec record;source text;matches integer;
begin
 for spec in select *from(values
 ('hris_private.set_staff_roles(jsonb)',$old$hris_private.save_company_account_legacy(target,email,base,desired_active,t.employee_id,null)$old$,$new$hris_private.save_company_account_scoped(target,email,base,desired_active,t.employee_id,null,v_roles)$new$,1),
 ('hris_private.commit_staff_provision(jsonb)',$old$hris_private.save_company_account_legacy(saved.user_id,saved.email,base,(p_request->>'active')::boolean,person,null)$old$,$new$hris_private.save_company_account_scoped(saved.user_id,saved.email,base,(p_request->>'active')::boolean,person,null,r)$new$,1),
 ('hris_private.my_hris_access_legacy()',$old$member.role='manager'$old$,$new$'team_review'=any(hris_private.staff_capabilities_for(member.company_id,member.user_id))$new$,1)
 )v(signature,needle,replacement,expected)
 loop
  source:=pg_get_functiondef(spec.signature::regprocedure);matches:=(length(source)-length(replace(source,spec.needle,'')))/length(spec.needle);
  if matches<>spec.expected then raise exception 'Review account safeguard compatibility for %',spec.signature;end if;
  execute replace(source,spec.needle,spec.replacement);
 end loop;
end;$account_safeguards$;

-- Reconcile any legacy changes between staged migrations without rewriting a
-- membership, original access event, receipt or prior immutable role event.
do $capture_current_memberships$
declare member record;
begin
 for member in select company_id,user_id from public.company_members order by company_id,user_id loop
  perform hris_private.reconcile_staff_assignment(member.company_id,member.user_id,null,
   'Captured current protected membership during account safeguard rollout; earlier access history retained.');
 end loop;
end;$capture_current_memberships$;
notify pgrst,'reload schema';
commit;
