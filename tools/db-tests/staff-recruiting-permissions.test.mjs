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
 let serial=1;const id=()=>`34000000-0000-4000-8000-${String(serial++).padStart(12,'0')}`;
 const owner=id(),subject=id(),portal=id();
 for(const user of[owner,subject,portal])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3::jsonb)',[user,`fictional-${user}@example.invalid`,JSON.stringify({role:'administrator',roles:['administrator','recruiter']})]);
 const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional recruiting permission company',$1)returning *",[owner])).rows[0];
 const session=async user=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user]);await db.exec('set role authenticated');};
 await session(owner);for(const user of[subject,portal])await db.query("select public.save_company_account($1,$2,'employee',true,null,null)",[user,`fictional-${user}@example.invalid`]);
 const members=(await db.query('select user_id,employee_id from public.company_members')).rows;const person=user=>members.find(p=>p.user_id===user).employee_id;
 const applicant=async(op,p)=>(await db.query('select public.manage_applicant($1,$2,$3::jsonb)r',[company.id,op,JSON.stringify(p)])).rows[0].r;
 const interview=async(op,p)=>(await db.query('select public.manage_interview($1,$2::jsonb)r',[op,JSON.stringify(p)])).rows[0].r;
 const hiring=async(op,p)=>(await db.query('select public.manage_hiring($1,$2::jsonb)r',[op,JSON.stringify(p)])).rows[0].r;
 const app={id:id(),full_name:'Fictional private applicant',email:'fictional-candidate@example.invalid',phone:'0000000000',position:'Fictional associate',source:'Fictional workflow',applied_on:'2026-01-01',notes:'Fictional private assessment',reason:'Fictional fixture'};await applicant('create',app);
 const file={id:id(),applicant_id:app.id,title:'Fictional resume',category:'other',notes:'Fictional private file',issued_on:null,expires_on:null,contract_start:null,contract_end:null,original_filename:'fictional.pdf',mime_type:'application/pdf',byte_size:100,sha256:'c'.repeat(64),revision:0};
 const reserved=(await db.query('select public.save_applicant_document($1,$2::jsonb)d',['reserve',JSON.stringify(file)])).rows[0].d;
 // Contract-only managed Storage fixture, not a hosted upload or S3 byte check.
 await db.query('insert into storage.objects(bucket_id,name,owner_id,metadata)values($1,$2,$3,$4::jsonb)',['applicant-documents',reserved.object_name,owner,JSON.stringify({mimetype:'application/pdf',size:100})]);
 await db.query('select public.save_applicant_document($1,$2::jsonb)',['finalize',JSON.stringify({id:file.id})]);
 await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company.id]);
 const cases=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
 let ownInterview=null,otherInterview=null,ownTask=null,otherTask=null,caseId=null;
 for(const[index,test]of cases.entries()){
  await session(owner);const context=(await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;const current=context.accounts.find(a=>a.user_id===subject);
  await db.query('select public.set_staff_roles($1::jsonb)',[JSON.stringify({id:id(),user_id:subject,active:true,expected_revision:current.revision,expected_base_role:current.base_role,expected_member_updated_at:current.member_updated_at,roles:test.roles,reason:'Reviewed fictional Recruiting role'})]);
  if(ownInterview===null){
   const now=Date.now();const schedule={id:id(),applicant_id:app.id,interviewer_employee_id:person(subject),title:'Fictional assigned interview',starts_at:new Date(now-7200000).toISOString(),ends_at:new Date(now-3600000).toISOString(),timezone:company.timezone,location:'Fictional room',brief:'Fictional skills discussion',reason:'Fictional assignment'};
   ownInterview=await interview('save',schedule);otherInterview=await interview('save',{...schedule,id:id(),interviewer_employee_id:person(owner),starts_at:new Date(now-14400000).toISOString(),ends_at:new Date(now-10800000).toISOString()});
   await interview('feedback',{id:otherInterview.id,expected_revision:0,rating:4,recommendation:'proceed',assessment:'Fictional independent feedback',reason:'Fictional feedback'});
  }
  if(ownTask&&ownTask.status!=='open')ownTask=await hiring('task_status',{id:ownTask.id,expected_revision:ownTask.revision,status:'open',reason:'Fictional next role case'});
  await session(subject);const recruiting=test.capabilities.includes('recruiting'),desktop=test.capabilities.includes('desktop');
  for(const table of['applicants','applicant_documents','applicant_document_events']){
   const n=(await db.query(`select count(*)::int n from public.${table}`)).rows[0].n;
   if(recruiting){assert.ok(n>0,'Recruiting fixture exists '+table);checks++;}else eq(n,0,'No applicant/assessment/file access '+table+' '+test.roles);
  }
  eq((await db.query("select count(*)::int n from storage.objects where bucket_id='applicant-documents'")).rows[0].n,recruiting?1:0,'No direct resume Storage bypass');
  eq((await db.query('select count(*)::int n from public.recruiting_interviews where id=$1',[ownInterview.id])).rows[0].n,desktop?1:0,'Assigned interview is visible to current staff only');
  eq((await db.query('select count(*)::int n from public.recruiting_interviews where id=$1',[otherInterview.id])).rows[0].n,recruiting?1:0,'Staff assignment is not company interview access');
  const feedbackRows=desktop?(await db.query('select revision from public.interview_feedback where interview_id=$1 and author_id=$2',[ownInterview.id,subject])).rows:[];
  const feedback={id:ownInterview.id,expected_revision:feedbackRows[0]?.revision??0,rating:4,recommendation:'proceed',assessment:'Fictional own assigned assessment',reason:'Fictional current role feedback'};
  if(desktop)eq((await interview('feedback',feedback)).author_id,subject,'Any assigned staff role records only its own feedback');else await deny('select public.manage_interview($1,$2::jsonb)',['feedback',JSON.stringify(feedback)],['42501']);
  await deny('select public.manage_interview($1,$2::jsonb)',['feedback',JSON.stringify({...feedback,id:otherInterview.id,expected_revision:1})],['42501']);
  const nextApp={...app,id:id(),full_name:'Fictional hire '+index};
  if(recruiting){
   await applicant('create',nextApp);
   const draft={id:id(),applicant_id:nextApp.id,job_title:'Fictional associate',department:'Fictional department',starts_on:'2027-01-04',amount_minor:2500000,currency:'PHP',pay_period:'monthly',terms:'Fictional terms only',expires_on:null,reason:'Fictional offer preparation'};
   let offer=await hiring('offer_save',draft);eq(offer.status,'draft','Recruiter prepares actual offer');
   if(!test.roles.includes('administrator'))await deny('select public.manage_hiring($1,$2::jsonb)',['offer_status',JSON.stringify({id:offer.id,expected_revision:offer.revision,status:'approved',reason:'Attempt protected approval'})],['42501']);
   await session(owner);offer=await hiring('offer_status',{id:offer.id,expected_revision:offer.revision,status:'approved',reason:'Fictional Administrator review'});
   await session(subject);for(const status of['sent','accepted'])offer=await hiring('offer_status',{id:offer.id,expected_revision:offer.revision,status,reason:'Fictional recorded status; no message sent'});
   const hire={id:id(),offer_id:offer.id,expected_revision:offer.revision,employee_code:'FICTIONAL-HIRE-'+index,reports_to:null,employment_type_id:null,work_location_id:null,reason:'Fictional accepted offer conversion'};
   const conversion=await hiring('hire',hire);eq((await hiring('hire',hire)).employee_id,conversion.employee_id,'Exact hire retry retains one employee and checklist');
   eq((await db.query('select count(*)::int n from public.onboarding_tasks where case_id=$1',[hire.id])).rows[0].n,5,'Recruiter conversion atomically starts five checklist tasks');
   if(!test.capabilities.includes('personnel')){
    eq((await db.query('select id from public.employee_contacts where employee_id=$1',[conversion.employee_id])).rows,[],'Hire conversion does not grant broad private contact access');
    eq((await db.query("update public.employees set job_title='Unauthorized post-hire change'where id=$1 returning id",[conversion.employee_id])).rows,[],'Recruiter cannot bypass Personnel employment maintenance');
   }
   if(ownTask===null){
    caseId=hire.id;await session(owner);const tasks=(await db.query('select *from public.onboarding_tasks where case_id=$1 order by id',[caseId])).rows;
    const assign=(task,employee)=>({id:task.id,expected_revision:task.revision,case_id:caseId,assigned_employee_id:employee,title:task.title,instructions:'Fictional shared task only',due_on:null,required:true,reason:'Fictional assigned responsibility'});
    ownTask=await hiring('task_save',assign(tasks[0],person(subject)));otherTask=await hiring('task_save',assign(tasks[1],person(owner)));await session(subject);
   }
  }else{
   await deny('select public.manage_applicant($1,$2,$3::jsonb)',[company.id,'create',JSON.stringify(nextApp)],['42501']);
   await deny('select public.manage_hiring($1,$2::jsonb)',['offer_save',JSON.stringify({id:id(),reason:'Attempt private offer'})],['42501']);
   await deny('select public.save_applicant_document($1,$2::jsonb)',['reserve',JSON.stringify({...file,id:id()})],['42501']);
   eq((await db.query('select *from public.hiring_offers')).rows,[],'Non-Recruiting staff cannot read salary offers');
   eq((await db.query('select *from public.hire_conversions')).rows,[],'Non-Recruiting staff cannot read applicant/employee conversion links');
  }
  eq((await db.query('select count(*)::int n from public.onboarding_cases where id=$1',[caseId])).rows[0].n,recruiting?1:0,'Assigned staff do not receive private onboarding case details');
  eq((await db.query('select count(*)::int n from public.onboarding_tasks where id=$1',[ownTask.id])).rows[0].n,desktop?1:0,'Exact assigned onboarding scope');
  eq((await db.query('select count(*)::int n from public.onboarding_tasks where id=$1',[otherTask.id])).rows[0].n,recruiting?1:0,'No unrelated onboarding task access');
  if(desktop){
   ownTask=await hiring('task_status',{id:ownTask.id,expected_revision:ownTask.revision,status:'done',reason:'Fictional own task completion'});eq(ownTask.status,'done','Assigned specialist completes own onboarding task');
   if(!recruiting)await deny('select public.manage_hiring($1,$2::jsonb)',['task_status',JSON.stringify({id:otherTask.id,expected_revision:otherTask.revision,status:'done',reason:'Attempt another task'})],['42501']);
  }else await deny('select public.manage_hiring($1,$2::jsonb)',['task_status',JSON.stringify({id:ownTask.id,expected_revision:ownTask.revision,status:'done',reason:'Attempt staff task'})],['42501']);
 }
 await session(owner);await db.exec('reset role');eq((await db.query('select count(*)::int n from auth.users')).rows[0].n,3,'Hiring never creates an Auth identity');
 console.log('Passed '+checks+' actual Recruiting/Storage/interview/offer/hire/onboarding authorization assertions against '+count+' migrations. No external messages, real uploads or Auth creation.');
}catch(error){console.error(JSON.stringify({message:error.message,code:error.code,where:error.where,stack:error.stack}));process.exitCode=1;}finally{await db.close();}
