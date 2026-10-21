-- Dedicated-company account access. Roles are protected database state, never JWT metadata.
begin;

create table public.company_members (
    company_id uuid not null references public.companies(id) on delete restrict,
    user_id uuid primary key references auth.users(id) on delete restrict,
    role text not null check (role in ('administrator','hr_generalist','manager','employee')),
    active boolean not null default true,
    employee_id uuid,
    manager_user_id uuid,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique(company_id,user_id),
    foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict,
    foreign key(company_id,manager_user_id) references public.company_members(company_id,user_id) on delete restrict,
    check (manager_user_id is distinct from user_id),
    check (not active or role not in ('manager','employee') or employee_id is not null)
);
create unique index company_members_active_employee_key on public.company_members(company_id,employee_id)
    where active and employee_id is not null;
create index company_members_manager_idx on public.company_members(company_id,manager_user_id,employee_id) where active;
create index company_members_manager_fk_idx on public.company_members(company_id,manager_user_id);
create index company_members_employee_fk_idx on public.company_members(company_id,employee_id);

create table public.account_access_events (
    id uuid primary key default gen_random_uuid(),
    company_id uuid not null references public.companies(id) on delete restrict,
    actor_id uuid not null references auth.users(id) on delete restrict,
    user_id uuid not null references auth.users(id) on delete restrict,
    action text not null check (action in ('account_added','access_changed')),
    details jsonb not null,
    created_at timestamptz not null default now()
);
create index account_access_events_company_date_idx on public.account_access_events(company_id,created_at desc);
create index account_access_events_actor_idx on public.account_access_events(actor_id);
create index account_access_events_user_idx on public.account_access_events(user_id);

alter table public.company_members enable row level security;
alter table public.account_access_events enable row level security;
revoke all on public.company_members,public.account_access_events from public,anon,authenticated;
grant select on public.company_members,public.account_access_events to authenticated;

-- Existing owner access is preserved. Future trusted company bootstrap also seeds its owner.
insert into public.company_members(company_id,user_id,role) select id,owner_id,'administrator' from public.companies;
create function hris_private.seed_company_owner() returns trigger language plpgsql security invoker set search_path='' as $$
begin
    insert into public.company_members(company_id,user_id,role) values(new.id,new.owner_id,'administrator');
    return new;
end;
$$;
create trigger companies_seed_owner after insert on public.companies
for each row execute function hris_private.seed_company_owner();

-- Narrow SECURITY DEFINER helpers read the protected membership table without recursive RLS.
-- They derive the actor exclusively from auth.uid(), use a fixed search_path and expose no arbitrary query.
create function hris_private.company_role(p_company uuid) returns text
language sql stable security definer set search_path='' as $$
    select m.role from public.company_members m
    where m.company_id=p_company and m.user_id=(select auth.uid()) and m.active;
$$;

create function hris_private.can_read_employee(p_company uuid,p_employee uuid) returns boolean
language sql stable security definer set search_path='' as $$
    select exists(select 1 from public.company_members m
        where m.company_id=p_company and m.user_id=(select auth.uid()) and m.active
        and (m.role in ('administrator','hr_generalist') or m.employee_id=p_employee
            or (m.role='manager' and exists(select 1 from public.company_members report
                where report.company_id=p_company and report.active and report.employee_id=p_employee
                  and report.manager_user_id=m.user_id))));
$$;

create function hris_private.can_record_employee(p_company uuid,p_employee uuid) returns boolean
language sql stable security definer set search_path='' as $$
    select exists(select 1 from public.company_members m
        where m.company_id=p_company and m.user_id=(select auth.uid()) and m.active
          and (m.role in ('administrator','hr_generalist') or m.employee_id=p_employee));
$$;

create function hris_private.can_review_employee(p_company uuid,p_employee uuid) returns boolean
language sql stable security definer set search_path='' as $$
    select exists(select 1 from public.company_members m
        where m.company_id=p_company and m.user_id=(select auth.uid()) and m.active
          and m.employee_id is distinct from p_employee
          and (m.role in ('administrator','hr_generalist') or (m.role='manager' and exists(
            select 1 from public.company_members report where report.company_id=p_company and report.active
              and report.employee_id=p_employee and report.manager_user_id=m.user_id))));
$$;

create function hris_private.employee_has_account(p_company uuid,p_employee uuid) returns boolean
language sql stable security definer set search_path='' as $$
    select hris_private.company_role(p_company) in ('administrator','hr_generalist') and exists(
        select 1 from public.company_members where company_id=p_company and employee_id=p_employee and active);
$$;

create function hris_private.my_hris_access() returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare member public.company_members;
begin
    select * into member from public.company_members where user_id=(select auth.uid()) and active;
    if not found then
        raise exception 'Your HRIS access is disabled or has not been assigned. Contact your administrator.' using errcode='42501';
    end if;
    return jsonb_build_object('company_id',member.company_id,'user_id',member.user_id,'role',member.role,
        'employee_id',member.employee_id,'direct_reports',coalesce((select jsonb_agg(r.employee_id order by r.employee_id)
          from public.company_members r where member.role='manager' and r.company_id=member.company_id
            and r.active and r.employee_id is not null and r.manager_user_id=member.user_id),'[]'::jsonb));
