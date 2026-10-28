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
 const folder=new URL('../../supabase/migrations/',import.meta.url);let count=0;const safeguardFile=(await readdir(folder)).find(f=>f.endsWith('_staff_account_safeguards.sql'));assert.ok(safeguardFile);
 for(const file of(await readdir(folder)).filter(f=>f.endsWith('.sql')&&f!==safeguardFile).sort()){
  try{await db.exec(await readFile(new URL(file,folder),'utf8'));}
  catch(e){throw Error('Fresh replay failed at '+file+': '+e.message,{cause:e});}
  await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[file.split('_')[0]]);count++;
 }

 let checks=0;const eq=(a,b,label)=>{assert.deepEqual(a,b,label);checks++;};
 const deny=async(sql,params=[],codes=['42501','23514'])=>{try{await db.query(sql,params);}catch(e){assert.ok(codes.includes(e.code),e.code+': '+e.message);checks++;return;}assert.fail('Expected denial: '+sql);};
 let serial=1;const id=()=>`38000000-0000-4000-8000-${String(serial++).padStart(12,'0')}`;
 const owner=id(),subject=id(),hr=id(),manager=id(),admin=id(),unassigned=id();
 const email=u=>`fictional-${u}@example.invalid`;
 for(const user of[owner,subject,hr,manager,admin,unassigned])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3::jsonb)',[user,email(user),JSON.stringify({role:'administrator',roles:['administrator']})]);
 const company=(await db.query("insert into public.companies(name,owner_id,timezone)values('Fictional account safeguards company',$1,'Asia/Manila')returning *",[owner])).rows[0];
 const session=async(user,role='authenticated')=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user??'']);await db.exec('set role '+role);};
 const rpc=async(name,...args)=>(await db.query(`select public.${name}(${args.map((_,i)=>'$'+(i+1)).join(',')})r`,args)).rows[0].r;
 await session(owner);for(const[u,role]of[[subject,'manager'],[hr,'hr_generalist'],[manager,'manager'],[admin,'administrator']])await rpc('save_company_account',u,email(u),role,true,null,null);
 const people=(await db.query('select user_id,employee_id from public.company_members')).rows;
 const employee=u=>people.find(p=>p.user_id===u).employee_id;
 await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company.id]);
 const current=async u=>{const c=await rpc('staff_role_admin_context',true);return c.accounts.find(a=>a.user_id===u);};
 const request=async(u,roles,active=true)=>{const a=await current(u);return{id:id(),user_id:u,roles,active,expected_revision:a.revision,expected_base_role:a.base_role,expected_member_updated_at:a.member_updated_at,reason:'Fictional reviewed account safeguard'};};
 const set=async(u,roles,active=true)=>rpc('set_staff_roles',JSON.stringify(await request(u,roles,active)));
 await session(owner);await set(subject,['manager','learning_performance_officer']);await set(hr,['hr_generalist','recruiter','learning_performance_officer','payroll_officer']);
 await rpc('save_company_account',subject,email(subject),'manager',false,null,null);
 await rpc('save_company_account',hr,email(hr),'manager',true,null,null);
 const publicSnapshot=async()=>{const snap={};for(const t of['company_members','account_access_events','employees','employee_contacts'])snap[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;return snap;};
 await db.exec('reset role');const prior=await publicSnapshot();
 const oldEvents=(await db.query('select *from hris_private.staff_role_events order by created_at,id')).rows;
 const oldReceipts=(await db.query('select *from hris_private.staff_role_change_receipts order by id')).rows;
 await db.exec(await readFile(new URL(safeguardFile,folder),'utf8'));count++;
 eq(await publicSnapshot(),prior,'Deployment reconciliation changes no original membership, personnel, contact or access event');
 eq((await db.query('select *from hris_private.staff_role_events where id=any($1::uuid[])order by created_at,id',[oldEvents.map(e=>e.id)])).rows,oldEvents,'All earlier role history stays byte-equivalent');
 eq((await db.query('select *from hris_private.staff_role_change_receipts order by id')).rows,oldReceipts,'Original retry receipts stay immutable');
 await session(owner);let a=await current(subject);eq([a.roles,a.active],[['learning_performance_officer','manager'],false],'Backfill captures status while preserving protected specialties');
 eq((await current(hr)).roles,['manager'],'Backfill reconciles a legitimate legacy base-role change');
 const baseline=(await db.query('select *from public.account_access_events where user_id=$1',[subject])).rows.length;
 await deny('select public.save_company_account($1,$2,$3,true,null,null)',[subject,email(subject),'manager']);
 await set(subject,['manager','learning_performance_officer']);
 const beforeDisable=await current(subject);await rpc('save_company_account',subject,email(subject),'manager',false,null,null);a=await current(subject);
 eq([a.roles,a.active,a.revision],[beforeDisable.roles,false,beforeDisable.revision+1],'Legacy disable preserves roles and advances protected revision');
 await db.exec('reset role');let event=(await db.query('select *from hris_private.staff_role_events where user_id=$1 order by revision desc limit 1',[subject])).rows[0];
 eq([event.actor_id,event.previous_active,event.active],[owner,true,false],'Legacy change records the actual Administrator and both access states');
 await session(owner);await set(subject,['manager','learning_performance_officer']);
 await db.exec('reset role');await db.query('update public.employees set reports_to=$1 where id=$2',[employee(subject),employee(manager)]);
 await session(owner);await set(subject,['learning_performance_officer']);
 eq((await current(subject)).base_role,'employee','Higher staff may retain reports without inheriting Manager authority');
 await session(subject);eq((await rpc('my_hris_access')).direct_reports,[],'Reports metadata alone never grants team-review authority');
 await session(owner);const beforeEmployee=await current(subject);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(await request(subject,['employee']))]);
 eq(await current(subject),beforeEmployee,'True portal-only Employee transition remains blocked until reports are reassigned');
 const five=['hr_generalist','recruiter','learning_performance_officer','payroll_officer','manager'];await set(subject,five);await session(subject);
 const combined=await rpc('my_hris_access');eq(combined.role,'hr_generalist','Legacy compatible full bundle stays narrow');eq(combined.direct_reports,[employee(manager)],'Explicit Manager duty works even with HR compatibility base');
 await session(owner);await deny('select public.save_company_account($1,$2,$3,false,$4,null)',[subject,email(subject),'hr_generalist',employee(hr)]);
 await deny('select public.save_company_account($1,$2,$3,false,null,null)',[owner,email(owner),'administrator']);
 await session(subject);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify({...await(async()=>({id:id(),user_id:subject,roles:five,active:false,expected_revision:1,expected_base_role:'hr_generalist',expected_member_updated_at:new Date().toISOString(),reason:'Fictional forged self change'}))()})]);
 await session(owner);await deny('select public.save_company_account($1,$2,$3,true,null,null)',[unassigned,email(unassigned),'employee']);
 const eventBefore=(await db.query('select count(*)::int n from public.account_access_events')).rows[0].n;
 const stateBefore=await current(subject);
 await db.exec('reset role');const rolesBefore=(await db.query('select *from hris_private.staff_role_assignments order by user_id')).rows;
 const allEventsBefore=(await db.query('select *from hris_private.staff_role_events order by id')).rows;
 await db.exec("create function hris_private.account_test_fault()returns trigger language plpgsql as $$begin if new.kind='assignment_changed'then raise exception'fictional account audit failure';end if;return new;end;$$;create trigger account_test_fault before insert on hris_private.staff_role_events for each row execute function hris_private.account_test_fault();");
 await session(owner);await deny('select public.save_company_account($1,$2,$3,false,null,null)',[subject,email(subject),'hr_generalist'],['P0001']);
 eq(await current(subject),stateBefore,'Audit failure restores membership, time, full roles and revision');eq((await db.query('select count(*)::int n from public.account_access_events')).rows[0].n,eventBefore,'Audit failure rolls back original access event too');
 await db.exec('reset role');eq((await db.query('select *from hris_private.staff_role_assignments order by user_id')).rows,rolesBefore,'Audit failure preserves all protected assignments');
 eq((await db.query('select *from hris_private.staff_role_events order by id')).rows,allEventsBefore,'Audit failure preserves all protected history');
 await db.exec('drop trigger account_test_fault on hris_private.staff_role_events;drop function hris_private.account_test_fault();');
 for(const role of['authenticated','service_role'])for(const signature of['hris_private.save_company_account_scoped(uuid,text,text,boolean,uuid,uuid,text[])','hris_private.reconcile_staff_assignment(uuid,uuid,uuid,text)'])eq((await db.query('select has_function_privilege($1,$2::regprocedure,$3)v',[role,signature,'EXECUTE'])).rows[0].v,false,'No direct prospective-role or reconciliation bypass '+role);
 await session(subject);await deny('select hris_private.save_company_account_scoped($1,$2,$3,true,$4,null,$5)',[subject,email(subject),'administrator',employee(subject),['administrator']]);
 await deny('select *from hris_private.staff_role_events');
 await db.exec('reset role');await deny('update hris_private.staff_role_events set reason=$1',['Rewrite history']);
 console.log('Passed '+checks+' account safeguard assertions across'+count+'migrations, including deployed-state reconciliation, legacy disables, supervisor identity, role-bound team scopes and atomic audit rollback.');
}catch(error){console.error(JSON.stringify({message:error.message,code:error.code,where:error.where,stack:error.stack}));process.exitCode=1;}finally{await db.close();}
