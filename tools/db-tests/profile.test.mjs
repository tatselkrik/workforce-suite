export async function runProfileTests({db,eq,deny,session,company,owner,unassigned}) {
  const manager='00000000-0000-0000-0000-000000000102',staff='00000000-0000-0000-0000-000000000103';
  await session(owner);
  const members=(await db.query('select user_id,employee_id from public.company_members')).rows;
  const own=user=>members.find(m=>m.user_id===user).employee_id;
  const supervisorName=(await db.query('select full_name from public.employees where id=$1',[own(owner)])).rows[0].full_name;
  await db.query('update public.employees set reports_to=$1 where id=$2',[own(owner),own(manager)]);
  await db.exec('reset role');await db.query('delete from public.employee_contacts where employee_id=$1',[own(manager)]);
  await session(manager);
  eq((await db.query('select public.my_hris_access() access')).rows[0].access.reports_to_name,supervisorName,'Profile receives supervisor name without unrelated employment access');
  eq((await db.query('select id from public.employees where id=$1',[own(owner)])).rows.length,0,'Supervisor name does not expose their employment record');
  await db.query("insert into public.employee_contacts(company_id,employee_id,phone) values($1,$2,'000 000 0000')",[company.id,own(manager)]);
  eq((await db.query("update public.employee_contacts set emergency_name='Synthetic Contact',emergency_relationship='Sibling',emergency_phone='000 000 0001' returning employee_id")).rows.map(r=>r.employee_id),[own(manager)],'Profile saves only own emergency details');
  eq((await db.query("update public.employee_contacts set address='Forbidden team change' where employee_id=$1 returning employee_id",[own(staff)])).rows.length,0,'Manager cannot update a report contact record');
  eq((await db.query("update public.employees set department='Forbidden' where id=$1 returning id",[own(manager)])).rows.length,0,'Manager cannot change own employment');
  await deny('update public.employee_contacts set employee_id=$1 where employee_id=$2',[own(staff),own(manager)],['42501']);
  await session(staff);
  eq((await db.query("update public.employee_contacts set address='Updated own fictional address' returning employee_id")).rows.map(r=>r.employee_id),[own(staff)],'Employee portal scope may update own contacts');
  await deny("update public.employee_contacts set emergency_name='Incomplete contact' where employee_id=$1",[own(staff)],['23514']);

  await db.exec('reset role');const account='00000000-0000-0000-0000-000000000221';
  await db.query("insert into auth.users(id,email) values($1,'combined-staff@example.invalid')",[account]);
  await session(owner);
  const person={company_id:company.id,employee_code:'COMBINED-STAFF',full_name:'Synthetic Combined Staff',department:'QA',job_title:'Test',joined_on:null,annual_leave_days:15,status:'active',reports_to:null};
  const contacts={phone:'000 000 0000',address:'Fictional combined address'};
  const saved=(await db.query("select public.save_staff_account($1,'combined-staff@example.invalid','hr_generalist',false,$2,$3) id",[account,person,contacts])).rows[0].id;
  eq((await db.query('select employee_id from public.company_members where user_id=$1',[account])).rows[0].employee_id,saved,'Combined account uses exactly the saved employee identity');
  eq((await db.query('select phone from public.employee_contacts where employee_id=$1',[saved])).rows[0].phone,contacts.phone,'Combined staff contacts saved');
  eq((await db.query("select count(*)::int n from public.employees where employee_code='COMBINED-STAFF'")).rows[0].n,1,'Staff provisioning does not create a second employee');
  await deny("select public.save_staff_account($1,'combined-staff@example.invalid','invalid-role',false,$2,$3)",[account,{...person,id:saved,full_name:'Must roll back'},{phone:'000 000 0002'}],['23514']);
  eq((await db.query('select full_name from public.employees where id=$1',[saved])).rows[0].full_name,person.full_name,'Invalid staff role rolls back employment changes');
  eq((await db.query('select phone from public.employee_contacts where employee_id=$1',[saved])).rows[0].phone,contacts.phone,'Invalid staff role rolls back contacts');
  await deny("select public.save_staff_account($1,'combined-staff@example.invalid','hr_generalist',false,$2,$3)",[account,{...person,id:own(manager)},contacts],['23514']);
  await session(manager);await deny("select public.save_staff_account($1,'combined-staff@example.invalid','hr_generalist',false,$2,$3)",[account,{...person,id:saved},contacts],['42501']);

  await session(owner);
  await db.exec('reset role');
  const bucket=(await db.query("select public,file_size_limit,allowed_mime_types from storage.buckets where id='company-branding'")).rows[0];
  eq(bucket,{public:false,file_size_limit:2097152,allowed_mime_types:['image/png']},'Company logo bucket is private and bounded to normalized PNGs');
  await session(owner);
  const path=company.id+'/logo.png';
  await db.query("insert into storage.objects(bucket_id,name,owner_id) values('company-branding',$1,$2)",[path,owner]);
  const revision=(await db.query('select public.set_company_logo(true) revision')).rows[0].revision;
  eq(typeof revision,'string','Administrator can publish company logo revision');
  await db.query("update storage.objects set metadata='{}' where bucket_id='company-branding' and name=$1",[path]);
  for(const user of [owner,manager,staff]) {
    await session(user);eq((await db.query("select name from storage.objects where bucket_id='company-branding'")).rows.map(r=>r.name),[path],'Assigned employee can read company logo');
  }
  for(const user of [manager,staff,unassigned]) {
    await session(user);await deny('select public.set_company_logo(true)',[],['42501']);
    eq((await db.query("update storage.objects set metadata='{}' where bucket_id='company-branding' returning name")).rows.length,0,'Nonadministrators cannot replace company logo');
    eq((await db.query("delete from storage.objects where bucket_id='company-branding' returning name")).rows.length,0,'Nonadministrators cannot remove company logo');
    await deny("insert into storage.objects(bucket_id,name,owner_id) values('company-branding',$1,$2)",[path,user],['42501']);
  }
  await session(unassigned);eq((await db.query("select name from storage.objects where bucket_id='company-branding'")).rows.length,0,'Unassigned account cannot read company logo');
  await session(owner);await deny("insert into storage.objects(bucket_id,name,owner_id) values('company-branding','foreign-company/logo.png',$1)",[owner],['42501']);
  await db.query('select public.set_company_logo(false)');
  eq((await db.query('select logo_version from public.companies where id=$1',[company.id])).rows[0].logo_version,null,'Removing logo restores initials');
  await session('', 'anon');eq((await db.query("select name from storage.objects where bucket_id='company-branding'")).rows.length,0,'Anonymous logo reads denied');
}
