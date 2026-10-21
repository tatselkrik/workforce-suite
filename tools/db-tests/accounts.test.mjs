export async function runAccessTests({db,eq,deny,session,employee,request,company,owner,unassigned}) {
  const hr='00000000-0000-0000-0000-000000000101';
  const manager='00000000-0000-0000-0000-000000000102';
  const staff='00000000-0000-0000-0000-000000000103';
  const peer='00000000-0000-0000-0000-000000000104';
  const admin2='00000000-0000-0000-0000-000000000105';
  const emails=new Map([[owner,'admin@example.invalid'],[hr,'hr@example.invalid'],[manager,'manager@example.invalid'],
    [staff,'staff@example.invalid'],[peer,'peer@example.invalid'],[admin2,'admin2@example.invalid']]);
  await db.exec('reset role');
  for(const id of [hr,manager,staff,peer,admin2]) {
    await db.query('insert into auth.users(id,email,email_confirmed_at) values($1,$2,now())',[id,emails.get(id)]);
  }
  const save=(id,role,person=null,boss=null,active=true)=>db.query(
    'select public.save_company_account($1,$2,$3,$4,$5,$6)',[id,emails.get(id),role,active,person,boss]);
  await session(owner);
  const hrPerson=await employee(company.id,'HR-ROLE');
  const managerPerson=await employee(company.id,'MANAGER-ROLE');
  const staffPerson=await employee(company.id,'STAFF-ROLE');
  const peerPerson=await employee(company.id,'PEER-ROLE');
  await save(hr,'hr_generalist',hrPerson.id);
  await save(manager,'manager',managerPerson.id);
  await save(staff,'employee',staffPerson.id,manager);
  await save(peer,'employee',peerPerson.id);
  await save(admin2,'administrator');
  eq((await db.query('select public.list_company_accounts() as accounts')).rows[0].accounts.length,6,'Administrator lists all accounts');
  eq((await db.query('select count(*)::int n from public.account_access_events')).rows[0].n,5,'Each grant creates an access audit event');
  await deny('update public.company_members set role=$1 where user_id=$2',['administrator',staff],['42501']);
  await deny('delete from public.account_access_events',[],['42501']);
  await deny('insert into public.company_members(company_id,user_id,role) values($1,$2,$3)',[company.id,unassigned,'administrator'],['42501']);
  await deny('select public.save_company_account($1,$2,$3,true,null,null)',[owner,emails.get(owner),'employee'],['23514']);
  await deny('select public.save_company_account($1,$2,$3,false,null,null)',[owner,emails.get(owner),'administrator'],['23514']);
  await deny('select public.save_company_account(null,$1,$2,true,null,null)',[emails.get(hr),'hr_generalist'],['23505']);
  await deny('select public.save_company_account($1,$2,$3,true,null,null)',[staff,emails.get(staff),'employee'],['23514']);
  await deny('select public.save_company_account($1,$2,$3,true,$4,null)',[peer,emails.get(peer),'employee',staffPerson.id],['23505']);
  await deny('select public.save_company_account($1,$2,$3,true,$4,$1)',[manager,emails.get(manager),'manager',managerPerson.id],['23514']);
  await deny('select public.save_company_account($1,$2,$3,true,$4,$5)',[peer,emails.get(peer),'employee',peerPerson.id,staff],['23514']);
  await save(manager,'manager',managerPerson.id,hr);
  await deny('select public.save_company_account($1,$2,$3,true,$4,$5)',[hr,emails.get(hr),'hr_generalist',hrPerson.id,manager],['23514']);
  await save(manager,'manager',managerPerson.id);
  await deny('select public.save_company_account($1,$2,$3,true,$4,null)',[manager,emails.get(manager),'employee',managerPerson.id],['23514']);

  await session(hr);
  eq((await db.query('select public.my_hris_access() a')).rows[0].a.role,'hr_generalist','HR role comes from protected membership');
  eq((await db.query('select * from public.employees')).rows.length>=4,true,'HR sees company employee records');
  await deny('select public.list_company_accounts()',[],['42501']);
  await deny('select hris_private.list_company_accounts()',[],['42501']);
  await deny('select public.save_company_account($1,$2,$3,true,null,null)',[unassigned,'unassigned@example.invalid','administrator'],['42501']);
  eq((await db.query("update public.companies set name='Forbidden' returning id")).rows.length,0,'HR cannot change company settings');
  await deny("update public.employees set status='archived' where id=$1",[staffPerson.id],['23514']);
  const hrEdit=(await db.query("update public.employees set job_title='QA role title' where id=$1 returning id",[staffPerson.id])).rows;
  eq(hrEdit.length,1,'HR can edit a different employee');
  const staffLeave=await request(company.id,staffPerson.id,'2028-02-07','2028-02-08');
  const ownHrLeave=await request(company.id,hrPerson.id,'2028-02-07','2028-02-08');
  eq((await db.query("update public.leave_requests set status='approved' where id=$1 returning id",[ownHrLeave.id])).rows.length,0,'HR cannot approve own leave');
  eq((await db.query('select * from public.account_access_events')).rows.length,0,'HR cannot read account access audit');

  await session(manager);
  const access=(await db.query('select public.my_hris_access() a')).rows[0].a;
  eq(access.direct_reports,[staffPerson.id],'Manager receives only assigned direct report');
  eq((await db.query('select id from public.employees order by employee_code')).rows.map(r=>r.id).sort(),[managerPerson.id,staffPerson.id].sort(),'Manager sees own record and direct report only');
  await deny('select public.list_company_accounts()',[],['42501']);
  eq((await db.query("update public.employees set full_name='Forbidden' where id=$1 returning id",[staffPerson.id])).rows.length,0,'Manager cannot edit employee records');
  await deny('insert into public.attendance(company_id,employee_id) values($1,$2)',[company.id,staffPerson.id],['42501']);
  const managerIn=(await db.query('insert into public.attendance(company_id,employee_id) values($1,$2) returning *',[company.id,managerPerson.id])).rows[0];
  eq(managerIn.employee_id,managerPerson.id,'Manager can record own attendance');
  eq((await db.query("update public.leave_requests set status='approved' where id=$1 returning id",[staffLeave.id])).rows.length,1,'Manager can approve direct-report leave without employee UPDATE rights');
  const managerLeave=await request(company.id,managerPerson.id,'2028-03-06','2028-03-06');
  eq((await db.query("update public.leave_requests set status='approved' where id=$1 returning id",[managerLeave.id])).rows.length,0,'Manager cannot approve own leave');

  await session(staff);
  await db.query("select set_config('request.jwt.claims',$1,false)",[JSON.stringify({user_metadata:{role:'administrator'},app_metadata:{role:'administrator'}})]);
  eq((await db.query('select public.my_hris_access() a')).rows[0].a.role,'employee','Forged JWT metadata cannot elevate database role');
  eq((await db.query('select id from public.employees')).rows.map(r=>r.id),[staffPerson.id],'Employee sees only own record');
  eq((await db.query('select * from public.company_members')).rows.length,1,'Employee reads only own membership');
  await deny('select public.list_company_accounts()',[],['42501']);
  eq((await db.query("update public.employees set annual_leave_days=100 where id=$1 returning id",[staffPerson.id])).rows.length,0,'Employee cannot change own allowance');
  eq((await db.query("update public.companies set annual_leave_days=100 returning id")).rows.length,0,'Employee cannot change company defaults');
  await deny('insert into public.attendance(company_id,employee_id) values($1,$2)',[company.id,peerPerson.id],['42501','23514']);
  const ownAttendance=(await db.query('insert into public.attendance(company_id,employee_id) values($1,$2) returning *',[company.id,staffPerson.id])).rows[0];
  eq((await db.query('update public.attendance set checked_out_at=now() where id=$1 returning id',[ownAttendance.id])).rows.length,1,'Employee can close own attendance');
  eq((await db.query('update public.attendance set checked_out_at=now() where id=$1 returning id',[managerIn.id])).rows.length,0,'Employee cannot close another attendance');
  const ownLeave=await request(company.id,staffPerson.id,'2028-04-03','2028-04-03');
  eq(ownLeave.status,'pending','Employee requests own pending leave');
  eq((await db.query("update public.leave_requests set status='approved' where id=$1 returning id",[ownLeave.id])).rows.length,0,'Employee cannot approve leave');
  await deny("insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on) values($1,$2,'sick','2028-04-03','2028-04-03')",[company.id,peerPerson.id],['42501','23514']);
  const photo=staff+'/avatar.png';
  await db.query("insert into storage.objects(bucket_id,name,owner_id) values('profile-photos',$1,$2)",[photo,staff]);
  eq((await db.query('select name from storage.objects')).rows.map(r=>r.name),[photo],'Employee profile photo remains private and usable');
  await deny("insert into storage.objects(bucket_id,name,owner_id) values('profile-photos',$1,$2)",[manager+'/avatar.png',staff],['42501']);

  await session(owner);
  await save(staff,'employee',staffPerson.id,manager,false);
  await session(staff);
  await deny('select public.my_hris_access()',[],['42501']);
  eq((await db.query('select * from public.companies')).rows.length,0,'Disabled membership loses company access with existing JWT');
  eq((await db.query('select * from public.employees')).rows.length,0,'Disabled account loses HR records');
  eq((await db.query('select * from storage.objects')).rows.length,0,'Disabled account loses photo access');
  await deny('insert into public.attendance(company_id,employee_id) values($1,$2)',[company.id,staffPerson.id],['42501','23514']);
  await session(owner);
  await save(staff,'employee',staffPerson.id,manager,true);
  await session(staff);
  eq((await db.query('select public.my_hris_access() a')).rows[0].a.role,'employee','Administrator can reactivate account');

  await session(unassigned);
  await deny('select public.my_hris_access()',[],['42501']);
  await deny('select public.list_company_accounts()',[],['42501']);
  await session('', 'anon');
  for (const rpc of ['my_hris_access','list_company_accounts']) await deny(`select public.${rpc}()`,[],['42501']);
  for (const table of ['company_members','account_access_events']) await deny(`select * from public.${table}`,[],['42501']);
}
