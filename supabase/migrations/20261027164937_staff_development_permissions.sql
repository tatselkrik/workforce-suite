-- Development administration separated from confidential HR notes/files.
-- No source record, publication, completion or career fact is rewritten.
begin;
-- Internal-only subject helper validates assigned reviewers. Own context remains
-- bound to auth.uid(); no caller-selectable permission inspection is granted.
create function hris_private.staff_capabilities_for(p_company uuid,p_user uuid)returns text[]
language sql stable security definer set search_path=''as $$
 select coalesce((select hris_private.staff_role_capabilities(
  case when coalesce(policy.runtime_ready,false)and a.user_id is not null and a.base_role=m.role
   and hris_private.compatible_staff_base_role(a.roles)=m.role then a.roles else hris_private.legacy_staff_roles(m.role)end)
 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id and e.status='active'
 left join hris_private.staff_role_assignments a on a.company_id=m.company_id and a.user_id=m.user_id
 left join hris_private.staff_role_runtime_policy policy on policy.company_id=m.company_id
 where m.company_id=p_company and m.user_id=p_user and m.active),'{}'::text[])
$$;
revoke all on function hris_private.staff_capabilities_for(uuid,uuid)from public,anon,authenticated,service_role;
create or replace function hris_private.current_staff_capabilities(p_company uuid)returns text[]
language sql stable security definer set search_path=''as $$select hris_private.staff_capabilities_for(p_company,(select auth.uid()))$$;

