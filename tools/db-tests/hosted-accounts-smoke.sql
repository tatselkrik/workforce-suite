-- Dedicated, initialized HRIS prototype only. No existing HR records are allowed.
-- Fixtures and access changes remain inside one transaction and are rolled back.
begin;
do $$
declare
    company uuid; installation_owner uuid; owner_email text;
    hr uuid:=gen_random_uuid(); manager uuid:=gen_random_uuid(); staff uuid:=gen_random_uuid(); outsider uuid:=gen_random_uuid();
    hr_email text; manager_email text; staff_email text;
    hr_person uuid; manager_person uuid; staff_person uuid; peer_person uuid;
    staff_leave uuid; own_leave uuid; attendance_id uuid; changed integer;
begin
    if (select count(*) from public.companies)<>1 or (select count(*) from auth.users)<>1
       or exists(select 1 from public.employees) or exists(select 1 from public.attendance)
       or exists(select 1 from public.leave_requests) then
        raise exception 'Run only on the dedicated, initialized prototype before entering HR records.';
    end if;
    select c.id,c.owner_id,u.email into strict company,installation_owner,owner_email
        from public.companies c join auth.users u on u.id=c.owner_id;
    hr_email:='qa-hr-'||hr::text||'@example.invalid';
    manager_email:='qa-manager-'||manager::text||'@example.invalid';
    staff_email:='qa-staff-'||staff::text||'@example.invalid';
    insert into auth.users(id,email,email_confirmed_at) values
        (hr,hr_email,now()),(manager,manager_email,now()),(staff,staff_email,now()),
        (outsider,'qa-outsider-'||outsider::text||'@example.invalid',now());
    perform set_config('request.jwt.claim.sub',installation_owner::text,true);
    execute 'set local role authenticated';
    assert public.my_hris_access()->>'role'='administrator','Owner administrator access was not preserved';
    insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days)
        values(company,'QA-HOST-HR','Hosted QA HR','QA','Test','2026-01-01',15) returning id into hr_person;
    insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days)
        values(company,'QA-HOST-MGR','Hosted QA Manager','QA','Test','2026-01-01',15) returning id into manager_person;
    insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days)
        values(company,'QA-HOST-STAFF','Hosted QA Staff','QA','Test','2026-01-01',15) returning id into staff_person;
    insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days)
        values(company,'QA-HOST-PEER','Hosted QA Peer','QA','Test','2026-01-01',15) returning id into peer_person;
    perform public.save_company_account(hr,hr_email,'hr_generalist',true,hr_person,null);
    perform public.save_company_account(manager,manager_email,'manager',true,manager_person,hr);
    perform public.save_company_account(staff,staff_email,'employee',true,staff_person,manager);
    assert jsonb_array_length(public.list_company_accounts())=4,'Owner account list failed';
    assert (select count(*) from public.account_access_events)=3,'Access audit writes failed';
    begin
        perform public.save_company_account(installation_owner,owner_email,'administrator',false,null,null);
        raise exception 'Owner was allowed to be disabled';
    exception when check_violation then null; end;
    begin
        perform public.save_company_account(hr,hr_email,'hr_generalist',true,hr_person,manager);
        raise exception 'Reporting cycle was allowed';
    exception when check_violation then null; end;
    begin
        update public.company_members set role='administrator' where user_id=staff;
        raise exception 'Direct membership write was allowed';
    exception when insufficient_privilege then null; end;

    perform set_config('request.jwt.claim.sub',hr::text,true);
    assert (select count(*) from public.employees)=4,'HR cannot read company employee records';
    begin
        perform public.list_company_accounts();
        raise exception 'HR was allowed to list accounts';
    exception when insufficient_privilege then null; end;
    update public.companies set annual_leave_days=100 where id=company;
    get diagnostics changed=row_count;
    assert changed=0,'HR could change company settings';
    update public.employees set job_title='QA updated' where id=staff_person;
    get diagnostics changed=row_count;
    assert changed=1,'HR could not edit employee records';
    begin
        update public.employees set status='archived' where id=staff_person;
        raise exception 'An active linked employee was archived';
    exception when check_violation then null; end;
    insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on)
        values(company,staff_person,'annual','2027-01-04','2027-01-04') returning id into staff_leave;
    insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on)
        values(company,hr_person,'annual','2027-01-04','2027-01-04') returning id into own_leave;
    update public.leave_requests set status='approved' where id=own_leave;
    get diagnostics changed=row_count;
    assert changed=0,'HR was allowed to approve own leave';
    assert (select count(*) from public.account_access_events)=0,'HR can read account audit';

    perform set_config('request.jwt.claim.sub',manager::text,true);
    assert (select count(*) from public.employees)=2,'Manager record scope is incorrect';
    assert public.my_hris_access()->'direct_reports'=jsonb_build_array(staff_person),'Manager direct reports are incorrect';
    update public.employees set job_title='Forbidden' where id=staff_person;
    get diagnostics changed=row_count;
    assert changed=0,'Manager could edit employee records';
    begin
        insert into public.attendance(company_id,employee_id) values(company,staff_person);
        raise exception 'Manager could record another employee attendance';
    exception when insufficient_privilege then null; end;
    insert into public.attendance(company_id,employee_id) values(company,manager_person);
    update public.leave_requests set status='approved' where id=staff_leave;
    get diagnostics changed=row_count;
    assert changed=1,'Manager could not review a direct report without employee update rights';
    insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on)
        values(company,manager_person,'sick','2027-02-01','2027-02-01') returning id into own_leave;
    update public.leave_requests set status='approved' where id=own_leave;
    get diagnostics changed=row_count;
    assert changed=0,'Manager could approve own leave';

    perform set_config('request.jwt.claim.sub',staff::text,true);
    perform set_config('request.jwt.claims',jsonb_build_object('sub',staff,'role','authenticated',
        'user_metadata',jsonb_build_object('role','administrator'),'app_metadata',jsonb_build_object('role','administrator'))::text,true);
    assert public.my_hris_access()->>'role'='employee','Metadata spoofed protected membership role';
    assert (select count(*) from public.employees)=1,'Employee record scope is incorrect';
    begin
        perform public.list_company_accounts();
        raise exception 'Employee could list accounts';
    exception when insufficient_privilege then null; end;
    update public.leave_requests set status='rejected' where id=staff_leave;
    get diagnostics changed=row_count;
    assert changed=0,'Employee could review leave';
    begin
        insert into public.attendance(company_id,employee_id) values(company,peer_person);
        raise exception 'Employee could record unrelated attendance';
    exception when insufficient_privilege or check_violation then null; end;
    insert into public.attendance(company_id,employee_id) values(company,staff_person) returning id into attendance_id;
    update public.attendance set checked_out_at=now() where id=attendance_id;
    get diagnostics changed=row_count;
    assert changed=1,'Employee own attendance failed';

    perform set_config('request.jwt.claim.sub',installation_owner::text,true);
    perform set_config('request.jwt.claims','{}',true);
    perform public.save_company_account(staff,staff_email,'employee',false,staff_person,manager);
    perform set_config('request.jwt.claim.sub',staff::text,true);
    begin
        perform public.my_hris_access();
        raise exception 'Disabled membership retained access';
    exception when insufficient_privilege then null; end;
    assert (select count(*) from public.companies)=0,'Disabled account can read company';
    assert (select count(*) from public.employees)=0,'Disabled account can read employees';
    perform set_config('request.jwt.claim.sub',outsider::text,true);
    begin
        perform public.my_hris_access();
        raise exception 'Unassigned identity has access';
    exception when insufficient_privilege then null; end;
    execute 'reset role';
    execute 'set local role anon';
    begin
        perform public.my_hris_access();
        raise exception 'Anonymous access RPC was allowed';
    exception when insufficient_privilege then null; end;
    begin
        perform * from public.company_members;
        raise exception 'Anonymous membership read was allowed';
    exception when insufficient_privilege then null; end;
    execute 'reset role';
end;
$$;
rollback;
select 'passed' as hosted_account_role_smoke, true as fixtures_rolled_back,
    (select count(*) from public.companies) as companies,
    (select count(*) from auth.users) as auth_users,
    (select count(*) from auth.sessions) as auth_sessions,
    (select count(*) from public.company_members) as members,
    (select count(*) from public.account_access_events) as access_events,
    (select count(*) from public.employees) as employees,
    (select count(*) from public.attendance) as attendance,
    (select count(*) from public.leave_requests) as leave_requests;
