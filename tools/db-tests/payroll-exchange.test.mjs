import assert from'node:assert/strict';
import{writeFile}from'node:fs/promises';
import{createHash}from'node:crypto';
export async function runPayrollExchangeTests({db,eq,session,company,owner,unassigned}){
 let checks=0;const equal=eq;eq=(a,b,label)=>{checks++;equal(a,b,label);};
 const id=n=>`aa530000-0000-4000-8000-${String(n).padStart(12,'0')}`;
 const q=async(sql,args=[])=>(await db.query(sql,args)).rows;
 const rpc=async(name,...args)=>(await q(`select public.${name}(${args.map((_,i)=>'$'+(i+1)).join(',')})r`,args))[0].r;
 const root=async()=>db.exec('reset role');
 let inTransaction=false;
 const bad=async(fn,codes=['23514'])=>{if(inTransaction)await db.exec('savepoint expected_denial');try{await fn();}catch(e){if(inTransaction)await db.exec('rollback to savepoint expected_denial;release savepoint expected_denial');eq(codes.includes(e.code),true,`Expected ${codes}; got ${e.code}: ${e.message}`);return;}assert.fail('Exchange operation should be rejected');};
 const actors={};await root();
 for(const[n,key,role]of[[1,'admin','administrator'],[2,'hr','hr_generalist'],[3,'manager','manager'],[4,'employee','employee']]){
  await root();await q('insert into auth.users(id,email,email_confirmed_at)values($1,$2,now())',[id(n),`exchange-${key}@example.invalid`]);await session(owner);await rpc('save_company_account',id(n),`exchange-${key}@example.invalid`,role,true,null,null);actors[key]=id(n);
 }
 await root();const tables=(await q("select tablename from pg_tables where schemaname='public'and tablename not like'payroll_exchange%'order by tablename")).map(r=>r.tablename),before={};
 for(const t of tables)before[t]=await q('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text');
 const source=(await q("select pg_get_functiondef('hris_private.payroll_report_rows_v2(uuid)'::regprocedure)f"))[0].f;
 const columns=['employee_name','department','received_on','gross_cents','net_cents','cash_cents','offset_cents','contained_source_net_cents','tax_adjustment_cents','review_amount_cents'].map(key=>({key,header:key}));
 const change={request_id:id(10),id:id(11),expected_revision:null,name:'Fictional local exchange',enabled:true,columns,reason:'PRIVATE_PROFILE_REASON'};
 const filter={starts_on:'2026-01-01',ends_on:'2026-12-31',axis:'issued',kind:'all',employee_id:null,search:'EMP-'};
 const request={id:id(20),profile_id:change.id,expected_revision:0,filter,reason:'PRIVATE_EXCHANGE_REASON'};
 const preview=async r=>rpc('preview_payroll_exchange',r);
 const prepare=async(r,p)=>rpc('prepare_payroll_exchange',r,p.basis.fingerprint);
 const vectors=[];
 await session(actors.admin);let state=await rpc('save_payroll_exchange_profile',change);eq(state.profiles.length,1,'Admin creates a versioned profile');
 eq((await rpc('save_payroll_exchange_profile',change)).profiles.length,1,'Exact profile retry is idempotent');
 await bad(()=>rpc('save_payroll_exchange_profile',{...change,name:'Different payload'}),['PT409']);
 for(const patch of[{columns:[{key:'payment_reference',header:'ref'}]},{columns:[{key:'cash_cents',header:'=formula'}]},{columns:[{key:'cash_cents',header:'Exchange_ID'}]},{columns:[columns[0],columns[0]]},{columns:[]},{enabled:'yes'},{reason:''},{extra:1},{expected_revision:0.5}])await bad(()=>rpc('save_payroll_exchange_profile',{...change,request_id:id(12),...patch}));
 await session(actors.hr);await bad(()=>rpc('save_payroll_exchange_profile',{...change,request_id:id(13)}),['42501']);
 await bad(()=>preview({...request,id:id(19),filter:{...filter,search:''}}),['23514']);
 let p=await preview(request);eq(p.basis.report.version,'payroll-report-v2','Exchange uses full amendment-aware projection');eq(p.basis.controls.length,p.basis.report.rows.length,'One exact control per issued source');eq(p.basis.report.rows.length>0,true,'Existing financial fixtures are exported');eq([...new Set(p.basis.report.rows.map(r=>r.kind))].sort(),['amendment','annual_tax','cutoff','final','monthly','thirteenth'],'Complete code-series scope retains all six source kinds');
 eq(createHash('sha256').update(p.basis.csv).digest('hex'),p.basis.sha256,'Exact UTF-8 checksum');eq(p.basis.csv.includes('PRIVATE_'),false,'Private references/reasons/evidence not exported');eq(p.basis.csv.split('\n')[0].includes('amount_unit'),true,'Money unit is mandatory');
 for(const key of['cash_cents','offset_cents'])eq(p.basis.report.rows.reduce((n,r)=>n+r[key],0),(await rpc('payroll_report_v2',filter)).rows.reduce((n,r)=>n+r[key],0),'Separate report amount retained: '+key);
 eq(p.basis.report.rows.filter(r=>r.kind==='annual_tax').every(r=>r.cash_cents===0),true,'Tax advice creates no exchange cash');eq(p.basis.report.rows.filter(r=>r.payment_state==='final_offset').every(r=>r.cash_cents===0),true,'Final source offsets are not duplicated cash');
 let d=await prepare(request,p);eq(d.basis.sha256,p.basis.sha256,'Prepared file equals preview exact bytes');eq((await prepare(request,p)).basis.sha256,d.basis.sha256,'Exact preparation retry returns original snapshot');
 vectors.push({request,preview:p,detail:d,access:await rpc('my_hris_access')});
 await bad(()=>rpc('prepare_payroll_exchange',{...request,reason:'Changed reason'},p.basis.fingerprint),['PT409']);
 await session(actors.admin);await bad(()=>prepare(request,p),['PT409']);await session(actors.hr);
 const absent=await rpc('payroll_exchange_attempt','prepare',id(999),{request:{...request,id:id(999)},fingerprint:p.basis.fingerprint});eq(absent.committed,false,'Uncertain identity can be proven absent without writing');eq((await rpc('payroll_exchange_attempt','prepare',request.id,{request,fingerprint:p.basis.fingerprint})).committed,true,'Committed attempt lookup returns bound proof');
 await bad(()=>rpc('payroll_exchange_attempt','prepare',request.id,{request:{...request,reason:'forged'},fingerprint:p.basis.fingerprint}),['PT409']);
 const lateRequest={...request,id:id(25)};const latePreview=await preview(lateRequest);const latePayload={request:lateRequest,fingerprint:latePreview.basis.fingerprint};
 eq((await rpc('payroll_exchange_attempt','prepare',lateRequest.id,latePayload)).committed,false,'Recovery closes an unused identity');
 eq((await rpc('payroll_exchange_attempt','prepare',lateRequest.id,latePayload)).committed,false,'Retired attempt lookup is idempotent');
 await bad(()=>prepare(lateRequest,latePreview),['PT409']);await session(actors.admin);await bad(()=>rpc('payroll_exchange_attempt','prepare',lateRequest.id,latePayload),['PT409']);await session(actors.hr);
 const c=p.basis.controls[0];const ack={id:id(30),exchange_id:request.id,reason:'PRIVATE_ACK_REASON',rows:[{record_key:c.record_key,row_sha256:c.row_sha256,expected_revision:0,status:'accepted',reference:'PRIVATE_ACK_REFERENCE',note:'Fictional local reconciliation'}]};
 const lateAck={...ack,id:id(35)};await rpc('payroll_exchange_attempt','ack',lateAck.id,lateAck);await bad(()=>rpc('ack_payroll_exchange',lateAck),['PT409']);
 for(const patch of[{rows:[{...ack.rows[0],row_sha256:'0'.repeat(64)}]},{rows:[{...ack.rows[0],expected_revision:1}]},{rows:[ack.rows[0],ack.rows[0]]},{rows:[{...ack.rows[0],status:'paid'}]},{rows:[{...ack.rows[0],reference:'bad\nref'}]},{rows:[]},{reason:''}])await bad(()=>rpc('ack_payroll_exchange',{...ack,...patch}),patch.rows?.[0]?.expected_revision===1?['PT409']:['23514']);
 eq((await rpc('payroll_exchange_detail',request.id)).events.length,0,'Rejected batches leave no partial events');
 d=await rpc('ack_payroll_exchange',ack);eq(d.events.length,1,'Acknowledgement records one immutable event');eq((await rpc('ack_payroll_exchange',ack)).events.length,1,'Exact acknowledgement retry is idempotent');
 await bad(()=>rpc('ack_payroll_exchange',{...ack,reason:'Different reason'}),['PT409']);await session(actors.admin);await bad(()=>rpc('ack_payroll_exchange',ack),['PT409']);await session(actors.hr);
 await bad(()=>rpc('ack_payroll_exchange',{...ack,id:id(31),rows:[{...ack.rows[0],expected_revision:1,status:'rejected'}]}),['PT409']);
 const reset={...ack,id:id(32),reason:'Reset after checking wrong manual response',rows:[{...ack.rows[0],expected_revision:1,status:'pending',note:'Linked correction reason'}]};await bad(()=>rpc('ack_payroll_exchange',reset),['42501']);
 await session(actors.admin);d=await rpc('ack_payroll_exchange',reset);eq(d.events.length,2,'Admin reset retains terminal outcome');
 await session(actors.hr);d=await rpc('ack_payroll_exchange',{...ack,id:id(33),rows:[{...ack.rows[0],expected_revision:2,status:'rejected'}]});eq(d.events.length,3,'Reviewed rejection follows linked reset');eq(d.events.map(e=>e.revision),[1,2,3],'Reconciliation revisions are contiguous');
 const history=(await rpc('payroll_exchange_state')).history.find(h=>h.id===request.id);eq(history.accepted,0,'Only latest outcome counted');eq(history.rejected,1,'Rejected count reconciles immutable history');
 const second=p.basis.controls[1];if(second){await bad(()=>rpc('ack_payroll_exchange',{...ack,id:id(34),rows:[{record_key:second.record_key,row_sha256:second.row_sha256,expected_revision:0,status:'accepted',reference:'',note:''},{...ack.rows[0],expected_revision:0}]}),['PT409']);eq((await rpc('payroll_exchange_detail',request.id)).events.length,3,'Late failing row rolls back earlier batch rows');}
 await session(actors.admin);const lateProfile={...change,request_id:id(17),expected_revision:0,name:'Late edit'};await rpc('payroll_exchange_attempt','profile',lateProfile.request_id,lateProfile);await bad(()=>rpc('save_payroll_exchange_profile',lateProfile),['PT409']);state=await rpc('save_payroll_exchange_profile',{...change,request_id:id(14),expected_revision:0,name:'Renamed disabled profile',enabled:false});eq(state.profiles[0].revision,1,'Configuration creates a new immutable version');
 await session(actors.hr);await bad(()=>preview({...request,id:id(21)}),['PT409']);eq((await rpc('payroll_exchange_detail',request.id)).basis.csv,p.basis.csv,'Disabled/renamed profile cannot rewrite historical bytes');eq((await prepare(request,p)).basis.csv,p.basis.csv,'Committed retry survives later disabled profile');
 await session(actors.admin);await rpc('save_payroll_exchange_profile',{...change,request_id:id(15),expected_revision:1});
 await session(actors.hr);const staleRequest={...request,id:id(22),expected_revision:2};const stale=await preview(staleRequest);
 await session(actors.admin);await rpc('save_payroll_exchange_profile',{...change,request_id:id(16),expected_revision:2,name:'New title'});await session(actors.hr);await bad(()=>prepare(staleRequest,stale),['PT409']);
 // A temporary local row generator checks exact limits/escaping, not financial rules.
 await root();const sample={...p.basis.report.rows[0],employee_name:'\u2007=Fictional "name"',department:'@Synthetic department'};const literal=JSON.stringify(sample).replaceAll("'","''");
 try{
  await db.exec(`create or replace function hris_private.payroll_report_rows_v2(cid uuid)returns setof jsonb language sql stable security invoker set search_path=''as $$select jsonb_set('${literal}'::jsonb,'{id}',to_jsonb(md5(n::text)::uuid::text))from generate_series(1,1001)n;$$;`);
  await session(actors.hr);const many={...request,id:id(23),expected_revision:3};await bad(()=>preview(many),['54000']);
  await root();await db.exec(`create or replace function hris_private.payroll_report_rows_v2(cid uuid)returns setof jsonb language sql stable security invoker set search_path=''as $$select jsonb_set('${literal}'::jsonb,'{id}',to_jsonb(md5(n::text)::uuid::text))from generate_series(1,1000)n;$$;`);
  await session(actors.hr);const exact=await preview(many);eq(exact.basis.controls.length,1000,'Exactly1000 rows are complete');eq(exact.basis.csv.includes("'\u2007=Fictional"),true,'Unicode-prefixed formula text escaped');vectors.push({request:many,preview:exact,access:await rpc('my_hris_access'),fixture:'size-and-escape-only'});
  await root();await db.exec(`create or replace function hris_private.payroll_report_rows_v2(cid uuid)returns setof jsonb language sql stable security invoker set search_path=''as $$select '${literal}'::jsonb;$$;`);await session(actors.hr);const one={...request,id:id(24),expected_revision:3};const originalPreview=await preview(one);
  await root();await db.exec(`create or replace function hris_private.payroll_report_rows_v2(cid uuid)returns setof jsonb language sql stable security invoker set search_path=''as $$select jsonb_set('${literal}'::jsonb,'{department}','"Changed source"');$$;`);await session(actors.hr);await bad(()=>prepare(one,originalPreview),['PT409']);
 }finally{await root();await db.exec(source);}
 for(const actor of[actors.manager,actors.employee,unassigned]){await session(actor);for(const fn of[()=>rpc('payroll_exchange_state'),()=>preview(request),()=>rpc('payroll_exchange_detail',request.id),()=>rpc('ack_payroll_exchange',ack),()=>rpc('payroll_exchange_attempt','prepare',request.id,{request,fingerprint:p.basis.fingerprint})])await bad(fn,['42501']);}
 await root();await db.exec('begin');inTransaction=true;await q('update auth.users set raw_user_meta_data=$1 where id=$2',[{role:'administrator'},actors.employee]);await session(actors.employee);await bad(()=>rpc('payroll_exchange_state'),['42501']);await root();await db.exec('rollback');inTransaction=false;
 await db.exec('begin');inTransaction=true;await q('update public.company_members set active=false where user_id=$1',[actors.hr]);await session(actors.hr);await bad(()=>rpc('payroll_exchange_state'),['42501']);await bad(()=>prepare(request,p),['42501']);await root();await db.exec('rollback');inTransaction=false;
 await db.exec('begin');inTransaction=true;await q("select set_config('request.jwt.claim.sub',$1,false)",[actors.admin]);await q('update public.company_members set active=false where user_id=$1',[actors.hr]);await q("update public.employees set status='archived'where id=(select employee_id from public.company_members where user_id=$1)",[actors.hr]);await session(actors.hr);await bad(()=>rpc('payroll_exchange_detail',request.id),['42501']);await root();await db.exec('rollback');inTransaction=false;
 await session(actors.hr);for(const t of['payroll_exchange_profiles','payroll_exchanges','payroll_exchange_ack_batches','payroll_exchange_ack_events','payroll_exchange_retired_attempts'])await bad(()=>q('select *from public.'+t),['42501']);
 await root();for(const t of['payroll_exchange_profiles','payroll_exchanges','payroll_exchange_ack_batches','payroll_exchange_ack_events','payroll_exchange_retired_attempts']){await bad(()=>q('delete from public.'+t));await bad(()=>q('update public.'+t+' set '+(t==='payroll_exchange_ack_events'?"note='overwrite'":"created_at=now()")));}
 await bad(()=>q('select hris_private.payroll_exchange_capacity($1,33554432)',[company.id]),['54000']);
 for(const t of tables)eq(await q('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text'),before[t],'Exchange workflow preserves original '+t);
 eq((await q("select pg_get_functiondef('hris_private.payroll_report_rows_v2(uuid)'::regprocedure)f"))[0].f,source,'Boundary fixture restores exact report function');
 const saved=(await rpc('payroll_exchange_state')).history;eq(saved.length,1,'Denied preparation/acknowledgement creates no extra exports');
 await session(actors.hr);vectors[0].final_detail=await rpc('payroll_exchange_detail',request.id);
 await writeFile(new URL('../../work/exchange053-sql-vectors.json',import.meta.url),JSON.stringify(vectors,null,2)+'\n');
 console.log(`Payroll exchange workflow: ${checks} assertions passed.`);
}
