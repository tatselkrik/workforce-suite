import assert from 'node:assert/strict';
export const reportView=(companyId,name='Synthetic columns',serial=1)=>({id:`00000000-0000-0000-0000-${String(91000+serial).padStart(12,'0')}`,company_id:companyId,name,period:'last30',filter:{kind:'attendance',starts_on:'2026-10-01',ends_on:'2026-10-22',employee_id:null,department:'',include_archived:true,include_missing:true,leave_status:null,columns:['Work date','Employee code']}});
export async function runReportPreferenceTests({db,eq,deny,session,company,owner,unassigned,legacy}) {
 const save=async(views,revision=0,companyId=company.id)=>(await db.query('select public.save_report_preferences($1,$2,$3::jsonb) result',[companyId,revision,JSON.stringify(views)])).rows[0].result;
 const view=(name='Synthetic columns',serial=1)=>reportView(company.id,name,serial);
 const no=(views,revision=0,codes=['23514'],companyId=company.id)=>deny('select public.save_report_preferences($1,$2,$3::jsonb)',[companyId,revision,JSON.stringify(views)],codes);
 await session(owner);
 let row=(await db.query('select * from public.report_preferences')).rows[0];eq(row.views,legacy,'Legacy same-company views copied once');eq(row.revision,1,'Backfill starts a guarded revision');
 row=await save([view()],1);eq(row.user_id,owner,'Server chooses the signed-in preference owner');eq(row.company_id,company.id,'Server chooses the protected membership company');eq(row.revision,2,'Successful save increments revision');eq(row.views[0].filter.columns,['Work date','Employee code'],'Column order retained');
 await no([],1,['PT409']);eq((await db.query('select revision from public.report_preferences')).rows[0].revision,2,'Stale save preserves newer preferences');
 for(const bad of [[view(),view()], [view('Synthetic'),view('SYNTHETIC',2)],Array.from({length:21},(_,i)=>view(`Synthetic ${i}`,i)),[view('')],[view('bad\nname')],{},[view('x'.repeat(81))],[{...view(),company_id:unassigned}],[{...view(),id:'00000000-0000-0000-0000-000000000000'}],[{...view(),padding:'x'.repeat(65536)}]]){await no(bad,2);}
 await no([], -1);await no([],2,['42501'],unassigned);
 const many=Array.from({length:20},(_,i)=>({...view(`Synthetic ${i} ${'測'.repeat(40)}`,i),filter:{...view().filter,department:'x'.repeat(80),columns:['Work date','Employee code','Full name','Gross minutes','Calculation status']}}));
 row=await save(many,2);eq(row.views.length,20,'Twenty larger layouts supported outside Auth tokens');eq(row.revision,3,'Larger collection uses the same revision guard');row=await save([],3);eq(row.views,[],'Deleting views clears only the preference array');eq(row.revision,4,'Deleting views remains revision guarded');
 for(const sql of ['update public.report_preferences set revision=100','delete from public.report_preferences',`insert into public.report_preferences(user_id,company_id) values('${owner}','${company.id}')`]){await deny(sql,[],['42501']);}
 await deny('select hris_private.valid_report_views($1,$2::jsonb)',[company.id,'[]'],['42501']);
 const users={hr:'00000000-0000-0000-0000-000000092001',manager:'00000000-0000-0000-0000-000000092002',employee:'00000000-0000-0000-0000-000000092003'};
 for(const [role,user]of Object.entries(users)) {
  await db.exec('reset role');await db.query('insert into auth.users(id,email,email_confirmed_at) values($1,$2,now())',[user,`report-preferences-${role}@example.invalid`]);
  await session(owner);await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[user,`report-preferences-${role}@example.invalid`,role==='hr'?'hr_generalist':role]);
 }
 for(const role of ['hr','manager']){await session(users[role]);eq((await db.query('select * from public.report_preferences')).rows,[],`${role} cannot read another account's views`);row=await save([view(`Synthetic ${role}`)]);eq(row.user_id,users[role],`${role} saves only own row`);eq((await db.query('select user_id from public.report_preferences')).rows,[{user_id:users[role]}],`${role} sees only own preferences`);}
 await session(owner);eq((await db.query('select user_id from public.report_preferences')).rows,[{user_id:owner}],'Administrator cannot inspect other personal views');
 await session(users.employee);await no([],0,['42501']);eq((await db.query('select * from public.report_preferences')).rows,[],'Portal-only employee cannot read desktop report preferences');
 await db.exec('reset role');await db.query("update auth.users set raw_user_meta_data='{"+'"role":"administrator"'+"}'::jsonb where id=$1",[users.employee]);await session(users.employee);await no([],0,['42501']);
 await session(owner);await db.query('select public.save_company_account($1,$2,$3,false,null,null)',[users.hr,'report-preferences-hr@example.invalid','hr_generalist']);await session(users.hr);eq((await db.query('select * from public.report_preferences')).rows,[],'Disabled staff loses saved report reads');await no([],1,['42501']);
 await session(unassigned);await no([],0,['42501']);await session('','anon');await deny('select * from public.report_preferences',[],['42501']);await no([],0,['42501']);
 await db.exec('reset role');eq((await db.query("select relrowsecurity from pg_class where oid='public.report_preferences'::regclass")).rows[0].relrowsecurity,true,'Preference table has RLS');eq((await db.query("select count(*)::int n from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='public' and p.prosecdef")).rows[0].n,0,'No new exposed definer function');
 assert.equal((await db.query('select count(*)::int n from public.report_preferences')).rows[0].n,3);
 await session(owner);
}
