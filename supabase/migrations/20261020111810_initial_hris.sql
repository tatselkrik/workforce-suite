-- Dedicated HRIS project only. No secret/service-role key is used by the desktop app.
begin;

create schema if not exists extensions;
create extension if not exists btree_gist with schema extensions;
set local search_path = public, extensions, pg_catalog;

create schema if not exists hris_private;
revoke all on schema hris_private from public, anon, authenticated;
grant usage on schema hris_private to authenticated;

create table public.companies (
    id uuid primary key default gen_random_uuid(),
    owner_id uuid not null unique references auth.users(id) on delete restrict,
    name text not null check (char_length(btrim(name)) between 1 and 100),
    timezone text not null default 'Asia/Manila' check (char_length(timezone) between 1 and 80),
    annual_leave_days integer not null default 15 check (annual_leave_days between 0 and 366),
    created_at timestamptz not null default now()
);

create table public.employees (
    id uuid primary key default gen_random_uuid(),
    company_id uuid not null references public.companies(id) on delete restrict,
    employee_code text not null check (char_length(btrim(employee_code)) between 1 and 30),
    full_name text not null check (char_length(btrim(full_name)) between 1 and 120),
    department text not null check (char_length(btrim(department)) between 1 and 80),
    job_title text not null check (char_length(btrim(job_title)) between 1 and 100),
    joined_on date not null check (joined_on between date '1900-01-01' and date '2200-12-31'),
    annual_leave_days integer not null check (annual_leave_days between 0 and 366),
    status text not null default 'active' check (status in ('active','archived')),
    created_at timestamptz not null default now(),
    unique (company_id,id)
);
create unique index employees_company_code_key on public.employees (company_id,lower(employee_code));

create table public.attendance (
    id uuid primary key default gen_random_uuid(),
    company_id uuid not null references public.companies(id) on delete restrict,
    employee_id uuid not null,
    work_date date not null default current_date,
    checked_in_at timestamptz not null default now(),
    checked_out_at timestamptz,
    foreign key (company_id,employee_id) references public.employees(company_id,id) on delete restrict,
    unique (company_id,employee_id,work_date),
    check (checked_out_at is null or checked_out_at >= checked_in_at)
);
create unique index attendance_one_open_shift on public.attendance (company_id,employee_id) where checked_out_at is null;
create index attendance_company_date_idx on public.attendance (company_id,work_date desc);

create table public.leave_requests (
    id uuid primary key default gen_random_uuid(),
    company_id uuid not null references public.companies(id) on delete restrict,
    employee_id uuid not null,
    kind text not null check (kind in ('annual','sick','unpaid')),
    starts_on date not null check (starts_on between date '1900-01-01' and date '2200-12-31'),
    ends_on date not null check (ends_on between date '1900-01-01' and date '2200-12-31'),
    reason text not null default '' check (char_length(reason) <= 500),
    status text not null default 'pending' check (status in ('pending','approved','rejected')),
    created_at timestamptz not null default now(),
    foreign key (company_id,employee_id) references public.employees(company_id,id) on delete restrict,
    check (ends_on >= starts_on and extract(year from starts_on) = extract(year from ends_on)),
    exclude using gist (employee_id with =, daterange(starts_on,ends_on,'[]') with &&)
        where (status in ('pending','approved'))
);
create index leave_company_employee_idx on public.leave_requests (company_id,employee_id);
create index leave_company_status_idx on public.leave_requests (company_id,status);

-- Helpers are private, SECURITY INVOKER, and unavailable as Data API RPCs.
create function hris_private.workdays(first_day date,last_day date) returns integer
language sql immutable strict security invoker set search_path = '' as $$
    select count(*)::integer from pg_catalog.generate_series(0,last_day-first_day) as offset_day
    where extract(isodow from (first_day+offset_day)) <= 5;
$$;

create function hris_private.validate_company() returns trigger
language plpgsql security invoker set search_path = '' as $$
begin
    if not exists (select 1 from pg_catalog.pg_timezone_names where name = new.timezone) then
        raise exception 'Use a valid IANA time zone.' using errcode = '23514';
    end if;
    new.name := btrim(new.name);
    return new;
end;
$$;
create trigger companies_validate before insert or update on public.companies
for each row execute function hris_private.validate_company();

create function hris_private.validate_employee() returns trigger
language plpgsql security invoker set search_path = '' as $$
declare used_days integer; company_zone text;
begin
    new.employee_code := upper(btrim(new.employee_code));
    new.full_name := btrim(new.full_name);
    new.department := btrim(new.department);
    new.job_title := btrim(new.job_title);
    if tg_op = 'UPDATE' then
        select timezone into company_zone from public.companies where id = old.company_id;
        select coalesce(sum(hris_private.workdays(r.starts_on,r.ends_on)),0)::integer into used_days
        from public.leave_requests r
        where r.employee_id = old.id and r.kind = 'annual' and r.status = 'approved'
          and extract(year from r.starts_on) = extract(year from (now() at time zone company_zone));
        if new.annual_leave_days < used_days then
            raise exception 'Allowance cannot be less than approved annual leave this year.' using errcode = '23514';
        end if;
    end if;
    return new;
end;
$$;
create trigger employees_validate before insert or update on public.employees
for each row execute function hris_private.validate_employee();

