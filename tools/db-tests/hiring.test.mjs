export async function runHiringTests({db,eq,deny,session,company,owner,unassigned}) {
 const id='00000000-0000-0000-0000-000000096100';
 const base={id,full_name:'Synthetic Applicant',email:'candidate@example.invalid',phone:'',position:'Operations Associate',source:'Referral',applied_on:'2026-01-01',notes:'Private HR notes\nSecond line',reason:'Application recorded'};
 const call=async(op,p,c=company.id)=>(await db.query('select public.manage_applicant($1,$2,$3::jsonb) result',[c,op,JSON.stringify(p)])).rows[0].result;
 const no=(op,p,codes=['23514'],c=company.id)=>deny('select public.manage_applicant($1,$2,$3::jsonb)',[c,op,JSON.stringify(p)],codes);
 const counts=async()=> (await db.query('select (select count(*)::int from public.employees) people,(select count(*)::int from auth.users) accounts')).rows[0];
 await db.exec('reset role');const before=await counts();
 await session(owner);
 eq((await db.query('select * from public.applicants')).rows,[],'Migration creates no invented applicants');
 let r=await call('create',base);
 eq([r.stage,r.archived,r.revision],['applied',false,0],'New application starts Applied');
 eq([r.created_by,r.company_id],[owner,company.id],'Protected identity derived from membership');
 let events=(await db.query('select * from public.applicant_events order by revision')).rows;
 eq(events.length,1,'One create event');eq(events[0].before_record,null,'Creation has no invented before record');
 eq([events[0].actor_id,events[0].actor_role],[owner,'administrator'],'Server snapshots protected actor');
 eq(events[0].after_record,r,'History is actual saved record');
 eq((await call('create',base)).revision,0,'Identical creation retry is idempotent');
 await no('create',{...base,full_name:'Changed retry'},['23505']);
 await no('create',{...base,reason:'Changed reason'},['23505']);
 await no('create',{...base,expected_revision:0});
 r=await call('edit',{...base,expected_revision:0,phone:'555-0100',reason:'Contact corrected'});
 eq([r.phone,r.revision],['555-0100',1],'Edit saves known fields with next revision');
 eq((await call('create',base)).phone,'555-0100','Delayed creation retry returns current record without overwriting');
 eq((await db.query('select count(*)::int n from public.applicant_events')).rows[0].n,2,'Replay adds no history event');
 await no('edit',{...base,expected_revision:0,reason:'Stale editor'},['PT409']);
 await no('stage',{id,expected_revision:0,stage:'screening',reason:'Stale stage'},['PT409']);
 await no('archive',{id,expected_revision:0,reason:'Stale archive'},['PT409']);
 await no('stage',{id,expected_revision:1,stage:'offer',reason:'Skip review'});
 await no('stage',{id,expected_revision:1,stage:'hired',reason:'Must not convert to employee'});
 let revision=1;
 for(const stage of ['screening','interview','offer','rejected','applied','withdrawn','applied']) {
  r=await call('stage',{id,expected_revision:revision,stage,reason:`Reviewed for ${stage}`});revision++;
  eq([r.stage,r.revision],[stage,revision],`Allowed transition to ${stage}`);
 }
 await no('stage',{id,expected_revision:revision,stage:'applied',reason:'No change'});
 r=await call('archive',{id,expected_revision:revision,reason:'Retained for history'});revision++;
 eq([r.archived,r.revision],[true,revision],'Archive preserves row');
 await no('edit',{...base,expected_revision:revision});
 await no('stage',{id,expected_revision:revision,stage:'screening',reason:'Archived'});
 await no('archive',{id,expected_revision:revision,reason:'Already archived'});
 r=await call('restore',{id,expected_revision:revision,reason:'Resume review'});revision++;
 eq([r.archived,r.revision],[false,revision],'Restore retains full history');
 await no('restore',{id,expected_revision:revision,reason:'Already active'});
 events=(await db.query('select * from public.applicant_events order by revision')).rows;
 eq(events.length,revision+1,'Exactly one immutable event per revision');
 for(let i=1;i<events.length;i++) eq(events[i].before_record,events[i-1].after_record,'Before/after history connects');
 eq(events.at(-1).kind,'restored','Restore event retained');
 const savedBefore=JSON.stringify(r), eventCount=events.length;
 const newId='00000000-0000-0000-0000-000000096101';
 for(const patch of [{id:null},{id:'bad'},{id:'00000000-0000-0000-0000-000000000000'},
  {full_name:''},{full_name:'x'.repeat(121)},{full_name:'Bad\tname'},{full_name:1},
  {email:'missing.at'},{email:'a@b.'},{email:'a@.b'},{email:'a@@b.c'},{email:'a @b.c'},{email:'x'.repeat(250)+'@b.co'},
  {phone:'x'.repeat(41)},{phone:'bad\tphone'},{position:''},{position:'x'.repeat(101)},
  {source:'x'.repeat(81)},{notes:'x'.repeat(2001)},{notes:'bad\rnotes'},
  {applied_on:'2099-01-01'},{applied_on:'1/1/2026'},{applied_on:null},
  {reason:''},{reason:'x'.repeat(501)},{reason:'bad\nreason'},{reason:1},
  {company_id:company.id},{stage:'offer'},{archived:true},{revision:99},{actor_id:owner},{employee_id:owner},{created_by:owner}]) {
  await no('create',{...base,id:newId,...patch});
 }
 await no('create',{...base,id:newId,applied_on:'2026-02-31'},['22008']);
 for(const p of [null,[],{},'text']) await no('create',p);
 await no('unknown',base);await no(null,base);
 for(const expected_revision of [null,-1,'1',1.5]) await no('stage',{id,expected_revision,stage:'screening',reason:'Invalid revision'});
 for(const stage of [null,99,'unknown']) await no('stage',{id,expected_revision:revision,stage,reason:'Invalid stage'});
 eq(JSON.stringify((await db.query('select to_jsonb(a) r from public.applicants a where id=$1',[id])).rows[0].r),savedBefore,'All rejected writes preserve applicant');
 eq((await db.query('select count(*)::int n from public.applicant_events')).rows[0].n,eventCount,'Rejected writes create no audit noise');
 await deny('insert into public.applicants(id,company_id,full_name,position,applied_on,created_by) values($1,$2,\'Injected\',\'Injected\',current_date,$3)',[newId,company.id,owner],['42501']);
 await deny('update public.applicants set full_name=\'Injected\' where id=$1',[id],['42501']);
 await deny('delete from public.applicants where id=$1',[id],['42501']);
 await deny('update public.applicant_events set reason=\'Injected\' where applicant_id=$1',[id],['42501']);
 await deny('delete from public.applicant_events where applicant_id=$1',[id],['42501']);
 // A late history failure must roll back the applicant mutation too.
 await db.exec('reset role');
 await db.exec(`create function public.synthetic_history_failure() returns trigger language plpgsql as $$begin raise exception 'Synthetic audit failure' using errcode='23514';end;$$;
  create trigger synthetic_history_failure before insert on public.applicant_events for each row execute function public.synthetic_history_failure();`);
 await session(owner);await no('stage',{id,expected_revision:revision,stage:'screening',reason:'Atomic save test'});
 eq((await db.query('select revision from public.applicants where id=$1',[id])).rows[0].revision,revision,'Audit failure rolls back applicant update');
 await db.exec('reset role');await db.exec('drop trigger synthetic_history_failure on public.applicant_events;drop function public.synthetic_history_failure();');
 await deny('update public.applicant_events set reason=\'Privileged rewrite\' where applicant_id=$1',[id],['42501']);
 await deny('delete from public.applicant_events where applicant_id=$1',[id],['42501']);
 eq(await counts(),before,'Applicant work creates neither employees nor Auth accounts');
 const users={hr:'00000000-0000-0000-0000-000000096001',manager:'00000000-0000-0000-0000-000000096002',employee:'00000000-0000-0000-0000-000000096003'};
 for(const [role,uid] of Object.entries(users)) await db.query('insert into auth.users(id,email,email_confirmed_at) values($1,$2,now())',[uid,`hiring-${role}@example.invalid`]);
 await session(owner);
 for(const [role,uid] of Object.entries(users)) await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[uid,`hiring-${role}@example.invalid`,role==='hr'?'hr_generalist':role]);
 await session(users.hr);
 eq((await db.query('select count(*)::int n from public.applicants')).rows[0].n,1,'HR can read applicants');
 eq((await db.query('select count(*)::int n from public.applicant_events')).rows[0].n,eventCount,'HR can read history');
 await no('create',base,['23505']);
 r=await call('stage',{id,expected_revision:revision,stage:'screening',reason:'HR review'});revision++;
 eq((await db.query('select actor_role from public.applicant_events where revision=$1',[revision])).rows[0].actor_role,'hr_generalist','HR event gets current protected role');
 const foreignCompany='00000000-0000-0000-0000-000000096999';
 await no('create',{...base,id:newId},['42501'],foreignCompany);
 for(const who of ['manager','employee']) {
  await session(users[who]);eq((await db.query('select * from public.applicants')).rows,[],`${who} cannot read applicants`);
  eq((await db.query('select * from public.applicant_events')).rows,[],`${who} cannot read history`);
  await no('create',{...base,id:newId},['42501']);await no('stage',{id,expected_revision:revision,stage:'interview',reason:'Unauthorized'},['42501']);
 }
 await db.exec('reset role');await db.query("update auth.users set raw_user_meta_data='{"+'"role":"administrator"'+"}'::jsonb where id=$1",[users.employee]);
 await session(users.employee);await no('create',{...base,id:newId},['42501']);
 await session(owner);await db.query('select public.save_company_account($1,$2,$3,false,null,null)',[users.hr,'hiring-hr@example.invalid','hr_generalist']);
 await session(users.hr);eq((await db.query('select * from public.applicants')).rows,[],'Disabled HR immediately loses read access');await no('create',{...base,id:newId},['42501']);
 await session(unassigned);await no('create',{...base,id:newId},['42501']);
 await session('','anon');await deny('select * from public.applicants',[],['42501']);await deny('select * from public.applicant_events',[],['42501']);await no('create',{...base,id:newId},['42501']);
 await db.exec('reset role');
 eq((await db.query("select count(*)::int n from pg_class where relname in ('applicants','applicant_events') and relrowsecurity")).rows[0].n,2,'Both applicant tables have RLS');
 eq((await db.query("select count(*)::int n from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='public' and p.prosecdef")).rows[0].n,0,'No public security definer');
 await session(owner);
}
