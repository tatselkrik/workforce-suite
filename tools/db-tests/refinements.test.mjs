export async function runRefinementTests({db,eq,deny,session,employee,company,owner,unassigned}) {
  await session(owner);
  const ownerPerson=(await db.query('select employee_id from public.company_members where user_id=$1',[owner])).rows[0].employee_id;
  eq(typeof ownerPerson,'string','Bootstrap creates the owner employee automatically');
  const ownerRow=(await db.query('select joined_on,department,job_title from public.employees where id=$1',[ownerPerson])).rows[0];
  eq(ownerRow,{joined_on:null,department:'',job_title:''},'Automatic employees do not invent employment details');
  await deny('select hris_private.create_account_employee($1,$2)',[company.id,owner],['42501']);
  await db.exec('reset role');
  const account='00000000-0000-0000-0000-000000000201';
  await db.query("insert into auth.users(id,email,raw_user_meta_data) values($1,'auto-person@example.invalid',$2)",[account,{hris_profile:{full_name:'Synthetic Auto Employee'}}]);
  await session(owner);
  await db.query("select public.save_company_account($1,'auto-person@example.invalid','hr_generalist',false,null,null)",[account]);
  const person=(await db.query('select employee_id from public.company_members where user_id=$1',[account])).rows[0].employee_id;
  eq((await db.query('select full_name from public.employees where id=$1',[person])).rows[0].full_name,'Synthetic Auto Employee','Account creation automatically creates its named employee');
  await db.query("select public.save_company_account($1,'auto-person@example.invalid','hr_generalist',false,null,null)",[account]);
  eq((await db.query('select employee_id from public.company_members where user_id=$1',[account])).rows[0].employee_id,person,'Editing account access keeps its employee identity');
  await deny("select public.save_company_account($1,'auto-person@example.invalid','hr_generalist',false,$2,null)",[account,ownerPerson],['23514']);
  const manager=(await db.query("select employee_id from public.company_members where role='manager'")).rows[0].employee_id;
  const report=await employee(company.id,'REPORT-NO-LOGIN');
  await db.query('update public.employees set reports_to=$1 where id=$2',[manager,report.id]);
  await deny('update public.employees set reports_to=$1 where id=$1',[report.id],['23514']);
  await deny('update public.employees set reports_to=$1 where id=$2',[report.id,manager],['23514']);
  await deny("update public.employees set status='archived' where id=$1",[manager],['23514']);
  const draft={id:report.id,company_id:company.id,employee_code:report.employee_code,full_name:'Atomic updated employee',department:'',job_title:'',
    joined_on:null,annual_leave_days:15,status:'active',reports_to:manager};
  await deny('select public.save_employee_record($1,$2)',[draft,{emergency_name:'Incomplete emergency'}],['23514']);
  eq((await db.query('select full_name from public.employees where id=$1',[report.id])).rows[0].full_name,report.full_name,'Failed contact validation rolls back employment edits');
  await db.query('select public.save_employee_record($1,$2)',[draft,{phone:'000 000 0000'}]);
  eq((await db.query('select phone from public.employee_contacts where employee_id=$1',[report.id])).rows[0].phone,'000 000 0000','Unified save persists contacts with employment');
  const newId=(await db.query('select public.save_employee_record($1,$2) person',[{...draft,id:null,employee_code:'ATOMIC-NEW',reports_to:null},{address:'Synthetic address'}])).rows[0].person;
  eq((await db.query('select employee_id from public.employee_contacts where employee_id=$1',[newId])).rows.length,1,'Unified new employee save creates its contact record');
  await session('00000000-0000-0000-0000-000000000102');
  eq((await db.query('select id from public.employees where id=$1',[report.id])).rows.length,1,'Manager sees direct reports that have no login account');
  await deny('select public.save_employee_record($1,$2)',[draft,{}],['42501']);
  await deny('select * from public.desktop_releases',[],['42501']);
  eq((await db.query('select public.desktop_update() feed')).rows[0].feed.release,null,'Staff update feed starts empty');
  await db.exec('reset role');
  await db.query(`insert into public.desktop_releases(version,platform,requires_schema,envelope,enabled) values
    ('0.1.6','windows-x64','20000101000000','{"payload":"synthetic","signature":"synthetic"}',true),
    ('0.1.7','windows-x64','99990101000000','{"payload":"future","signature":"synthetic"}',true)`);
  await session(owner);
  eq((await db.query('select public.desktop_update() feed')).rows[0].feed.release.payload,'synthetic','Update feed withholds releases whose schema is not applied');
  await deny("update public.desktop_releases set enabled=true",[],['42501']);
  await deny("insert into storage.objects(bucket_id,name,owner_id) values('desktop-updates','forbidden.exe',$1)",[owner],['42501']);
  await db.exec('reset role');
  await db.query("insert into storage.objects(bucket_id,name,owner_id) values('desktop-updates','0.1.6/test.exe',$1)",[owner]);
  for(const roleUser of [owner,'00000000-0000-0000-0000-000000000101','00000000-0000-0000-0000-000000000102']) {
    await session(roleUser);eq((await db.query("select name from storage.objects where bucket_id='desktop-updates'")).rows.length,1,'Assigned desktop staff may download update objects');
  }
  for(const deniedUser of ['00000000-0000-0000-0000-000000000103',unassigned]) {
    await session(deniedUser);await deny('select public.desktop_update()',[],['42501']);
    eq((await db.query("select name from storage.objects where bucket_id='desktop-updates'")).rows.length,0,'Portal/unassigned users cannot download desktop updates');
  }
  await session('', 'anon');await deny('select public.desktop_update()',[],['42501']);
}
