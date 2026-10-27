// Fresh chronological migration replay; managed Auth/Storage are local fixtures.
import{readFile,readdir,writeFile}from'node:fs/promises';
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
 const functions=(await db.query("select n.nspname schema,p.proname name,p.oid::regprocedure::text identity,p.prosecdef definer,p.prosrc source,has_function_privilege('authenticated',p.oid,'EXECUTE')authenticated,has_function_privilege('anon',p.oid,'EXECUTE')anonymous from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'and not exists(select 1 from pg_depend d where d.objid=p.oid and d.deptype='e')order by n.nspname,p.proname,p.oid::regprocedure::text")).rows;
 const policies=(await db.query("select schemaname schema,tablename,policyname,cmd,roles::text,qual,with_check from pg_policies where schemaname in('public','storage')order by schemaname,tablename,policyname")).rows;
 const tables=(await db.query("select schemaname schema,tablename from pg_tables where schemaname in('public','hris_private')order by schemaname,tablename")).rows;
 await writeFile(new URL('../../work/staff-permissions-inventory.json',import.meta.url),JSON.stringify({migration_count:count,functions,policies,tables},null,2));
 const roleChecks=functions.filter(f=>/hr_generalist|company_role\(|company_members/.test(f.source));
 console.log(JSON.stringify({migrations:count,functions:functions.length,role_checked_functions:roleChecks.length,policies:policies.length,tables:tables.length}));
 console.log('Role-sensitive functions: '+roleChecks.map(f=>f.schema+'.'+f.name).join(', '));
}finally{await db.close();}
