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
 const deny=async(sql,params=[],codes=['42501'])=>{try{await db.query(sql,params);}catch(e){assert.ok(codes.includes(e.code),e.code+': '+e.message);checks++;return;}assert.fail('Expected denial: '+sql);};
 let serial=1;const id=()=>`35000000-0000-4000-8000-${String(serial++).padStart(12,'0')}`;
 const owner=id(),subject=id(),recipient=id();
 for(const user of[owner,subject,recipient])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3::jsonb)',[user,`fictional-${user}@example.invalid`,JSON.stringify({role:'administrator',roles:['administrator']})]);
 const company=(await db.query("insert into public.companies(name,owner_id,timezone)values('Fictional operations permission company',$1,'Asia/Manila')returning *",[owner])).rows[0];
 const session=async user=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user]);await db.exec('set role authenticated');};
 await session(owner);for(const user of[subject,recipient])await db.query("select public.save_company_account($1,$2,'employee',true,null,null)",[user,`fictional-${user}@example.invalid`]);
 const people=(await db.query('select m.user_id,e.id,e.employee_code from public.company_members m join public.employees e on e.id=m.employee_id')).rows;
 const target=people.find(p=>p.user_id===recipient),own=people.find(p=>p.user_id===subject);
 await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company.id]);
 const cases=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
 const rpc=async(name,...args)=>(await db.query(`select public.${name}(${args.map((_,i)=>'$'+(i+1)).join(',')})r`,args)).rows[0].r;
 for(const[index,test]of cases.entries()){
  await session(owner);const context=await rpc('staff_role_admin_context',true);const current=context.accounts.find(a=>a.user_id===subject);
  await rpc('set_staff_roles',JSON.stringify({id:id(),user_id:subject,active:true,expected_revision:current.revision,expected_base_role:current.base_role,expected_member_updated_at:current.member_updated_at,roles:test.roles,reason:'Reviewed fictional operations role'}));
  await db.exec('reset role');
  const principal=async(scope,steps,submitter=null)=>(await db.query('select hris_private.approval_principal_allowed($1,$2,$3,$4,$5::jsonb,$6)c',[company.id,scope,target.id,submitter,JSON.stringify(steps),subject])).rows[0].c;
  const hr=test.capabilities.includes('leave_administration'),desktop=test.capabilities.includes('desktop');
  eq(await principal('leave',[{kind:'hr'}]),hr,'HR step uses selected principal roles, not the Administrator caller');
  eq(await principal('leave',[{kind:'person',employee_id:own.id}]),desktop,'Named person authority is request-specific current staff');
  eq(await principal('leave',[{kind:'person',employee_id:own.id}],subject),false,'Submission exclusion survives combined roles');
  eq(await principal('payroll',[{kind:'hr'}]),false,'Operational permission never delegates payroll authority');
  const day=(await db.query("select((now()at time zone 'Asia/Manila')::date-$1::integer)::text d",[index+2])).rows[0].d;
  const importDay=(await db.query("select((now()at time zone 'Asia/Manila')::date-$1::integer)::text d",[index+22])).rows[0].d;
  await session(subject);
  const credit=await rpc('leave_credit_context');eq([credit.company_id,credit.user_id,credit.role],[company.id,subject,currentRole(test.roles)],'Credits retain own protected actor and compatibility role');
  for(const name of['leave_dated_context','approval_delegation_context']){
   if(desktop){const c=await rpc(name);eq([c.company_id,c.user_id,c.role],[company.id,subject,currentRole(test.roles)],'Staff context supports seven-role assignments '+name);}
   else await deny(`select public.${name}()`);
  }
  const change={employee_id:target.id,attendance_id:null,expected_revision:null,work_date:day,checked_in_at:`${day}T08:00:00+08:00`,checked_out_at:`${day}T17:00:00+08:00`,reason:'Fictional reasoned historical correction'};
  const batch={id:id(),expected_timezone:company.timezone,reason:'Fictional batch review',reference:'Fictional private reference',rows:[{id:id(),source_row:2,employee_code:target.employee_code,work_date:importDay,checked_in_at:`${importDay}T08:00:00+08:00`,checked_out_at:`${importDay}T17:00:00+08:00`,reason:'Fictional imported correction',reference:'Fictional row reference'}]};
  if(hr){
   await rpc('correct_attendance',JSON.stringify(change));const saved=(await db.query('select id,employee_id from public.attendance where employee_id=$1 and work_date=$2',[target.id,day])).rows[0];eq(saved.employee_id,target.id,'HR Generalist specialty records actual historical correction');
   const audit=(await db.query('select actor_id,actor_role from public.attendance_corrections where attendance_id=$1',[saved.id])).rows[0];eq(audit,{actor_id:subject,actor_role:currentRole(test.roles)},'Actual protected actor is retained in correction audit');
   const preview=await rpc('preview_attendance_import',JSON.stringify(batch));eq(preview.ready,true,'Specialist HR previews a real pending-request batch');
   await rpc('post_attendance_import',JSON.stringify(batch),preview.fingerprint);
   const history=await rpc('attendance_import_context');const posted=history.batches.find(b=>b.id===batch.id);eq({actor_id:posted.actor_id,actor_role:posted.actor_role},{actor_id:subject,actor_role:currentRole(test.roles)},'Protected import history retains actual importer without employee impersonation');
   await deny('select *from public.attendance_import_batches');
  }else{
   await deny('select public.correct_attendance($1::jsonb)',[JSON.stringify(change)]);
   await deny('select public.preview_attendance_import($1::jsonb)',[JSON.stringify(batch)]);
   await deny('select public.post_attendance_import($1::jsonb,$2)',[JSON.stringify(batch),'a'.repeat(64)]);
   eq((await db.query('select count(*)::int n from public.attendance_corrections')).rows[0].n,0,'Private global correction history stays HR-only');
   await deny('select public.preview_leave_opening($1::jsonb)',['{}']);
  }
 }
 console.log('Passed '+checks+' operational/context/principal/correction/import checks across14role combinations and '+count+' migrations. No approval, clock or payroll protection was bypassed.');
 function currentRole(roles){return roles.includes('administrator')?'administrator':['hr_generalist','recruiter','learning_performance_officer','payroll_officer'].every(r=>roles.includes(r))?'hr_generalist':roles.includes('manager')?'manager':'employee';}
}catch(error){console.error(JSON.stringify({message:error.message,code:error.code,where:error.where,stack:error.stack}));process.exitCode=1;}finally{await db.close();}
