export async function runCorrectionTests({db,eq,deny,session,employee,company,owner,unassigned}) {
  await session(owner);
  const person=await employee(company.id,'CORRECTION-QA');
  await db.query('update public.employees set joined_on=null where id=$1',[person.id]);
  const base=(await db.query(`select ((clock_timestamp() at time zone $1)::date-3)::text as work_day`,[company.timezone])).rows[0].work_day;
  const utc=async(day,time)=>(await db.query('select ($1::date+$2::time) at time zone $3 as t',[day,time,company.timezone])).rows[0].t.toISOString();
  const change={employee_id:person.id,work_date:base,checked_in_at:await utc(base,'08:00'),checked_out_at:await utc(base,'17:00'),reason:'QA: missed check-in verified against attendance log'};
  const save=async(c)=>(await db.query('select public.correct_attendance($1::jsonb) id',[JSON.stringify(c)])).rows[0].id;
  await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify({...change,reason:'   '})],['23514']);
  for(const reason of ['a'.repeat(501),'line\nbreak','tab\tcontrol']) {
    await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify({...change,reason})],['23514']);
  }
  const future=(await db.query("select (current_date+3)::text as work_day")).rows[0].work_day;
  for(const c of [
    {...change,work_date:future,checked_in_at:await utc(future,'08:00'),checked_out_at:null},
    {...change,checked_out_at:await utc(base,'07:00')},
    {...change,work_date:null},
    {...change,checked_in_at:null},
    {...change,checked_in_at:'infinity'},
    {...change,employee_id:'00000000-0000-0000-0000-000000000999'},
  ]) await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify(c)],['23514','42501']);
  const id=await save(change);
  const row=async()=>(await db.query('select * from public.attendance where id=$1',[id])).rows[0];
  eq((await row()).work_date.toISOString().slice(0,10),base,'Historical entry keeps company work date');
  eq((await row()).checked_in_at.toISOString(),change.checked_in_at,'Historical check-in keeps approved custom time');
  eq((await row()).revision,0,'New attendance starts at revision zero');
  const initial=(await db.query('select * from public.attendance_corrections where attendance_id=$1',[id])).rows[0];
  eq(initial.before_record,null,'New historical entry records missing previous state');
  eq(initial.actor_id,owner,'Audit actor comes from the current session');
  eq(initial.reason,change.reason,'Required correction reason persists');
  eq(Math.abs(Date.now()-initial.created_at.getTime())<10000,true,'Correction audit uses server time');
  await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify(change)],['23505']);
  const edit={...change,attendance_id:id,expected_revision:0,checked_in_at:await utc(base,'08:15'),reason:'QA: corrected arrival after supervisor verification'};
  await save(edit);
  eq((await row()).revision,1,'Correcting existing attendance increments revision');
  const events=(await db.query('select * from public.attendance_corrections where attendance_id=$1 order by created_at',[id])).rows;
  eq(events.length,2,'Correction creates a second immutable event');
  eq(new Date(events[1].before_record.checked_in_at).toISOString(),change.checked_in_at,'Audit preserves previous check-in');
  eq(new Date(events[1].after_record.checked_in_at).toISOString(),edit.checked_in_at,'Audit preserves corrected check-in');
  await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify({...edit,checked_in_at:change.checked_in_at})],['PT409']);
  await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify({...edit,expected_revision:1})],['23514']);
  await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify({...edit,expected_revision:null})],['PT409']);
  await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify({...edit,employee_id:initial.actor_id,expected_revision:1})],['42501','23514']);
  for (const sql of ['update public.attendance_corrections set reason=\'forged\'','delete from public.attendance_corrections',
    'insert into public.attendance_corrections default values','insert into hris_private.attendance_correction_context values(txid_current(),gen_random_uuid(),auth.uid())',
    'update public.attendance set revision=99 where id=$1','update public.attendance set checked_in_at=now() where id=$1']) {
    await deny(sql,sql.includes('$1')?[id]:[],['42501']);
  }
  eq((await db.query('select hris_private.is_attendance_correction($1) yes',[id])).rows[0].yes,false,'Correction context is removed after success');
  await db.query("select set_config('hris.attendance_correction',$1,false)",[id]);
  await deny('update public.attendance set checked_out_at=null where id=$1',[id],['23514']);
  await db.exec('reset role');
  eq((await db.query('select count(*)::int n from hris_private.attendance_correction_context')).rows[0].n,0,'Failed/successful corrections leave no privileged context');
  const hr='00000000-0000-0000-0000-000000000501', manager='00000000-0000-0000-0000-000000000502';
  await db.query(`insert into auth.users(id,email,email_confirmed_at) values($1,'correction-hr@example.invalid',now()),($2,'correction-manager@example.invalid',now())`,[hr,manager]);
  await session(owner);
  await db.query(`select public.save_company_account($1,'correction-hr@example.invalid','hr_generalist',true,null,null)`,[hr]);
  await db.query(`select public.save_company_account($1,'correction-manager@example.invalid','manager',true,null,null)`,[manager]);
  await session(hr);
  await save({...edit,expected_revision:1,checked_in_at:await utc(base,'08:30'),reason:'QA: HR correction'});
  eq((await row()).revision,2,'HR Generalist can correct attendance');
  eq((await db.query('select actor_id from public.attendance_corrections where attendance_id=$1 order by created_at desc limit 1',[id])).rows[0].actor_id,hr,'HR audit retains actual actor');
  for(const uid of [manager,unassigned]) {
    await session(uid);
    await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify({...edit,expected_revision:2})],['42501']);
    eq((await db.query('select * from public.attendance_corrections')).rows.length,0,'Manager/unassigned account cannot read private correction reasons');
  }
  await session('', 'anon');
  await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify(change)],['42501']);
  await deny('select * from public.attendance_corrections',[],['42501']);
  await session(owner);
  await db.query(`select public.save_company_account($1,'correction-hr@example.invalid','hr_generalist',false,null,null)`,[hr]);
  await session(hr);
  await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify(edit)],['42501']);
  await session(owner);
  const beforeCount=(await db.query('select count(*)::int n from public.attendance_corrections')).rows[0].n;
  await db.query('update public.employees set joined_on=current_date where id=$1',[person.id]);
  await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify({...edit,expected_revision:2})],['23514']);
  await db.query("update public.employees set joined_on=null,status='archived' where id=$1",[person.id]);
  await save({...edit,expected_revision:2,checked_in_at:await utc(base,'09:00'),reason:'QA: historical archived record correction'});
  eq((await row()).revision,3,'Archived history remains correctable by HR');
  eq((await db.query('select count(*)::int n from public.attendance_corrections')).rows[0].n,beforeCount+1,'Failed correction appends no audit event');
  // Overnight intervals can touch, but never overlap, another work-date record.
  const p2=await employee(company.id,'CORRECTION-OVERNIGHT');
  await db.query('update public.employees set joined_on=null where id=$1',[p2.id]);
  const next=(await db.query('select ($1::date+1)::text as work_day',[base])).rows[0].work_day;
  await save({...change,employee_id:p2.id,checked_in_at:await utc(base,'22:00'),checked_out_at:await utc(next,'06:00')});
  const overlap={...change,employee_id:p2.id,work_date:next,checked_in_at:await utc(next,'05:00'),checked_out_at:await utc(next,'14:00')};
  await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify(overlap)],['23P01']);
  await save({...overlap,checked_in_at:await utc(next,'06:00')});
  eq((await db.query('select count(*)::int n from public.attendance where employee_id=$1',[p2.id])).rows[0].n,2,'Adjacent overnight shift is allowed');
  const p3=await employee(company.id,'CORRECTION-OPEN');
  await db.query('update public.employees set joined_on=null where id=$1',[p3.id]);
  const openId=await save({...change,employee_id:p3.id,checked_out_at:null});
  await deny('insert into public.attendance(company_id,employee_id) values($1,$2)',[company.id,p3.id],['23505','23P01']);
  await db.query('update public.attendance set checked_out_at=now() where id=$1',[openId]);
  await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify({...change,employee_id:p3.id,attendance_id:openId,expected_revision:0})],['PT409']);
  eq((await db.query('select revision from public.attendance where id=$1',[openId])).rows[0].revision,1,'Routine server checkout also invalidates stale correction drafts');
}
