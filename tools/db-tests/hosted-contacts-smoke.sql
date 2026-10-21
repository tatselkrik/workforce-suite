-- Dedicated prototype role checks. Generated actors/records roll back together.
-- Existing records and real account metadata are never modified.
begin;
do $$
declare
    company uuid; installation_owner uuid;
    hr uuid:=gen_random_uuid(); manager uuid:=gen_random_uuid(); staff uuid:=gen_random_uuid(); outsider uuid:=gen_random_uuid();
    hr_person uuid; manager_person uuid; staff_person uuid; peer_person uuid;
    hr_email text; manager_email text; staff_email text; changed integer; marker text:=gen_random_uuid()::text;
begin
    if (select count(*) from public.companies)<>1 then raise exception 'Dedicated single-company prototype required.'; end if;
    select id,owner_id into strict company,installation_owner from public.companies;
    hr_email:='qa-contacts-hr-'||hr::text||'@example.invalid';
    manager_email:='qa-contacts-manager-'||manager::text||'@example.invalid';
    staff_email:='qa-contacts-staff-'||staff::text||'@example.invalid';
    insert into auth.users(id,email,email_confirmed_at) values
        (hr,hr_email,now()),(manager,manager_email,now()),(staff,staff_email,now()),
        (outsider,'qa-contacts-outsider-'||outsider::text||'@example.invalid',now());
    perform set_config('request.jwt.claim.sub',installation_owner::text,true);
    execute 'set local role authenticated';
    insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days)
        values(company,'QACH-'||left(marker,20),'Synthetic contacts HR','QA - synthetic','QA','2026-01-01',15) returning id into hr_person;
    insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days)
        values(company,'QACM-'||left(marker,20),'Synthetic contacts Manager','QA - synthetic','QA','2026-01-01',15) returning id into manager_person;
    insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days)
        values(company,'QACS-'||left(marker,20),'Synthetic contacts Employee','QA - synthetic','QA','2026-01-01',15) returning id into staff_person;
    insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days)
        values(company,'QACP-'||left(marker,20),'Synthetic contacts Peer','QA - synthetic','QA','2026-01-01',15) returning id into peer_person;
    perform public.save_company_account(hr,hr_email,'hr_generalist',true,hr_person,null);
    perform public.save_company_account(manager,manager_email,'manager',true,manager_person,null);
    perform public.save_company_account(staff,staff_email,'employee',true,staff_person,manager);
    insert into public.employee_contacts(company_id,employee_id,phone,personal_email,address,emergency_name,emergency_relationship,emergency_phone)
        select company,id,' 000 000 0000 ',' sample@example.invalid ',' Fictional address ',' Sample Contact ',' Sibling ','000 000 0001'
        from public.employees where id in (manager_person,staff_person,peer_person);
    assert (select phone from public.employee_contacts where employee_id=staff_person)='000 000 0000','Server normalization failed';
    begin
        update public.employee_contacts set employee_id=employee_id where employee_id=staff_person;
        raise exception 'Employee identity was client editable';
    exception when insufficient_privilege then null; end;
    begin
        update public.employee_contacts set updated_at=now() where employee_id=staff_person;
        raise exception 'Timestamp was client editable';
    exception when insufficient_privilege then null; end;
    begin
        update public.employee_contacts set emergency_phone='' where employee_id=staff_person;
        raise exception 'Incomplete emergency contact was accepted';
    exception when check_violation then null; end;
    begin
        update public.employee_contacts set phone='call me' where employee_id=staff_person;
        raise exception 'Invalid phone was accepted';
    exception when check_violation then null; end;
    begin
        update public.employee_contacts set personal_email='invalid' where employee_id=staff_person;
        raise exception 'Invalid email was accepted';
    exception when check_violation then null; end;
    begin
        update public.employee_contacts set address=repeat('x',501) where employee_id=staff_person;
        raise exception 'Oversized address was accepted';
    exception when check_violation then null; end;

    perform set_config('request.jwt.claim.sub',hr::text,true);
    assert (select count(*) from public.employee_contacts where employee_id in (manager_person,staff_person,peer_person))=3,'HR contact read failed';
    insert into public.employee_contacts(company_id,employee_id) values(company,hr_person);
    update public.employee_contacts set address='Updated fictional address' where employee_id=staff_person;
    get diagnostics changed=row_count;
    assert changed=1,'HR contact update failed';

    perform set_config('request.jwt.claim.sub',manager::text,true);
    assert (select count(*) from public.employees where id=staff_person)=1,'Manager direct-report fixture missing';
    assert (select count(*) from public.employee_contacts)=1,'Manager read another employee private contact';
    assert (select employee_id from public.employee_contacts)=manager_person,'Manager own contact missing';
    update public.employee_contacts set address='Forbidden' where employee_id in (manager_person,staff_person,peer_person);
    get diagnostics changed=row_count;
    assert changed=0,'Manager could edit HR contacts';
    begin
        insert into public.employee_contacts(company_id,employee_id) values(company,manager_person);
        raise exception 'Manager could insert contacts';
    exception when insufficient_privilege then null; end;

    perform set_config('request.jwt.claim.sub',staff::text,true);
    assert (select count(*) from public.employee_contacts)=1,'Employee private contact scope failed';
    assert (select employee_id from public.employee_contacts)=staff_person,'Employee own contact missing';
    update public.employee_contacts set phone='000 000 0002' where employee_id=staff_person;
    get diagnostics changed=row_count;
    assert changed=0,'Employee could edit HR contacts';

    perform set_config('request.jwt.claim.sub',installation_owner::text,true);
    perform public.save_company_account(staff,staff_email,'employee',false,staff_person,manager);
    perform set_config('request.jwt.claim.sub',staff::text,true);
    assert not exists(select 1 from public.employee_contacts),'Disabled user retained contacts';
    perform set_config('request.jwt.claim.sub',outsider::text,true);
    assert not exists(select 1 from public.employee_contacts),'Unassigned user read contacts';
    execute 'set local role anon';
    begin
        perform 1 from public.employee_contacts;
        raise exception 'Anonymous user read contacts';
    exception when insufficient_privilege then null; end;
    execute 'reset role';
end;
$$;
rollback;
select 'Passed hosted contact validation, HR edit, manager/employee own-only reads, and revocation checks; fixtures rolled back.' as result;
