-- Dedicated prototype. All generated identities, contacts, logo metadata and changes roll back.
begin;
do $$
declare
    company uuid; owner_user uuid;
    manager uuid:=gen_random_uuid(); hr uuid:=gen_random_uuid(); portal uuid:=gen_random_uuid(); outsider uuid:=gen_random_uuid(); candidate uuid:=gen_random_uuid();
    manager_person uuid; hr_person uuid; portal_person uuid; combined_person uuid;
    manager_email text:='qa-profile-manager-'||manager::text||'@example.invalid';
    hr_email text:='qa-profile-hr-'||hr::text||'@example.invalid';
    portal_email text:='qa-profile-portal-'||portal::text||'@example.invalid';
    candidate_email text:='qa-profile-staff-'||candidate::text||'@example.invalid';
    person jsonb; changed integer; owner_person uuid; supervisor_name text;
begin
    assert (select count(*) from public.companies)=1,'Dedicated single-company database required';
    select id,owner_id into strict company,owner_user from public.companies;
    insert into auth.users(id,email) values(manager,manager_email),(hr,hr_email),(portal,portal_email),
        (outsider,'qa-profile-outsider-'||outsider::text||'@example.invalid'),(candidate,candidate_email);
    perform set_config('request.jwt.claim.sub',owner_user::text,true);execute 'set local role authenticated';
    perform public.save_company_account(manager,manager_email,'manager',true,null,null);
    perform public.save_company_account(hr,hr_email,'hr_generalist',true,null,null);
    perform public.save_company_account(portal,portal_email,'employee',true,null,null);
    select employee_id into strict manager_person from public.company_members where user_id=manager;
    select employee_id into strict hr_person from public.company_members where user_id=hr;
    select employee_id into strict portal_person from public.company_members where user_id=portal;
    select employee_id into strict owner_person from public.company_members where user_id=owner_user;
    select full_name into strict supervisor_name from public.employees where id=owner_person;
    update public.employees set reports_to=owner_person where id=manager_person;
    insert into public.employee_contacts(company_id,employee_id,phone) values(company,hr_person,'000 000 0000');
    update public.employees set reports_to=manager_person where id=portal_person;
    perform set_config('request.jwt.claim.sub',manager::text,true);
    assert public.my_hris_access()->>'reports_to_name'=supervisor_name,'Profile supervisor name missing';
    assert not exists(select 1 from public.employees where id=owner_person),'Supervisor employment leaked through Profile';
    insert into public.employee_contacts(company_id,employee_id,phone,emergency_name,emergency_relationship,emergency_phone)
        values(company,manager_person,'000 000 0000','Synthetic emergency contact','Sibling','000 000 0001');
    update public.employee_contacts set address='Updated own fictional address' where employee_id=manager_person;
    get diagnostics changed=row_count;assert changed=1,'Manager cannot save own contact';
    update public.employee_contacts set phone='000 000 0002' where employee_id=hr_person;
    get diagnostics changed=row_count;assert changed=0,'Manager can modify another person contacts';
    update public.employees set department='Forbidden' where id=manager_person;
    get diagnostics changed=row_count;assert changed=0,'Manager can edit employment';
    begin update public.employee_contacts set employee_id=portal_person where employee_id=manager_person;
        raise exception 'Contact identity transfer allowed';exception when insufficient_privilege then null;end;
    begin update public.employee_contacts set emergency_phone='' where employee_id=manager_person;
        raise exception 'Incomplete emergency details accepted';exception when check_violation then null;end;
    perform set_config('request.jwt.claim.sub',portal::text,true);
    insert into public.employee_contacts(company_id,employee_id,phone) values(company,portal_person,'000 000 0000');
    update public.employee_contacts set phone='000 000 0002' where employee_id=portal_person;
    get diagnostics changed=row_count;assert changed=1,'Portal employee cannot edit own contact';

    perform set_config('request.jwt.claim.sub',owner_user::text,true);
    person:=jsonb_build_object('company_id',company,'employee_code','QA-COMBINED-'||left(candidate::text,12),'full_name','Synthetic Combined Staff',
        'department','QA','job_title','Synthetic test','joined_on',null,'reports_to',null,'annual_leave_days',15,'status','active');
    combined_person:=public.save_staff_account(candidate,candidate_email,'hr_generalist',false,person,'{"phone":"000 000 0000"}');
    assert (select employee_id from public.company_members where user_id=candidate)=combined_person,'Staff identity differs from saved employee';
    assert (select count(*) from public.employees where employee_code=upper(person->>'employee_code'))=1,'Saved employee identity missing or duplicated';
    person:=person||jsonb_build_object('id',combined_person,'full_name','Must roll back');
    begin perform public.save_staff_account(candidate,candidate_email,'invalid-role',false,person,'{"phone":"000 000 0009"}');
        raise exception 'Invalid staff role accepted';exception when check_violation then null;end;
    assert (select full_name from public.employees where id=combined_person)='Synthetic Combined Staff','Failed role saved partial employment';
    assert (select phone from public.employee_contacts where employee_id=combined_person)='000 000 0000','Failed role saved partial contacts';
    perform set_config('request.jwt.claim.sub',hr::text,true);
    begin perform public.save_staff_account(candidate,candidate_email,'hr_generalist',false,person,'{}');
        raise exception 'HR can manage staff access';exception when insufficient_privilege then null;end;

    -- Managed Storage metadata is read-only in hosted SQL. Exercise object operations
    -- through the real Storage API probe; the local suite separately checks its RLS.
    execute 'reset role';
    assert exists(select 1 from storage.buckets where id='company-branding' and not public
        and file_size_limit=2097152 and allowed_mime_types=array['image/png']),'Private logo bucket configuration missing';
    assert (select count(*) from pg_policies where schemaname='storage' and tablename='objects'
        and policyname in ('company_logo_read','company_logo_create','company_logo_update','company_logo_delete'))=4,
        'Company logo policies missing';
    perform set_config('request.jwt.claim.sub',owner_user::text,true);execute 'set local role authenticated';
    perform public.set_company_logo(true);
    assert (select logo_version is not null from public.companies where id=company),'Logo revision not saved';
    perform set_config('request.jwt.claim.sub',manager::text,true);
    begin perform public.set_company_logo(false);raise exception 'Manager can remove company logo';exception when insufficient_privilege then null;end;
    perform set_config('request.jwt.claim.sub',owner_user::text,true);
    perform public.save_company_account(manager,manager_email,'manager',false,null,null);
    perform set_config('request.jwt.claim.sub',manager::text,true);
    update public.employee_contacts set phone='000 000 0009' where employee_id=manager_person;
    get diagnostics changed=row_count;assert changed=0,'Disabled staff can edit own contacts';
    assert not exists(select 1 from storage.objects where bucket_id='company-branding'),'Disabled staff can read logo';
    perform set_config('request.jwt.claim.sub',outsider::text,true);
    assert not exists(select 1 from storage.objects where bucket_id='company-branding'),'Unassigned account can read logo';
    execute 'set local role anon';
    assert not exists(select 1 from storage.objects where bucket_id='company-branding'),'Anonymous can read logo';
    execute 'reset role';
end;
$$;
rollback;
select 'Passed hosted self-service contacts, employment protection, atomic staff identity and company-logo role checks; all fixtures rolled back.' as result;