create function hris_private.record_attendance() returns trigger
language plpgsql security invoker set search_path = '' as $$
declare company_zone text;
begin
    if tg_op = 'INSERT' then
        if not exists (select 1 from public.employees e where e.id = new.employee_id and e.company_id = new.company_id and e.status = 'active') then
            raise exception 'Choose an active employee in your company.' using errcode = '23514';
        end if;
        select timezone into company_zone from public.companies where id = new.company_id;
        new.checked_in_at := clock_timestamp();
        new.work_date := (new.checked_in_at at time zone company_zone)::date;
        new.checked_out_at := null;
    else
        if old.checked_out_at is not null or new.checked_out_at is null then
            raise exception 'Only an open attendance record can be checked out.' using errcode = '23514';
        end if;
        new.checked_out_at := clock_timestamp();
    end if;
    return new;
end;
$$;
create trigger attendance_record before insert or update on public.attendance
for each row execute function hris_private.record_attendance();

create function hris_private.validate_leave() returns trigger
language plpgsql security invoker set search_path = '' as $$
declare allowance integer; employee_state text; used_days integer;
begin
    -- Serialize approvals and allowance changes for an employee.
    select annual_leave_days,status into allowance,employee_state from public.employees
    where id = new.employee_id and company_id = new.company_id for update;
    if not found then
        raise exception 'Choose an employee in your company.' using errcode = '23514';
    end if;
    if tg_op = 'INSERT' then
        new.status := 'pending';
        if employee_state <> 'active' then
            raise exception 'Archived employees cannot receive new leave requests.' using errcode = '23514';
        end if;
    elsif old.status <> 'pending' or new.status not in ('approved','rejected') then
        raise exception 'Only pending requests can be approved or rejected.' using errcode = '23514';
    end if;
    if hris_private.workdays(new.starts_on,new.ends_on) = 0 then
        raise exception 'Choose at least one Monday-Friday workday.' using errcode = '23514';
    end if;
    if new.status = 'approved' then
        if employee_state <> 'active' then
            raise exception 'Archived employees cannot have new leave approved.' using errcode = '23514';
        end if;
        if new.kind = 'annual' then
            select coalesce(sum(hris_private.workdays(r.starts_on,r.ends_on)),0)::integer into used_days
            from public.leave_requests r where r.employee_id = new.employee_id and r.id <> new.id
              and r.kind = 'annual' and r.status = 'approved'
              and extract(year from r.starts_on) = extract(year from new.starts_on);
            if used_days + hris_private.workdays(new.starts_on,new.ends_on) > allowance then
                raise exception 'This request exceeds the annual leave allowance.' using errcode = '23514';
            end if;
        end if;
    end if;
    return new;
end;
$$;
create trigger leave_validate before insert or update on public.leave_requests
for each row execute function hris_private.validate_leave();

-- Trigger functions still run as the calling user. Only their private helper is
-- executable within that user's trigger context. This schema is not exposed by
-- the Data API; its trigger functions have no direct EXECUTE grant.
revoke all on all functions in schema hris_private from public, anon, authenticated;
grant execute on function hris_private.workdays(date,date) to authenticated;

alter table public.companies enable row level security;
alter table public.employees enable row level security;
alter table public.attendance enable row level security;
alter table public.leave_requests enable row level security;

revoke all on public.companies,public.employees,public.attendance,public.leave_requests from public,anon,authenticated;
grant select on public.companies,public.employees,public.attendance,public.leave_requests to authenticated;
grant insert (owner_id,name,timezone,annual_leave_days) on public.companies to authenticated;
grant update (name,timezone,annual_leave_days) on public.companies to authenticated;
grant insert (company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days,status) on public.employees to authenticated;
grant update (employee_code,full_name,department,job_title,joined_on,annual_leave_days,status) on public.employees to authenticated;
grant insert (company_id,employee_id) on public.attendance to authenticated;
grant update (checked_out_at) on public.attendance to authenticated;
grant insert (company_id,employee_id,kind,starts_on,ends_on,reason) on public.leave_requests to authenticated;
grant update (status) on public.leave_requests to authenticated;

create policy companies_read on public.companies for select to authenticated
using (owner_id = (select auth.uid()));
create policy companies_create on public.companies for insert to authenticated
with check (owner_id = (select auth.uid()));
create policy companies_update on public.companies for update to authenticated
using (owner_id = (select auth.uid())) with check (owner_id = (select auth.uid()));

create policy employees_read on public.employees for select to authenticated
using (company_id in (select id from public.companies where owner_id = (select auth.uid())));
create policy employees_create on public.employees for insert to authenticated
with check (company_id in (select id from public.companies where owner_id = (select auth.uid())));
create policy employees_update on public.employees for update to authenticated
using (company_id in (select id from public.companies where owner_id = (select auth.uid())))
with check (company_id in (select id from public.companies where owner_id = (select auth.uid())));

create policy attendance_read on public.attendance for select to authenticated
using (company_id in (select id from public.companies where owner_id = (select auth.uid())));
create policy attendance_create on public.attendance for insert to authenticated
with check (company_id in (select id from public.companies where owner_id = (select auth.uid())));
create policy attendance_update on public.attendance for update to authenticated
using (company_id in (select id from public.companies where owner_id = (select auth.uid())))
with check (company_id in (select id from public.companies where owner_id = (select auth.uid())));

create policy leave_read on public.leave_requests for select to authenticated
using (company_id in (select id from public.companies where owner_id = (select auth.uid())));
create policy leave_create on public.leave_requests for insert to authenticated
with check (company_id in (select id from public.companies where owner_id = (select auth.uid())));
create policy leave_update on public.leave_requests for update to authenticated
using (company_id in (select id from public.companies where owner_id = (select auth.uid())))
with check (company_id in (select id from public.companies where owner_id = (select auth.uid())));

-- No delete grants or policies: archive employee records and retain history.
commit;