do $development_permissions$
declare spec record;definition text;source text;matches integer;
begin
 for spec in select *from(values
 ('hris_private.manage_performance(text,jsonb)',$old$role in ('administrator','hr_generalist','manager')$old$,$new$hris_private.has_staff_capability(company_id,'desktop')$new$,2),
 ('hris_private.manage_learning(text,jsonb)',$old$role in('administrator','hr_generalist','manager')$old$,$new$hris_private.has_staff_capability(company_id,'desktop')$new$,2),
 ('hris_private.manage_career(text,jsonb)',$old$role in('administrator','hr_generalist','manager')$old$,$new$hris_private.has_staff_capability(company_id,'desktop')$new$,2),
 ('hris_private.manage_performance(text,jsonb)',$old$m.role not in ('administrator','hr_generalist') then raise exception 'HR notes$old$,$new$not hris_private.has_staff_capability(m.company_id,'private_hr_notes') then raise exception 'HR notes$new$,1),
 ('hris_private.manage_performance(text,jsonb)',$old$m.role not in ('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(m.company_id,'development_administration')$new$,4),
 ('hris_private.manage_performance(text,jsonb)',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'development_administration')$new$,3),
 ('hris_private.manage_learning(text,jsonb)',$old$m.role not in('administrator','hr_generalist')then raise exception 'Certification files$old$,$new$not hris_private.has_staff_capability(m.company_id,'personnel_documents')then raise exception 'Certification files$new$,1),
 ('hris_private.manage_learning(text,jsonb)',$old$m.role not in('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(m.company_id,'development_administration')$new$,4),
 ('hris_private.manage_career(text,jsonb)',$old$m.role not in('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(m.company_id,'private_hr_notes')$new$,1),
 ('hris_private.manage_career(text,jsonb)',$old$if hris_private.career_manage(m.company_id,task.employee_id)is not true then raise exception$old$,$new$if hris_private.career_manage(m.company_id,task.employee_id)is not true and not(hris_private.has_staff_capability(m.company_id,'private_hr_notes')and m.employee_id<>task.employee_id and(p_record->>'status'='waived'or(task.status='waived'and p_record->>'status'='open')))then raise exception$new$,1),
 ('hris_private.manage_career(text,jsonb)',$old$if task.status<>'open'then raise exception 'Only open milestones accept progress.'$old$,$new$if not hris_private.has_staff_capability(m.company_id,'development_administration')and m.employee_id<>task.employee_id and not(hris_private.has_staff_capability(m.company_id,'team_review')and person.reports_to=m.employee_id)then raise exception 'Only the employee, current Manager or development staff update shared progress.'using errcode='42501';end if; if task.status<>'open'then raise exception 'Only open milestones accept progress.'$new$,1),
 ('hris_private.career_scope(uuid,uuid)',$old$m.role in('administrator','hr_generalist')$old$,$new$(hris_private.has_staff_capability(m.company_id,'development_administration')or hris_private.has_staff_capability(m.company_id,'private_hr_notes'))$new$,1),
 ('hris_private.performance_scope(uuid,uuid)',$old$m.role in ('administrator','hr_generalist')$old$,$new$(hris_private.has_staff_capability(m.company_id,'development_administration')or hris_private.has_staff_capability(m.company_id,'private_hr_notes'))$new$,1),
 ('hris_private.performance_review_read(uuid,uuid)',$old$m.role in ('administrator','hr_generalist')$old$,$new$(hris_private.has_staff_capability(m.company_id,'development_administration')or hris_private.has_staff_capability(m.company_id,'private_hr_notes'))$new$,1),
 ('hris_private.career_manage(uuid,uuid)',$old$m.role in('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'development_administration')$new$,1),
 ('hris_private.learning_scope(uuid,uuid)',$old$m.role in('administrator','hr_generalist')$old$,$new$(hris_private.has_staff_capability(m.company_id,'development_administration')or hris_private.has_staff_capability(m.company_id,'personnel_documents'))$new$,1),
 ('hris_private.performance_event_read(uuid,uuid)',$old$m.role in ('administrator','hr_generalist')$old$,$new$(hris_private.has_staff_capability(m.company_id,'private_hr_notes')or(v.note_id is null and hris_private.has_staff_capability(m.company_id,'development_administration')))$new$,1),
 ('hris_private.performance_reviewer_valid(uuid,uuid,uuid)',$old$m.role in ('administrator','hr_generalist')$old$,$new$('development_administration'=any(hris_private.staff_capabilities_for(p_company,m.user_id)))$new$,1)
,
 ('hris_private.manage_learning(text,jsonb)',$old$if p_operation='training_progress'then$old$,$new$if p_operation='training_progress'then if not hris_private.has_staff_capability(m.company_id,'development_administration')and m.employee_id<>t.employee_id and not(hris_private.has_staff_capability(m.company_id,'team_review')and exists(select 1 from public.employees progress_subject where progress_subject.company_id=m.company_id and progress_subject.id=t.employee_id and progress_subject.reports_to=m.employee_id))then raise exception 'Only the employee, current Manager or development staff record progress.'using errcode='42501';end if;$new$,1)
 )as reviewed(identity,needle,replacement,expected)loop
  select p.prosrc,pg_catalog.pg_get_functiondef(p.oid)into source,definition from pg_catalog.pg_proc p where p.oid=spec.identity::regprocedure;
  matches:=(length(source)-length(replace(source,spec.needle,'')))/length(spec.needle);
  if matches<>spec.expected then raise exception 'Development permission source changed: %',spec.identity;end if;
  execute replace(definition,spec.needle,spec.replacement);
 end loop;
end;$development_permissions$;
alter policy performance_hr_notes_read on public.performance_hr_notes using(hris_private.has_staff_capability(company_id,'private_hr_notes'));
alter policy certification_evidence_read on public.certification_evidence using(hris_private.has_staff_capability(company_id,'personnel_documents'));
alter policy performance_cycles_read on public.performance_cycles using(
 hris_private.has_staff_capability(company_id,'development_administration')or hris_private.has_staff_capability(company_id,'private_hr_notes')or exists(select 1 from public.performance_reviews r where r.cycle_id=performance_cycles.id and r.company_id=performance_cycles.company_id and hris_private.performance_review_read(r.company_id,r.id)));
alter policy learning_events_read on public.learning_events using(
 hris_private.has_staff_capability(company_id,'personnel_documents')or(evidence_id is null and(
 exists(select 1 from public.employee_training t where t.company_id=learning_events.company_id and t.id=learning_events.training_id and hris_private.learning_scope(t.company_id,t.employee_id))or
 exists(select 1 from public.employee_certifications c where c.company_id=learning_events.company_id and c.id=learning_events.certificate_id and hris_private.learning_scope(c.company_id,c.employee_id)))));
commit;
