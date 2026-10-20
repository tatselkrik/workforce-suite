-- Run only on the dedicated, empty HRIS database as its project administrator.
-- All fixture identities and HR records are rolled back, even on a failed check.
begin;
do $$ begin
    if exists (select 1 from public.companies) then
        raise exception 'Run this smoke test before initializing the live company.';
    end if;
end $$;
insert into auth.users (id) values
    ('00000000-0000-0000-0000-000000000011'),
    ('00000000-0000-0000-0000-000000000012');
insert into public.companies (owner_id, name, timezone)
    values ('00000000-0000-0000-0000-000000000011', 'Temporary QA company', 'Asia/Manila');
do $$ begin
    begin
        insert into public.companies (owner_id, name)
            values ('00000000-0000-0000-0000-000000000012', 'Forbidden second company');
        raise exception 'Singleton company constraint did not reject a second company.';
    exception when unique_violation then null;
    end;
end $$;

set local role authenticated;
select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-000000000011', true);
do $$
declare company_id uuid; person_id uuid; attendance_id uuid; request_id uuid;
    actual_in timestamptz; actual_out timestamptz;
begin
    select id into strict company_id from public.companies;
    begin
        insert into public.companies (owner_id, name)
            values ('00000000-0000-0000-0000-000000000011', 'Forbidden client setup');
        raise exception 'Authenticated clients must not create a company.';
    exception when insufficient_privilege then null;
    end;
    insert into public.employees (company_id, employee_code, full_name, department,
        job_title, joined_on, annual_leave_days)
    values (company_id, ' qa-live ', ' Temporary QA Employee ', 'QA', 'Test', '2026-01-01', 2)
    returning id into person_id;
    if not exists (select 1 from public.employees where id = person_id
        and employee_code = 'QA-LIVE' and full_name = 'Temporary QA Employee') then
        raise exception 'Employee validation or owner access failed.';
    end if;
    insert into public.attendance (company_id, employee_id)
        values (company_id, person_id) returning id, checked_in_at into attendance_id, actual_in;
    if abs(extract(epoch from (clock_timestamp() - actual_in))) > 60 then
        raise exception 'Attendance did not use the server clock.';
    end if;
    begin
        insert into public.attendance (company_id, employee_id) values (company_id, person_id);
        raise exception 'Duplicate check-in was allowed.';
    exception when unique_violation then null;
    end;
    update public.attendance set checked_out_at = '1900-01-01' where id = attendance_id
        returning checked_out_at into actual_out;
    if actual_out < actual_in then raise exception 'Server check-out timestamp failed.'; end if;
    insert into public.leave_requests (company_id, employee_id, kind, starts_on, ends_on)
        values (company_id, person_id, 'annual', '2027-01-04', '2027-01-05')
        returning id into request_id;
    update public.leave_requests set status = 'approved' where id = request_id;
    insert into public.leave_requests (company_id, employee_id, kind, starts_on, ends_on)
        values (company_id, person_id, 'annual', '2027-01-06', '2027-01-06')
        returning id into request_id;
    begin
        update public.leave_requests set status = 'approved' where id = request_id;
        raise exception 'Approval exceeded the annual allowance.';
    exception when check_violation then null;
    end;
end $$;

select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-000000000012', true);
do $$ begin
    if exists (select 1 from public.companies)
        or exists (select 1 from public.employees)
        or exists (select 1 from public.attendance)
        or exists (select 1 from public.leave_requests) then
        raise exception 'An unrelated Auth account can read HR data.';
    end if;
end $$;
reset role;
set local role anon;
do $$
declare table_name text;
begin
    foreach table_name in array array['companies', 'employees', 'attendance', 'leave_requests'] loop
        begin
            execute format('select * from public.%I', table_name);
            raise exception 'Anonymous HR table access was allowed.';
        exception when insufficient_privilege then null;
        end;
    end loop;
end $$;
reset role;
rollback;
select 'passed' as hosted_smoke,
    (select count(*) from public.companies) as companies_after_rollback,
    (select count(*) from public.employees) as employees_after_rollback,
    (select count(*) from auth.users where id in
        ('00000000-0000-0000-0000-000000000011', '00000000-0000-0000-0000-000000000012'))
        as fixture_identities_after_rollback;
