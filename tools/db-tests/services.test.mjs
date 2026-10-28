// Fresh chronological migration replay; managed Auth/Storage are local fixtures.
import{readFile,readdir}from'node:fs/promises';
import assert from'node:assert/strict';
import{PGlite}from'@electric-sql/pglite';
import{btree_gist}from'@electric-sql/pglite/contrib/btree_gist';
const db=new PGlite({extensions:{btree_gist}});
try{
 await db.exec(`create role anon nologin;create role authenticated nologin;create role service_role nologin bypassrls;
 create schema supabase_migrations;create table supabase_migrations.schema_migrations(version text primary key);
 create schema auth;create table auth.users(id uuid primary key,email text unique,email_confirmed_at timestamptz,raw_user_meta_data jsonb default'{}'::jsonb);
 create function auth.uid()returns uuid language sql stable as $$select nullif(current_setting('request.jwt.claim.sub',true),'')::uuid$$;
 grant usage on schema auth to authenticated;grant execute on function auth.uid()to authenticated;
 create schema storage;create table storage.buckets(id text primary key,name text,public boolean default false,file_size_limit bigint,allowed_mime_types text[]);
 create table storage.objects(id uuid primary key default gen_random_uuid(),bucket_id text references storage.buckets(id),name text,owner_id text,metadata jsonb,unique(bucket_id,name));
 alter table storage.objects enable row level security;grant usage on schema storage to authenticated,anon;grant select,insert,update,delete on storage.objects to authenticated,anon;`);
 const folder=new URL('../../supabase/migrations/',import.meta.url);let count=0;
 let preservation=null;
 const originals=async()=>{const tables=(await db.query("select schemaname,tablename from pg_tables where schemaname in('public','hris_private')order by schemaname,tablename")).rows;const facts={};for(const t of tables){assert(/^[a-z0-9_]+$/.test(t.tablename));facts[t.schemaname+'.'+t.tablename]=(await db.query('select to_jsonb(x)r from '+t.schemaname+'.'+t.tablename+' x order by to_jsonb(x)::text')).rows;}return{facts,auth:(await db.query('select to_jsonb(x)r from auth.users x order by id')).rows,functions:(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest,p.proacl::text,p.proowner from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows};};
 for(const file of(await readdir(folder)).filter(f=>f.endsWith('.sql')).sort()){
  if(file.endsWith('_employee_services.sql'))preservation=await originals();
  try{await db.exec(await readFile(new URL(file,folder),'utf8'));}
  catch(e){throw Error('Fresh replay failed at '+file+': '+e.message,{cause:e});}
  if(file.endsWith('_employee_services.sql')){const after=await originals();for(const [name,rows]of Object.entries(preservation.facts))assert.deepEqual(after.facts[name],rows,name);assert.deepEqual(after.auth,preservation.auth);for(const f of preservation.functions)assert.deepEqual(after.functions.find(v=>v.identity===f.identity),f);}
  await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[file.split('_')[0]]);count++;
 }

 


let checks=0,n=1;const id=()=>'67000000-0000-4000-8000-'+String(n++).padStart(12,'0');
const eq=(a,b,label)=>{assert.deepEqual(a,b,label);checks++;};
const deny=async(sql,args=[],codes=['42501','23514','23505','22P02','54000','22007','22008'])=>{try{await db.query(sql,args)}catch(e){assert(codes.includes(e.code),e.code+': '+e.message);checks++;return}assert.fail('Expected denial '+sql)};
const owner=id(),hr=id(),manager=id(),person=id(),other=id(),specialist=id();
const email=u=>'services-'+u+'@example.invalid';
for(const u of[owner,hr,manager,person,other,specialist])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3)',[u,email(u),JSON.stringify({role:'administrator',staff_roles:['administrator']})]);
const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional employee services',$1)returning id",[owner])).rows[0].id;
const session=async(user,role='authenticated')=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user??'']);await db.exec('set role '+role)};
const rpc=async(name,arg)=>(await db.query('select public.'+name+'('+(arg===undefined?'':'$1::jsonb')+')r',arg===undefined?[]:[JSON.stringify(arg)])).rows[0].r;
await session(owner);for(const[u,r]of[[hr,'hr_generalist'],[manager,'manager'],[person,'employee'],[specialist,'employee']])await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[u,email(u),r]);
await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company]);
const employees=Object.fromEntries((await db.query('select user_id,employee_id from public.company_members where company_id=$1',[company])).rows.map(r=>[r.user_id,r.employee_id]));
const role=async(user,roles,active=true)=>{await session(owner);const real=(await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;const a=real.accounts.find(x=>x.user_id===user);return rpc('set_staff_roles',{id:id(),user_id:user,expected_revision:a.revision,expected_base_role:a.base_role,expected_member_updated_at:a.member_updated_at,roles,active,reason:'Fictional services access review'});};
const read=()=>rpc('employee_service_context'),own=()=>rpc('portal_service_context'),save=r=>rpc('save_employee_service',r),saveOwn=r=>rpc('portal_save_service',r);
const request=(actor,rid,revision,change)=>({id:id(),company_id:company,actor_id:actor,request_id:rid,expected_revision:revision,change});
const submit=(actor,employee=employees[actor])=>request(actor,id(),0,{kind:'submit',employee_id:employee,title:'Fictional employment document request',category:'employment_document',priority:'normal',body:'Fictional request; no official document is created automatically'});
const ownSubmit=()=>({id:id(),request_id:id(),expected_revision:0,change:{kind:'submit',title:'Fictional own benefits question',category:'benefits_question',priority:'normal',body:'Fictional shared employee message'}});
await session(owner);eq((await read()).requests,[],'Empty service queue');eq((await own()).requests,[],'Admin portal stays own');
const first=submit(owner,employees[person]);let receipt=await save(first),rid=first.request_id;
eq(await save(first),receipt,'Exact assisted retry');eq(receipt.revision,1,'First revision');
let c=await read();eq(c.requests[0].created_by,owner,'Actual submitter retained');eq(c.requests[0].employee_id,employees[person],'Separate requester retained');eq(c.events.length,1,'Initial shared history');eq(c.handlers.length,3,'Active staff handlers only');eq((await own()).requests,[],'Admin does not inherit company queue in portal');
await session(hr);eq((await read()).requests.length,1,'HR coordinates queue');
const route=request(hr,rid,1,{kind:'route',assignee_user_id:manager,due_on:'2026-11-03',body:'Fictional reviewed routing to manager'});await save(route);
await session(manager);c=await read();eq(c.requests.length,1,'Specific assignment grants service access');eq(c.handlers,[],'Assigned staff has no routing directory');eq((await own()).requests,[],'Assigned work is absent from own portal');
await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify(request(manager,rid,2,{kind:'route',assignee_user_id:hr,due_on:null,body:'Cannot route without personnel responsibility'}))]);
await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify(request(manager,rid,2,{kind:'cancel',body:'Cannot cancel another requester service'}))]);
const progress=request(manager,rid,2,{kind:'progress',status:'in_progress',body:'Fictional handler started reviewing'});await save(progress);
await session(person);await deny('select public.employee_service_context()');c=await own();eq(c.requests.length,1,'Requester sees assisted service in portal');eq(c.requests[0].messages.length,3,'Shared history retained');eq(typeof c.requests[0].handler_name,'string','Captured handler name');

