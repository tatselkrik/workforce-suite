export async function runShiftTests({db,eq,deny,session,employee,company,owner,unassigned}) {
 const all=async table=>(await db.query(`select * from public.${table}`)).rows;
 const save=async t=>(await db.query('select public.save_shift_template($1::jsonb) id',[JSON.stringify(t)])).rows[0].id;
 const templates=async()=>all('shift_templates');
 const assignment=async p=>(await db.query('select public.assign_shift($1::jsonb) id',[JSON.stringify(p)])).rows[0].id;
 const day={id:null,expected_revision:null,name:'Synthetic day shift',days:[1,2,3,4,5,6,7],start_minute:480,end_minute:1020,works_on_holidays:false,active:true};
 await session(owner);const id=await save(day);let t=(await templates()).find(t=>t.id===id);
 eq([t.name,t.start_minute,t.end_minute,t.revision],[day.name,480,1020,0],'HR shift created with an explicit initial revision');
 eq((await all('shift_template_versions')).filter(v=>v.template_id===id).length,1,'Initial template version retains server actor and time');
 for(const change of [{start_minute:1440},{end_minute:480},{days:[]},{days:[1,1]},{days:[0]},{days:[1.5]},{name:' '},{name:'Invalid\nname'},{active:null},{works_on_holidays:'yes'},{start_minute:1.5}]) await deny('select public.save_shift_template($1::jsonb)',[JSON.stringify({...day,...change})],['23514']);
 await deny('select public.save_shift_template($1::jsonb)',[JSON.stringify({...day,name:day.name.toUpperCase()})],['23505']);
 const person=await employee(company.id,'SHIFT-QA-PERSON',15);
 const args={employee_id:person.id,template_id:id,expected_template_revision:0,expected_timezone:'Asia/Manila',effective_on:'2027-01-04',expected_sequence:0,reason:'Synthetic initial assignment'};
 const a=await assignment(args);let first=(await all('shift_assignments')).find(x=>x.id===a);
 eq([first.sequence,first.plan.template_revision,first.plan.timezone,first.actor_id],[1,0,'Asia/Manila',owner],'Server derives employee sequence, shift snapshot, time zone and actor');
 await deny('select public.assign_shift($1::jsonb)',[JSON.stringify(args)],['PT409']);
 for(const change of [{expected_template_revision:20},{expected_timezone:'UTC'}]) await deny('select public.assign_shift($1::jsonb)',[JSON.stringify({...args,expected_sequence:1,...change})],['PT409']);
 for(const change of [{reason:''},{reason:'bad\nreason'},{effective_on:'1800-01-01'},{effective_on:'2020-01-01'}]) await deny('select public.assign_shift($1::jsonb)',[JSON.stringify({...args,expected_sequence:1,...change})],['23514']);
 const renamed={...t,expected_revision:t.revision,name:'Synthetic renamed shift',start_minute:540};await save(renamed);
 eq((await all('shift_assignments')).find(x=>x.id===a).plan.name,day.name,'Template edits do not rewrite employee snapshots');
 await deny('select public.save_shift_template($1::jsonb)',[JSON.stringify(renamed)],['PT409']);
 eq((await all('shift_template_versions')).filter(v=>v.template_id===id).length,2,'Every template version is retained');
 await assignment({...args,expected_sequence:1,expected_template_revision:1,effective_on:'2027-02-01',reason:'Synthetic future schedule'});
 await assignment({...args,template_id:null,expected_template_revision:null,expected_sequence:2,effective_on:'2027-02-01',reason:'Synthetic return to company calendar'});
 const plan=async date=>(await db.query('select hris_private.shift_plan_at($1,$2,$3) plan',[company.id,person.id,date])).rows[0].plan;
 eq((await plan('2027-01-31')).start_minute,480,'Earlier dates retain the original shift');
 eq(await plan('2027-02-01'),null,'Latest same-date change can return to company calendar');
 eq((await all('shift_assignments')).filter(a=>a.employee_id===person.id).length,3,'Same-date replacement preserves every assignment event');
 const night=await save({...day,name:'Synthetic night shift',start_minute:1320,end_minute:360});
 const early=await save({...day,name:'Synthetic early shift',start_minute:240,end_minute:720});
 await assignment({...args,template_id:night,expected_sequence:3,effective_on:'2027-03-01'});
 await deny('select public.assign_shift($1::jsonb)',[JSON.stringify({...args,template_id:early,expected_sequence:4,effective_on:'2027-03-02'})],['23P01']);
 eq((await all('shift_assignments')).filter(a=>a.employee_id===person.id).length,4,'Overlapping transition rolls back its assignment event');
 const workdate=async at=>(await db.query('select hris_private.attendance_work_date_at($1,$2,$3)::text d',[company.id,person.id,at])).rows[0].d;
 eq(await workdate('2027-03-01T17:00:00Z'),'2027-03-01','After-midnight night check-in uses the starting date');
 eq(await workdate('2027-03-01T22:00:00Z'),'2027-03-02','End boundary is exclusive');
 const correction={attendance_id:null,employee_id:person.id,work_date:'2026-01-05',checked_in_at:'2026-01-05T01:00:00Z',checked_out_at:'2026-01-05T09:00:00Z',expected_revision:null,reason:'Synthetic shift snapshot check'};
 await assignment({...args,template_id:night,expected_sequence:4,effective_on:'2026-01-01'});
 // A direct HR correction can create a historical row. It records the server-owned plan.
 const record=(await db.query('select public.correct_attendance($1::jsonb) id',[JSON.stringify(correction)])).rows[0].id;
 let attendance=(await db.query('select * from public.attendance where id=$1',[record])).rows[0];
 eq([attendance.planned_shift.name,attendance.planned_shift.template_revision],['Synthetic night shift',0],'New correction retains the shift plan selected on the server');
 const overnightCorrection={...correction,work_date:'2026-01-04',checked_in_at:'2026-01-04T17:00:00Z',checked_out_at:'2026-01-04T22:00:00Z'};
 const nightRecord=(await db.query('select public.correct_attendance($1::jsonb) id',[JSON.stringify(overnightCorrection)])).rows[0].id;
 eq((await db.query('select work_date::text d from public.attendance where id=$1',[nightRecord])).rows[0].d,'2026-01-04','Guarded correction accepts after-midnight check-in on the saved overnight start date');
 await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify({...overnightCorrection,attendance_id:nightRecord,expected_revision:9})],['PT409']);
 await assignment({...args,template_id:null,expected_template_revision:null,expected_sequence:5,effective_on:'2026-01-01'});
 attendance=(await db.query('select * from public.attendance where id=$1',[record])).rows[0];
 eq(attendance.planned_shift.name,'Synthetic night shift','Later schedule corrections do not rewrite recorded attendance plan');
 await deny('update public.attendance set planned_shift=$1 where id=$2',[JSON.stringify({name:'Forged'}),record],['42501']);
 for(const table of ['shift_templates','shift_template_versions','shift_assignments']) {
  await deny(`delete from public.${table}`,[],['42501']);
  await deny(`update public.${table} set company_id=$1`,[company.id],['42501']);
 }
 const roleUsers={manager:'00000000-0000-0000-0000-000000009101',hr:'00000000-0000-0000-0000-000000009102',portal:'00000000-0000-0000-0000-000000009103'};const ids={};
 for(const [role,uid] of Object.entries(roleUsers)) {await db.exec('reset role');await db.query('insert into auth.users(id,email,email_confirmed_at) values($1,$2,now())',[uid,`shift-${role}@example.invalid`]);await session(owner);await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[uid,`shift-${role}@example.invalid`,role==='hr'?'hr_generalist':role==='portal'?'employee':'manager']);ids[role]=(await db.query('select employee_id from public.company_members where user_id=$1',[uid])).rows[0].employee_id;}
 await db.query('update public.employees set reports_to=$1 where id=$2',[ids.manager,ids.portal]);
 await assignment({...args,employee_id:ids.portal,expected_sequence:0,expected_template_revision:1,effective_on:'2027-01-04'});
 await session(roleUsers.manager);eq((await all('shift_assignments')).every(a=>a.employee_id===ids.manager||a.employee_id===ids.portal),true,'Manager sees only self and current direct-report assignments');
 eq((await all('shift_assignments')).length,1,'Manager can view assigned direct-report schedule');eq((await all('shift_template_versions')).length,0,'Manager cannot read HR template audit');
 for(const user of [roleUsers.manager,roleUsers.portal,unassigned]) {await session(user);await deny('select public.save_shift_template($1::jsonb)',[JSON.stringify({...day,name:'Forbidden'})],['42501']);await deny('select public.assign_shift($1::jsonb)',[JSON.stringify({...args,expected_sequence:6})],['42501']);}
 await session(roleUsers.portal);eq((await all('shift_assignments')).every(a=>a.employee_id===ids.portal),true,'Portal role has own schedule read scope only');
 await session(unassigned);eq((await all('shift_assignments')).length,0,'Unassigned account reads no schedules');eq((await all('shift_templates')).length,0,'Unassigned account reads no templates');
 await session('', 'anon');for(const table of ['shift_templates','shift_template_versions','shift_assignments'])await deny(`select * from public.${table}`,[],['42501']);await deny('select public.save_shift_template($1::jsonb)',[JSON.stringify(day)],['42501']);
 await session(roleUsers.hr);const hrShift=await save({...day,name:'Synthetic HR managed shift'});eq(Boolean(hrShift),true,'HR Generalist can create a shift');
 await session(owner);t=(await templates()).find(t=>t.id===id);await save({...t,expected_revision:t.revision,active:false});await deny('select public.assign_shift($1::jsonb)',[JSON.stringify({...args,expected_sequence:6,expected_template_revision:2,effective_on:'2027-04-01'})],['23514']);
 eq((await all('shift_assignments')).find(x=>x.id===a).plan.name,day.name,'Deactivation preserves existing assignment history');
 await db.query('select public.save_company_account($1,$2,$3,false,$4,null)',[roleUsers.hr,'shift-hr@example.invalid','hr_generalist',ids.hr]);
 await session(roleUsers.hr);eq((await all('shift_templates')).length,0,'Disabled HR immediately loses schedule read access');eq((await all('shift_assignments')).length,0,'Disabled HR immediately loses assignment scope');await deny('select public.save_shift_template($1::jsonb)',[JSON.stringify({...day,name:'Disabled write'})],['42501']);
 await session(owner);
 const routine=await employee(company.id,'SHIFT-ROUTINE-QA',15);
 const local=(await db.query("select (clock_timestamp() at time zone 'Asia/Manila')::date::text today,extract(hour from clock_timestamp() at time zone 'Asia/Manila')::int h")).rows[0];
 const start=((local.h+2)%24)*60, end=((local.h+1)%24)*60;
 const rt=await save({...day,name:'Synthetic clock-relative shift',start_minute:start,end_minute:end});
 await assignment({...args,employee_id:routine.id,template_id:rt,expected_template_revision:0,expected_sequence:0,effective_on:'2026-01-01'});
 const actual=(await db.query('insert into public.attendance(company_id,employee_id) values($1,$2) returning *',[company.id,routine.id])).rows[0];
 eq(actual.planned_shift.template_id,rt,'Routine attendance captures its server-selected plan');
 eq(Math.abs(new Date(actual.checked_in_at).getTime()-Date.now())<10000,true,'Scheduled routine check-in remains server timed');
 const expectedDay=(await db.query('select hris_private.attendance_work_date_at($1,$2,$3)::text d',[company.id,routine.id,actual.checked_in_at])).rows[0].d;
 eq(new Date(actual.work_date).toISOString().slice(0,10),expectedDay,'Real attendance trigger uses the same schedule-aware attribution helper');
 const closed=(await db.query("update public.attendance set checked_out_at=now() where id=$1 returning *",[actual.id])).rows[0];
 eq([closed.planned_shift,closed.revision],[actual.planned_shift,1],'Routine checkout retains the original plan and increments revision');

}
