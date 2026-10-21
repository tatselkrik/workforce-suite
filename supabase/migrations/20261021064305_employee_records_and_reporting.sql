-- One employee identity per HRIS account; reporting relationships belong to employees.
begin;
alter table public.employees alter column joined_on drop not null;
alter table public.employees drop constraint employees_department_check;
alter table public.employees drop constraint employees_job_title_check;
alter table public.employees add constraint employees_department_check check(char_length(btrim(department))<=80);
alter table public.employees add constraint employees_job_title_check check(char_length(btrim(job_title))<=100);
alter table public.employees add column reports_to uuid;
alter table public.employees add constraint employees_reports_to_company_fk foreign key(company_id,reports_to)
    references public.employees(company_id,id) on delete restrict;
alter table public.employees add constraint employees_not_own_manager check(reports_to is distinct from id);
create index employees_reports_to_idx on public.employees(company_id,reports_to);
grant insert(reports_to),update(reports_to) on public.employees to authenticated;

-- This unexposed helper is executable only by trusted functions/triggers, never API callers.
create sequence hris_private.employee_codes;
revoke all on sequence hris_private.employee_codes from public,anon,authenticated;
create function hris_private.create_account_employee(p_company uuid,p_user uuid) returns uuid
language plpgsql security definer set search_path='' as $$
declare person uuid; employee_name text; allowance integer; sequence_value text; code text;
begin
    perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(p_company::text,7));
    select coalesce(nullif(btrim(raw_user_meta_data #>> '{hris_profile,full_name}'),''),
        nullif(btrim(raw_user_meta_data->>'full_name'),''),split_part(email,'@',1))
        into employee_name from auth.users where id=p_user;
    if employee_name is null or btrim(employee_name)='' then
        raise exception 'An account with an employee name is required.' using errcode='23514';
    end if;
    select annual_leave_days into strict allowance from public.companies where id=p_company;
    loop
        sequence_value:=nextval('hris_private.employee_codes')::text;
        code:='EMP-'||lpad(sequence_value,greatest(6,char_length(sequence_value)),'0');
        exit when not exists(select 1 from public.employees where company_id=p_company and lower(employee_code)=lower(code));
    end loop;
    insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days)
        values(p_company,code,left(btrim(employee_name),120),'','',null,allowance)
        returning id into person;
    return person;
end;
$$;
revoke all on function hris_private.create_account_employee(uuid,uuid) from public,anon,authenticated;

do $$ declare member record; begin
    for member in select company_id,user_id from public.company_members where employee_id is null loop
        update public.company_members set employee_id=hris_private.create_account_employee(member.company_id,member.user_id)
            where user_id=member.user_id;
    end loop;
end; $$;
-- Preserve existing reporting lines before retiring the account-level field.
update public.employees e set reports_to=manager.employee_id
    from public.company_members member join public.company_members manager on manager.user_id=member.manager_user_id
    where e.id=member.employee_id and e.company_id=member.company_id;
update public.company_members set manager_user_id=null where manager_user_id is not null;
alter table public.company_members alter column employee_id set not null;
drop index public.company_members_active_employee_key;
create unique index company_members_employee_identity_idx on public.company_members(company_id,employee_id);

create or replace function hris_private.seed_company_owner() returns trigger
language plpgsql security invoker set search_path='' as $$
begin
    insert into public.company_members(company_id,user_id,role,employee_id)
        values(new.id,new.owner_id,'administrator',hris_private.create_account_employee(new.id,new.owner_id));
    return new;
end;
$$;

create or replace function hris_private.can_read_employee(p_company uuid,p_employee uuid) returns boolean
language sql stable security definer set search_path='' as $$
    select exists(select 1 from public.company_members m where m.company_id=p_company and m.user_id=(select auth.uid()) and m.active
        and (m.role in ('administrator','hr_generalist') or m.employee_id=p_employee or (m.role='manager' and exists(
            select 1 from public.employees e where e.company_id=p_company and e.id=p_employee and e.reports_to=m.employee_id and e.status='active'))));
$$;
create or replace function hris_private.can_review_employee(p_company uuid,p_employee uuid) returns boolean
language sql stable security definer set search_path='' as $$
    select exists(select 1 from public.company_members m where m.company_id=p_company and m.user_id=(select auth.uid()) and m.active
        and m.employee_id is distinct from p_employee and (m.role in ('administrator','hr_generalist') or (m.role='manager' and exists(
            select 1 from public.employees e where e.company_id=p_company and e.id=p_employee and e.reports_to=m.employee_id and e.status='active'))));
$$;
create or replace function hris_private.my_hris_access() returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare member public.company_members;
begin
    select * into member from public.company_members where user_id=(select auth.uid()) and active;
    if not found then raise exception 'Your HRIS access is disabled or has not been assigned. Contact your administrator.' using errcode='42501'; end if;
    return jsonb_build_object('company_id',member.company_id,'user_id',member.user_id,'role',member.role,
        'employee_id',member.employee_id,'direct_reports',coalesce((select jsonb_agg(e.id order by e.id) from public.employees e
            where member.role='manager' and e.company_id=member.company_id and e.reports_to=member.employee_id and e.status='active'),'[]'::jsonb));
end;
$$;

create or replace function hris_private.validate_employee() returns trigger
language plpgsql security invoker set search_path='' as $$
declare used_days integer; company_zone text;
begin
    perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(new.company_id::text,7));
    if new.reports_to is not null then
        if new.reports_to=new.id or not exists(select 1 from public.employees where company_id=new.company_id and id=new.reports_to and status='active') then
            raise exception 'Choose another active employee as the reporting manager.' using errcode='23514';
        end if;
        if exists(with recursive chain as (
            select id,reports_to,array[id] as path from public.employees where id=new.reports_to and company_id=new.company_id
            union all select e.id,e.reports_to,c.path||e.id from public.employees e join chain c on c.reports_to=e.id
                where e.company_id=new.company_id and not e.id=any(c.path)
        ) select 1 from chain where id=new.id) then
            raise exception 'Reporting relationships cannot contain a cycle.' using errcode='23514';
        end if;
    end if;
    if new.status='archived' and exists(select 1 from public.employees where company_id=new.company_id and reports_to=new.id and status='active') then
        raise exception 'Reassign direct reports before archiving this employee.' using errcode='23514';
    end if;
    new.employee_code:=upper(btrim(new.employee_code)); new.full_name:=btrim(new.full_name);
    new.department:=btrim(new.department); new.job_title:=btrim(new.job_title);
    if tg_op='UPDATE' then
        perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(old.id::text,0));
        if new.status='archived' and hris_private.employee_has_account(old.company_id,old.id) then
            raise exception 'An active account is linked to this employee. Ask an administrator to deactivate or unlink it first.' using errcode='23514';
        end if;
        select timezone into company_zone from public.companies where id=old.company_id;
        select coalesce(sum(hris_private.workdays(r.starts_on,r.ends_on)),0)::integer into used_days
            from public.leave_requests r where r.employee_id=old.id and r.kind='annual' and r.status='approved'
              and extract(year from r.starts_on)=extract(year from(now() at time zone company_zone));
        if new.annual_leave_days<used_days then
            raise exception 'Allowance cannot be less than approved annual leave this year.' using errcode='23514';
        end if;
    end if;
    return new;
