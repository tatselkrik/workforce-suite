-- Keep Manager credit facts identical to the current employee-directory scope.
-- HR/Admin retain archived personnel history. Applied migration62 stays intact.
begin;
create or replace function hris_private.leave_credit_context()returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;result jsonb;today date;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active;
 if not found or not exists(select 1 from public.employees where id=m.employee_id and company_id=m.company_id and status='active')then raise exception 'Active employee membership is required.'using errcode='42501';end if;
 select(now()at time zone timezone)::date into strict today from public.companies where id=m.company_id;
 if(select count(*)from public.leave_credit_entries where company_id=m.company_id)>5000 then raise exception 'Credit history exceeds the supported review limit.'using errcode='54000';end if;
 with scoped as materialized(select c.*from public.leave_credit_entries c join public.employees e on e.company_id=c.company_id and e.id=c.employee_id
 where c.company_id=m.company_id and(m.role in('administrator','hr_generalist')or c.employee_id=m.employee_id or(m.role='manager'and e.reports_to=m.employee_id and e.status='active')))
 select jsonb_build_object('version','leave-credits-v1','company_id',m.company_id,'user_id',m.user_id,'employee_id',m.employee_id,'role',m.role,'as_of',today,
 'revision',coalesce((select revision from public.leave_credit_state where company_id=m.company_id),0),
 'entries',coalesce((select jsonb_agg(jsonb_build_object('id',id,'employee_id',employee_id,'type_code',type_code,'effective_on',effective_on,'units',units,'reverses_id',reverses_id)order by company_revision)from scoped),'[]'),
 'audit',case when m.role in('administrator','hr_generalist')then coalesce((select jsonb_agg(jsonb_build_object('id',id,'reason',reason,'reference',reference,'created_by_name',created_by_name,'posted_at',posted_at,'company_revision',company_revision)order by company_revision)from scoped),'[]')else'[]'::jsonb end)into result;
 return result;
end;$$;
commit;
