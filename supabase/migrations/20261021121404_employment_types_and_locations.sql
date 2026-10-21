-- Company-owned choices; nullable assignments preserve existing employee records.
begin;
create table public.employment_types (
    id uuid primary key default gen_random_uuid(),
    company_id uuid not null references public.companies(id) on delete restrict,
    name text not null check (char_length(btrim(name)) between 1 and 80 and name !~ '[[:cntrl:]]'),
    active boolean not null default true,
    revision bigint not null default 0 check (revision>=0),
    unique(company_id,id)
);
create unique index employment_types_name_key on public.employment_types(company_id,lower(name));
create table public.work_locations (
    id uuid primary key default gen_random_uuid(),
    company_id uuid not null references public.companies(id) on delete restrict,
    name text not null check (char_length(btrim(name)) between 1 and 80 and name !~ '[[:cntrl:]]'),
    active boolean not null default true,
    revision bigint not null default 0 check (revision>=0),
    unique(company_id,id)
);
create unique index work_locations_name_key on public.work_locations(company_id,lower(name));
alter table public.employment_types enable row level security;
alter table public.work_locations enable row level security;
revoke all on public.employment_types,public.work_locations from public,anon,authenticated;
grant select on public.employment_types,public.work_locations to authenticated;
create policy employment_types_read on public.employment_types for select to authenticated
    using (hris_private.company_role(company_id) is not null);
create policy work_locations_read on public.work_locations for select to authenticated
    using (hris_private.company_role(company_id) is not null);

alter table public.employees
    add column employment_type_id uuid,
    add column work_location_id uuid,
    add foreign key(company_id,employment_type_id) references public.employment_types(company_id,id) on delete restrict,
    add foreign key(company_id,work_location_id) references public.work_locations(company_id,id) on delete restrict;
create index employees_employment_type_idx on public.employees(company_id,employment_type_id);
create index employees_work_location_idx on public.employees(company_id,work_location_id);
grant insert(employment_type_id,work_location_id),update(employment_type_id,work_location_id) on public.employees to authenticated;

-- Match employee validation's company lock to serialize choice deactivation/assignment.
create function hris_private.validate_employment_assignments() returns trigger
language plpgsql security invoker set search_path='' as $$
begin
    perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(new.company_id::text,7));
    if new.employment_type_id is not null then
        if tg_op='INSERT' or new.employment_type_id is distinct from old.employment_type_id then
            if not exists(select 1 from public.employment_types where company_id=new.company_id and id=new.employment_type_id and active) then
                raise exception 'Choose an active employment type. Existing inactive assignments can be retained.' using errcode='23514';
            end if;
        end if;
    end if;
    if new.work_location_id is not null then
        if tg_op='INSERT' or new.work_location_id is distinct from old.work_location_id then
            if not exists(select 1 from public.work_locations where company_id=new.company_id and id=new.work_location_id and active) then
                raise exception 'Choose an active work location. Existing inactive assignments can be retained.' using errcode='23514';
            end if;
        end if;
    end if;
    return new;
end;
$$;
revoke all on function hris_private.validate_employment_assignments() from public,anon,authenticated;
create trigger employees_employment_assignments before insert or update on public.employees
    for each row execute function hris_private.validate_employment_assignments();

create function hris_private.save_employment_option(p_kind text,p_option jsonb) returns uuid
language plpgsql security definer set search_path='' as $$
declare company uuid; target uuid; label text; available boolean; expected bigint; saved uuid;
begin
    select company_id into company from public.company_members where user_id=(select auth.uid()) and active and role='administrator';
    if company is null then raise exception 'Only administrators can manage employment choices.' using errcode='42501'; end if;
    perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(company::text,7));
    if hris_private.company_role(company) is distinct from 'administrator' then
        raise exception 'Your administrator access changed. Sign in again.' using errcode='42501';
    end if;
    if p_kind is null or p_kind not in ('employment_type','work_location') or jsonb_typeof(p_option) is distinct from 'object' then
        raise exception 'Choose a valid employment setting.' using errcode='23514';
    end if;
    if jsonb_typeof(p_option->'name') is distinct from 'string' or jsonb_typeof(p_option->'active') is distinct from 'boolean' then
        raise exception 'Enter a name and a valid availability setting.' using errcode='23514';
    end if;
    label:=btrim(p_option->>'name'); available:=(p_option->>'active')::boolean;
    target:=(p_option->>'id')::uuid; expected:=(p_option->>'expected_revision')::bigint;
    if label is null or char_length(label) not between 1 and 80 or label ~ '[[:cntrl:]]' or available is null then
        raise exception 'Enter a name of 1-80 characters and a valid availability setting.' using errcode='23514';
    end if;
    if target is null then
        if expected is not null then raise exception 'Refresh before adding a choice.' using errcode='23514'; end if;
        if p_kind='employment_type' then
            insert into public.employment_types(company_id,name,active) values(company,label,available) returning id into saved;
        else
            insert into public.work_locations(company_id,name,active) values(company,label,available) returning id into saved;
        end if;
    else
        if p_kind='employment_type' then
            update public.employment_types set name=label,active=available,revision=revision+1
                where company_id=company and id=target and revision=expected returning id into saved;
        else
            update public.work_locations set name=label,active=available,revision=revision+1
                where company_id=company and id=target and revision=expected returning id into saved;
        end if;
        if saved is null then raise sqlstate 'PT409' using message='This choice changed or is unavailable. Cancel, refresh and reopen it before saving.'; end if;
    end if;
    return saved;