end;
$$;
create function public.my_hris_access() returns jsonb
language sql stable security invoker set search_path='' as $$ select hris_private.my_hris_access(); $$;

create function hris_private.list_company_accounts() returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare company uuid;
begin
    select company_id into company from public.company_members
        where user_id=(select auth.uid()) and active and role='administrator';
    if company is null then raise exception 'Only administrators can manage accounts.' using errcode='42501'; end if;
    return coalesce((select jsonb_agg(jsonb_build_object('user_id',m.user_id,'email',u.email,'role',m.role,
        'active',m.active,'employee_id',m.employee_id,'manager_user_id',m.manager_user_id,
        'email_confirmed',u.email_confirmed_at is not null,'is_owner',c.owner_id=m.user_id) order by lower(u.email))
        from public.company_members m join auth.users u on u.id=m.user_id join public.companies c on c.id=m.company_id
        where m.company_id=company),'[]'::jsonb);
end;
$$;
create function public.list_company_accounts() returns jsonb
language sql stable security invoker set search_path='' as $$ select hris_private.list_company_accounts(); $$;

create function hris_private.save_company_account(p_user_id uuid,p_email text,p_role text,p_active boolean,
    p_employee_id uuid,p_manager_user_id uuid) returns uuid
language plpgsql security definer set search_path='' as $$
declare actor uuid := auth.uid(); company uuid; owner uuid; target uuid; target_email text;
    previous public.company_members; existing boolean; hierarchy_cycle boolean;
begin
    -- Lock the company to serialize access changes, manager validation and last-admin protection.
    select company_id into company from public.company_members where user_id=actor and active and role='administrator';
    if company is null then raise exception 'Only administrators can manage accounts.' using errcode='42501'; end if;
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
    if p_active and p_role in ('manager','employee') and p_employee_id is null then
        raise exception 'Link an employee record for this role.' using errcode='23514';
    end if;
    if p_employee_id is not null then
        -- The employee archive trigger uses this same lock. An active link and archive cannot race.
        perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(p_employee_id::text,0));
    end if;
    if p_employee_id is not null and not exists(select 1 from public.employees
        where id=p_employee_id and company_id=company and (status='active' or not p_active)) then
        raise exception 'Choose an active employee in this company.' using errcode='23514';
    end if;
    if p_manager_user_id is not null then
        if p_manager_user_id=target or not exists(select 1 from public.company_members where user_id=p_manager_user_id
            and company_id=company and active and role in ('administrator','hr_generalist','manager')) then
            raise exception 'Choose another active Administrator, HR Generalist or Manager.' using errcode='23514';
        end if;
        with recursive chain(user_id,manager_user_id,path) as (
            select user_id,manager_user_id,array[user_id] from public.company_members where user_id=p_manager_user_id
            union all select m.user_id,m.manager_user_id,chain.path||m.user_id
            from public.company_members m join chain on chain.manager_user_id=m.user_id
            where not m.user_id=any(chain.path)
        ) select exists(select 1 from chain where user_id=target) into hierarchy_cycle;
        if hierarchy_cycle then raise exception 'Reporting relationships cannot contain a cycle.' using errcode='23514'; end if;
    end if;
    if p_active and p_role='employee' and exists(select 1 from public.company_members
        where manager_user_id=target and active) then
        raise exception 'Reassign this manager''s direct reports before changing the role to Employee.' using errcode='23514';
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
create function public.save_company_account(p_user_id uuid,p_email text,p_role text,p_active boolean,
    p_employee_id uuid,p_manager_user_id uuid) returns uuid
language sql security invoker set search_path='' as $$
    select hris_private.save_company_account(p_user_id,p_email,p_role,p_active,p_employee_id,p_manager_user_id);
$$;

-- No direct client INSERT/UPDATE/DELETE exists for memberships or access history.
create policy members_read on public.company_members for select to authenticated
using (user_id=(select auth.uid()) or hris_private.company_role(company_id)='administrator');
create policy account_events_read on public.account_access_events for select to authenticated
using (hris_private.company_role(company_id)='administrator');

alter policy companies_read on public.companies using (hris_private.company_role(id) is not null);
alter policy companies_update on public.companies using (hris_private.company_role(id)='administrator')
    with check (hris_private.company_role(id)='administrator');
alter policy employees_read on public.employees using (hris_private.can_read_employee(company_id,id));
alter policy employees_create on public.employees with check (hris_private.company_role(company_id) in ('administrator','hr_generalist'));
alter policy employees_update on public.employees using (hris_private.company_role(company_id) in ('administrator','hr_generalist'))
    with check (hris_private.company_role(company_id) in ('administrator','hr_generalist'));
