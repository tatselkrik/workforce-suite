-- Synthetic PostgreSQL subjects, not Auth tokens. Every fixture rolls back.
begin;
do $$
declare c uuid;owner_id uuid;hr_id uuid:=gen_random_uuid();manager_id uuid:=gen_random_uuid();hr_email text;manager_email text;
 batch uuid:=gen_random_uuid();failed uuid:=gen_random_uuid();rows jsonb;bad jsonb;code text;before_people integer;before_contacts integer;before_history integer;
begin
 if (select count(*) from public.companies)<>1 or to_regprocedure('public.import_employees(uuid,jsonb,text)') is null then raise exception 'Expected initialized import deployment.';end if;
 select id,companies.owner_id into strict c,owner_id from public.companies;
 hr_email:='qa-import-hr-'||hr_id||'@example.invalid';manager_email:='qa-import-manager-'||manager_id||'@example.invalid';
 insert into auth.users(id,email,email_confirmed_at) values(hr_id,hr_email,clock_timestamp()),(manager_id,manager_email,clock_timestamp());
 perform set_config('request.headers','{"x-hris-leave-contract":"2"}',true);perform set_config('request.jwt.claim.sub',owner_id::text,true);execute 'set local role authenticated';
 perform public.save_company_account(hr_id,hr_email,'hr_generalist',true,null,null);perform public.save_company_account(manager_id,manager_email,'manager',true,null,null);
 select count(*) into before_people from public.employees;select count(*) into before_contacts from public.employee_contacts;select count(*) into before_history from public.employment_history;
 code:=upper('QI-'||substr(batch::text,1,8));
 rows:=jsonb_build_array(jsonb_build_object('source_row',2,'employee',jsonb_build_object('id',owner_id,'company_id',manager_id,'employee_code',code||'-1','full_name','Synthetic rollback imported one','department','Synthetic Imports','job_title','Synthetic fixture','joined_on','2020-01-01','annual_leave_days',15,'status','archived'),'contacts',jsonb_build_object('phone','000 000 0901','emergency_name','Synthetic Contact','emergency_relationship','Sibling','emergency_phone','000 000 0902'),'references','{}'::jsonb),jsonb_build_object('source_row',4,'employee',jsonb_build_object('employee_code',code||'-2','full_name','Synthetic rollback imported two','annual_leave_days',15),'contacts','{}'::jsonb,'references','{}'::jsonb));
 perform set_config('request.jwt.claim.sub',manager_id::text,true);begin perform public.import_employees(batch,rows,'Synthetic forbidden import');raise exception 'Manager import allowed';exception when insufficient_privilege then null;end;
 perform set_config('request.jwt.claim.sub',hr_id::text,true);perform public.import_employees(batch,rows,'Synthetic rollback onboarding');
 assert (select count(*)=before_people+2 from public.employees),'Batch employee count incorrect';assert (select count(*)=before_contacts+2 from public.employee_contacts),'Batch contact count incorrect';assert (select count(*)=before_history+2 from public.employment_history),'Batch history is not once per employee';
 assert (select actor_id=hr_id and actor_role='hr_generalist' and row_count=2 and payload_sha256 ~ '^[0-9a-f]{64}$' from public.employee_import_batches where id=batch),'Protected actor/count/hash missing';
 assert (select count(*)=2 from public.employee_import_items where batch_id=batch),'Import source links missing';
 assert (select count(*)=2 from public.employees where employee_code in (code||'-1',code||'-2') and company_id=c and status='active' and id<>owner_id),'Forged identity/company/status accepted';
 assert not exists(select 1 from public.company_members m join public.employee_import_items i on i.employee_id=m.employee_id where i.batch_id=batch),'Import created sign-in accounts';
 perform public.import_employees(batch,rows,'Synthetic rollback onboarding');assert (select count(*)=before_people+2 from public.employees),'Retry duplicated employees';assert (select count(*)=before_history+2 from public.employment_history),'Retry duplicated employment history';
 begin perform public.import_employees(batch,rows,'Different details');raise exception 'Changed retry accepted';exception when sqlstate 'PT409' then null;end;
 bad:=jsonb_set(rows,'{0,employee,employee_code}',to_jsonb(code||'-3'));bad:=jsonb_set(bad,'{1,employee,employee_code}',to_jsonb(code||'-4'));bad:=jsonb_set(bad,'{1,contacts}',jsonb_build_object('phone','invalid phone'));
 begin perform public.import_employees(failed,bad,'Synthetic atomic rollback');raise exception 'Invalid contact imported';exception when check_violation then null;end;
 assert not exists(select 1 from public.employee_import_batches where id=failed),'Failed batch retained';assert not exists(select 1 from public.employees where employee_code in (code||'-3',code||'-4')),'Partial employee import saved';
 begin perform public.import_employees(gen_random_uuid(),rows,'Synthetic existing code');raise exception 'Existing employee silently updated';exception when sqlstate 'PT409' then null;end;
 begin delete from public.employee_import_items where batch_id=batch;raise exception 'Import history deletion allowed';exception when insufficient_privilege then null;end;
 begin update public.employee_import_batches set reason='forged' where id=batch;raise exception 'Import audit edit allowed';exception when insufficient_privilege then null;end;
 perform set_config('request.jwt.claim.sub',manager_id::text,true);assert not exists(select 1 from public.employee_import_batches),'Manager read HR import history';assert not exists(select 1 from public.employee_import_items),'Manager read import links';
 perform set_config('request.jwt.claim.sub',owner_id::text,true);perform public.save_company_account(hr_id,hr_email,'hr_generalist',false,null,null);
 perform set_config('request.jwt.claim.sub',hr_id::text,true);assert not exists(select 1 from public.employee_import_batches),'Disabled HR read import history';begin perform public.import_employees(gen_random_uuid(),rows,'Synthetic disabled import');raise exception 'Disabled HR imported';exception when insufficient_privilege then null;end;
 perform set_config('request.jwt.claim.sub','',true);execute 'set local role anon';begin perform public.import_employees(gen_random_uuid(),rows,'Synthetic anonymous import');raise exception 'Anonymous imported';exception when insufficient_privilege then null;end;
 execute 'reset role';assert not exists(select 1 from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='public' and p.prosecdef),'Public definer exposed';
end;$$;
rollback;
