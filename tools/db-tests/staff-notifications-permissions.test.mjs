// Fresh chronological migration replay; managed Auth/Storage are local fixtures.
import{readFile,readdir}from'node:fs/promises';
import assert from'node:assert/strict';
import{PGlite}from'@electric-sql/pglite';
import{btree_gist}from'@electric-sql/pglite/contrib/btree_gist';
const db=new PGlite({extensions:{btree_gist}});let phase='bootstrap';
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
 let serial=1;const id=()=>`37000000-0000-4000-8000-${String(serial++).padStart(12,'0')}`;
 const owner=id(),subject=id(),team=id(),outside=id(),archived=id(),foreignOwner=id(),unassigned=id();
 for(const user of[owner,subject,team,outside,archived,foreignOwner,unassigned])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3::jsonb)',[user,`fictional-${user}@example.invalid`,JSON.stringify({role:'administrator',roles:['administrator']})]);
 const company=(await db.query("insert into public.companies(name,owner_id,timezone)values('Fictional notification permission company',$1,'Asia/Manila')returning *",[owner])).rows[0];
 const foreign={id:id()};await deny("insert into public.companies(name,owner_id)values('Fictional second company',$1)",[foreignOwner],['23505']);
 const session=async(user,role='authenticated')=>{assert.ok(['authenticated','anon'].includes(role));await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user]);await db.exec('set role '+role);};
 await session(owner);for(const user of[subject,team,outside,archived])await db.query("select public.save_company_account($1,$2,'employee',true,null,null)",[user,`fictional-${user}@example.invalid`]);
 const people=(await db.query('select m.user_id,e.id from public.company_members m join public.employees e on e.id=m.employee_id')).rows;
 const employee=user=>people.find(p=>p.user_id===user).id;
 const rpc=async(name,...args)=>(await db.query(`select public.${name}(${args.map((_,i)=>'$'+(i+1)).join(',')})r`,args)).rows[0].r;
 await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company.id]);
 const today=(await db.query("select (now()at time zone 'Asia/Manila')::date::text d")).rows[0].d;
 const day=async n=>(await db.query('select($1::date+$2::integer)::text d',[today,n])).rows[0].d;
 const training=[],certificates=[];
 for(const user of[subject,team,outside,archived]){
  const cid=user===foreignOwner?foreign.id:company.id;
  const tid=id(),cid2=id();training.push({user,id:tid});certificates.push({user,id:cid2});
  await db.query("insert into public.employee_training(id,company_id,employee_id,employee_name,employee_code,title,starts_on,due_on,status,progress_note,created_by)values($1,$2,$3,'Fictional Person','SYNTHETIC','Fictional Safety Training',$4,$5,'planned','PRIVATE_PROGRESS_SENTINEL',$6)",[tid,cid,employee(user),await day(1),await day(2),user===foreignOwner?foreignOwner:owner]);
  await db.query("insert into public.employee_certifications(id,company_id,employee_id,employee_name,employee_code,title,reference_number,issued_on,expires_on,status,created_by)values($1,$2,$3,'Fictional Person','SYNTHETIC','Fictional Certificate','PRIVATE_CERTIFICATE_ID',$4,$5,'valid',$6)",[cid2,cid,employee(user),await day(-20),await day(4),user===foreignOwner?foreignOwner:owner]);
 }
 const cycle=id(),review=id(),publication=id();
 await db.query("insert into public.performance_cycles(id,company_id,name,starts_on,ends_on,due_on,created_by)values($1,$2,'Fictional Specialist Cycle',$3,$4,$5,$6)",[cycle,company.id,await day(-20),await day(-1),await day(2),owner]);
 for(const [rid,user,reviewer,status]of[[review,team,subject,'draft'],[publication,outside,owner,'submitted']])await db.query("insert into public.performance_reviews(id,company_id,cycle_id,employee_id,employee_name,employee_code,cycle_name,starts_on,ends_on,due_on,reviewer_employee_id,reviewer_name,status,assessment,created_by)values($1,$2,$3,$4,'Fictional Person','SYNTHETIC','Fictional Specialist Cycle',$5,$6,$7,$8,'Fictional Reviewer',$9,'PRIVATE_ASSESSMENT_SENTINEL',$10)",[rid,company.id,cycle,employee(user),await day(-20),await day(-1),await day(2),employee(reviewer),status,owner]);
 await session(owner);await rpc('save_company_account',archived,`fictional-${archived}@example.invalid`,'employee',false,employee(archived),null);await db.exec('reset role');
 await db.query("update public.employees set status='archived'where id=$1",[employee(archived)]);
 const object=id();await db.query("insert into storage.objects(id,bucket_id,name,owner_id)values($1,'desktop-updates','fictional/permission-test.bin',$2)",[object,owner]);
 const sourceDigest=async()=>(await db.query("select md5(jsonb_build_array((select jsonb_agg(to_jsonb(t)order by id)from public.employee_training t),(select jsonb_agg(to_jsonb(t)order by id)from public.employee_certifications t),(select jsonb_agg(to_jsonb(t)order by id)from public.performance_reviews t),(select jsonb_agg(to_jsonb(t)order by id)from storage.objects t))::text)d")).rows[0].d;
 const before=await sourceDigest();
 const roles=async(assigned,active=true)=>{await session(owner);await db.exec('reset role');await db.query('update public.employees set reports_to=null where id=$1',[employee(team)]);await session(owner);const c=await rpc('staff_role_admin_context',true),a=c.accounts.find(a=>a.user_id===subject);await rpc('set_staff_roles',JSON.stringify({id:id(),user_id:subject,active,expected_revision:a.revision,expected_base_role:a.base_role,expected_member_updated_at:a.member_updated_at,roles:assigned,reason:'Fictional reviewed notification duties'}));};
 const ownTraining=training.find(t=>t.user===subject).id;
 const ownKey=`training-due:${ownTraining}:0:soon`;
 const cases=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
 for(const test of cases){
  phase='roles '+test.roles.join(',');
  await roles(test.roles);const desktop=test.capabilities.includes('desktop'),dev=test.capabilities.includes('development_administration'),manager=test.roles.includes('manager');
  if(manager){await db.exec('reset role');await db.query('update public.employees set reports_to=$1 where id=$2',[employee(subject),employee(team)]);}
  await session(subject);
  const portal=await rpc('lifecycle_notifications','portal');
  eq(portal.items.length,3,'Portal gets only own training start/due and own certificate expiry '+test.roles);
  eq(portal.items.every(i=>i.employee_id===employee(subject)),true,'Portal stays own-only even for staff '+test.roles);
  await rpc('lifecycle_notification_reads','portal',[ownKey],true,false);
  eq((await rpc('lifecycle_notifications','portal')).read_keys.includes(ownKey),true,'Own portal markers work for all roles');
  const downloads=await db.query('select id from storage.objects where id=$1',[object]);
  eq(downloads.rows.map(r=>r.id),desktop?[object]:[],'Private updater Storage follows current Desktop capability '+test.roles);
  await deny('select *from public.desktop_releases');
  await deny('insert into storage.objects(bucket_id,name)values($1,$2)',['desktop-updates','fictional/forbidden-'+id()]);
  if(!desktop){
   for(const name of['desktop_update'])await deny(`select public.${name}()`);
   await deny('select public.lifecycle_notifications($1)',['desktop']);
   await deny('select public.lifecycle_notification_reads($1,$2::text[],true,false)',['desktop',[ownKey]]);
   await deny('select public.set_notification_reads($1,$2::text[],true,false)',[company.id,[`checkout:${id()}:0`]]);
   continue;
  }
  const update=await rpc('desktop_update');eq(update.schema_version.length,14,'Specialist staff can check private catalog');
  let inbox=await rpc('lifecycle_notifications','desktop');
  eq(inbox.items.some(i=>i.target_id===publication),dev,'Publication task belongs to Development administration');
  eq(inbox.items.some(i=>i.target_id===review),dev||manager,'Only eligible assigned reviewer receives task');
  for(const user of[subject,team,outside,archived]){
   const allowed=user===subject||(user===team&&(dev||manager))||(user===outside&&dev);
   eq(inbox.items.some(i=>i.target_id===training.find(t=>t.user===user).id),allowed,'Current training subject scope '+user+' '+test.roles);
   eq(inbox.items.some(i=>i.target_id===certificates.find(t=>t.user===user).id),allowed,'Current certificate subject scope '+user+' '+test.roles);
  }
  for(const sentinel of['PRIVATE_PROGRESS_SENTINEL','PRIVATE_CERTIFICATE_ID','PRIVATE_ASSESSMENT_SENTINEL','raw_user_meta_data','staff_roles','payment_reference'])eq(JSON.stringify(inbox).includes(sentinel),false,'Minimal projection excludes '+sentinel);
  const oldKey=`checkout:${id()}:0`;await rpc('staff_notification_reads',company.id,[oldKey,ownKey],true,false);
  eq((await db.query('select read_keys from public.notification_read_state where user_id=$1',[subject])).rows[0].read_keys.includes(oldKey),true,'Mixed staff reads retain own legacy marker');
  eq((await db.query("select read_keys from public.lifecycle_notification_read_state where user_id=$1 and surface='desktop'",[subject])).rows[0].read_keys.includes(ownKey),true,'Native lifecycle markers use distinct own surface');
  await deny('select public.set_notification_reads($1,$2::text[],true,false)',[foreign.id,[oldKey]]);
  const beforeKeys=(await db.query('select read_keys from public.notification_read_state where user_id=$1',[subject])).rows[0].read_keys;
  await deny('select public.staff_notification_reads($1,$2::text[],true,false)',[company.id,[`checkout:${id()}:0`,`training-due:${id()}:0:soon`]],['PT409']);
  eq((await db.query('select read_keys from public.notification_read_state where user_id=$1',[subject])).rows[0].read_keys,beforeKeys,'Invalid mixed operation rolls back all read markers');
  if(dev){
   const foreignKey=inbox.items.find(i=>i.target_id===training.find(t=>t.user===outside).id).key;
   await deny('select public.lifecycle_notification_reads($1,$2::text[],true,false)',['portal',[foreignKey]],['PT409']);
  }
 }
 phase='revocation';await roles(['learning_performance_officer']);await session(subject);let inbox=await rpc('lifecycle_notifications','desktop');
 const former=inbox.items.find(i=>i.target_id===publication).key;
 await roles(['payroll_officer']);await session(subject);eq((await rpc('lifecycle_notifications','desktop')).items.some(i=>i.key===former),false,'Same-base specialty change removes publication task immediately');
 await deny('select public.lifecycle_notification_reads($1,$2::text[],true,false)',['desktop',[former]],['PT409']);
 await roles(['payroll_officer'],false);await db.exec('reset role');await db.query("update public.employees set status='archived'where id=$1",[employee(subject)]);await session(subject);
 for(const name of['desktop_update'])await deny(`select public.${name}()`);
 await deny('select public.lifecycle_notifications($1)',['portal']);await deny('select public.lifecycle_notifications($1)',['desktop']);
 eq((await db.query('select id from storage.objects where id=$1',[object])).rows,[],'Archived former staff cannot download updates despite forged metadata');
 await session(owner);await db.exec('reset role');await db.query("update public.employees set status='active'where id=$1",[employee(subject)]);await roles(['payroll_officer'],false);await session(subject);
 await deny('select public.desktop_update()');await deny('select public.lifecycle_notifications($1)',['desktop']);
 eq((await db.query('select id from storage.objects where id=$1',[object])).rows,[],'Disabled membership revokes updater download');
 for(const user of[unassigned,null]){await session(user,user?'authenticated':'anon');await deny('select public.desktop_update()');await deny('select public.lifecycle_notifications($1)',['portal']);eq((await db.query('select id from storage.objects where id=$1',[object])).rows,[],'Unassigned/anonymous user cannot fetch private update');}
 await db.exec('reset role');eq(await sourceDigest(),before,'Notice/read/update checks preserve all fictional source records');
 for(const [name,args]of[['hris_private.staff_capabilities_for(uuid,uuid)',[company.id,subject]],['hris_private.lifecycle_notification_rows(uuid,uuid,text,date,timestamp with time zone)',[]]])eq((await db.query("select has_function_privilege('authenticated',$1::regprocedure,'EXECUTE')allowed",[name])).rows[0].allowed,false,'Internal capability/projection helper is not client callable');
 console.log('Passed '+checks+' notification/updater assertions across14role combinations and '+count+' migrations. Portal own-only, minimal notices, current roles/reporting, read rollback, revocation and private updater policies pass.');
}catch(error){console.error(JSON.stringify({phase,message:error.message,code:error.code,where:error.where,stack:error.stack}));process.exitCode=1;}finally{await db.close();}
