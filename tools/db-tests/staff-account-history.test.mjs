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
 let serial=1;const id=()=>`39000000-0000-4000-8000-${String(serial++).padStart(12,'0')}`;
 const owner=id(),subject=id(),target=id(),unassigned=id();const email=u=>`fictional-${u}@example.invalid`;
 for(const u of[owner,subject,target,unassigned])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3::jsonb)',[u,email(u),JSON.stringify({role:'administrator',roles:['administrator']})]);
 const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional audit company',$1)returning *",[owner])).rows[0];
 const session=async(user,role='authenticated')=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user??'']);await db.exec('set role '+role);};
 const rpc=async(name,...args)=>(await db.query(`select public.${name}(${args.map((_,i)=>'$'+(i+1)).join(',')})r`,args)).rows[0].r;
 await session(owner);for(const u of[subject,target])await rpc('save_company_account',u,email(u),'employee',true,null,null);
 const employee=(await db.query('select employee_id from public.company_members where user_id=$1',[target])).rows[0].employee_id;
 await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company.id]);
 // Represent an earlier source that never stored access status; preserve unknown.
 await db.query("insert into hris_private.staff_role_events(company_id,user_id,kind,roles,revision,reason)values($1,$2,'legacy_backfill',array['employee'],2,'Fictional earlier baseline with unknown status')",[company.id,target]);
 await db.query('update hris_private.staff_role_assignments set revision=2 where company_id=$1 and user_id=$2',[company.id,target]);
 await db.query("insert into public.account_access_events(company_id,actor_id,user_id,action,details)values($1,$2,$3,'access_changed',$4::jsonb)",[company.id,owner,target,JSON.stringify({role:'employee',password:'PRIVATE_PASSWORD_SENTINEL',email:'PRIVATE_EMAIL_SENTINEL',employee_id:employee,source_bundle:'PRIVATE_SOURCE_SENTINEL'})]);
 const beforeRole=(await db.query('select *from hris_private.staff_role_events where user_id=$1 order by revision',[target])).rows;
 const beforeAccess=(await db.query('select *from public.account_access_events where user_id=$1 order by created_at,id',[target])).rows;
 await session(owner);let h=await rpc('staff_account_history',target);
 eq([h.company_id,h.actor_id,h.user_id,h.employee_id,h.revision],[company.id,owner,target,employee,2],'History binds current Administrator, company and selected employee source');
 eq([h.role_event_count,h.access_event_count],[2,2],'Complete source counts are explicit');
 eq([h.role_events[1].active,h.role_events[1].previous_active,h.role_events[1].actor_name],[null,null,null],'Missing historical facts stay unknown');
 eq(h.access_events[1].active,null,'Missing original access status is not inferred from current access');
 eq(h.current_roles,['employee'],'Current and historical roles remain distinct');
 for(const text of['PRIVATE_PASSWORD_SENTINEL','PRIVATE_EMAIL_SENTINEL','PRIVATE_SOURCE_SENTINEL','raw_user_meta_data','request_payload','email_confirmed','staff_provision_intents'])eq(JSON.stringify(h).includes(text),false,'Audit whitelists only required history '+text);
 const cases=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
 for(const test of cases){
  await session(owner);const c=await rpc('staff_role_admin_context',true),a=c.accounts.find(a=>a.user_id===subject);
  await rpc('set_staff_roles',JSON.stringify({id:id(),user_id:subject,active:true,expected_revision:a.revision,expected_base_role:a.base_role,expected_member_updated_at:a.member_updated_at,roles:test.roles,reason:'Fictional audit permission duties'}));
  await session(subject);
  if(test.capabilities.includes('account_administration')){
   h=await rpc('staff_account_history',target);eq([h.actor_id,h.user_id],[subject,target],'Independent Administrator can inspect history');
  }else{
   await deny('select public.staff_account_history($1)',[target]);await deny('select public.staff_account_history($1)',[subject]);
   eq((await db.query('select id from public.account_access_events where user_id=$1',[target])).rows,[],'Raw original history remains Admin-only');
  }
  await deny('select *from hris_private.staff_role_events');
 }
 await session(owner);await deny('select public.staff_account_history($1)',[unassigned]);await deny('select public.staff_account_history($1)',[null],['23514']);
 await rpc('save_company_account',target,email(target),'employee',false,null,null);h=await rpc('staff_account_history',target);
 eq(h.active,false,'Administrator retains disabled-account history');eq(h.revision,3,'Legacy disable and new audit projection share protected revision');
 eq(h.role_events[2].previous_active,null,'An earlier unknown status is not rewritten during later capture');eq(h.role_events[2].active,false,'New access fact remains explicit');
 await session(unassigned);await deny('select public.staff_account_history($1)',[target]);await session(null,'anon');await deny('select public.staff_account_history($1)',[target]);
 await db.exec('reset role');eq((await db.query('select *from hris_private.staff_role_events where id=any($1::uuid[])order by revision',[beforeRole.map(e=>e.id)])).rows,beforeRole,'History reads never mutate old role facts');
 eq((await db.query('select *from public.account_access_events where id=any($1::uuid[])order by created_at,id',[beforeAccess.map(e=>e.id)])).rows,beforeAccess,'History reads never mutate original raw events');
 await db.query("insert into hris_private.staff_role_events(company_id,user_id,kind,roles,revision,reason)select $1,$2,'legacy_backfill',array['employee'],n,'Fictional complete-view capacity fixture'from generate_series(4,1001)n",[company.id,target]);
 await session(owner);await deny('select public.staff_account_history($1)',[target],['54000']);
 console.log('Passed '+checks+' account history scope/unknown/minimality/immutability/capacity assertions across14roles and'+count+'migrations.');
}catch(error){console.error(JSON.stringify({message:error.message,code:error.code,where:error.where,stack:error.stack}));process.exitCode=1;}finally{await db.close();}