alter policy attendance_read on public.attendance using (hris_private.can_read_employee(company_id,employee_id));
alter policy attendance_create on public.attendance with check (hris_private.can_record_employee(company_id,employee_id));
alter policy attendance_update on public.attendance using (hris_private.can_record_employee(company_id,employee_id))
    with check (hris_private.can_record_employee(company_id,employee_id));
alter policy leave_read on public.leave_requests using (hris_private.can_read_employee(company_id,employee_id));
alter policy leave_create on public.leave_requests with check (hris_private.can_record_employee(company_id,employee_id));
alter policy leave_update on public.leave_requests using (hris_private.can_review_employee(company_id,employee_id))
    with check (hris_private.can_review_employee(company_id,employee_id));

-- Members' private photos remain accessible only to their own active account.
alter policy hris_profile_photos_select on storage.objects using (
    bucket_id='profile-photos' and name=(select auth.uid())::text||'/avatar.png' and owner_id=(select auth.uid())::text
    and exists(select 1 from public.company_members where user_id=(select auth.uid()) and active));
alter policy hris_profile_photos_insert on storage.objects with check (
    bucket_id='profile-photos' and name=(select auth.uid())::text||'/avatar.png' and owner_id=(select auth.uid())::text
    and exists(select 1 from public.company_members where user_id=(select auth.uid()) and active));
alter policy hris_profile_photos_update on storage.objects using (
    bucket_id='profile-photos' and name=(select auth.uid())::text||'/avatar.png' and owner_id=(select auth.uid())::text
    and exists(select 1 from public.company_members where user_id=(select auth.uid()) and active))
    with check (bucket_id='profile-photos' and name=(select auth.uid())::text||'/avatar.png' and owner_id=(select auth.uid())::text
    and exists(select 1 from public.company_members where user_id=(select auth.uid()) and active));
alter policy hris_profile_photos_delete on storage.objects using (
    bucket_id='profile-photos' and name=(select auth.uid())::text||'/avatar.png' and owner_id=(select auth.uid())::text
    and exists(select 1 from public.company_members where user_id=(select auth.uid()) and active));

-- Advisory locks serialize leave-budget decisions without granting managers employee UPDATE.
-- Allowance edits use the same employee lock; identifiers only choose a lock, never access.
create or replace function hris_private.validate_employee() returns trigger
language plpgsql security invoker set search_path='' as $$
declare used_days integer; company_zone text;
begin
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
create or replace function hris_private.validate_leave() returns trigger
language plpgsql security invoker set search_path='' as $$
declare allowance integer; employee_state text; used_days integer;
begin
    perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(new.employee_id::text,0));
    select annual_leave_days,status into allowance,employee_state from public.employees
        where id=new.employee_id and company_id=new.company_id;
    if not found then raise exception 'Choose an employee in your company.' using errcode='23514'; end if;
    if tg_op='INSERT' then
        new.status:='pending';
        if employee_state<>'active' then raise exception 'Archived employees cannot receive new leave requests.' using errcode='23514'; end if;
    elsif old.status<>'pending' or new.status not in ('approved','rejected') then
        raise exception 'Only pending requests can be approved or rejected.' using errcode='23514';
    end if;
    if hris_private.workdays(new.starts_on,new.ends_on)=0 then
        raise exception 'Choose at least one Monday-Friday workday.' using errcode='23514';
    end if;
    if new.status='approved' then
        if employee_state<>'active' then raise exception 'Archived employees cannot have new leave approved.' using errcode='23514'; end if;
        if new.kind='annual' then
            select coalesce(sum(hris_private.workdays(r.starts_on,r.ends_on)),0)::integer into used_days
                from public.leave_requests r where r.employee_id=new.employee_id and r.id<>new.id
                  and r.kind='annual' and r.status='approved' and extract(year from r.starts_on)=extract(year from new.starts_on);
            if used_days+hris_private.workdays(new.starts_on,new.ends_on)>allowance then
                raise exception 'This request exceeds the annual leave allowance.' using errcode='23514';
            end if;
        end if;
    end if;
    return new;
end;
$$;

revoke all on all functions in schema hris_private from public,anon,authenticated;
grant execute on function hris_private.workdays(date,date), hris_private.company_role(uuid),
    hris_private.can_read_employee(uuid,uuid), hris_private.can_record_employee(uuid,uuid),
    hris_private.can_review_employee(uuid,uuid), hris_private.employee_has_account(uuid,uuid), hris_private.my_hris_access(),
    hris_private.list_company_accounts(), hris_private.save_company_account(uuid,text,text,boolean,uuid,uuid) to authenticated;
revoke all on function public.my_hris_access(),public.list_company_accounts(),
    public.save_company_account(uuid,text,text,boolean,uuid,uuid) from public,anon,authenticated;
grant execute on function public.my_hris_access(),public.list_company_accounts(),
    public.save_company_account(uuid,text,text,boolean,uuid,uuid) to authenticated;

commit;
