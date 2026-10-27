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
 let serial=1;const id=()=>`32000000-0000-4000-8000-${String(serial++).padStart(12,'0')}`;
 const owner=id(),unassigned=id(),subject=id();
 for(const user of[owner,unassigned,subject])await db.query('insert into auth.users(id,email,email_confirmed_at)values($1,$2,now())',[user,`fictional-${user}@example.invalid`]);
 const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional development permission company',$1)returning *",[owner])).rows[0];
 const session=async(user,role='authenticated')=>{assert.ok(['authenticated','anon'].includes(role));await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user]);await db.exec('set role '+role);};
 for(const [file,name]of[['performance.test.mjs','runPerformanceTests'],['learning.test.mjs','runLearningTests'],['career.test.mjs','runCareerTests']]){const module=await import('./'+file);await module[name]({db,eq,deny,session,company,owner,unassigned});}
 await session(owner);await db.query("select public.save_company_account($1,$2,'employee',true,null,null)",[subject,`fictional-${subject}@example.invalid`]);
 const subjectEmployee=(await db.query('select employee_id from public.company_members where user_id=$1',[subject])).rows[0].employee_id;
 await db.exec('reset role');const target=(await db.query("select id from public.employees where company_id=$1 and status='active'and id<>$2 and id<>(select employee_id from public.company_members where user_id=$3)order by id limit 1",[company.id,subjectEmployee,owner])).rows[0].id;
 const candidate=id();await db.query("insert into public.employees(company_id,id,employee_code,full_name,reports_to,department,job_title,joined_on,annual_leave_days,status)values($1,$2,$3,$4,$5,'Fictional department','Fictional staff','2026-01-01',15,'active')",[company.id,candidate,'FICTIONAL-REVIEW-CANDIDATE','Fictional reviewer subject',null]);
 await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company.id]);
 const cases=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
 for(const test of cases){
  await session(owner);const context=(await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;const current=context.accounts.find(a=>a.user_id===subject);
  await db.query('select public.set_staff_roles($1::jsonb)',[JSON.stringify({id:id(),user_id:subject,active:true,expected_revision:current.revision,expected_base_role:current.base_role,expected_member_updated_at:current.member_updated_at,roles:test.roles,reason:'Reviewed fictional development assignment'})]);
  const dev=test.capabilities.includes('development_administration'),privateNotes=test.capabilities.includes('private_hr_notes'),privateFiles=test.capabilities.includes('personnel_documents');
  // Caller is an Administrator. Reviewer validity must use the REVIEWER's protected
  // assignment, not the caller's capabilities or forged metadata.
  await db.exec('reset role');
  await db.query('update public.employees set reports_to=$1 where id=$2',[test.roles.includes('manager')?subjectEmployee:null,candidate]);
  eq((await db.query('select hris_private.performance_reviewer_valid($1,$2,$3)c',[company.id,candidate,subjectEmployee])).rows[0].c,dev||test.roles.includes('manager'),'Assigned reviewer permissions use the correct person '+test.roles);
  await session(subject);
  for(const [table,allowed]of[['performance_hr_notes',privateNotes],['certification_evidence',privateFiles]]){
   const visible=(await db.query(`select count(*)::int n from public.${table}`)).rows[0].n;
   if(allowed){assert.ok(visible>0,'Authorized private history exists '+table);checks++;}else eq(visible,0,'Confidential RLS '+table+' '+test.roles);
  }
  if(!privateNotes)eq((await db.query('select count(*)::int n from public.performance_events where note_id is not null')).rows[0].n,0,'Private note events never reach development-only accounts');
  if(!privateFiles)eq((await db.query('select count(*)::int n from public.learning_events where evidence_id is not null')).rows[0].n,0,'Document bindings and their event snapshots remain private');
  if(test.capabilities.includes('desktop')){
   const directory=(await db.query('select public.staff_workflow_people()d')).rows[0].d;
   eq([directory.company_id,directory.user_id,directory.schema_version,directory.runtime_ready],[company.id,subject,1,true],'Workflow directory binds its actual caller');
   eq(directory.person_count,directory.people.length,'No silent staff omissions');
   eq(directory.people.find(p=>p.employee_id===subjectEmployee).roles,test.roles,'Workflow person has the current protected roles');
   for(const person of directory.people){eq(Object.keys(person).sort(),['employee_id','full_name','role','roles'],'No account IDs, emails or private personnel data in assignment list');}
  }else await deny('select public.staff_workflow_people()',[],['42501']);
  const cycle={id:id(),name:'Fictional specialty cycle '+serial,starts_on:'2026-01-01',ends_on:'2026-12-31',due_on:'2027-01-15',reason:'Reviewed cycle assignment'};
  const training={id:id(),employee_id:target,title:'Fictional specialty training',provider:'',starts_on:null,ends_on:null,due_on:null,required:false,reason:'Reviewed training assignment'};
  for(const [name,op,payload]of[['manage_performance','cycle_save',cycle],['manage_learning','training_save',training]]){
   const sql=`select public.${name}($1,$2::jsonb)r`;if(dev){const r=(await db.query(sql,[op,JSON.stringify(payload)])).rows[0].r;eq(r.id,payload.id,'Development specialist creates actual '+op);}else await deny(sql,[op,JSON.stringify(payload)],['42501']);
  }
  if(privateNotes){
   const note=(await db.query('select *from public.performance_hr_notes order by id limit 1')).rows[0];
   const result=(await db.query('select public.manage_performance($1,$2::jsonb)r',['note_save',JSON.stringify({id:note.id,expected_revision:note.revision,review_id:note.review_id,note:'Reviewed fictional confidential note',reason:'Private HR review'})])).rows[0].r;
   eq(result.revision,note.revision+1,'HR-only staff can maintain private notes without development administration');
  }else await deny('select public.manage_performance($1,$2::jsonb)',['note_save',JSON.stringify({id:id(),reason:'Attempt private HR write'})],['42501']);
  if(!privateFiles)await deny('select public.manage_learning($1,$2::jsonb)',['evidence_save',JSON.stringify({id:id(),reason:'Attempt private file binding'})],['42501']);
  await deny('select hris_private.staff_capabilities_for($1,$2)',[company.id,owner],['42501']);
  await session(owner);await db.exec('reset role');await db.query('update public.employees set reports_to=null where id=$1',[candidate]);
 }
 console.log('Passed '+checks+' development/workflow/specialty/private-field assertions against '+count+' migrations; legacy workflows and assigned-reviewer boundaries verified.');
}catch(error){console.error(JSON.stringify({message:error.message,code:error.code,where:error.where,stack:error.stack}));process.exitCode=1;}finally{await db.close();}