end;
$$;
create or replace function hris_private.save_company_account(p_user_id uuid,p_email text,p_role text,p_active boolean,
    p_employee_id uuid,p_manager_user_id uuid) returns uuid
language plpgsql security definer set search_path='' as $$
declare actor uuid := auth.uid(); company uuid; owner uuid; target uuid; target_email text;
    previous public.company_members; existing boolean; hierarchy_cycle boolean;
begin
    -- Lock the company to serialize access changes, manager validation and last-admin protection.
    select company_id into company from public.company_members where user_id=actor and active and role='administrator';
    if company is null then raise exception 'Only administrators can manage accounts.' using errcode='42501'; end if;
    perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(company::text,7));
    select owner_id into owner from public.companies where id=company for update;
    if not exists(select 1 from public.company_members where user_id=actor and company_id=company and active and role='administrator') then
        raise exception 'Your administrator access changed. Sign in again.' using errcode='42501';
    end if;
    if p_role is null or p_role not in ('administrator','hr_generalist','manager','employee') or p_active is null then
        raise exception 'Choose a valid role and access status.' using errcode='23514';
    end if;
    if p_user_id is null then
        select id,email into target,target_email from auth.users where lower(email)=lower(btrim(p_email));
    else
        select id,email into target,target_email from auth.users where id=p_user_id;
        if lower(target_email) is distinct from lower(btrim(p_email)) then
            raise exception 'The account email changed. Refresh before saving.' using errcode='23514';
        end if;
    end if;
    if target is null then raise exception 'No HRIS account exists for that email. Choose New account to create it.' using errcode='23514'; end if;
    select * into previous from public.company_members where user_id=target;
    existing := found;
    if p_user_id is null and existing then raise exception 'This account already has access. Edit it from the account list.' using errcode='23505'; end if;
    if existing and previous.company_id <> company then
        raise exception 'This account belongs to another company.' using errcode='42501';
    end if;
    if target=owner and (p_role<>'administrator' or not p_active) then
        raise exception 'The installation administrator must remain active with Administrator access.' using errcode='23514';
    end if;
    if target=actor and (p_role<>previous.role or p_active<>previous.active) then
        raise exception 'Another administrator must change your role or access status.' using errcode='23514';
    end if;
    if existing and previous.active and previous.role='administrator' and (not p_active or p_role<>'administrator')
      and not exists(select 1 from public.company_members where company_id=company and active
          and role='administrator' and user_id<>target) then
        raise exception 'Keep at least one active administrator.' using errcode='23514';
    end if;
    if p_manager_user_id is not null then
        raise exception 'Set Reports to in the employee record.' using errcode='23514';
    end if;
    if existing and p_employee_id is not null and p_employee_id<>previous.employee_id then
        raise exception 'This account already has an employee record. Edit it in Employees.' using errcode='23514';
    end if;
    if p_employee_id is null then
        p_employee_id:=case when existing then previous.employee_id else hris_private.create_account_employee(company,target) end;
    end if;
    if p_employee_id is not null then
        -- The employee archive trigger uses this same lock. An active link and archive cannot race.
        perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(p_employee_id::text,0));
    end if;
    if p_employee_id is not null and not exists(select 1 from public.employees
        where id=p_employee_id and company_id=company and (status='active' or not p_active)) then
        raise exception 'Choose an active employee in this company.' using errcode='23514';
    end if;
    if p_active and p_role='employee' and exists(select 1 from public.employees
        where company_id=company and reports_to=p_employee_id and status='active') then
        raise exception 'Reassign direct reports before changing this supervisor to Employee access.' using errcode='23514';
    end if;
    insert into public.company_members(company_id,user_id,role,active,employee_id,manager_user_id)
        values(company,target,p_role,p_active,p_employee_id,p_manager_user_id)
        on conflict(user_id) do update set role=excluded.role,active=excluded.active,employee_id=excluded.employee_id,
            manager_user_id=excluded.manager_user_id,updated_at=clock_timestamp();
    insert into public.account_access_events(company_id,actor_id,user_id,action,details)
        values(company,actor,target,case when existing then 'access_changed' else 'account_added' end,
            jsonb_build_object('old_role',previous.role,'role',p_role,'old_active',previous.active,'active',p_active,
                'employee_id',p_employee_id,'manager_user_id',p_manager_user_id));
    return target;
