// Real checked-in migration chain with fictional Auth/Storage and no hosted writes.
import assert from 'node:assert/strict';
import {readFile,readdir}from'node:fs/promises';
import{PGlite}from'@electric-sql/pglite';
import{btree_gist}from'@electric-sql/pglite/contrib/btree_gist';
const db=new PGlite({extensions:{btree_gist}});let checks=0;
const eq=(a,b,label)=>{assert.deepEqual(a,b,label);checks++;};
const deny=async(sql,args=[],codes=['42501','23514','23505','22P02'])=>{try{await db.query(sql,args);}catch(e){assert(codes.includes(e.code),`${e.code}: ${e.message}`);checks++;return;}assert.fail('Expected denial: '+sql);};
const id=n=>'00000000-0000-0000-0000-'+String(n).padStart(12,'0');
const owner=id(11),hr=id(12),manager=id(13),person=id(14),foreignOwner=id(15),admin2=id(16),archived=id(17);
const email=u=>'fixture-'+u.slice(-3)+'@example.invalid';
const session=async(user,role='authenticated')=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user??'']);await db.exec('set role '+role);};
let serial=100;
const context=async()=> (await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;
const change=async(user,roles,reason='Assign reviewed staff responsibilities')=>{
 const row=(await context()).accounts.find(a=>a.user_id===user);assert(row,'Target exists in protected role context');
 return{id:id(serial++),user_id:user,expected_revision:row.revision,expected_base_role:row.base_role,
 expected_member_updated_at:row.member_updated_at,active:row.active,roles,reason};
};
const save=async(request)=>(await db.query('select public.set_staff_roles($1::jsonb)r',[JSON.stringify(request)])).rows[0].r;
const account=async(user)=>(await db.query('select *from public.company_members where user_id=$1',[user])).rows[0];
try{
 await db.exec(`create role anon nologin;create role authenticated nologin;create role service_role nologin bypassrls;
 create schema supabase_migrations;create table supabase_migrations.schema_migrations(version text primary key);
 create schema auth;create table auth.users(id uuid primary key,email text unique,email_confirmed_at timestamptz,raw_user_meta_data jsonb default'{}'::jsonb);
 create function auth.uid()returns uuid language sql stable as $$select nullif(current_setting('request.jwt.claim.sub',true),'')::uuid$$;
 grant usage on schema auth to authenticated;grant execute on function auth.uid()to authenticated;
 create schema storage;create table storage.buckets(id text primary key,name text,public boolean default false,file_size_limit bigint,allowed_mime_types text[]);
 create table storage.objects(id uuid primary key default gen_random_uuid(),bucket_id text references storage.buckets(id),name text,owner_id text,metadata jsonb,unique(bucket_id,name));
 alter table storage.objects enable row level security;grant usage on schema storage to authenticated,anon;grant select,insert,update,delete on storage.objects to authenticated,anon;`);
 const folder=new URL('../../supabase/migrations/',import.meta.url);
 for(const f of(await readdir(folder)).filter(f=>f.endsWith('.sql')).sort()){
  try{await db.exec(await readFile(new URL(f,folder),'utf8'));}catch(e){throw Error('Migration '+f+': '+e.message,{cause:e});}
  await db.query('insert into supabase_migrations.schema_migrations values($1)',[f.split('_')[0]]);
 }
 for(const u of[owner,hr,manager,person,foreignOwner,admin2,archived])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3)',[u,email(u),JSON.stringify({full_name:'Fictional '+u.slice(-3)})]);
 const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional role transaction company',$1)returning id",[owner])).rows[0].id;
 await deny("insert into public.companies(name,owner_id)values('Fictional other company',$1)",[foreignOwner],['23505']);
 await session(owner);
 for(const[u,r]of[[hr,'hr_generalist'],[manager,'manager'],[person,'employee'],[admin2,'administrator'],[archived,'employee']])await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[u,email(u),r]);
 eq((await db.query('select public.my_hris_access()c')).rows[0].c.staff_contract_version,1,'Protected server advertises role contract');
 eq((await db.query('select public.staff_role_context()c')).rows[0].c.runtime_ready,false,'Runtime starts disabled for new companies too');
 const request=await change(person,['learning_performance_officer']);
 await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(request)],['42501']);
 await deny('update hris_private.staff_role_runtime_policy set runtime_ready=true');
 await deny('select hris_private.my_hris_access_legacy()');
 await deny('select hris_private.save_company_account_legacy($1,$2,$3,true,null,null)',[person,email(person),'administrator']);
 for(const u of[hr,manager,person]){await session(u);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(request)],['42501']);await deny('select public.staff_role_admin_context()');}
 await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company]);
 await session(owner);const receipt=await save(request);
 eq(receipt.active,true,'Receipt binds reviewed sign-in access');eq(receipt.roles,['learning_performance_officer'],'Combined development assignment');eq(receipt.base_role,'employee','Specialist cannot inherit broad legacy HR');eq(receipt.revision,2,'First explicit assignment follows baseline');eq(receipt.actor_id,owner,'Actor comes from protected membership');
 eq(await save(request),receipt,'Exact retry returns immutable receipt');
 await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify({...request,reason:'Changed retry reason'})],['42501']);
 await session(admin2);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(request)],['42501']);
 await session(person);let own=(await db.query('select public.staff_role_context()c')).rows[0].c;
 eq(own.roles,['learning_performance_officer'],'Own protected roles refreshed');eq(own.runtime_ready,true,'Only local fixture enables runtime for transaction tests');
 eq(own.base_role,'employee','Old clients remain portal-only for a specialist');
 await deny('select public.list_company_accounts()');
 await deny('select *from hris_private.staff_role_change_receipts');
 await session(owner);
 await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify({...request,id:id(serial++),roles:['payroll_officer']})],['23514']);
 // Disabling retains the assignment and its reasoned history; only the new contract restores it.
 const disabled={...await change(person,['learning_performance_officer'],'Disable reviewed access'),active:false};
 const disabledReceipt=await save(disabled);eq(disabledReceipt.active,false,'Disable receipt is explicit');
 await session(person);await deny('select public.staff_role_context()');
 await session(owner);const disabledView=(await context()).accounts.find(a=>a.user_id===person);
 eq(disabledView.active,false,'Disabled account remains in Admin history');eq(disabledView.roles,['learning_performance_officer'],'Disabling preserves roles');
 await deny('select public.save_company_account($1,$2,$3,true,null,null)',[person,email(person),'employee'],['42501']);
 const restore={...await change(person,['learning_performance_officer'],'Restore reviewed access'),active:true};
 eq((await save(restore)).active,true,'New role contract restores sign-in access');
 eq(await save(disabled),disabledReceipt,'Old exact retry retains its original inactive receipt');
 await db.exec('reset role');const statusEvent=(await db.query('select previous_active,active from hris_private.staff_role_events where id=$1',[disabled.id])).rows[0];
 eq(statusEvent,{previous_active:true,active:false},'Audit captures prior and new sign-in status');
 await session(admin2);const disableOwner={...await change(owner,['administrator']),active:false};
 await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(disableOwner)],['23514']);
 await session(owner);
 const combined=await change(person,['payroll_officer','hr_generalist']);const combinedReceipt=await save(combined);
 eq(combinedReceipt.roles,['hr_generalist','payroll_officer'],'Multi-role canonical assignment');eq(combinedReceipt.base_role,'employee','Partial HR bundle cannot inherit all legacy HR permissions');
 const managed=await change(person,['manager','learning_performance_officer']);await save(managed);
 await db.exec('reset role');let target=await account(person);const subordinate=await account(manager);
 await db.query('update public.employees set reports_to=$1 where id=$2',[target.employee_id,subordinate.employee_id]);
 await session(owner);const before=(await context()).accounts.find(a=>a.user_id===person);
 const noManager=await change(person,['employee']);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(noManager)],['23514']);
 eq((await context()).accounts.find(a=>a.user_id===person),before,'Failed hierarchy change leaves revision and identity intact');
 const staleTime=await change(hr,['recruiter']);await db.exec('reset role');await db.query("update public.company_members set updated_at=updated_at+interval'1second'where user_id=$1",[hr]);
 await session(owner);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(staleTime)],['23514']);
 const self=await change(owner,['administrator']);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(self)],['23514']);
 await session(admin2);const removeOwner=await change(owner,['employee']);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(removeOwner)],['23514']);
 await session(owner);const foreignRequest={...await change(hr,['recruiter']),user_id:foreignOwner};await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(foreignRequest)],['42501']);
 await db.query('select public.save_company_account($1,$2,$3,false,null,null)',[archived,email(archived),'employee']);
 await db.exec('reset role');const archivedMember=await account(archived);await db.query("update public.employees set status='archived'where id=$1",[archivedMember.employee_id]);
 await session(owner);const archiveGrant=await change(archived,['recruiter']);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(archiveGrant)],['23514']);
 // Old clients must not silently replace a specialist assignment with their four-role selector.
 await deny('select public.save_company_account($1,$2,$3,true,null,null)',[person,email(person),'manager'],['42501']);
 await deny('select public.save_company_account($1,$2,$3,true,null,null)',[manager,email(manager),'hr_generalist'],['42501']);
 // A recorded assignment is independent of future source changes and still retryable.
 eq(await save(request),receipt,'Older exact receipt survives a later assignment');
 await db.exec('reset role');await db.query('update auth.users set raw_user_meta_data=$1 where id=$2',[JSON.stringify({role:'administrator',roles:['administrator']}),person]);
 await session(person);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(await(async()=>({...request,id:id(serial++)}))())],['42501']);
 await session(admin2);const secondary=await change(person,['manager','payroll_officer']);const secondaryReceipt=await save(secondary);eq(secondaryReceipt.actor_id,admin2,'Independent current Administrator may assign');
 await session(owner);await db.query('select public.save_company_account($1,$2,$3,false,null,null)',[admin2,email(admin2),'administrator']);
 await session(admin2);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(secondary)],['42501']);
 await db.exec('reset role');eq((await db.query('select count(*)::int n from hris_private.staff_role_change_receipts where id=$1',[secondary.id])).rows[0].n,1,'Revocation preserves original receipt');
 for(const t of['staff_role_events','staff_role_change_receipts']){await deny('update hris_private.'+t+" set created_at=now()");await deny('delete from hris_private.'+t);}
 await session(owner);const invalid=await change(hr,['recruiter']);
 for(const patch of[{roles:['employee','recruiter']},{roles:['payroll_officer','payroll_officer']},{roles:['learning_officer']},{roles:['performance_officer']},{roles:[]},{active:null},{active:'true'},{active:undefined},{reason:''},{reason:'line\nbreak'},{reason:'x'.repeat(501)},{id:id(0)},{expected_revision:0},{unexpected:'value'}])await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify({...invalid,id:id(serial++),...patch})]);
 // The old access event, new baseline, assignment and receipt must roll back together on audit failure.
 const fault=await change(hr,['recruiter']);await db.exec('reset role');
 const snapshot={};for(const t of['company_members','account_access_events'])snapshot[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
 const eventCount=(await db.query('select count(*)::int n from hris_private.staff_role_events')).rows[0].n;
 await db.exec("create function hris_private.role_test_fault()returns trigger language plpgsql as $$begin if new.kind='assignment_changed'then raise exception'fictional audit failure';end if;return new;end;$$;create trigger role_test_fault before insert on hris_private.staff_role_events for each row execute function hris_private.role_test_fault();");
 await session(owner);await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify(fault)],['P0001']);await db.exec('reset role');
 for(const t of Object.keys(snapshot))eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,snapshot[t],'Audit failure restores '+t);
 eq((await db.query('select count(*)::int n from hris_private.staff_role_events')).rows[0].n,eventCount,'Audit failure restores new history');
 eq((await db.query('select count(*)::int n from hris_private.staff_role_change_receipts where id=$1',[fault.id])).rows[0].n,0,'Failed audit creates no retry receipt');
 await db.exec('drop trigger role_test_fault on hris_private.staff_role_events;drop function hris_private.role_test_fault();');
 await session(owner);const recovered=await save(fault);eq(recovered.id,fault.id,'Same failed request can succeed after fault is resolved');

 console.log('Passed '+checks+' staff-role transaction assertions against the actual migration chain. Runtime enabled only inside this local fixture.');
}catch(error){console.error(JSON.stringify({message:error.message,code:error.code,where:error.where,query:error.query,frame:error.stack?.split('\n').filter(line=>line.includes('staff-role-transactions.test.mjs')).slice(-3)}));process.exitCode=1;}finally{await db.close();}