const browserKeys=['id','code','title','category','priority','status','handler_name','due_on','revision','created_at','updated_at','can_reply','can_cancel','messages'].sort();
eq(Object.keys(c.requests[0]).sort(),browserKeys,'Own ticket field allowlist');
for(const e of c.requests[0].messages)eq(Object.keys(e).sort(),['id','revision','kind','actor_name','body','status','created_at'].sort(),'Own message field allowlist');
const ownReply={id:id(),request_id:rid,expected_revision:3,change:{kind:'reply',body:'Fictional requester clarified the request'}};const ownReceipt=await saveOwn(ownReply);eq(await saveOwn(ownReply),ownReceipt,'Own reply exact retry');
const newOwn=ownSubmit();await saveOwn(newOwn);eq((await own()).requests.length,2,'Own request can be submitted');
for(const invalid of[{...newOwn,change:{...newOwn.change,employee_id:employees[owner]}},{...newOwn,actor_id:owner},{...newOwn,company_id:company},{...ownReply,id:id(),change:{kind:'route',assignee_user_id:person,body:'Portal cannot route'}},{...ownReply,id:id(),change:{kind:'progress',status:'completed',body:'Portal cannot impersonate handling staff'}}])await deny('select public.portal_save_service($1::jsonb)',[JSON.stringify(invalid)]);
await session(hr);c=await read();const reroute=request(hr,rid,4,{kind:'route',assignee_user_id:null,due_on:null,body:'Fictional return to the HR service queue'});await save(reroute);
await session(manager);eq((await read()).requests,[],'Unassigned manager has no team service access');await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify(progress)]);
await session(hr);await save(request(hr,rid,5,{kind:'progress',status:'completed',body:'Fictional service completed after manual review'}));
await session(person);c=await own();eq(c.requests.find(x=>x.id===rid).can_reply,false,'Terminal controls hidden');eq(c.requests.find(x=>x.id===rid).messages.length,6,'Complete retained history');await deny('select public.portal_save_service($1::jsonb)',[JSON.stringify({...ownReply,id:id(),expected_revision:6})]);
await session(manager);await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify(request(manager,rid,6,{kind:'progress',status:'in_progress',body:'Only HR can reopen terminal work'}))]);
await session(hr);await save(request(hr,rid,6,{kind:'progress',status:'in_progress',body:'Fictional reviewed reopening'}));
await session(person);const cancel={id:id(),request_id:rid,expected_revision:7,change:{kind:'cancel',body:'Fictional requester withdrew this service'}};await saveOwn(cancel);eq((await own()).requests.find(x=>x.id===rid).status,'cancelled','Requester can cancel active own service');
await role(specialist,['recruiter']);await session(hr);c=await read();eq(c.handlers.some(x=>x.user_id===specialist),true,'Protected specialist is a routing candidate');
await save(request(hr,newOwn.request_id,1,{kind:'route',assignee_user_id:specialist,due_on:null,body:'Fictional specialist assignment'}));
await session(specialist);eq((await read()).requests.map(x=>x.id),[newOwn.request_id],'Specialist gets exact assigned work');eq((await own()).requests,[],'Specialist portal remains own');
const note=request(specialist,newOwn.request_id,2,{kind:'reply',body:'Fictional specialist shared reply'});await save(note);
await role(specialist,['employee']);await session(specialist);await deny('select public.employee_service_context()');await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify(note)]);eq((await own()).requests,[],'Role removal retains only own access');
await session(hr);const hrOwn=submit(hr);await save(hrOwn);const hrRoute=request(hr,hrOwn.request_id,1,{kind:'route',assignee_user_id:manager,due_on:null,body:'Fictional own request routing before role removal'});await save(hrRoute);
await role(hr,['manager']);await session(hr);eq((await read()).requests.some(x=>x.id===hrOwn.request_id),true,'Former HR retains own service access');await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify(hrRoute)]);await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify(route)]);
await role(hr,['hr_generalist']);
for(const user of[owner,hr,manager,person]){await session(user);for(const table of['service_requests','service_events'])await deny('select * from hris_private.'+table);await deny('select hris_private.service_member(false)');}
for(const r of['anon','service_role']){await session(null,r);for(const f of['employee_service_context','portal_service_context'])await deny('select public.'+f+'()');await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify(first)]);await deny('select public.portal_save_service($1::jsonb)',[JSON.stringify(newOwn)]);}
const fixture=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
for(const test of fixture){await role(manager,test.roles);await session(manager);if(test.capabilities.includes('desktop')){const v=await read();eq(v.can_route,test.capabilities.includes('personnel'),'Protected role routing');eq(v.requests.every(x=>test.capabilities.includes('personnel')||x.employee_id===employees[manager]||x.assignee_user_id===manager),true,'Own or exact assigned staff scope');}else await deny('select public.employee_service_context()');const v=await own();eq(v.requests.every(x=>x.id!==rid&&x.id!==newOwn.request_id),true,'All protected roles remain own-only in portal');}
await role(manager,['manager']);
await session(other);await deny('select public.employee_service_context()');await deny('select public.portal_service_context()');
await session(hr);const stable=await read(),target=stable.requests.find(x=>x.id===newOwn.request_id);
for(const invalid of[{...first,id:id(),expected_revision:0,change:{...first.change,employee_id:other}},{...first,id:id(),request_id:id(),company_id:other},{...first,id:id(),request_id:id(),actor_id:person},{...first,id:id(),request_id:id(),unexpected:true},{...first,id:id(),request_id:id(),expected_revision:'0'},{...first,id:id(),request_id:id(),change:{...first.change,body:' padded '}},{...first,id:id(),request_id:id(),change:{...first.change,body:'Forbidden\u0001control'}},{...first,id:id(),request_id:id(),change:{...first.change,body:'x'.repeat(25000)}},{...first,id:id(),request_id:id(),change:{...first.change,title:123}},{...first,id:id(),request_id:id(),change:{...first.change,category:'case'}},{...first,id:id(),request_id:id(),change:{...first.change,salary:40000}}])await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify(invalid)]);
for(const change of[{kind:'route',assignee_user_id:person,due_on:null,body:'Cannot route to an ordinary employee'},{kind:'route',assignee_user_id:manager,due_on:'1999-01-01',body:'Invalid due date'}])await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify(request(hr,newOwn.request_id,target.revision,change))]);

