export async function runHistoryTests({db,eq,deny,session,employee,company,owner,unassigned}) {
  await session(owner);
  const rows=async id=>(await db.query('select * from public.employment_history where employee_id=$1 order by sequence',[id])).rows;
  const person=await employee(company.id,'HISTORY-QA');let history=await rows(person.id);
  eq(history.length,1,'Employee INSERT has exactly one automatic event');
  eq(history[0].kind,'created','New employee event is distinct from migration starting snapshot');
  eq(history[0].actor_id,owner,'Actor comes from authenticated protected membership');
  eq(history[0].actor_role,'administrator','Actor role is protected membership data');
  eq(history[0].before_record,null,'Creation has no fabricated prior employment');
  eq(history[0].after_record.employee.full_name,person.full_name,'Created employee saved snapshot');
  eq(Object.keys(history[0].after_record.employee).sort(),['id','company_id','employee_code','full_name','department','job_title','joined_on','annual_leave_days','status','reports_to','employment_type_id','work_location_id'].sort(),'Only explicit employment fields are captured');
  await db.query("update public.employees set department='Changed department',job_title='Senior associate' where id=$1",[person.id]);
  history=await rows(person.id);eq(history.length,2,'One employee update creates one atomic event');
  eq(history[1].before_record.employee.department,person.department,'Before snapshot retains original department');
  eq(history[1].after_record.employee.department,'Changed department','After snapshot captures saved department');
  eq(history[1].after_record.employee.job_title,'Senior associate','Multiple changed fields share one event');
  eq(history[1].sequence>history[0].sequence,true,'Server sequence orders equal-time events');
  await db.query('update public.employees set full_name=full_name where id=$1',[person.id]);
  await db.query(`select public.save_employee_record(to_jsonb(e),'{"phone":"000 555 0111"}'::jsonb) from public.employees e where id=$1`,[person.id]);
  eq((await rows(person.id)).length,2,'No-op and contacts-only combined save create no employment noise');
  await deny(`select public.save_employee_record(to_jsonb(e)||'{"department":"Failed combined save"}'::jsonb,'{"emergency_name":"Incomplete"}'::jsonb) from public.employees e where id=$1`,[person.id],['23514']);
  eq((await rows(person.id)).length,2,'Invalid contact save rolls back employment and its event');
  eq((await db.query('select department from public.employees where id=$1',[person.id])).rows[0].department,'Changed department','Employee rollback accompanies history rollback');
  const option=(await db.query('select * from public.employment_types order by id limit 1')).rows[0];
  const location=(await db.query('select * from public.work_locations order by id limit 1')).rows[0];
  const supervisor=await employee(company.id,'HISTORY-MANAGER');
  await db.query('update public.employees set employment_type_id=$1,work_location_id=$2,reports_to=$3 where id=$4',[option.id,location.id,supervisor.id,person.id]);
  history=await rows(person.id);const linked=history.at(-1);
  eq(linked.after_record.employment_type_name,option.name,'Employment type label is copied at save time');
  eq(linked.after_record.work_location_name,location.name,'Location label is copied at save time');
  eq(linked.after_record.reports_to_name,supervisor.full_name,'Reporting manager name is copied at save time');
  await db.query(`select public.save_employment_option('employment_type',jsonb_build_object('id',id,'name','History renamed option','active',active,'expected_revision',revision)) from public.employment_types where id=$1`,[option.id]);
  await db.query("update public.employees set full_name='History renamed supervisor' where id=$1",[supervisor.id]);
  eq((await rows(person.id)).at(-1),linked,'Related renames cannot rewrite historical labels or create employee events');
  for(const sql of [
    "insert into public.employment_history(company_id,employee_id,kind,actor_name,after_record) values($1,$2,'created','Forged','{}')",
    "update public.employment_history set actor_name='Forged' where company_id=$1 and employee_id=$2",
    "delete from public.employment_history where company_id=$1 and employee_id=$2"
  ])await deny(sql,[company.id,person.id],['42501']);
  await deny('select hris_private.employment_state(e) from public.employees e where id=$1',[person.id],['42501']);
  await deny('select hris_private.record_employment_history()',[],['42501']);
  const beforeFailure=(await rows(person.id)).length;
  await deny(`select public.save_staff_account($1,'admin@example.invalid','employee',false,to_jsonb(e)||'{"department":"Failed role save"}'::jsonb,'{}'::jsonb)
    from public.employees e join public.company_members m on m.employee_id=e.id where m.user_id=$1`,[owner],['23514']);
  eq((await rows(person.id)).length,beforeFailure,'Failed unrelated staff operation adds no event');
  const own=(await db.query('select employee_id from public.company_members where user_id=$1',[owner])).rows[0].employee_id;
  const ownBefore=(await rows(own)).length;
  await deny(`select public.save_staff_account($1,'admin@example.invalid','employee',false,to_jsonb(e)||'{"department":"Failed role save"}'::jsonb,'{}'::jsonb) from public.employees e where id=$2`,[owner,own],['23514']);
  eq((await rows(own)).length,ownBefore,'Combined staff failure rolls back its own employee history');
  const hr='00000000-0000-0000-0000-000000000701';
  await db.exec('reset role');await db.query("insert into auth.users(id,email,email_confirmed_at) values($1,'history-hr@example.invalid',now())",[hr]);
  await session(owner);await db.query("select public.save_company_account($1,'history-hr@example.invalid','hr_generalist',true,null,null)",[hr]);
  await session(hr);await db.query("update public.employees set job_title='HR saved title' where id=$1",[person.id]);
  history=await rows(person.id);eq(history.at(-1).actor_id,hr,'HR save records the actual HR actor');
  eq(history.at(-1).actor_role,'hr_generalist','HR role is server-derived');
  eq(history.length,beforeFailure+1,'HR can read employment history');
  for(const actor of ['00000000-0000-0000-0000-000000000102','00000000-0000-0000-0000-000000000103',unassigned]) {
    await session(actor);eq((await db.query('select * from public.employment_history')).rows.length,0,'Manager/Employee/unassigned cannot read history, including own');
  }
  await session(owner);await db.query("select public.save_company_account($1,'history-hr@example.invalid','hr_generalist',false,null,null)",[hr]);
  await session(hr);eq((await db.query('select * from public.employment_history')).rows.length,0,'Disabled HR loses history immediately');
  await session('','anon');await deny('select * from public.employment_history',[],['42501']);
  await session(owner);await db.query("update public.employees set status='archived' where id=$1",[person.id]);
  history=await rows(person.id);eq(history.at(-1).after_record.employee.status,'archived','Archiving adds an event and retains the timeline');
  eq(history[0].kind,'created','Original creation remains after archiving');
  await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub','',false)");
  await db.query("update public.employees set job_title='Operator correction' where id=$1",[person.id]);
  const system=(await rows(person.id)).at(-1);eq([system.actor_id,system.actor_name,system.actor_role],[null,'System',null],'Trusted operator write without HR context is explicitly System');
  await deny("update public.employment_history set actor_name='Operator tamper' where id=$1",[system.id],['42501']);
  await deny('delete from public.employment_history where id=$1',[system.id],['42501']);
  eq((await db.query("select relrowsecurity from pg_class where oid='public.employment_history'::regclass")).rows[0].relrowsecurity,true,'Exposed history table enforces RLS');
  eq((await db.query("select prosecdef,proconfig from pg_proc where oid='hris_private.record_employment_history()'::regprocedure")).rows[0],{prosecdef:true,proconfig:['search_path=""']},'Narrow non-exposed trigger uses fixed empty search path');
  await session(owner);
}
