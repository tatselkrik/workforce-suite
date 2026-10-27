-- Recruiting company records and exact assigned interview/onboarding scopes.
-- Keep independent offer approval, atomic hire conversion, no Auth creation,
-- own feedback, applicant-file reservations and immutable history unchanged.
begin;
do $recruiting_permissions$
declare spec record;definition text;source text;matches integer;
begin
 for spec in select *from(values
 ('hris_private.manage_applicant(uuid,text,jsonb)',$old$hris_private.company_role(p_company_id) is null or hris_private.company_role(p_company_id) not in ('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(p_company_id,'recruiting')$new$,1),
 ('hris_private.manage_applicant(uuid,text,jsonb)',$old$role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(company_id,'recruiting')$new$,1),
 ('hris_private.record_applicant_document_event()',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'recruiting')$new$,1),
 ('hris_private.save_applicant_document(text,jsonb)',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'recruiting')$new$,1),
 ('hris_private.save_applicant_document(text,jsonb)',$old$role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(company_id,'recruiting')$new$,1),
 ('hris_private.can_read_interview(uuid,uuid)',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'recruiting')$new$,1),
 ('hris_private.can_read_interview(uuid,uuid)',$old$m.role='manager'$old$,$new$hris_private.has_staff_capability(m.company_id,'desktop')$new$,1),
 ('hris_private.can_read_onboarding_task(uuid,uuid)',$old$m.role in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'recruiting')$new$,1),
 ('hris_private.can_read_onboarding_task(uuid,uuid)',$old$m.role='manager'$old$,$new$hris_private.has_staff_capability(m.company_id,'desktop')$new$,1),
 ('hris_private.manage_interview(text,jsonb)',$old$cm.role in ('administrator','hr_generalist','manager')$old$,$new$('desktop'=any(hris_private.staff_capabilities_for(cm.company_id,cm.user_id)))$new$,1),
 ('hris_private.manage_interview(text,jsonb)',$old$role in ('administrator','hr_generalist','manager')$old$,$new$hris_private.has_staff_capability(company_id,'desktop')$new$,2),
 ('hris_private.manage_interview(text,jsonb)',$old$m.role not in ('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(m.company_id,'recruiting')$new$,1),
 ('hris_private.manage_hiring(text,jsonb)',$old$cm.role in ('administrator','hr_generalist','manager')$old$,$new$('desktop'=any(hris_private.staff_capabilities_for(cm.company_id,cm.user_id)))$new$,1),
 ('hris_private.manage_hiring(text,jsonb)',$old$role in ('administrator','hr_generalist','manager')$old$,$new$hris_private.has_staff_capability(company_id,'desktop')$new$,2),
 ('hris_private.manage_hiring(text,jsonb)',$old$m.role not in ('administrator','hr_generalist')$old$,$new$not hris_private.has_staff_capability(m.company_id,'recruiting')$new$,3),
 ('hris_private.manage_hiring(text,jsonb)',$old$m.role='manager'$old$,$new$not hris_private.has_staff_capability(m.company_id,'recruiting')$new$,2),
 ('hris_private.recruiting_interviewers()',$old$hris_private.company_role(m.company_id) in ('administrator','hr_generalist')$old$,$new$hris_private.has_staff_capability(m.company_id,'recruiting')$new$,1)
 )as reviewed(identity,needle,replacement,expected)loop
  select p.prosrc,pg_catalog.pg_get_functiondef(p.oid)into source,definition from pg_catalog.pg_proc p where p.oid=spec.identity::regprocedure;
  matches:=(length(source)-length(replace(source,spec.needle,'')))/length(spec.needle);
  if matches<>spec.expected then raise exception 'Recruiting permission source changed: %',spec.identity;end if;
  execute replace(definition,spec.needle,spec.replacement);
 end loop;
end;$recruiting_permissions$;
do $recruiting_policies$
declare spec record;expression text;needle text:=$needle$hris_private.company_role(company_id) = ANY (ARRAY['administrator'::text, 'hr_generalist'::text])$needle$;matches integer;
begin
 for spec in select *from(values
 ('applicant_document_events','applicant_document_events_read','qual',1),
 ('applicant_documents','applicant_documents_read','qual',1),
 ('applicant_events','applicant_events_hr_read','qual',1),
 ('applicants','applicants_hr_read','qual',1),
 ('hire_conversions','hire_conversions_hr_read','qual',1),
 ('hiring_offer_events','hiring_offer_events_hr_read','qual',1),
 ('hiring_offers','hiring_offers_hr_read','qual',1),
 ('interview_events','interview_events_read','qual',1),
 ('interview_feedback','interview_feedback_read','qual',1),
 ('onboarding_cases','onboarding_cases_hr_read','qual',1),
 ('onboarding_events','onboarding_events_hr_read','qual',1),
 ('onboarding_tasks','onboarding_tasks_hr_read','qual',1)
 )as reviewed(table_name,policy_name,expression_kind,expected)loop
  select case when spec.expression_kind='qual'then p.qual else p.with_check end into expression
   from pg_catalog.pg_policies p where p.schemaname='public'and p.tablename=spec.table_name and p.policyname=spec.policy_name;
  matches:=(length(expression)-length(replace(expression,needle,'')))/length(needle);
  if matches is distinct from spec.expected then raise exception 'Recruiting policy source changed: %.%',spec.table_name,spec.policy_name;end if;
  execute format('alter policy %I on public.%I %s (%s)',spec.policy_name,spec.table_name,
   case when spec.expression_kind='qual'then 'using'else 'with check'end,
   replace(expression,needle,'hris_private.has_staff_capability(company_id,''recruiting'')'));
 end loop;
end;$recruiting_policies$;
-- Audit columns retain the four-code membership compatibility contract.
-- These values are descriptive; protected capabilities govern every write.
alter table public.applicant_events drop constraint applicant_events_actor_role_check;
alter table public.applicant_events add constraint applicant_events_actor_role_check check(actor_role in('administrator','hr_generalist','manager','employee'));
alter table public.applicant_document_events drop constraint applicant_document_events_actor_role_check;
alter table public.applicant_document_events add constraint applicant_document_events_actor_role_check check(actor_role in('administrator','hr_generalist','manager','employee'));
alter table public.interview_feedback drop constraint interview_feedback_author_role_check;
alter table public.interview_feedback add constraint interview_feedback_author_role_check check(author_role in('administrator','hr_generalist','manager','employee'));
commit;
