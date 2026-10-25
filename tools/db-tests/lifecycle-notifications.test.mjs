import assert from 'node:assert/strict';
export async function runLifecycleTests({db,eq,deny,session,company,owner,unassigned}){
 const id=n=>`ba460000-0000-4000-8000-${String(n).padStart(12,'0')}`;
 const cid=company.id;const people={};
 for(const [i,key,role]of [[1,'manager','manager'],[2,'subject','employee'],[3,'outsider','employee'],[4,'hr','hr_generalist']]){
  const user=id(i);await db.exec('reset role');await db.query('insert into auth.users(id,email,email_confirmed_at)values($1,$2,now())',[user,`lifecycle-${key}@example.invalid`]);
  await session(owner);await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[user,`lifecycle-${key}@example.invalid`,role]);
  people[key]={user,employee:(await db.query('select employee_id from public.company_members where user_id=$1',[user])).rows[0].employee_id};
 }
 await db.exec('reset role');await db.query("update public.employees set reports_to=$1 where company_id=$2 and id=$3",[people.manager.employee,cid,people.subject.employee]);
 const today=(await db.query("select (now()at time zone timezone)::date::text d from public.companies where id=$1",[cid])).rows[0].d;
 const day=async n=>(await db.query('select ($1::date+$2::int)::text d',[today,n])).rows[0].d;
 const before=await day(-14),end=await day(-1),due=await day(2),soon=await day(1);
 const cycle=id(10);await db.query("insert into public.performance_cycles(id,company_id,name,starts_on,ends_on,due_on,created_by)values($1,$2,'Fictional Lifecycle Cycle',$3,$4,$5,$6)",[cycle,cid,before,end,due,owner]);
 for(const [n,key,reviewer,status]of [[11,'subject','manager','draft'],[12,'outsider','hr','submitted'],[13,'manager','hr','published']]){
  const p=people[key];await db.query("insert into public.performance_reviews(id,company_id,cycle_id,employee_id,employee_name,employee_code,cycle_name,starts_on,ends_on,due_on,reviewer_employee_id,reviewer_name,status,published_at,published_by,assessment,created_by)values($1,$2,$3,$4,'Fictional Person','SYNTHETIC','Fictional Lifecycle Cycle',$5,$6,$7,$8,'Fictional Reviewer',$9,case when $9='published'then now()end,case when $9='published'then $10::uuid end,'PRIVATE_ASSESSMENT_SENTINEL',$10)",[id(n),cid,cycle,p.employee,before,end,due,people[reviewer].employee,status,owner]);
 }
 await db.query("insert into public.performance_hr_notes(id,company_id,review_id,note,created_by)values($1,$2,$3,'PRIVATE_HR_NOTE_SENTINEL',$4)",[id(14),cid,id(11),owner]);
 for(const [n,key,status]of [[21,'subject','planned'],[22,'manager','in_progress'],[23,'outsider','planned'],[24,'subject','cancelled']]){
  await db.query("insert into public.employee_training(id,company_id,employee_id,employee_name,employee_code,title,starts_on,due_on,status,progress_note,created_by)values($1,$2,$3,'Fictional Person','SYNTHETIC','Fictional Safety Training',$4,$5,$6,'PRIVATE_PROGRESS_SENTINEL',$7)",[id(n),cid,people[key].employee,soon,due,status,owner]);
 }
 for(const [n,key,status,archived]of [[31,'subject','valid',false],[32,'manager','valid',true],[33,'outsider','revoked',false]]){
  await db.query("insert into public.employee_certifications(id,company_id,employee_id,employee_name,employee_code,title,reference_number,issued_on,expires_on,status,archived,created_by)values($1,$2,$3,'Fictional Person','SYNTHETIC','Fictional Certificate','PRIVATE_CERTIFICATE_ID',$4,$5,$6,$7,$8)",[id(n),cid,people[key].employee,before,due,status,archived,owner]);
 }
 // Stored fictional advice is a projection fixture, not a payroll calculation.
 const run=(await db.query("select id,month,rule_version from public.payroll_runs where company_id=$1 and status in('approved','paid')order by month limit 1",[cid])).rows[0];assert.ok(run);
 await db.query("insert into public.payroll_slips(id,company_id,run_id,employee_id,month,company_name,item,approved_by,rule_version)values($1,$2,$3,$4,$5,'Fictional Company','{\"PRIVATE_SOURCE_BUNDLE\":true,\"totals\":{\"net\":12345}}', $6,$7)",[id(40),cid,run.id,people.subject.employee,run.month,owner,run.rule_version]);
 await db.query("insert into public.payroll_payment_receipts(id,company_id,employee_id,kind,slip_id,run_id,amount_cents,paid_on,payment_reference,reason,origin,recorded_by)values($1,$2,$3,'monthly',$4,$5,12345,$6,'PRIVATE_BANK_REFERENCE','PRIVATE_RECEIPT_REASON','individual',$7)",[id(41),cid,people.subject.employee,id(40),run.id,await day(-2),owner]);
 const feed=async(surface='desktop')=>(await db.query('select public.lifecycle_notifications($1)r',[surface])).rows[0].r;
 const mark=async(keys,read=true,reset=false,surface='desktop')=>(await db.query('select public.lifecycle_notification_reads($1,$2::text[],$3,$4)r',[surface,keys,read,reset])).rows[0].r;
 const mixed=async(keys,read=true,reset=false)=>(await db.query('select public.staff_notification_reads($1,$2::text[],$3,$4)r',[cid,keys,read,reset])).rows[0].r;
 const lifecycleNo=(keys,read=true,reset=false,surface='desktop',codes=['23514'])=>deny('select public.lifecycle_notification_reads($1,$2::text[],$3,$4)',[surface,keys,read,reset],codes);
 await session(people.manager.user);let m=await feed();eq(m.employee_id,people.manager.employee,'Lifecycle identity is caller bound');eq(m.surface,'desktop','Desktop surface is explicit');
 const sourceDigestQuery="select md5((select jsonb_agg(to_jsonb(r)order by r.id)::text from public.payroll_slips r where r.id=$1)|| (select jsonb_agg(to_jsonb(r)order by r.id)::text from public.payroll_payment_receipts r where r.id=$2))d";
 await db.exec('reset role');const sourceDigest=(await db.query(sourceDigestQuery,[id(40),id(41)])).rows[0].d;await session(people.manager.user);
 const keys=()=>m.items.map(r=>r.key);const task=`appraisal-review:${id(11)}:0:soon`;const ownTraining=`training-due:${id(22)}:0:soon`;
 eq(keys().includes(task),true,'Current assigned reviewer gets deadline');eq(keys().includes(`appraisal-published:${id(13)}:0`),true,'Own published review notice');
 eq(m.items.some(r=>r.employee_id===people.outsider.employee),false,'Manager sees no unrelated employee');
 eq(m.items.some(r=>r.kind.startsWith('payroll-')),false,'Manager receives no other employee pay notices');
 eq(m.items.some(r=>r.target_id===id(24)||r.target_id===id(32)||r.target_id===id(33)),false,'Cancelled training and archived/revoked certificates excluded');
 eq(m.items.some(r=>r.target_id===id(31)),true,'Current-report certificate expiry is visible');
 await db.exec('reset role');
 for(const [offset,stage]of [[0,'soon'],[2,'today'],[3,'expired']]){
  const rows=(await db.query("select alert_key from hris_private.lifecycle_notification_rows($1,$2,'manager',$3::date,now())where target_id=$4",[cid,people.manager.employee,await day(offset),id(11)])).rows;
  eq(rows[0].alert_key,`appraisal-review:${id(11)}:0:${stage}`,'Appraisal date stage '+stage);
 }
 await session(people.manager.user);
 for(const token of ['PRIVATE_ASSESSMENT_SENTINEL','PRIVATE_HR_NOTE_SENTINEL','PRIVATE_PROGRESS_SENTINEL','PRIVATE_CERTIFICATE_ID','PRIVATE_SOURCE_BUNDLE','PRIVATE_BANK_REFERENCE','PRIVATE_RECEIPT_REASON'])eq(JSON.stringify(m).includes(token),false,'Projection omits '+token);
 let saved=await mark([task]);eq(saved.read_keys,[task],'Read stores key only');saved=await mark([ownTraining]);eq(saved.read_keys.length,2,'Independent reads merge');saved=await mark([task]);eq(saved.read_keys.length,2,'Read is idempotent');
 saved=await mark([task],false);eq(saved.read_keys,[ownTraining],'Unread removes selected marker only');
 const old=`checkout:${id(90)}:0`;await mixed([old,task]);eq((await db.query('select read_keys from public.notification_read_state where user_id=$1',[people.manager.user])).rows[0].read_keys,[old],'Legacy row keeps legacy keys only');
 eq((await db.query('select read_keys from public.lifecycle_notification_read_state where user_id=$1 and surface=\'desktop\'',[people.manager.user])).rows[0].read_keys.includes(task),true,'New keys have separate table');
 const invalid=`training-due:${id(999)}:0:soon`;await deny('select public.staff_notification_reads($1,$2::text[],true,false)',[cid,[`checkout:${id(91)}:0`,invalid]],['PT409']);
 eq((await db.query('select read_keys from public.notification_read_state where user_id=$1',[people.manager.user])).rows[0].read_keys,[old],'Mixed failure rolls back legacy part');
 for(const malformed of [null,[],[task,task],['arbitrary text'],[task.replace(':0:',':-1:')],[task.replace(':0:',':00:')],[task.replace(id(11),'00000000-0000-0000-0000-000000000000')]])await lifecycleNo(malformed);
 await lifecycleNo([task],null);await lifecycleNo([task],true,true);await lifecycleNo([invalid],true,false,'desktop',['PT409']);
 await deny('insert into public.lifecycle_notification_read_state(user_id,company_id,surface)values($1,$2,\'desktop\')',[people.manager.user,cid],['42501']);
 await deny('select *from hris_private.lifecycle_notification_rows($1,$2,\'administrator\',$3::date,now())',[cid,people.subject.employee,today],['42501']);
 const ownPortal=await feed('portal');eq(ownPortal.items.every(r=>r.employee_id===people.manager.employee),true,'Staff portal surface remains own only');eq(ownPortal.items.some(r=>r.kind==='appraisal-review'||r.kind==='appraisal-publish'),false,'Portal has no reviewer tasks');
 await session(people.subject.user);await deny('select public.lifecycle_notifications()',[],['42501']);const e=await feed('portal');
 eq(e.items.some(r=>r.key===`payroll-monthly-issued:${id(40)}:0`),true,'Employee sees own issued advice');const receipt=e.items.find(r=>r.key===`payroll-receipt:${id(41)}:0`);eq(Boolean(receipt),true,'Backdated individual receipt recorded now generates a notice');eq(receipt.date,today,'Receipt notice date follows recording event');eq(receipt.detail.includes(await day(-2)),true,'Received date stays explicit in detail');
 eq(e.items.every(r=>r.employee_id===people.subject.employee),true,'Employee surface is strictly own');
 await mark([receipt.key],true,false,'portal');eq((await feed('portal')).read_keys,[receipt.key],'Portal surface read state is independent');
 await session(owner);const openDay=await day(-2);const attendanceId=(await db.query('select public.correct_attendance($1::jsonb)r',[JSON.stringify({attendance_id:null,employee_id:people.subject.employee,work_date:openDay,checked_in_at:openDay+'T00:00:00Z',checked_out_at:null,expected_revision:null,reason:'Fictional lifecycle open-shift reminder'})])).rows[0].r;await session(people.subject.user);
 const portal=async(offset=0,limit=25,unread=false)=>(await db.query('select public.portal_notifications_v2($1,$2,$3)r',[offset,limit,unread])).rows[0].r;
 const portalMark=async(keys,read=true,reset=false)=>(await db.query('select public.portal_notification_reads_v2($1::text[],$2,$3)r',[keys,read,reset])).rows[0].r;
 let p=await portal();const attendanceRevision=(await db.query('select revision from public.attendance where id=$1',[attendanceId])).rows[0].revision;
 const oldPortal=`checkout:${attendanceId}:${attendanceRevision}`,issued=`payroll-monthly-issued:${id(40)}:0`;
 eq(p.version,'portal-notifications-v2','Portal has explicit new contract');eq(p.items.some(r=>r.key===oldPortal),true,'V2 contains old own notice: '+JSON.stringify(p.items.map(r=>r.key)));eq(p.items.some(r=>r.key===issued),true,'V2 contains new own notice');
 const oldOnly=(await db.query('select public.portal_notifications()r')).rows[0].r;eq(oldOnly.items.some(r=>r.kind.startsWith('payroll-')),false,'Running old portal receives only original categories');
 await portalMark([oldPortal,issued]);p=await portal();eq(p.items.find(r=>r.key===oldPortal).read,true,'V2 marks actual old item');eq(p.items.find(r=>r.key===issued).read,true,'V2 marks actual new item');
 eq((await db.query('select public.portal_notifications()r')).rows[0].r.items.find(r=>r.key===oldPortal).read,true,'Old portal can still read its valid marker');
 await deny('select public.portal_notification_reads_v2($1::text[],false,false)',[[oldPortal,invalid]],['PT409']);eq((await portal()).items.find(r=>r.key===oldPortal).read,true,'V2 mixed failure rolls back legacy write');
 const onlyUnread=await portal(0,25,true);eq(onlyUnread.total_items,onlyUnread.unread_count,'V2 unread count and filter agree');eq(onlyUnread.items.every(r=>!r.read),true,'V2 unread page contains unread rows only');
 eq((await portal(25)).items,[],'Empty later page retains valid totals');
 await deny('select public.portal_notifications_v2($1,25,false)',[-1],['23514']);await deny('select public.portal_notifications_v2(0,$1,false)',[0],['23514']);
 await deny('select public.portal_notification_reads_v2($1::text[],true,false)',[[`appraisal-review:${id(11)}:0:soon`]],['PT409']);
 await portalMark([],false,true);eq((await portal()).unread_count,(await portal()).total_items,'V2 reset clears both personal marker stores only');
 await session(people.hr.user);const h=await feed();eq(h.items.some(r=>r.key===`appraisal-publish:${id(12)}:0`),true,'HR sees eligible submitted publication');eq(h.items.some(r=>r.kind.startsWith('payroll-')),false,'HR feed does not include other people payroll');
 await db.exec('reset role');eq((await db.query(sourceDigestQuery,[id(40),id(41)])).rows[0].d,sourceDigest,'Notice reads and marker writes preserve advice/receipt source facts');
 const finalReceipt=(await db.query('select employee_id,id from public.payroll_payment_receipts where company_id=$1 and final_case_id is not null limit 1',[cid])).rows[0];assert.ok(finalReceipt,'Prior final-settlement fixture required');
 const finalRows=(await db.query('select *from hris_private.lifecycle_notification_rows($1,$2,\'employee\',$3::date,now())where alert_key=$4',[cid,finalReceipt.employee_id,today,`payroll-receipt:${finalReceipt.id}:0`])).rows;
 eq(finalRows.length,1,'Contained final-settlement source has own recorded notice');eq(finalRows[0].detail.includes('not a second full-net cash payment'),true,'Final source receipt does not claim a second cash payment');
 await db.exec('reset role');await db.query("update public.employees set reports_to=null where id=$1",[people.subject.employee]);await session(people.manager.user);m=await feed();eq(m.items.some(r=>r.employee_id===people.subject.employee),false,'Reporting change removes obsolete tasks');await lifecycleNo([task],true,false,'desktop',['PT409']);
 await session(owner);await db.query('select public.save_company_account($1,$2,\'employee\',true,null,null)',[people.manager.user,'lifecycle-manager@example.invalid']);
 await session(people.manager.user);await deny('select public.lifecycle_notifications()',[],['42501']);eq((await db.query("select *from public.lifecycle_notification_read_state where surface='desktop'")).rows,[],'Demoted employee cannot select desktop marker row');
 await db.exec('reset role');await db.query("update auth.users set raw_user_meta_data='{\"role\":\"administrator\"}'where id=$1",[people.manager.user]);await session(people.manager.user);await deny('select public.lifecycle_notifications()',[],['42501']);
 await session(owner);await db.query('select public.save_company_account($1,$2,\'employee\',false,null,null)',[people.subject.user,'lifecycle-subject@example.invalid']);await session(people.subject.user);await deny('select public.lifecycle_notifications(\'portal\')',[],['42501']);await lifecycleNo([],false,true,'portal',['42501']);
 await session(unassigned);await deny('select public.lifecycle_notifications()',[],['42501']);await session('','anon');await deny('select public.lifecycle_notifications()',[],['42501']);await deny('select *from public.lifecycle_notification_read_state',[],['42501']);
 await db.exec('reset role');eq((await db.query("select relrowsecurity from pg_class where oid='public.lifecycle_notification_read_state'::regclass")).rows[0].relrowsecurity,true,'Lifecycle history has RLS');
 eq((await db.query("select count(*)::int n from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='public'and p.prosecdef")).rows[0].n,0,'No public security definer introduced');
 await session(owner);
}