end;
$$;
create function public.save_employment_option(p_kind text,p_option jsonb) returns uuid
language sql security invoker set search_path='' as $$ select hris_private.save_employment_option(p_kind,p_option); $$;
revoke all on function hris_private.save_employment_option(text,jsonb),public.save_employment_option(text,jsonb) from public,anon,authenticated;
grant execute on function hris_private.save_employment_option(text,jsonb),public.save_employment_option(text,jsonb) to authenticated;

-- Reusable starter choices are data, editable by each company's administrator.
create function hris_private.seed_employment_options() returns trigger
language plpgsql security invoker set search_path='' as $$
begin
    insert into public.employment_types(company_id,name)
        select new.id,label from (values('Full-time'),('Part-time'),('Contract'),('Internship')) as choices(label);
    insert into public.work_locations(company_id,name)
        select new.id,label from (values('Main office'),('Remote')) as choices(label);
    return new;
end;
$$;
revoke all on function hris_private.seed_employment_options() from public,anon,authenticated;
create trigger companies_seed_employment_options after insert on public.companies
    for each row execute function hris_private.seed_employment_options();
insert into public.employment_types(company_id,name)
    select c.id,choices.label from public.companies c cross join (values('Full-time'),('Part-time'),('Contract'),('Internship')) as choices(label);
insert into public.work_locations(company_id,name)
    select c.id,choices.label from public.companies c cross join (values('Main office'),('Remote')) as choices(label);

-- One transaction still saves employment, private contacts and (via save_staff_account) staff access.
-- Older clients omitting these keys retain assignments; explicit JSON null clears them.
create or replace function public.save_employee_record(p_employee jsonb,p_contacts jsonb) returns uuid
language plpgsql security invoker set search_path='' as $$
declare person uuid:=(p_employee->>'id')::uuid; company uuid:=(p_employee->>'company_id')::uuid;
begin
    if jsonb_typeof(p_employee) is distinct from 'object' or jsonb_typeof(p_contacts) is distinct from 'object' then
        raise exception 'Enter valid employee and contact details.' using errcode='23514';
    end if;
    if person is null then
        insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days,status,reports_to,employment_type_id,work_location_id)
        values(company,p_employee->>'employee_code',p_employee->>'full_name',coalesce(p_employee->>'department',''),
            coalesce(p_employee->>'job_title',''),nullif(p_employee->>'joined_on','')::date,(p_employee->>'annual_leave_days')::integer,
            p_employee->>'status',(p_employee->>'reports_to')::uuid,(p_employee->>'employment_type_id')::uuid,(p_employee->>'work_location_id')::uuid) returning id into person;
    else
        update public.employees set employee_code=p_employee->>'employee_code',full_name=p_employee->>'full_name',
            department=coalesce(p_employee->>'department',''),job_title=coalesce(p_employee->>'job_title',''),
            joined_on=nullif(p_employee->>'joined_on','')::date,annual_leave_days=(p_employee->>'annual_leave_days')::integer,
            status=p_employee->>'status',reports_to=(p_employee->>'reports_to')::uuid,
            employment_type_id=case when p_employee ? 'employment_type_id' then (p_employee->>'employment_type_id')::uuid else employment_type_id end,
            work_location_id=case when p_employee ? 'work_location_id' then (p_employee->>'work_location_id')::uuid else work_location_id end
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
commit;
