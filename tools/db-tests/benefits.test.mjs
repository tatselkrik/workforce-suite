// Fresh chronological migration replay; managed Auth/Storage are local fixtures.
import{readFile,readdir,mkdir,writeFile}from'node:fs/promises';
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
  if(file.endsWith('_benefits_administration.sql'))preservation=await originals();
  try{await db.exec(await readFile(new URL(file,folder),'utf8'));}
  catch(e){throw Error('Fresh replay failed at '+file+': '+e.message,{cause:e});}
  if(file.endsWith('_benefits_administration.sql')){const after=await originals();for(const [name,rows]of Object.entries(preservation.facts))assert.deepEqual(after.facts[name],rows,name);assert.deepEqual(after.auth,preservation.auth);for(const f of preservation.functions)assert.deepEqual(after.functions.find(v=>v.identity===f.identity),f);}
  await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[file.split('_')[0]]);count++;
 }

 


let checks=0,n=1;const id=()=>'68000000-0000-4000-8000-'+String(n++).padStart(12,'0');
const eq=(a,b,label)=>{assert.deepEqual(a,b,label);checks++;};
const deny=async(sql,args=[],codes=['42501','23514','23505','22P02','54000','22007','22008'])=>{try{await db.query(sql,args)}catch(e){assert(codes.includes(e.code),e.code+': '+e.message);checks++;return}assert.fail('Expected denial '+sql)};
const owner=id(),hr=id(),manager=id(),person=id(),other=id(),specialist=id();
const email=u=>'benefits-'+u+'@example.invalid';
for(const u of[owner,hr,manager,person,other,specialist])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3)',[u,email(u),JSON.stringify({role:'administrator',staff_roles:['administrator']})]);
const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional benefits administration',$1)returning id",[owner])).rows[0].id;
const session=async(user,role='authenticated')=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user??'']);await db.exec('set role '+role)};
const rpc=async(name,arg)=>(await db.query('select public.'+name+'('+(arg===undefined?'':'$1::jsonb')+')r',arg===undefined?[]:[JSON.stringify(arg)])).rows[0].r;
await session(owner);for(const[u,r]of[[hr,'hr_generalist'],[manager,'manager'],[person,'employee'],[specialist,'employee']])await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[u,email(u),r]);
await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company]);
const employees=Object.fromEntries((await db.query('select user_id,employee_id from public.company_members where company_id=$1',[company])).rows.map(r=>[r.user_id,r.employee_id]));
const role=async(user,roles,active=true)=>{await session(owner);const real=(await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;const a=real.accounts.find(x=>x.user_id===user);return rpc('set_staff_roles',{id:id(),user_id:user,expected_revision:a.revision,expected_base_role:a.base_role,expected_member_updated_at:a.member_updated_at,roles,active,reason:'Fictional services access review'});};

const read=()=>rpc('employee_benefit_context'),save=r=>rpc('save_employee_benefit',r);
const request=(actor,subject,revision,kind,data,body='Fictional company-reviewed benefit change')=>({id:id(),company_id:company,actor_id:actor,subject_id:subject,expected_revision:revision,body,change:{kind,data}});
const plan=()=>({code:'HEALTH-01',name:'Fictional health coverage',category:'health_insurance',description:'Company-reviewed prototype benefit with no insurer connection',eligibility_guidance:'HR records reviewed eligibility against the current employee source',dependent_limit:2,available:true});
await session(owner);let c=await read();eq(c.plans,[],'Empty benefits catalog');eq(c.enrollments,[],'Empty enrollments');eq(c.events,[],'No history truncation');eq(c.user_id,owner,'Actor-bound context');
const vectors=[structuredClone(c)];
const planId=id(),first=request(owner,planId,0,'plan',plan());let receipt=await save(first);eq(await save(first),receipt,'Exact plan retry');eq(receipt.revision,1,'Initial plan revision');
c=await read();eq(c.plans[0].data,plan(),'Plan contents retained');eq(c.events[0].current.kind,'plan','Typed plan state');eq(c.events[0].previous,null,'Explicit initial history');
const employee=c.employees.find(e=>e.id===employees[person]);const enrollment=()=>({plan_id:planId,plan_revision:1,plan_snapshot:plan(),employee_source:employee,eligibility:'eligible',eligibility_basis:'Explicit reviewed company conditions; no statutory assumption',status:'active',coverage_from:'2026-10-01',coverage_to:null,dependents:[]});
const enrId=id(),enrRequest=request(owner,enrId,0,'enrollment',enrollment());await save(enrRequest);eq(await save(enrRequest),await save(enrRequest),'Exact enrollment retry');
c=await read();eq(c.enrollments[0].data.employee_source,employee,'Current employment captured');eq(c.enrollments[0].revision,1,'First enrollment revision');eq(c.events.length,2,'Two immutable source records');
vectors.push(structuredClone(c));const oldEnr=structuredClone(c.enrollments[0]);
const revised=plan();revised.name='Fictional changed plan name';const changePlan=request(owner,planId,1,'plan',revised);await save(changePlan);
c=await read();eq(c.enrollments[0],oldEnr,'Changing plan does not rewrite existing review');eq(c.plans[0].revision,2,'Changed plan version');
await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify(request(owner,id(),0,'enrollment',enrollment()))]);
const refreshed={...enrollment(),plan_revision:2,plan_snapshot:revised,eligibility_basis:'Fresh explicit review of revised plan and current employment'};
await save(request(owner,enrId,1,'enrollment',refreshed));eq((await read()).enrollments[0].revision,2,'Explicit enrollment re-review');
await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify(request(owner,id(),0,'enrollment',refreshed))]);
for(const eligibility of['pending','ineligible'])await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify(request(owner,id(),0,'enrollment',{...refreshed,eligibility}))]);
await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify({...enrRequest,id:id(),actor_id:hr})]);
await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify({...first,body:'Changed retry payload'})]);
await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify(request(owner,id(),0,'plan',revised))]);
for(const extra of[{...first,id:id(),private_salary:100},{...first,id:id(),change:{...first.change,ignored:true}},{...first,id:id(),change:{kind:'plan',data:{...plan(),unknown:1}}}])await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify(extra)]);
const dep={id:id(),full_name:'Fictional dependent',relationship:'child',birth_date:'2020-01-01',coverage_from:'2026-10-01',coverage_to:null};
let dependentReview={...refreshed,dependents:[dep,{...dep,id:id()}]};await save(request(owner,enrId,2,'enrollment',dependentReview));
c=await read();eq(c.enrollments[0].data.dependents.length,2,'Namesakes retained with distinct identities');vectors.push(structuredClone(c));
for(const ds of[[dep,dep],[dep,{...dep,id:id()},{...dep,id:id()}],[{...dep,coverage_from:'2026-09-30'}],[{...dep,birth_date:'2027-01-01'}]])await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify(request(owner,enrId,3,'enrollment',{...dependentReview,dependents:ds}))]);
await session(hr);c=await read();eq(c.plans.length,1,'HR current personnel access');eq(c.enrollments.length,1,'HR enrollment administration');
const roleCases=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
for(const test of roleCases){await role(specialist,test.roles);await session(specialist);if(test.capabilities.includes('personnel')){eq((await read()).user_id,specialist,'Authorized role combination');}else{await deny('select public.employee_benefit_context()');await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify({...first,actor_id:specialist})]);}}
for(const user of[owner,hr,manager,person,specialist]){await session(user);for(const table of['benefit_records','benefit_events']){await deny('select * from hris_private.'+table);await deny('delete from hris_private.'+table);}}