await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify({...route,change:{...route.change,body:'Changed retry payload'}})]);
eq(await read(),stable,'Rejected requests preserve records/history');
await db.exec('reset role');await db.exec("create function public.service_qa_fault()returns trigger language plpgsql as $$begin raise exception'Fictional audit fault';end$$;create trigger service_qa_fault before insert on hris_private.service_events for each row execute function public.service_qa_fault();");
await session(hr);await deny('select public.save_employee_service($1::jsonb)',[JSON.stringify(request(hr,newOwn.request_id,target.revision,{kind:'reply',body:'Fictional audit rollback test'}))],['P0001']);eq(await read(),stable,'Audit failure rolls back complete service change');
await db.exec('reset role');await db.exec('drop trigger service_qa_fault on hris_private.service_events;drop function public.service_qa_fault()');
await deny('update hris_private.service_events set body=body',[],['23514']);await deny('delete from hris_private.service_events',[],['23514']);await deny('delete from hris_private.service_requests',[],['23514']);
eq((await db.query("select bool_and(not prosecdef)v from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='public'and proname in('employee_service_context','save_employee_service','portal_service_context','portal_save_service')")).rows[0].v,true,'Public wrappers are invokers');
await role(person,['employee'],false);await session(person);await deny('select public.portal_service_context()');await deny('select public.portal_save_service($1::jsonb)',[JSON.stringify(cancel)]);

// Explicit overflow and foreign target failures preserve the prior stored request.
await db.exec('reset role');const beforeBudget=(await db.query('select count(*)::int n from hris_private.service_requests')).rows[0].n;
await db.exec('begin');
await db.query(`insert into hris_private.service_requests(id,company_id,employee_id,requester_code,requester_name,created_by,code,title,category,priority,status,revision,created_at,updated_at)
 select u,$1,$2,'QA-BUDGET','Fictional budget employee',$3,'SR-'||upper(replace(u::text,'-','')),'Fictional request budget','general_support','normal','submitted',1,now(),now() from(select gen_random_uuid()u from generate_series(1,1001))x`,[company,employees[person],owner]);
await session(owner);await deny('select public.employee_service_context()',[],['54000']);await db.exec('rollback');await db.exec('reset role');
eq((await db.query('select count(*)::int n from hris_private.service_requests')).rows[0].n,beforeBudget,'Overflow fixture rolled back');
console.log(JSON.stringify({status:'PASS',assertions:checks,migrations:count,original_record_sets_preserved:Object.keys(preservation.facts).length,original_functions_preserved:preservation.functions.length}));
}finally{await db.close()}
