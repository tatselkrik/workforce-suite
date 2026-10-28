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
 const originals=async()=>{const tables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows;const facts={};for(const t of tables){assert(/^[a-z0-9_]+$/.test(t.tablename));facts[t.tablename]=(await db.query('select to_jsonb(x)r from public.'+t.tablename+' x order by to_jsonb(x)::text')).rows;}return{facts,auth:(await db.query('select to_jsonb(x)r from auth.users x order by id')).rows,functions:(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest,p.proacl::text,p.proowner from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows};};
 for(const file of(await readdir(folder)).filter(f=>f.endsWith('.sql')).sort()){
  if(file==='20261029022500_employee_relations.sql')preservation=await originals();
  try{await db.exec(await readFile(new URL(file,folder),'utf8'));}
  catch(e){throw Error('Fresh replay failed at '+file+': '+e.message,{cause:e});}
  if(file==='20261029022500_employee_relations.sql'){const after=await originals();assert.deepEqual(after.facts,preservation.facts);assert.deepEqual(after.auth,preservation.auth);for(const f of preservation.functions)assert.deepEqual(after.functions.find(v=>v.identity===f.identity),f);}
  await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[file.split('_')[0]]);count++;
 }

 

let checks=0,n=1;const id=()=>'66000000-0000-4000-8000-'+String(n++).padStart(12,'0');
const eq=(a,b,label)=>{assert.deepEqual(a,b,label);checks++;};
const deny=async(sql,args=[],codes=['42501','23514','23505','22P02','54000'])=>{try{await db.query(sql,args)}catch(e){assert(codes.includes(e.code),e.code+': '+e.message);checks++;return}assert.fail('Expected denial '+sql)};
const owner=id(),hr1=id(),hr2=id(),manager=id(),person=id(),other=id();const users=[owner,hr1,hr2,manager,person,other];
const email=u=>'relations-'+u+'@example.invalid';
for(const u of users)await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3)',[u,email(u),JSON.stringify({role:'administrator',staff_roles:['administrator']})]);
const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional confidential cases',$1)returning id",[owner])).rows[0].id;
const session=async(user,role='authenticated')=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user??'']);await db.exec('set role '+role)};
const rpc=async(name,arg)=>(await db.query('select public.'+name+'('+(arg===undefined?'':'$1::jsonb')+')r',arg===undefined?[]:[JSON.stringify(arg)])).rows[0].r;
await session(owner);for(const[u,r]of[[hr1,'hr_generalist'],[hr2,'hr_generalist'],[manager,'manager'],[person,'employee']])await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[u,email(u),r]);
await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company]);
const employee=(await db.query('select employee_id from public.company_members where user_id=$1',[person])).rows[0].employee_id;
const role=async(user,roles,active=true)=>{await session(owner);const real=(await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;const a=real.accounts.find(x=>x.user_id===user);return rpc('set_staff_roles',{id:id(),user_id:user,expected_revision:a.revision,expected_base_role:a.base_role,expected_member_updated_at:a.member_updated_at,roles,active,reason:'Fictional case access check'});};
const read=()=>rpc('employee_relation_context'),save=r=>rpc('save_employee_relation',r);
const request=(actor,cid,revision,change)=>({id:id(),company_id:company,actor_id:actor,case_id:cid,expected_revision:revision,change});
const create=(actor,handler=actor)=>request(actor,id(),0,{kind:'create',employee_id:employee,owner_id:handler,title:'Fictional workplace concern',category:'workplace_concern',priority:'normal',body:'Fictional initial confidential description'});
await session(owner);eq((await read()).cases.length,0,'Starts empty');
let first=create(owner,hr1),receipt=await save(first),cid=first.case_id;
eq(receipt.revision,1,'Create revision');eq(await save(first),receipt,'Exact create retry');
let c=await read();eq(c.cases.length,1,'Admin oversight');eq(c.events.length,1,'Immutable initial event');eq(c.events[0].body,first.change.body,'Description preserved');eq(c.handlers.length,3,'Only active HR/Admin handlers');
await session(hr2);eq((await read()).cases,[],'Unassigned HR sees no case metadata');await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(request(hr2,cid,1,{kind:'note',body:'Unassigned attempt'}))]);
await session(hr1);eq((await read()).cases[0].id,cid,'Assigned HR reads');await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify({...first,actor_id:hr1})]);
const self=create(hr1);await save(self);await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(create(hr1,hr2))]);
let note=request(hr1,cid,1,{kind:'note',body:'Fictional follow-up interview recorded'}),r2=await save(note);eq(r2.revision,2,'Note revision');eq(await save(note),r2,'Exact note retry');
for(const invalid of[{...note,id:id(),actor_id:owner},{...note,id:id(),company_id:other},{...note,id:id(),unexpected:true},{...note,id:'00000000-0000-0000-0000-000000000000'},{...note,id:id(),expected_revision:-1},{...note,id:id(),change:{kind:'note',body:'  padded  '}},{...note,id:id(),change:{kind:'note',body:'ok'}},{...note,id:id(),change:{kind:'erase',body:'Never delete history'}},{...note,change:{kind:'note',body:'Changed retry payload'}}])await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(invalid)]);
const fid=id(),follow=request(hr1,cid,2,{kind:'follow_up',follow_up_id:fid,due_on:'2026-11-03',body:'Fictional confidential check-in'});await save(follow);
await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(request(hr1,cid,3,{kind:'status',status:'resolved',body:'Cannot skip pending task'}))]);
await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(request(hr1,cid,3,{kind:'assign',owner_id:hr2,body:'Cannot assign a different handler'}))]);
await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(request(hr1,cid,3,{kind:'complete',follow_up_id:id(),body:'Foreign task denied'}))]);
await save(request(hr1,cid,3,{kind:'complete',follow_up_id:fid,body:'Fictional check-in completed with a retained note'}));
await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(request(hr1,cid,4,{kind:'complete',follow_up_id:fid,body:'Cannot complete twice'}))]);
await save(request(hr1,cid,4,{kind:'status',status:'resolved',body:'Fictional outcome and reviewed resolution'}));
await save(request(hr1,cid,5,{kind:'status',status:'closed',body:'Fictional case closure after completed follow-up'}));
await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(request(hr1,cid,6,{kind:'note',body:'Closed case must first be reopened'}))]);
await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(request(hr1,cid,6,{kind:'status',status:'in_review',body:'HR cannot reopen a closed case'}))]);
await session(owner);await save(request(owner,cid,6,{kind:'status',status:'in_review',body:'Fictional Administrator reopening reason'}));
await save(request(owner,cid,7,{kind:'assign',owner_id:hr2,body:'Fictional reviewed handover to another HR handler'}));
c=await read();eq(c.cases.find(x=>x.id===cid).revision,8,'All changes revisioned');eq(c.events.filter(x=>x.case_id===cid).length,8,'Complete history');eq(c.follow_ups[0].completion_note,'Fictional check-in completed with a retained note','Completion retained');
await session(hr1);eq((await read()).cases.map(x=>x.id),[self.case_id],'Former handler loses case');await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(note)]);
await session(hr2);eq((await read()).cases[0].id,cid,'New handler receives case/history');await save(request(hr2,cid,8,{kind:'note',body:'Fictional receiving HR review'}));
await role(hr2,['hr_generalist'],false);await session(hr2);await deny('select public.employee_relation_context()');await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(request(hr2,cid,9,{kind:'note',body:'Disabled token cannot save'}))]);
await role(hr2,['hr_generalist']);
const fixture=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
for(const test of fixture){await role(manager,test.roles);await session(manager);if(test.capabilities.includes('personnel')){const v=await read();eq(v.user_id,manager,'Current actor');eq(v.cases.length,test.roles.includes('administrator')?2:0,'Only Admin sees unassigned cases');}else{await deny('select public.employee_relation_context()');await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(create(manager))]);}}
await session(other);await deny('select public.employee_relation_context()');
for(const user of[owner,hr1,person]){await session(user);for(const table of['relation_cases','relation_events','relation_follow_ups'])await deny('select *from hris_private.'+table);await deny('select hris_private.relation_member()');}
for(const r of['anon','service_role']){await session(null,r);await deny('select public.employee_relation_context()');await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(first)]);}
await session(owner);
const stable=await read(),current=stable.cases.find(c=>c.id===cid);
for(const change of[{kind:'note',body:'\u00a0Leading nonbreaking space'},{kind:'note',body:'Contains\u0001control'},{kind:'note',body:'Trailing newline\n'},{kind:'note',body:1234},{kind:'note',body:'A valid note',unexpected:'forbidden'},{kind:'follow_up',body:'Fictional out-of-range date',follow_up_id:id(),due_on:'1999-12-31'},{kind:'assign',body:'Cannot assign a manager',owner_id:person},{kind:'status',body:'Invalid status',status:'deleted'}])await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(request(owner,cid,current.revision,change))]);
await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify({...request(owner,cid,current.revision,{kind:'note',body:'String revision rejected'}),expected_revision:String(current.revision)})]);
await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(request(owner,cid,current.revision,{kind:'note',body:'a'.repeat(25000)}))]);
eq(await read(),stable,'Rejected payloads preserve exact cases/history');
const foreign=create(owner);foreign.change.employee_id=other;await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(foreign)]);
const orphan=create(owner);orphan.change.owner_id=other;await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(orphan)]);
// Keep requests and full retry receipts out of the projected event feed.
eq(stable.events.every(e=>!('payload'in e)&&!('receipt'in e)),true,'Minimal event envelope');
await session(owner);const before=await read(),fault=request(owner,cid,9,{kind:'note',body:'Fictional audit rollback test'});
await db.exec('reset role');await db.exec("create function public.relation_qa_fault()returns trigger language plpgsql as $$begin raise exception'Fictional audit fault';end$$;create trigger relation_qa_fault before insert on hris_private.relation_events for each row execute function public.relation_qa_fault();");
await session(owner);await deny('select public.save_employee_relation($1::jsonb)',[JSON.stringify(fault)],['P0001']);eq(await read(),before,'Audit failure rolls back case changes');
await db.exec('reset role');await db.exec('drop trigger relation_qa_fault on hris_private.relation_events;drop function public.relation_qa_fault()');
await deny('update hris_private.relation_events set body=body',[],['23514']);await deny('delete from hris_private.relation_events',[],['23514']);await deny('delete from hris_private.relation_follow_ups',[],['23514']);await deny('delete from hris_private.relation_cases',[],['23514']);
eq((await db.query("select bool_and(not prosecdef)v from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='public'and proname in('employee_relation_context','save_employee_relation')")).rows[0].v,true,'Public wrappers invoker-only');
const totals=(await db.query('select(select count(*)::int from public.attendance)a,(select count(*)::int from public.leave_requests)l,(select count(*)::int from public.employees)e')).rows[0];eq(totals.a,0,'No clocks changed');eq(totals.l,0,'No leave changed');eq(totals.e,5,'No employee created by case work');
// Context overflow fails explicitly; the fixture insertion is always rolled back.
await db.exec('begin');
await db.query(`insert into hris_private.relation_cases(id,company_id,employee_id,owner_id,code,title,category,priority,status,revision,created_at,updated_at)
 select gen_random_uuid(),$1,$2,$3,'BUDGET-'||i,'Fictional budget case','other','normal','open',1,now(),now()from generate_series(1,1001)i`,[company,employee,owner]);
await session(owner);await deny('select public.employee_relation_context()',[],['54000']);await db.exec('rollback');
console.log(JSON.stringify({status:'PASS',assertions:checks,migrations:count,role_cases:fixture.length,original_public_sets_preserved:Object.keys(preservation.facts).length,original_functions_preserved:preservation.functions.length}));
}finally{await db.close()}