end;
$$;

-- Invoker/RLS authorization plus one transaction for employment, contacts and reporting.
create function public.save_employee_record(p_employee jsonb,p_contacts jsonb) returns uuid
language plpgsql security invoker set search_path='' as $$
declare person uuid:=(p_employee->>'id')::uuid; company uuid:=(p_employee->>'company_id')::uuid;
begin
    if jsonb_typeof(p_employee) is distinct from 'object' or jsonb_typeof(p_contacts) is distinct from 'object' then
        raise exception 'Enter valid employee and contact details.' using errcode='23514';
    end if;
    if person is null then
        insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days,status,reports_to)
        values(company,p_employee->>'employee_code',p_employee->>'full_name',coalesce(p_employee->>'department',''),
            coalesce(p_employee->>'job_title',''),nullif(p_employee->>'joined_on','')::date,(p_employee->>'annual_leave_days')::integer,
            p_employee->>'status',(p_employee->>'reports_to')::uuid) returning id into person;
    else
        update public.employees set employee_code=p_employee->>'employee_code',full_name=p_employee->>'full_name',
            department=coalesce(p_employee->>'department',''),job_title=coalesce(p_employee->>'job_title',''),
            joined_on=nullif(p_employee->>'joined_on','')::date,annual_leave_days=(p_employee->>'annual_leave_days')::integer,
            status=p_employee->>'status',reports_to=(p_employee->>'reports_to')::uuid
            where id=person and company_id=company returning id into person;
        if not found then raise exception 'Employee not found or your role cannot edit this record.' using errcode='42501'; end if;
    end if;
    insert into public.employee_contacts(company_id,employee_id,phone,personal_email,address,emergency_name,emergency_relationship,emergency_phone,emergency_alternate_phone)
        values(company,person,coalesce(p_contacts->>'phone',''),coalesce(p_contacts->>'personal_email',''),coalesce(p_contacts->>'address',''),
            coalesce(p_contacts->>'emergency_name',''),coalesce(p_contacts->>'emergency_relationship',''),coalesce(p_contacts->>'emergency_phone',''),coalesce(p_contacts->>'emergency_alternate_phone',''))
        on conflict(company_id,employee_id) do update set phone=excluded.phone,personal_email=excluded.personal_email,address=excluded.address,
            emergency_name=excluded.emergency_name,emergency_relationship=excluded.emergency_relationship,emergency_phone=excluded.emergency_phone,
            emergency_alternate_phone=excluded.emergency_alternate_phone;
    return person;
end;
$$;
revoke all on function public.save_employee_record(jsonb,jsonb) from public,anon,authenticated;
grant execute on function public.save_employee_record(jsonb,jsonb) to authenticated;
commit;