await session(null,'anon');await deny('select public.employee_benefit_context()');await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify(first)]);
await session(owner,'service_role');await deny('select public.employee_benefit_context()');
await session(owner);await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify({...first,company_id:other})]);
// A prepared source must not survive a canonical employment edit.
let nextPerson=(await read()).employees.find(e=>e.id===employees[hr]);let prepared={...refreshed,employee_source:nextPerson,dependents:[]};
await db.exec('reset role');await db.query("update public.employees set department='Changed since review'where id=$1",[nextPerson.id]);
await session(owner);await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify(request(owner,id(),0,'enrollment',prepared))]);
// Archiving retains coverage; only an explicit reviewed ending changes it.
await role(person,['employee'],false);await db.exec('reset role');await db.query("update public.employees set status='archived'where id=$1",[employee.id]);
await session(owner);c=await read();eq(c.enrollments[0].data.status,'active','Archive never silently ends benefits');
let ended={...dependentReview,status:'ended',coverage_to:'2026-10-28',dependents:dependentReview.dependents.map(d=>({...d,coverage_to:'2026-10-28'}))};
await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify(request(owner,enrId,3,'enrollment',{...ended,dependents:[]}))]);
const endRequest=request(owner,enrId,3,'enrollment',ended);await save(endRequest);eq(await save(endRequest),await save(endRequest),'Ending exact retry');
c=await read();eq(c.enrollments[0].data.status,'ended','Explicit ended coverage');eq(c.enrollments[0].revision,4,'Complete enrollment revisions');eq(c.events.length,6,'Plan and enrollment history retained');vectors.push(structuredClone(c));
await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify(request(owner,enrId,4,'enrollment',{...ended,status:'active',coverage_to:null}))]);
await role(hr,['manager']);await session(hr);await deny('select public.employee_benefit_context()');await deny('select public.save_employee_benefit($1::jsonb)',[JSON.stringify({...endRequest,actor_id:hr})]);
await session(owner);const auditBefore=(await read()).events;
await db.exec('reset role');await deny('update hris_private.benefit_events set body=$1 where id=$2',['Cannot rewrite',first.id]);await deny('delete from hris_private.benefit_records where id=$1',[planId]);
await session(owner);eq((await read()).events,auditBefore,'Rejected audit mutations retain exact history');
// The no-charge context guard rejects excess records and rolls the fixture back.
await db.exec('reset role');await db.exec('begin');
await db.query(`insert into hris_private.benefit_records(id,company_id,kind,revision,data,created_at,updated_at)
select gen_random_uuid(),$1,'plan',1,$2::jsonb||jsonb_build_object('code','BUDGET-'||n),now(),now()from generate_series(1,501)n`,[company,JSON.stringify(plan())]);
await session(owner);await deny('select public.employee_benefit_context()',[],['54000']);await db.exec('rollback');await db.exec('reset role');
eq((await db.query('select count(*)::int n from hris_private.benefit_records')).rows[0].n,2,'Overflow fixture rolled back');
const vectorFolder=new URL('../../work/benefits-db-vectors/',import.meta.url);await mkdir(vectorFolder,{recursive:true});for(const [i,v]of vectors.entries())await writeFile(new URL('context-'+i+'.json',vectorFolder),JSON.stringify(v));
console.log(JSON.stringify({status:'PASS',assertions:checks,migrations:count,original_record_sets_preserved:Object.keys(preservation.facts).length,original_functions_preserved:preservation.functions.length}));
}finally{await db.close()}
