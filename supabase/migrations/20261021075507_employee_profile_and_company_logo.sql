-- Employee is the person record; staff access adds a role to that same identity.
begin;

create function hris_private.can_edit_employee_contacts(p_company uuid,p_employee uuid) returns boolean
language sql stable security definer set search_path='' as $$
    select exists(select 1 from public.company_members m where m.user_id=(select auth.uid()) and m.active
        and m.company_id=p_company and (m.role in ('administrator','hr_generalist') or m.employee_id=p_employee));
$$;
revoke all on function hris_private.can_edit_employee_contacts(uuid,uuid) from public,anon,authenticated;
grant execute on function hris_private.can_edit_employee_contacts(uuid,uuid) to authenticated;
drop policy employee_contacts_create on public.employee_contacts;
drop policy employee_contacts_update on public.employee_contacts;
create policy employee_contacts_create on public.employee_contacts for insert to authenticated
    with check(hris_private.can_edit_employee_contacts(company_id,employee_id));
create policy employee_contacts_update on public.employee_contacts for update to authenticated
    using(hris_private.can_edit_employee_contacts(company_id,employee_id))
    with check(hris_private.can_edit_employee_contacts(company_id,employee_id));

-- Save employment, contacts and an existing/new staff role atomically, under the caller's RLS.
create function public.save_staff_account(p_user_id uuid,p_email text,p_role text,p_active boolean,p_employee jsonb,p_contacts jsonb) returns uuid
language plpgsql security invoker set search_path='' as $$
declare company uuid:=(p_employee->>'company_id')::uuid; person uuid; previous_person uuid;
begin
    if hris_private.company_role(company) is distinct from 'administrator' then
        raise exception 'Only administrators can manage staff roles.' using errcode='42501';
    end if;
    select employee_id into previous_person from public.company_members where user_id=p_user_id;
    if found and previous_person is distinct from (p_employee->>'id')::uuid then
        raise exception 'Edit the same employee record for this account.' using errcode='23514';
    end if;
    person:=public.save_employee_record(p_employee,p_contacts);
    perform public.save_company_account(p_user_id,p_email,p_role,p_active,person,null);
    return person;
end;
$$;
revoke all on function public.save_staff_account(uuid,text,text,boolean,jsonb,jsonb) from public,anon,authenticated;
grant execute on function public.save_staff_account(uuid,text,text,boolean,jsonb,jsonb) to authenticated;

alter table public.companies add column logo_version uuid;
grant update(logo_version) on public.companies to authenticated;
create function public.set_company_logo(p_present boolean) returns uuid
language plpgsql security invoker set search_path='' as $$
declare company uuid; revision uuid;
begin
    select company_id into company from public.company_members where user_id=(select auth.uid()) and active and role='administrator';
    if company is null or p_present is null then
        raise exception 'Only administrators can change the company logo.' using errcode='42501';
    end if;
    update public.companies set logo_version=case when p_present then gen_random_uuid() else null end where id=company returning logo_version into revision;
    return revision;
end;
$$;
revoke all on function public.set_company_logo(boolean) from public,anon,authenticated;
grant execute on function public.set_company_logo(boolean) to authenticated;

insert into storage.buckets(id,name,public,file_size_limit,allowed_mime_types)
    values('company-branding','company-branding',false,2097152,array['image/png']);
create policy company_logo_read on storage.objects for select to authenticated using(
    bucket_id='company-branding' and exists(select 1 from public.company_members m where m.user_id=(select auth.uid())
        and m.active and name=m.company_id::text||'/logo.png'));
create policy company_logo_create on storage.objects for insert to authenticated with check(
    bucket_id='company-branding' and exists(select 1 from public.company_members m where m.user_id=(select auth.uid())
        and m.active and m.role='administrator' and name=m.company_id::text||'/logo.png'));
create policy company_logo_update on storage.objects for update to authenticated using(
    bucket_id='company-branding' and exists(select 1 from public.company_members m where m.user_id=(select auth.uid())
        and m.active and m.role='administrator' and name=m.company_id::text||'/logo.png'))
    with check(bucket_id='company-branding' and exists(select 1 from public.company_members m where m.user_id=(select auth.uid())
        and m.active and m.role='administrator' and name=m.company_id::text||'/logo.png'));
create policy company_logo_delete on storage.objects for delete to authenticated using(
    bucket_id='company-branding' and exists(select 1 from public.company_members m where m.user_id=(select auth.uid())
        and m.active and m.role='administrator' and name=m.company_id::text||'/logo.png'));
-- Own Profile can show the supervisor name without exposing their employment record.
create or replace function hris_private.my_hris_access() returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare member public.company_members;
begin
    select * into member from public.company_members where user_id=(select auth.uid()) and active;
    if not found then raise exception 'Your HRIS access is disabled or has not been assigned. Contact your administrator.' using errcode='42501'; end if;
    return jsonb_build_object('company_id',member.company_id,'user_id',member.user_id,'role',member.role,
        'employee_id',member.employee_id,'reports_to_name',(select supervisor.full_name from public.employees person join public.employees supervisor on supervisor.id=person.reports_to and supervisor.company_id=person.company_id where person.id=member.employee_id and person.company_id=member.company_id),'direct_reports',coalesce((select jsonb_agg(e.id order by e.id) from public.employees e
            where member.role='manager' and e.company_id=member.company_id and e.reports_to=member.employee_id and e.status='active'),'[]'::jsonb));
end;
$$;

commit;
