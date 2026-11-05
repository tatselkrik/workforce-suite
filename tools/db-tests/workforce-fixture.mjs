import {readFile,readdir} from 'node:fs/promises';
import assert from 'node:assert/strict';

export async function replayWorkforceFixture(db){
 await db.exec(`create role anon nologin;create role authenticated nologin;create role service_role nologin bypassrls;
 create schema supabase_migrations;create table supabase_migrations.schema_migrations(version text primary key);
 create schema auth;create table auth.users(id uuid primary key,email text unique,email_confirmed_at timestamptz,raw_user_meta_data jsonb default'{}'::jsonb);
 create function auth.uid()returns uuid language sql stable as $$select nullif(current_setting('request.jwt.claim.sub',true),'')::uuid$$;
 grant usage on schema auth to authenticated;grant execute on function auth.uid()to authenticated;
 create schema storage;create table storage.buckets(id text primary key,name text,public boolean default false,file_size_limit bigint,allowed_mime_types text[]);
 create table storage.objects(id uuid primary key default gen_random_uuid(),bucket_id text references storage.buckets(id),name text,owner_id text,metadata jsonb,unique(bucket_id,name));
 alter table storage.objects enable row level security;grant usage on schema storage to authenticated,anon;grant select,insert,update,delete on storage.objects to authenticated,anon;`);
 const folder=new URL('../../supabase/migrations/',import.meta.url);
 const facts=async()=>{
  const rows={};
  for(const {schemaname,tablename}of(await db.query("select schemaname,tablename from pg_tables where schemaname in('public','hris_private')order by 1,2")).rows){
   assert(/^[a-z0-9_]+$/.test(schemaname+tablename));
   rows[schemaname+'.'+tablename]=(await db.query('select to_jsonb(x)r from '+schemaname+'.'+tablename+' x order by to_jsonb(x)::text')).rows;
  }
  return rows;
 };
 for(const file of(await readdir(folder)).filter(f=>f.endsWith('.sql')).sort()){
  const preserve=file.endsWith('_workforce_attendance_and_work_reports.sql')?await facts():null;
  try{await db.exec(await readFile(new URL(file,folder),'utf8'));}
  catch(e){throw Error('Fresh replay failed at '+file+': '+e.message,{cause:e});}
  if(preserve){const after=await facts();for(const [table,rows]of Object.entries(preserve))assert.deepEqual(after[table],rows,'Prior rows preserved: '+table);}
  await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[file.split('_')[0]]);
 }
}
