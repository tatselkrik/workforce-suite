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
 for(const file of(await readdir(folder)).filter(f=>f.endsWith('.sql')).sort()){
  try{await db.exec(await readFile(new URL(file,folder),'utf8'));}
  catch(e){throw Error('Fresh replay failed at '+file+': '+e.message,{cause:e});}
  await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[file.split('_')[0]]);count++;
 }

 let checks=0;const eq=(a,b,label)=>{assert.deepEqual(a,b,label);checks++;};
 const deny=async(sql,params=[],codes=['42501','23514'])=>{try{await db.query(sql,params);}catch(e){assert.ok(codes.includes(e.code),e.code+': '+e.message);checks++;return;}assert.fail('Expected denial: '+sql);};
 let serial=1;const id=()=>`36000000-0000-4000-8000-${String(serial++).padStart(12,'0')}`;
 const owner=id(),principal=id(),delegate=id(),person=id(),outside=id();
 for(const user of[owner,principal,delegate,person,outside])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3::jsonb)',[user,`fictional-${user}@example.invalid`,JSON.stringify({role:'administrator',roles:['administrator']})]);
 const company=(await db.query("insert into public.companies(name,owner_id,timezone)values('Fictional delegation permission company',$1,'Asia/Manila')returning *",[owner])).rows[0];
 const session=async user=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user]);await db.query("select set_config('request.headers',$1,false)",[JSON.stringify({'x-hris-leave-contract':'2'})]);await db.exec('set role authenticated');};
 await session(owner);for(const user of[principal,delegate,person,outside])await db.query("select public.save_company_account($1,$2,'employee',true,null,null)",[user,`fictional-${user}@example.invalid`]);
 const people=(await db.query('select m.user_id,e.id from public.company_members m join public.employees e on e.id=m.employee_id')).rows;
 const employee=user=>people.find(p=>p.user_id===user).id;
 await db.exec('reset role');await db.query("update public.employees set joined_on='2026-01-01',annual_leave_days=30 where company_id=$1",[company.id]);
 await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company.id]);
 const rpc=async(name,...args)=>(await db.query(`select public.${name}(${args.map((_,i)=>'$'+(i+1)).join(',')})r`,args)).rows[0].r;
 const roles=async(user,assigned)=>{await session(owner);const c=await rpc('staff_role_admin_context',true),a=c.accounts.find(a=>a.user_id===user);await rpc('set_staff_roles',JSON.stringify({id:id(),user_id:user,active:true,expected_revision:a.revision,expected_base_role:a.base_role,expected_member_updated_at:a.member_updated_at,roles:assigned,reason:'Fictional reviewed delegation duties'}));};
 await roles(principal,['payroll_officer']);await roles(delegate,['recruiter']);await roles(outside,['learning_performance_officer']);
 await session(owner);const context=await rpc('approval_delegation_staff_context');
 eq(context.version,'approval-delegation-v2','Separate new context contract');
 eq(context.staff.map(s=>s.user_id).sort(),[owner,principal,delegate,outside].sort(),'Only current desktop staff, including compatible Employee-base specialties, are candidates');
 eq(context.staff.find(s=>s.user_id===delegate).staff_roles,['recruiter'],'Candidate roles are protected registry data');
 const today=context.as_of;const plus=async n=>(await db.query('select($1::date+$2::integer)::text d',[today,n])).rows[0].d;
 const grant={operation:'grant',id:id(),expected_revision:0,scope:'leave',principal_id:principal,delegate_id:delegate,starts_on:today,ends_on:await plus(7),reason:'PRIVATE_STAFF_DELEGATION_REASON',reference:'PRIVATE_STAFF_DELEGATION_REFERENCE'};
 const receipt=await rpc('save_approval_delegation',JSON.stringify(grant));eq(await rpc('save_approval_delegation',JSON.stringify(grant)),receipt,'Exact actor/payload retry retains one grant');
 await deny('select public.save_approval_delegation($1::jsonb)',[JSON.stringify({...grant,reason:'Changed intent'})],['PT409']);
 const saved=(await rpc('approval_delegation_staff_context')).grants[0];
 eq([saved.principal_staff_roles,saved.delegate_staff_roles,saved.current],[['payroll_officer'],['recruiter'],true],'Immutable actual roles are saved with compatible Employee base');
 await deny('select public.save_approval_delegation($1::jsonb)',[JSON.stringify({...grant,id:id(),scope:'attendance',expected_revision:1})]);
 await deny('select public.save_approval_delegation($1::jsonb)',[JSON.stringify({...grant,id:id(),scope:'payroll',expected_revision:1})]);
 let policy=(await db.query('select *from public.leave_policies where company_id=$1',[company.id])).rows[0];
 await rpc('save_leave_policy',{workweek:[1,2,3,4,5,6,7],holidays:[],types:policy.types.map(t=>t.code==='annual'?{...t,active:true,use_employee_allowance:true,accrual:'upfront',approval_chain:[{kind:'person',employee_id:employee(principal)}]}:t)},policy.revision);
 policy=(await db.query('select *from public.leave_policies where company_id=$1',[company.id])).rows[0];
 const submit=async day=>{await session(person);return rpc('portal_submit_leave','annual',day,day,'full','Fictional named approval',policy.revision);};
 const request=await submit(await plus(1));await session(delegate);
 let c=await rpc('approval_delegation_staff_context');eq(c.permits.some(p=>p.request_id===request),true,'Named protected staff delegation is request-specific');
 eq(c.audit,[],'Other staff receive no private HR audit');eq(c.staff,[],'Other staff receive no Administrator candidate directory');
 eq((await db.query('select employee_id from public.employee_contacts where employee_id=$1',[employee(person)])).rows,[],'Assigned leave does not reveal contacts');
 await deny('select *from public.approval_delegations');await deny('select *from hris_private.approval_delegation_role_bindings');
 await rpc('review_leave',request,0,true,'Fictional delegated staff decision');
 c=await rpc('approval_delegation_staff_context');eq(c.receipts.length,1,'Actual substitute decision retains immutable linked receipt');eq(c.receipts[0].principal_id,principal,'Original named principal preserved');
 const pending=await submit(await plus(2));
 await roles(delegate,['learning_performance_officer']);await session(delegate);c=await rpc('approval_delegation_staff_context');
 eq([c.role,c.grants[0].current,c.permits.some(p=>p.request_id===pending)],['employee',false,false],'Different responsibilities invalidate a grant despite unchanged Employee base and desktop eligibility');
 await deny('select public.review_leave($1,0,true,$2)',[pending,'Changed roles denied']);
 eq(c.receipts.length,1,'Prior acted receipt remains visible without reactivating authority');
 await roles(delegate,['recruiter']);await session(delegate);eq((await rpc('approval_delegation_staff_context')).grants[0].current,true,'Restored identical roles may resume unrevoked grant within dates');
 await roles(principal,['recruiter']);await session(delegate);eq((await rpc('approval_delegation_staff_context')).grants[0].current,false,'Principal specialty change also invalidates same-base grant');
 await roles(principal,['payroll_officer']);await session(owner);
 await rpc('save_approval_delegation',JSON.stringify({operation:'revoke',id:id(),expected_revision:1,delegation_id:grant.id,reason:'Fictional early return'}));
 await session(delegate);eq((await rpc('approval_delegation_staff_context')).grants[0].current,false,'Explicit revocation prevents restoration');
 await session(outside);eq((await rpc('approval_delegation_staff_context')).grants,[],'Unassigned staff cannot view other grants');
 await session(person);await deny('select public.approval_delegation_staff_context()');await deny('select public.save_approval_delegation($1::jsonb)',[JSON.stringify(grant)]);
 await db.exec('reset role');await deny('update hris_private.approval_delegation_role_bindings set delegate_roles=$1 where delegation_id=$2',[['manager'],grant.id],['23514']);
 eq((await db.query('select count(*)::int n from public.approval_delegations where id=$1',[grant.id])).rows[0].n,1,'No duplicate or rewritten grants');
 const cases=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
 for(const p of cases)for(const d of cases)for(const scope of['leave','attendance','overtime']){
  const has=(a,c)=>a.capabilities.includes(c);let expected=false;
  if(has(p,'desktop')&&has(d,'desktop'))expected=has(p,'account_administration')?has(d,'account_administration'):has(p,'leave_administration')?has(d,'leave_administration'):scope==='leave'||(has(p,'team_review')&&(has(d,'team_review')||has(d,'attendance_administration')));
  eq((await db.query('select hris_private.staff_delegation_substitute($1,$2,$3)b',[scope,p.roles,d.roles])).rows[0].b,expected,'Shared14case substitution grades '+scope);
 }
 console.log('Passed '+checks+' staff delegation checks across '+count+' migrations; same-base role changes, scoped real decisions, retries, restoration, revocation and private denials pass.');
}catch(error){console.error(JSON.stringify({message:error.message,code:error.code,where:error.where,stack:error.stack}));process.exitCode=1;}finally{await db.close();}
