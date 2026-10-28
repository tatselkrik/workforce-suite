-- Own-only portal notices stay distinct from capability-scoped native alerts.
-- Signed package/update verification and published financial facts are unchanged.
begin;
do $notification_permissions$
declare spec record;source text;matches integer;
begin
 for spec in select *from(values
 ('hris_private.lifecycle_notification_rows(uuid,uuid,text,date,timestamp with time zone)',$old$p_role in('administrator','hr_generalist')$old$,$new$p_role<>'employee'and 'development_administration'=any(hris_private.staff_capabilities_for(p_company,(select sm.user_id from public.company_members sm where sm.company_id=p_company and sm.employee_id=p_employee and sm.active)))$new$,2),
 ('hris_private.lifecycle_notification_rows(uuid,uuid,text,date,timestamp with time zone)',$old$p_role='manager'$old$,$new$p_role<>'employee'and 'team_review'=any(hris_private.staff_capabilities_for(p_company,(select sm.user_id from public.company_members sm where sm.company_id=p_company and sm.employee_id=p_employee and sm.active)))$new$,1),
 ('hris_private.lifecycle_notification_rows(uuid,uuid,text,date,timestamp with time zone)',$old$p_role in('administrator','hr_generalist','manager')$old$,$new$p_role<>'employee'and 'desktop'=any(hris_private.staff_capabilities_for(p_company,(select sm.user_id from public.company_members sm where sm.company_id=p_company and sm.employee_id=p_employee and sm.active)))$new$,1),
 ('hris_private.lifecycle_notifications(text)',$old$m.role not in('administrator','hr_generalist','manager')$old$,$new$not hris_private.has_staff_capability(m.company_id,'desktop')$new$,1),
 ('hris_private.lifecycle_notifications(text)',$old$case when p_surface='portal'then'employee'else m.role end$old$,$new$case when p_surface='portal'then'employee'else'desktop'end$new$,1),
 ('hris_private.lifecycle_notification_reads(text,text[],boolean,boolean)',$old$m.role not in('administrator','hr_generalist','manager')$old$,$new$not hris_private.has_staff_capability(m.company_id,'desktop')$new$,1),
 ('hris_private.lifecycle_notification_reads(text,text[],boolean,boolean)',$old$case when p_surface='portal'then'employee'else m.role end$old$,$new$case when p_surface='portal'then'employee'else'desktop'end$new$,1),
 ('hris_private.staff_notification_reads(uuid,text[],boolean,boolean)',$old$m.role not in('administrator','hr_generalist','manager')$old$,$new$not hris_private.has_staff_capability(m.company_id,'desktop')$new$,1),
 ('hris_private.set_notification_reads(uuid,text[],boolean,boolean)',$old$role in ('administrator','hr_generalist','manager')$old$,$new$hris_private.has_staff_capability(company_id,'desktop')$new$,1),
 ('hris_private.desktop_update()',$old$role in ('administrator','hr_generalist','manager')$old$,$new$hris_private.has_staff_capability(company_id,'desktop')$new$,1)
 )v(signature,needle,replacement,expected)
 loop
  source:=pg_get_functiondef(spec.signature::regprocedure);matches:=(length(source)-length(replace(source,spec.needle,'')))/length(spec.needle);
  if matches<>spec.expected then raise exception 'Unexpected notification predicate in %: expected %, found %',spec.signature,spec.expected,matches;end if;
  execute replace(source,spec.needle,spec.replacement);
 end loop;
end;$notification_permissions$;
alter policy hris_desktop_updates_download on storage.objects using(
 bucket_id='desktop-updates'and exists(select 1 from public.company_members m where m.user_id=(select auth.uid())and m.active and hris_private.has_staff_capability(m.company_id,'desktop')));
alter policy notification_read_state_own on public.notification_read_state using(
 user_id=(select auth.uid())and hris_private.has_staff_capability(company_id,'desktop'));
alter policy lifecycle_notification_state_own on public.lifecycle_notification_read_state using(
 user_id=(select auth.uid())and exists(select 1 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id
 where m.company_id=lifecycle_notification_read_state.company_id and m.user_id=(select auth.uid())and m.active and e.status='active'
 and(surface='portal'or hris_private.has_staff_capability(m.company_id,'desktop'))));
notify pgrst,'reload schema';
commit;
