-- Dedicated prototype only. All generated actors, records and catalog fixtures roll back.
begin;
do $$
declare
    company uuid; owner_user uuid; owner_person uuid;
    hr uuid:=gen_random_uuid(); manager uuid:=gen_random_uuid(); staff uuid:=gen_random_uuid(); outsider uuid:=gen_random_uuid();
    hr_person uuid; manager_person uuid; staff_person uuid; report uuid; draft jsonb;
    hr_email text:='qa-refinement-hr-'||hr::text||'@example.invalid';
    manager_email text:='qa-refinement-manager-'||manager::text||'@example.invalid';
    staff_email text:='qa-refinement-staff-'||staff::text||'@example.invalid';
    changed integer; feed jsonb;
begin
    assert (select count(*) from public.companies)=1,'Dedicated single-company prototype required';
    assert not exists(select 1 from public.company_members where employee_id is null),'Account has no employee';
    select id,owner_id into strict company,owner_user from public.companies;
    select employee_id into strict owner_person from public.company_members where user_id=owner_user;
    assert exists(select 1 from public.employees where id=owner_person),'Original administrator employee missing';
    insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data) values
        (hr,hr_email,now(),'{"hris_profile":{"full_name":"Synthetic automatic HR employee"}}'),
        (manager,manager_email,now(),'{"full_name":"Synthetic automatic Manager"}'),
        (staff,staff_email,now(),'{"full_name":"Synthetic automatic Employee"}'),
        (outsider,'qa-refinement-outsider-'||outsider::text||'@example.invalid',now(),'{}');
    perform set_config('request.jwt.claim.sub',owner_user::text,true);
    execute 'set local role authenticated';
    perform public.save_company_account(hr,hr_email,'hr_generalist',true,null,null);
    perform public.save_company_account(manager,manager_email,'manager',true,null,null);
    perform public.save_company_account(staff,staff_email,'employee',true,null,null);
    select employee_id into strict hr_person from public.company_members where user_id=hr;
    select employee_id into strict manager_person from public.company_members where user_id=manager;
    select employee_id into strict staff_person from public.company_members where user_id=staff;
    assert (select full_name from public.employees where id=hr_person)='Synthetic automatic HR employee','Automatic employee name missing';
    assert (select joined_on is null and department='' and job_title='' from public.employees where id=hr_person),'Invented employment details';
    perform public.save_company_account(hr,hr_email,'hr_generalist',true,null,null);
    assert (select employee_id from public.company_members where user_id=hr)=hr_person,'Account retry changed identity';
    begin
        perform public.save_company_account(hr,hr_email,'hr_generalist',true,owner_person,null);
        raise exception 'Employee identity was reassigned';
    exception when check_violation then null; end;
    begin
        perform hris_private.create_account_employee(company,outsider);
        raise exception 'Client could call trusted employee helper';
    exception when insufficient_privilege then null; end;
    update public.employees set reports_to=manager_person where id=staff_person;
    report:=public.save_employee_record(jsonb_build_object('company_id',company,'employee_code','QA-NO-LOGIN-'||left(manager::text,12),
        'full_name','Synthetic report without an account','department','','job_title','','joined_on',null,
        'annual_leave_days',15,'status','active','reports_to',manager_person),'{"phone":"000 000 0000"}');
    select to_jsonb(e) into draft from public.employees e where id=report;
    begin
        perform public.save_employee_record(draft||'{"full_name":"Must roll back"}', '{"emergency_name":"Incomplete"}');
        raise exception 'Invalid emergency contact was accepted';
    exception when check_violation then null; end;
    assert (select full_name from public.employees where id=report)='Synthetic report without an account','Partial employment save';
    perform public.save_employee_record(draft||'{"job_title":"Synthetic verified title"}', '{"phone":"000 000 0001"}');
    assert (select job_title from public.employees where id=report)='Synthetic verified title','Employment save failed';
    assert (select phone from public.employee_contacts where employee_id=report)='000 000 0001','Contact save failed';
    begin
        update public.employees set reports_to=id where id=report;
        raise exception 'Self reporting accepted';
    exception when check_violation then null; end;
    begin
        update public.employees set reports_to=report where id=manager_person;
        raise exception 'Reporting cycle accepted';
    exception when check_violation then null; end;
    begin
        update public.employees set status='archived' where id=manager_person;
        raise exception 'Manager with active reports archived';
    exception when check_violation then null; end;

    perform set_config('request.jwt.claim.sub',hr::text,true);
    assert exists(select 1 from public.employee_contacts where employee_id=report),'HR contact scope failed';
    perform set_config('request.jwt.claim.sub',manager::text,true);
    assert exists(select 1 from public.employees where id=report),'Manager cannot read report without login';
    assert (public.my_hris_access()->'direct_reports') ? report::text,'Report missing from manager scope';
    assert not exists(select 1 from public.employee_contacts where employee_id=report),'Manager can read team private contacts';
    begin
        perform public.save_employee_record(draft,'{}');
        raise exception 'Manager can edit employment';
    exception when insufficient_privilege then null; end;
    begin
        perform 1 from public.desktop_releases;
        raise exception 'Staff can read private release catalog';
    exception when insufficient_privilege then null; end;
    feed:=public.desktop_update();
    assert feed->>'schema_version' ~ '^[0-9]{14}$','Update schema history unavailable';
    begin
        insert into storage.objects(bucket_id,name,owner_id) values('desktop-updates','forbidden.exe',manager::text);
        raise exception 'Staff can upload updates';
    exception when insufficient_privilege then null; end;
    execute 'reset role';
    insert into public.desktop_releases(version,platform,requires_schema,envelope,enabled) values
        ('999998.0.0','windows-x64','20000101000000','{"payload":"synthetic-compatible","signature":"fixture"}',true),
        ('999999.0.0','windows-x64','99990101000000','{"payload":"synthetic-future","signature":"fixture"}',true);
    execute 'set local role authenticated';
    feed:=public.desktop_update();
    assert feed->'release'->>'payload'='synthetic-compatible','Incompatible schema offered';
    begin
        update public.desktop_releases set enabled=true where version='999999.0.0';
        raise exception 'Staff can enable updates';
    exception when insufficient_privilege then null; end;
    perform set_config('request.jwt.claim.sub',staff::text,true);
    begin perform public.desktop_update(); raise exception 'Portal employee can check desktop updates';
    exception when insufficient_privilege then null; end;
    perform set_config('request.jwt.claim.sub',owner_user::text,true);
    perform public.save_company_account(hr,hr_email,'hr_generalist',false,null,null);
    perform set_config('request.jwt.claim.sub',hr::text,true);
    assert not exists(select 1 from public.employee_contacts),'Disabled HR retained contacts';
    begin perform public.desktop_update(); raise exception 'Disabled staff can check updates';
    exception when insufficient_privilege then null; end;
    perform set_config('request.jwt.claim.sub',outsider::text,true);
    begin perform public.desktop_update(); raise exception 'Unassigned account can check updates';
    exception when insufficient_privilege then null; end;
    execute 'set local role anon';
    begin perform public.desktop_update(); raise exception 'Anonymous can check updates';
    exception when insufficient_privilege then null; end;
    execute 'reset role';
end;
$$;
rollback;
select 'Passed hosted automatic identities, atomic save, reporting, private-contact scopes and update-channel permission/schema checks; fixtures rolled back.' as result;
