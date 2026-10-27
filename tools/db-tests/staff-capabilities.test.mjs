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
 const owner='30000000-0000-4000-8000-000000000001',subject='30000000-0000-4000-8000-000000000002';
 for(const user of[owner,subject])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3)',[user,'fixture-'+user.slice(-3)+'@example.invalid',JSON.stringify({full_name:'Fictional capability actor',role:'administrator',roles:['administrator']})]);
 const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional capability company',$1)returning id",[owner])).rows[0].id;
 const session=async user=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user]);await db.exec('set role authenticated');};
 const caps=async target=>(await db.query('select hris_private.current_staff_capabilities($1)c',[target])).rows[0].c;
 await session(owner);await db.query("select public.save_company_account($1,$2,'employee',true,null,null)",[subject,'fixture-'+subject.slice(-3)+'@example.invalid']);
 const cases=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
 eq(await caps(company),cases[0].capabilities,'Owner remains highest authority during disabled rollout');
 await session(subject);eq(await caps(company),['own_records'],'Forged metadata cannot grant desktop or administrative capabilities');
 await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company]);
 let serial=10;
 for(const test of cases){
  await session(owner);const context=(await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;const current=context.accounts.find(a=>a.user_id===subject);
  const change={id:'30000000-0000-4000-8000-'+String(serial++).padStart(12,'0'),user_id:subject,active:true,expected_revision:current.revision,expected_base_role:current.base_role,expected_member_updated_at:current.member_updated_at,roles:test.roles,reason:'Reviewed role matrix assignment'};
  await db.query('select public.set_staff_roles($1::jsonb)',[JSON.stringify(change)]);await session(subject);
  eq(await caps(company),test.capabilities,'Protected capability matrix '+test.roles.join(','));
  for(const capability of cases[0].capabilities)eq((await db.query('select hris_private.has_staff_capability($1,$2)c',[company,capability])).rows[0].c,test.capabilities.includes(capability),'Explicit capability '+capability);
  eq(await caps(owner),[],'A different company cannot be selected by the caller');
  eq((await db.query("select hris_private.has_staff_capability($1,'future_unknown_capability')c",[company])).rows[0].c,false,'Unknown capability is never granted');
 }
 await session(owner);await db.query("select public.save_company_account($1,$2,'employee',false,null,null)",[subject,'fixture-'+subject.slice(-3)+'@example.invalid']);await session(subject);eq(await caps(company),[],'Revoked membership has no company capabilities');
 await session(owner);await db.exec('reset role');await db.query("update public.employees set status='archived'where id=(select employee_id from public.company_members where user_id=$1)",[subject]);await session(subject);eq(await caps(company),[],'Archived person has no capabilities');
 console.log('Passed '+checks+' protected capability assertions against'+count+'actual migrations; shared native/SQL role cases agree.');

}catch(error){console.error(JSON.stringify({message:error.message,code:error.code,where:error.where}));process.exitCode=1;}finally{await db.close();}
