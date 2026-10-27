-- Operational capabilities only; preserve charges, snapshots, independent
-- review/creator exclusions, reservations, correction reasons and issued-pay locks.
begin;
do $operations_permissions$
declare spec record;definition text;source text;matches integer;
begin
 for spec in select *from(values
 ('hris_private.assign_shift(jsonb)',$old$role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(company_id,'attendance_administration')$new$,1),
 ('hris_private.assign_shift(jsonb)',$old$hris_private.company_role(member.company_id) not in ('administrator','hr_generalist') or hris_private.company_role(member.company_id) is null$old$,$new$not hris_private.has_staff_capability(member.company_id,'attendance_administration')$new$,1),
 ('hris_private.save_shift_template(jsonb)',$old$role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(company_id,'attendance_administration')$new$,1),
 ('hris_private.save_shift_template(jsonb)',$old$hris_private.company_role(member.company_id) not in ('administrator','hr_generalist') or hris_private.company_role(member.company_id) is null$old$,$new$not hris_private.has_staff_capability(member.company_id,'attendance_administration')$new$,1),
 ('hris_private.apply_attendance_calculation_policy(jsonb)',$old$role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(company_id,'attendance_administration')$new$,1),
 ('hris_private.apply_attendance_calculation_policy(jsonb)',$old$hris_private.company_role(member.company_id) is null or hris_private.company_role(member.company_id) not in ('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(member.company_id,'attendance_administration')$new$,1),
 ('hris_private.manage_attendance_break(text,jsonb)',$old$hris_private.company_role(member.company_id) not in ('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(member.company_id,'attendance_administration')$new$,1),
 ('hris_private.correct_attendance(jsonb)',$old$role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(company_id,'attendance_administration')$new$,1),
 ('hris_private.correct_attendance(jsonb)',$old$role='manager'$old$,$new$hris_private.has_staff_capability(company_id,'team_review')$new$,1),
 ('hris_private.record_attendance_calculation()',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'attendance_administration')$new$,1),
 ('hris_private.record_attendance_calculation()',$old$m.role='manager'$old$,$new$hris_private.has_staff_capability(m.company_id,'team_review')$new$,1),
 ('hris_private.attendance_import_member()',$old$role in('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(company_id,'attendance_administration')$new$,1),
 ('hris_private.leave_import_member()',$old$role in('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(company_id,'leave_administration')$new$,1),
 ('hris_private.cancel_leave(uuid,bigint,text)',$old$member.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(member.company_id,'leave_administration')$new$,2),
 ('hris_private.cancel_leave(uuid,bigint,text)',$old$member.role not in ('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(member.company_id,'leave_administration')$new$,1),
 ('hris_private.preview_leave_opening(jsonb)',$old$m.role not in('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(m.company_id,'leave_administration')$new$,1),
 ('hris_private.preview_leave_openings(jsonb)',$old$m.role not in('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(m.company_id,'leave_administration')$new$,1),
 ('hris_private.leave_credit_context()',$old$m.role in('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'leave_administration')$new$,2),
 ('hris_private.leave_credit_context()',$old$m.role='manager'$old$,$new$hris_private.has_staff_capability(m.company_id,'team_review')$new$,1),
 ('hris_private.leave_dated_context()',$old$m.role in('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'leave_administration')$new$,3),
 ('hris_private.leave_dated_context()',$old$m.role='manager'$old$,$new$hris_private.has_staff_capability(m.company_id,'team_review')$new$,2),
 ('hris_private.leave_dated_context()',$old$m.role='employee'$old$,$new$not hris_private.has_staff_capability(m.company_id,'desktop')$new$,1),
 ('hris_private.can_read_leave_receipt(uuid,jsonb)',$old$m.role<>'employee'$old$,$new$hris_private.has_staff_capability(m.company_id,'desktop')$new$,1),
 ('hris_private.approval_principal_allowed(uuid,text,uuid,uuid,jsonb,uuid)',$old$cm.role<>'employee'$old$,$new$('desktop'=any(hris_private.staff_capabilities_for(cm.company_id,cm.user_id)))$new$,1),
 ('hris_private.approval_principal_allowed(uuid,text,uuid,uuid,jsonb,uuid)',$old$when 'reviewer' then m.role in('administrator','hr_generalist')$old$,$new$when 'reviewer' then ('leave_administration'=any(hris_private.staff_capabilities_for(m.company_id,m.user_id)))$new$,1),
 ('hris_private.approval_principal_allowed(uuid,text,uuid,uuid,jsonb,uuid)',$old$when 'hr' then m.role in('administrator','hr_generalist')$old$,$new$when 'hr' then ('leave_administration'=any(hris_private.staff_capabilities_for(m.company_id,m.user_id)))$new$,1),
 ('hris_private.approval_principal_allowed(uuid,text,uuid,uuid,jsonb,uuid)',$old$m.role in('administrator','hr_generalist')$old$,$new$('attendance_administration'=any(hris_private.staff_capabilities_for(m.company_id,m.user_id)))$new$,1),
 ('hris_private.approval_principal_allowed(uuid,text,uuid,uuid,jsonb,uuid)',$old$m.role='manager'$old$,$new$('team_review'=any(hris_private.staff_capabilities_for(m.company_id,m.user_id)))$new$,2),
 ('hris_private.submit_leave(uuid,text,date,date,text,text)',$old$m.role<>'employee'$old$,$new$('desktop'=any(hris_private.staff_capabilities_for(m.company_id,m.user_id)))$new$,1),
 ('hris_private.submit_leave(uuid,text,date,date,text,text)',$old$m.role in('administrator','hr_generalist')$old$,$new$('leave_administration'=any(hris_private.staff_capabilities_for(m.company_id,m.user_id)))$new$,2),
 ('hris_private.submit_leave(uuid,text,date,date,text,text)',$old$m.role='manager'$old$,$new$('team_review'=any(hris_private.staff_capabilities_for(m.company_id,m.user_id)))$new$,1)
,
 ('hris_private.save_leave_policy_before_dated(jsonb,bigint)',$old$m.role<>'employee'$old$,$new$('desktop'=any(hris_private.staff_capabilities_for(m.company_id,m.user_id)))$new$,1),
 ('hris_private.validate_dated_policy(uuid,jsonb,jsonb)',$old$m.role<>'employee'$old$,$new$('desktop'=any(hris_private.staff_capabilities_for(m.company_id,m.user_id)))$new$,1),
 ('hris_private.approval_delegation_context()',$old$where cm.user_id=(select auth.uid())and cm.active and cm.role<>'employee'$old$,$new$where cm.user_id=(select auth.uid())and cm.active and hris_private.has_staff_capability(cm.company_id,'desktop')$new$,1),
 ('hris_private.approval_delegation_context()',$old$m.role in('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'leave_administration')$new$,4)
,
 ('hris_private.save_report_preferences(uuid,bigint,jsonb)',$old$role in ('administrator','hr_generalist','manager')$old$,$new$(hris_private.has_staff_capability(company_id,'general_hr_reports')or hris_private.has_staff_capability(company_id,'team_review'))$new$,1)
 )as reviewed(identity,needle,replacement,expected)loop
  select p.prosrc,pg_catalog.pg_get_functiondef(p.oid)into source,definition from pg_catalog.pg_proc p where p.oid=spec.identity::regprocedure;
  matches:=(length(source)-length(replace(source,spec.needle,'')))/length(spec.needle);
  if matches<>spec.expected then raise exception 'Operational permission source changed: %',spec.identity;end if;
  execute replace(definition,spec.needle,spec.replacement);
 end loop;
end;$operations_permissions$;
alter policy attendance_break_events_read on public.attendance_break_events using(hris_private.has_staff_capability(company_id,'attendance_administration'));
alter policy attendance_calculation_events_read on public.attendance_calculation_events using(hris_private.has_staff_capability(company_id,'attendance_administration'));
alter policy attendance_calculation_policy_events_read on public.attendance_calculation_policy_events using(hris_private.has_staff_capability(company_id,'attendance_administration'));
alter policy attendance_corrections_read on public.attendance_corrections using(hris_private.has_staff_capability(company_id,'attendance_administration'));
alter policy shift_versions_read on public.shift_template_versions using(hris_private.has_staff_capability(company_id,'attendance_administration'));
alter policy attendance_calculation_policies_read on public.attendance_calculation_policies using(hris_private.has_staff_capability(company_id,'desktop'));
alter policy shift_templates_read on public.shift_templates using(hris_private.has_staff_capability(company_id,'desktop'));
alter policy leave_credit_hr_history on public.leave_credit_entries using(hris_private.has_staff_capability(company_id,'leave_administration'));
alter table public.attendance_corrections drop constraint attendance_corrections_actor_role_check;
alter table public.attendance_corrections add constraint attendance_corrections_actor_role_check check(actor_role in('administrator','hr_generalist','manager','employee'));
alter table public.attendance_calculation_events drop constraint attendance_calculation_events_actor_role_check;
alter table public.attendance_calculation_events add constraint attendance_calculation_events_actor_role_check check(actor_role in('administrator','hr_generalist','manager','employee'));
alter table public.attendance_import_batches drop constraint attendance_import_batches_actor_role_check;
alter table public.attendance_import_batches add constraint attendance_import_batches_actor_role_check check(actor_role in('administrator','hr_generalist','manager','employee'));
alter table public.attendance_import_withdrawals drop constraint attendance_import_withdrawals_actor_role_check;
alter table public.attendance_import_withdrawals add constraint attendance_import_withdrawals_actor_role_check check(actor_role in('administrator','hr_generalist','manager','employee'));
alter table public.leave_import_batches drop constraint leave_import_batches_actor_role_check;
alter table public.leave_import_batches add constraint leave_import_batches_actor_role_check check(actor_role in('administrator','hr_generalist','manager','employee'));
alter policy report_preferences_read on public.report_preferences using(user_id=(select auth.uid())and(hris_private.has_staff_capability(company_id,'general_hr_reports')or hris_private.has_staff_capability(company_id,'team_review')));
commit;
