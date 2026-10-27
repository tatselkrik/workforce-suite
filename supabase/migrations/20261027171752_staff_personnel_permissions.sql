-- Personnel, private contacts/201 files and assigned clearance permissions.
-- Runtime gate stays FALSE. No employee/history/file fact is rewritten.
begin;
do $personnel_permissions$
declare spec record;definition text;source text;matches integer;
begin
 for spec in select *from(values
 ('hris_private.can_edit_employee_contacts(uuid,uuid)',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'personnel')$new$,1),
 ('hris_private.can_read_employee(uuid,uuid)',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'personnel')$new$,1),
 ('hris_private.can_record_employee(uuid,uuid)',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'attendance_administration')$new$,1),
 ('hris_private.can_review_employee(uuid,uuid)',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'leave_administration')$new$,1),
 ('hris_private.record_employee_document_event()',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'personnel_documents')$new$,1),
 ('hris_private.save_employee_document(text,jsonb)',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'personnel_documents')$new$,1),
 ('hris_private.can_read_employee(uuid,uuid)',$old$m.role='manager'$old$,$new$hris_private.has_staff_capability(m.company_id,'team_review')$new$,1),
 ('hris_private.can_review_employee(uuid,uuid)',$old$m.role='manager'$old$,$new$hris_private.has_staff_capability(m.company_id,'team_review')$new$,1),
 ('hris_private.employee_has_account(uuid,uuid)',$old$hris_private.company_role(p_company) in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(p_company,'personnel')$new$,1),
 ('hris_private.minimum_annual_allowance(uuid,integer)',$old$coalesce(hris_private.company_role(company),'')not in('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(company,'leave_administration')$new$,1),
 ('hris_private.minimum_annual_allowance_before_dated(uuid,integer)',$old$coalesce(hris_private.company_role(company),'') not in ('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(company,'leave_administration')$new$,1),
 ('hris_private.record_employment_history()',$old$m.role in ('administrator','hr_generalist')$old$,$new$(hris_private.has_staff_capability(m.company_id,'personnel')or hris_private.has_staff_capability(m.company_id,'recruiting'))$new$,1),
 ('hris_private.import_employees(uuid,jsonb,text)',$old$role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(company_id,'personnel')$new$,2),
 ('hris_private.exit_readiness(uuid)',$old$role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(company_id,'personnel')$new$,1),
 ('exit_readiness_list()',$old$role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(company_id,'personnel')$new$,1),
 ('hris_private.manage_exit(text,jsonb)',$old$cm.role in ('administrator','hr_generalist','manager')$old$,$new$('desktop'=any(hris_private.staff_capabilities_for(cm.company_id,cm.user_id)))$new$,1),
 ('hris_private.manage_exit(text,jsonb)',$old$role in ('administrator','hr_generalist','manager')$old$,$new$hris_private.has_staff_capability(company_id,'desktop')$new$,2),
 ('hris_private.manage_exit(text,jsonb)',$old$m.role not in ('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(m.company_id,'personnel')$new$,3),
 ('hris_private.manage_exit(text,jsonb)',$old$m.role='manager'$old$,$new$not hris_private.has_staff_capability(m.company_id,'personnel')$new$,2),
 ('hris_private.can_read_clearance_task(uuid,uuid)',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'personnel')$new$,1),
 ('hris_private.can_read_clearance_task(uuid,uuid)',$old$m.role='manager'$old$,$new$hris_private.has_staff_capability(m.company_id,'desktop')$new$,1)
,
 ('hris_private.can_edit_employee_contacts(uuid,uuid)',$old$select exists($old$,$new$select hris_private.has_staff_capability(p_company,'own_records')and exists($new$,1),
 ('hris_private.can_read_employee(uuid,uuid)',$old$select exists($old$,$new$select hris_private.has_staff_capability(p_company,'own_records')and exists($new$,1),
 ('hris_private.can_record_employee(uuid,uuid)',$old$select exists($old$,$new$select hris_private.has_staff_capability(p_company,'own_records')and exists($new$,1),
 ('hris_private.can_review_employee(uuid,uuid)',$old$select exists($old$,$new$select hris_private.has_staff_capability(p_company,'own_records')and exists($new$,1)
 )as reviewed(identity,needle,replacement,expected)loop
  select p.prosrc,pg_catalog.pg_get_functiondef(p.oid)into source,definition from pg_catalog.pg_proc p where p.oid=spec.identity::regprocedure;
  matches:=(length(source)-length(replace(source,spec.needle,'')))/length(spec.needle);
  if matches<>spec.expected then raise exception 'Personnel permission source changed: %',spec.identity;end if;
  execute replace(definition,spec.needle,spec.replacement);
 end loop;
end;$personnel_permissions$;
alter policy employees_create on public.employees with check(hris_private.has_staff_capability(company_id,'personnel'));
alter policy employees_update on public.employees using(hris_private.has_staff_capability(company_id,'personnel'))with check(hris_private.has_staff_capability(company_id,'personnel'));
-- Purpose-specific read-only employment directory. Do NOT broaden can_read_employee:
-- that helper also protects attendance, leave, shifts and other record scopes.
-- Contact, emergency, personnel-file, private-note and financial tables are separate.
alter policy employees_read on public.employees using(hris_private.can_read_employee(company_id,id)
 or hris_private.has_staff_capability(company_id,'recruiting')
 or hris_private.has_staff_capability(company_id,'development_administration')
 or hris_private.has_staff_capability(company_id,'payroll_preparation'));
alter policy employee_contacts_read on public.employee_contacts using(hris_private.can_edit_employee_contacts(company_id,employee_id));
alter policy employee_documents_read on public.employee_documents using(hris_private.has_staff_capability(company_id,'personnel_documents'));
alter policy employee_document_events_read on public.employee_document_events using(hris_private.has_staff_capability(company_id,'personnel_documents'));
alter policy employment_history_read on public.employment_history using(hris_private.has_staff_capability(company_id,'personnel'));
alter policy employee_import_batches_read on public.employee_import_batches using(hris_private.has_staff_capability(company_id,'personnel'));
alter policy employee_import_items_read on public.employee_import_items using(hris_private.has_staff_capability(company_id,'personnel'));
alter policy exit_cases_hr_read on public.exit_cases using(hris_private.has_staff_capability(company_id,'personnel'));
alter policy exit_events_scoped_read on public.exit_events using(hris_private.has_staff_capability(company_id,'personnel')or(task_id is not null and hris_private.can_read_clearance_task(company_id,task_id)));
-- Audit columns retain the four-code membership compatibility contract.
-- These values are descriptive; protected capabilities govern every write.
alter table public.employment_history drop constraint employment_history_actor_role_check;
alter table public.employment_history add constraint employment_history_actor_role_check check(actor_role in('administrator','hr_generalist','manager','employee'));
alter table public.employee_document_events drop constraint employee_document_events_actor_role_check;
alter table public.employee_document_events add constraint employee_document_events_actor_role_check check(actor_role in('administrator','hr_generalist','manager','employee'));
alter table public.employee_import_batches drop constraint employee_import_batches_actor_role_check;
alter table public.employee_import_batches add constraint employee_import_batches_actor_role_check check(actor_role in('administrator','hr_generalist','manager','employee'));
commit;
