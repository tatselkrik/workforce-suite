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
 const deny=async(sql,params=[],codes=['42501','23514','23505','PT409'])=>{try{await db.query(sql,params);}catch(e){assert.ok(codes.includes(e.code),e.code+': '+e.message);checks++;return;}assert.fail('Expected denial: '+sql);};
 let serial=1;const id=()=>`33000000-0000-4000-8000-${String(serial++).padStart(12,'0')}`;
 const owner=id(),subject=id(),unassigned=id();
 for(const user of[owner,subject,unassigned])await db.query('insert into auth.users(id,email,email_confirmed_at)values($1,$2,now())',[user,`fictional-${user}@example.invalid`]);
 const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional personnel privacy company',$1)returning *",[owner])).rows[0];
 const session=async(user,role='authenticated')=>{assert.ok(['authenticated','anon'].includes(role));await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user]);await db.exec('set role '+role);};
 const {runLearningTests}=await import('./learning.test.mjs');await runLearningTests({db,eq,deny,session,company,owner,unassigned});
 await session(owner);await db.query("select public.save_company_account($1,$2,'employee',true,null,null)",[subject,`fictional-${subject}@example.invalid`]);
 const subjectEmployee=(await db.query('select employee_id from public.company_members where user_id=$1',[subject])).rows[0].employee_id;
 const target=(await db.query('select public.save_employee_record($1::jsonb,$2::jsonb)e',[JSON.stringify({company_id:company.id,employee_code:'FICTIONAL-PERM-TARGET',full_name:'Fictional personnel subject',department:'Fictional department',job_title:'Fictional role',joined_on:'2026-01-01',annual_leave_days:15,status:'active',reports_to:null,employment_type_id:null,work_location_id:null}),JSON.stringify({phone:'0000000000',personal_email:'fictional@example.invalid',emergency_name:'Fictional emergency person',emergency_relationship:'Fictional contact',emergency_phone:'0000000000'})])).rows[0].e;
 // Simulated managed Storage metadata only: no S3 bytes or hosted upload is claimed.
 const file={id:id(),employee_id:target,title:'Fictional personnel file',category:'other',notes:'Fictional private metadata',issued_on:null,expires_on:null,contract_start:null,contract_end:null,original_filename:'fictional.pdf',mime_type:'application/pdf',byte_size:100,sha256:'b'.repeat(64)};
 const reserved=(await db.query('select public.save_employee_document($1,$2::jsonb)d',['reserve',JSON.stringify(file)])).rows[0].d;
 await db.query('insert into storage.objects(bucket_id,name,owner_id,metadata)values($1,$2,$3,$4::jsonb)',['employee-documents',reserved.object_name,owner,JSON.stringify({mimetype:'application/pdf',size:100})]);
 await db.query('select public.save_employee_document($1,$2::jsonb)',['finalize',JSON.stringify({id:file.id})]);
 const exit=(await db.query('select public.manage_exit($1,$2::jsonb)e',['case_save',JSON.stringify({id:id(),employee_id:target,last_working_on:'2026-10-31',category:'resignation',private_notes:'Fictional confidential separation detail',reason:'Fictional private workflow fixture'})])).rows[0].e;
 await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company.id]);
 const original=(await db.query('select to_jsonb(e)e from public.employees e where id=$1',[target])).rows[0].e;
 const cases=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
 let ownTask=null,otherTask=null;
 const runExit=async(op,r)=>(await db.query('select public.manage_exit($1,$2::jsonb)r',[op,JSON.stringify(r)])).rows[0].r;
 for(const test of cases){
  await session(owner);const context=(await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;const current=context.accounts.find(a=>a.user_id===subject);
  await db.query('select public.set_staff_roles($1::jsonb)',[JSON.stringify({id:id(),user_id:subject,active:true,expected_revision:current.revision,expected_base_role:current.base_role,expected_member_updated_at:current.member_updated_at,roles:test.roles,reason:'Reviewed fictional personnel role'})]);
  if(ownTask===null){
   const task={id:id(),case_id:exit.id,assigned_employee_id:subjectEmployee,title:'Fictional assigned clearance',instructions:'Fictional task only',due_on:null,required:true,reason:'Fictional task assignment'};
   ownTask=await runExit('task_save',task);otherTask=await runExit('task_save',{...task,id:id(),assigned_employee_id:context.accounts.find(a=>a.user_id===owner).employee_id});
  }else if(ownTask.status!=='open')ownTask=await runExit('task_status',{id:ownTask.id,expected_revision:ownTask.revision,status:'open',reason:'Fictional next role case'});
  await session(subject);
  const personnel=test.capabilities.includes('personnel'),files=test.capabilities.includes('personnel_documents'),desktop=test.capabilities.includes('desktop');
  eq((await db.query('select count(*)::int n from public.employee_contacts where employee_id=$1',[target])).rows[0].n,personnel?1:0,'Other employee contacts/emergency details stay Personnel-only '+test.roles);
  for(const table of['employee_documents','employee_document_events','employment_history']){
   const n=(await db.query(`select count(*)::int n from public.${table}`)).rows[0].n;
   if(personnel){assert.ok(n>0,'Authorized fixture exists '+table);checks++;}else eq(n,0,'Private record denial '+table+' '+test.roles);
  }
  const objects=(await db.query("select count(*)::int n from storage.objects where bucket_id='employee-documents'")).rows[0].n;
  if(files){assert.ok(objects>0,'HR can read existing private file object metadata');checks++;}else eq(objects,0,'No direct Storage bypass for specialist personnel files');
  eq((await db.query('select count(*)::int n from public.exit_cases where id=$1',[exit.id])).rows[0].n,personnel?1:0,'Private separation details do not enter assigned staff scope');
  const upload=['reserve',JSON.stringify({...file,id:id()})];
  if(files){eq((await db.query('select public.save_employee_document($1,$2::jsonb)d',upload)).rows[0].d.created_by,subject,'Protected HR staff can reserve private personnel uploads');}
  else await deny('select public.save_employee_document($1,$2::jsonb)',upload,['42501']);

  const canonicalUpdate={...original,job_title:'Reviewed role '+test.roles.join(' ')};
  if(personnel){eq((await db.query('select public.save_employee_record($1::jsonb,$2::jsonb)e',[JSON.stringify(canonicalUpdate),JSON.stringify({phone:'0000000000',personal_email:'fictional@example.invalid'})])).rows[0].e,target,'HR Generalist specialist can atomically maintain employment/contact data');}
  else{
   eq((await db.query("update public.employees set job_title='Unauthorized change'where id=$1 returning id",[target])).rows,[],'Direct employment UPDATE denied by RLS');
   await deny('select public.save_employee_record($1::jsonb,$2::jsonb)',[JSON.stringify(canonicalUpdate),JSON.stringify({phone:'9999999999'})],['42501']);
  }
  await db.query("insert into public.employee_contacts(company_id,employee_id,phone)values($1,$2,'0000000000')on conflict(company_id,employee_id)do update set phone=excluded.phone",[company.id,subjectEmployee]);
  eq((await db.query('select phone from public.employee_contacts where employee_id=$1',[subjectEmployee])).rows[0].phone,'0000000000','Every role keeps own contact maintenance');
  eq((await db.query('select count(*)::int n from public.clearance_tasks where id=$1',[ownTask.id])).rows[0].n,desktop?1:0,'Only current staff can see their assigned clearance');
  eq((await db.query('select count(*)::int n from public.clearance_tasks where id=$1',[otherTask.id])).rows[0].n,personnel?1:0,'Assignment is not broad clearance access');
  if(desktop){
   ownTask=await runExit('task_status',{id:ownTask.id,expected_revision:ownTask.revision,status:'done',reason:'Fictional assigned work completed'});eq(ownTask.status,'done','Specialist can complete their own assigned task');
   if(!personnel){
    await deny('select public.manage_exit($1,$2::jsonb)',['task_status',JSON.stringify({id:otherTask.id,expected_revision:otherTask.revision,status:'done',reason:'Attempt another task'})],['42501']);
    await deny('select public.manage_exit($1,$2::jsonb)',['task_status',JSON.stringify({id:ownTask.id,expected_revision:ownTask.revision,status:'waived',reason:'Attempt HR waiver'})],['23514']);
   }
  }else await deny('select public.manage_exit($1,$2::jsonb)',['task_status',JSON.stringify({id:ownTask.id,expected_revision:ownTask.revision,status:'done',reason:'Attempt portal-only staff task'})],['42501']);
 }
 await session(owner);await db.query("select public.save_company_account($1,$2,'employee',false,null,null)",[subject,`fictional-${subject}@example.invalid`]);await session(subject);
 eq((await db.query('select count(*)::int n from public.employee_contacts')).rows[0].n,0,'Revocation removes even own private contacts');
 await deny('select public.staff_workflow_people()',[],['42501']);
 console.log('Passed '+checks+' Personnel/contact/file/Storage/clearance authorization assertions against '+count+' migrations; assigned tasks and private case boundaries verified.');
}catch(error){console.error(JSON.stringify({message:error.message,code:error.code,where:error.where,stack:error.stack}));process.exitCode=1;}finally{await db.close();}
