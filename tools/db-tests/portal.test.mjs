export async function runPortalTests({db,eq,deny,session,company,owner,unassigned}) {
 const uid='00000000-0000-4000-8000-000000120001',other='00000000-0000-4000-8000-000000120002';
 await db.exec('reset role');
 const preserve={};for(const t of ['employees','attendance','leave_requests','payroll_runs','payroll_items','payroll_slips'])preserve[t]=(await db.query(`select to_jsonb(t)r from public.${t} t order by id`)).rows;
 for(const [u,email]of[[uid,'portal-employee@example.invalid'],[other,'portal-other@example.invalid']]){
  await db.exec('reset role');await db.query('insert into auth.users(id,email,email_confirmed_at)values($1,$2,now())',[u,email]);await session(owner);await db.query("select public.save_company_account($1,$2,'employee',true,null,null)",[u,email]);
 }
 await db.exec('reset role');const members=(await db.query('select user_id,employee_id from public.company_members where user_id=any($1::uuid[])',[[uid,other]])).rows;const eid=members.find(m=>m.user_id===uid).employee_id,oeid=members.find(m=>m.user_id===other).employee_id;
 await db.query("update public.employees set joined_on='2025-01-01'where id=any($1::uuid[])",[[eid,oeid]]);
 const old=(await db.query('select to_jsonb(p)p from public.leave_policies p where company_id=$1',[company.id])).rows[0].p;
 const type={code:'portal_test',name:'Portal test',active:true,paid:false,use_employee_allowance:false,annual_units:30,accrual:'unlimited',carryover_units:0,carryover_expiry_month:3,approval_chain:[{kind:'reviewer',employee_id:null}]};
 await session(owner);await db.query('select public.save_leave_policy($1::jsonb,$2)',[JSON.stringify({...old,types:[...old.types,type]}),old.revision]);
 const context=async()=> (await db.query("select public.portal_leave_context('2026-11-02')c")).rows[0].c;
 await session(uid);let c=await context();eq(c.employee_id,eid,'Context belongs to current employee');eq(c.company_id,company.id,'Context remains in company');eq(c.types.find(t=>t.code==='portal_test').balance,null,'Unlimited balance is explicit');eq(c.types.find(t=>t.code==='portal_test').unlimited,true,'Unlimited policy flag');eq(JSON.stringify(c).includes('approval_chain'),false,'Context excludes staff approval targets');
 const submit=async(rev=c.policy_revision)=> (await db.query("select public.portal_submit_leave('portal_test','2026-11-02','2026-11-30','full','Synthetic portal filing',$1)id",[rev])).rows[0].id;
 await deny("select public.portal_submit_leave('portal_test','2026-11-02','2026-11-30','full','Stale policy',$1)",[c.policy_revision-1],['PT409']);
 const rid=await submit();let r=(await db.query('select * from public.leave_requests where id=$1',[rid])).rows[0];eq(r.employee_id,eid,'Submit infers employee rather than taking a client identity');eq(r.status,'pending','New portal filing awaits review');eq(r.policy_revision,c.policy_revision,'Filed charge retains reviewed policy');eq(r.charged_units>0,true,'Filed scheduled days carry saved units');
 await session(other);await deny('select public.portal_cancel_leave($1,0,$2)',[rid,'Foreign own-portal cancellation'],['42501']);eq((await context()).employee_id,oeid,'Second account cannot select first identity');
 await session(uid);await deny('select public.portal_cancel_leave($1,99,$2)',[rid,'Stale cancel'],['PT409']);await db.query('select public.portal_cancel_leave($1,0,$2)',[rid,'Synthetic cancellation']);r=(await db.query('select * from public.leave_requests where id=$1',[rid])).rows[0];eq([r.status,r.revision],['cancelled',1],'Own cancellation retains revision');eq((await db.query('select count(*)::int n from public.leave_events where request_id=$1',[rid])).rows[0].n,2,'Submission/cancellation history retained');
 await session(unassigned);await deny("select public.portal_leave_context('2026-11-02')",[],['42501']);
 await session(owner);await db.query("select public.save_company_account($1,$2,'employee',false,$3,null)",[uid,'portal-employee@example.invalid',eid]);await session(uid);await deny("select public.portal_leave_context('2026-11-02')",[],['42501']);await deny("select public.portal_submit_leave('portal_test','2026-11-02','2026-11-30','full','Disabled',$1)",[c.policy_revision],['42501']);
 await session(owner);await db.query("select public.save_company_account($1,$2,'administrator',true,$3,null)",[uid,'portal-employee@example.invalid',eid]);await session(uid);
 const past=(await db.query("select public.portal_submit_leave('portal_test','2026-01-02','2026-01-30','full','Synthetic past approval',$1)id",[c.policy_revision])).rows[0].id;
 await session(owner);await db.query('select public.review_leave($1,0,true,$2)',[past,'Synthetic independent review']);await session(uid);
 await deny('select public.portal_cancel_leave($1,1,$2)',[past,'Portal past cancellation'],['23514']);
 await db.query('select public.cancel_leave($1,1,$2)',[past,'Staff desktop authority retained']);eq((await db.query('select status from public.leave_requests where id=$1',[past])).rows[0].status,'cancelled','Portal restriction does not remove existing staff desktop authority');
 await session(owner);await db.query("select public.save_company_account($1,$2,'employee',false,$3,null)",[uid,'portal-employee@example.invalid',eid]);
 await db.exec('reset role');for(const fn of ['portal_leave_context(date)','portal_submit_leave(text,date,date,text,text,bigint)','portal_cancel_leave(uuid,bigint,text)'])eq((await db.query('select prosecdef from pg_proc where oid=$1::regprocedure',['public.'+fn])).rows[0].prosecdef,false,'Portal public wrapper is invoker');
 for(const[t,rows]of Object.entries(preserve)){let actual=(await db.query(`select to_jsonb(t)r from public.${t} t order by id`)).rows;actual=actual.filter(x=>![eid,oeid].includes(x.r.id)&&![eid,oeid].includes(x.r.employee_id));eq(actual,rows,'Portal preserves earlier '+t);}
 await session('','anon');await deny('select public.portal_leave_context(null)',[],['42501']);await deny('select public.portal_cancel_leave($1,0,$2)',[rid,'Anonymous'],['42501']);await deny("select public.portal_submit_leave('portal_test','2026-11-02','2026-11-30','full','Anonymous',0)",[],['42501']);
}
