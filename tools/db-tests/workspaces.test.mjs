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

 
let passed=0,seq=1;
const wid=()=>'64000000-0000-4000-8000-'+String(seq++).padStart(12,'0');
const owner=wid(),other=wid(),subject=wid(),foreign=wid(),all=['organization','development','hiring','exit_clearance','payroll','reports'];
const email=u=>'workspace-'+u+'@example.invalid';
for(const u of[owner,other,subject,foreign])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3::jsonb)',[u,email(u),JSON.stringify({role:'administrator'})]);
const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional workspaces',$1)returning id",[owner])).rows[0].id;
const company2=wid();
const session=async(user,role='authenticated')=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user??'']);await db.exec('set role '+role);};
const rpc=async(name,arg)=>(await db.query('select public.'+name+'('+(arg===undefined?'':'$1::jsonb')+')r',arg===undefined?[]:[JSON.stringify(arg)])).rows[0].r;
const eq=(a,b,label)=>{assert.deepEqual(a,b,label);passed++;};
const deny=async(sql,args=[],codes=['42501','23514','22P02','54000'])=>{try{await db.query(sql,args)}catch(e){assert(codes.includes(e.code),e.code+': '+e.message);passed++;return}assert.fail('Expected denial '+sql)};
await session(owner);for(const[u,role]of[[other,'administrator'],[subject,'manager']])await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[u,email(u),role]);
await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company]);
await session(owner);const initial=await rpc('desktop_workspace_context');
eq(initial,{schema_version:1,company_id:company,user_id:owner,revision:0,enabled:all,updated_at:null},'All sections preserved by default');
const request=(ctx,enabled=all)=>({id:wid(),company_id:ctx.company_id,actor_id:ctx.user_id,expected_revision:ctx.revision,enabled});
const write=p=>rpc('save_desktop_workspaces',p);
const first=request(initial,['development','reports']),receipt=await write(first);
eq(receipt.revision,1,'First revision');eq(receipt.enabled,['development','reports'],'Reviewed values');eq(await write(first),receipt,'Exact receipt retry');
const c1=await rpc('desktop_workspace_context');eq(c1.enabled,['development','reports'],'Read current settings');
for(const t of['desktop_workspace_settings','desktop_workspace_events'])await deny('select *from hris_private.'+t);
for(const p of[{...request(c1),id:'00000000-0000-0000-0000-000000000000'},{...request(c1),company_id:company2},{...request(c1),actor_id:other},{...request(c1),enabled:['payroll','payroll']},{...request(c1),enabled:['unknown']},{...request(c1),enabled:[null]},{...request(c1),enabled:'all'},{...request(c1),expected_revision:null},{...request(c1),expected_revision:-1},{...request(c1),unexpected:true}])await deny('select public.save_desktop_workspaces($1::jsonb)',[JSON.stringify(p)]);
await session(other);const r2=await write(request(await rpc('desktop_workspace_context'),[]));eq(r2.revision,2,'Second Admin current source');eq((await rpc('desktop_workspace_context')).enabled,[],'All optional sections hidden');
await deny('select public.save_desktop_workspaces($1::jsonb)',[JSON.stringify({...first,actor_id:other})]);
await session(owner);eq(await write(first),receipt,'Earlier receipt after later changes');await deny('select public.save_desktop_workspaces($1::jsonb)',[JSON.stringify(request(c1))]);
await session(foreign);await deny('select public.desktop_workspace_context()');await deny('select public.save_desktop_workspaces($1::jsonb)',[JSON.stringify(first)]);
const set=async(roles,active=true)=>{await session(owner);const ctx=(await db.query('select public.staff_role_admin_context(true)r')).rows[0].r,a=ctx.accounts.find(x=>x.user_id===subject);await rpc('set_staff_roles',{id:wid(),user_id:subject,expected_revision:a.revision,expected_base_role:a.base_role,expected_member_updated_at:a.member_updated_at,roles,active,reason:'Fictional workspace checks'});};
const fixture=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
for(const test of fixture){
 await set(test.roles);await session(subject);
 if(test.capabilities.includes('desktop')){const c=await rpc('desktop_workspace_context');eq(c.user_id,subject,'Bound subject');eq(c.enabled,[],'Visibility independent of role');if(test.roles.includes('administrator'))eq((await write(request(c,[]))).enabled,[],'Only current Admin writes');else await deny('select public.save_desktop_workspaces($1::jsonb)',[JSON.stringify(request(c))]);}
 else await deny('select public.desktop_workspace_context()');
 eq((await db.query("select hris_private.has_staff_capability($1,'payroll_preparation')v",[company])).rows[0].v,test.capabilities.includes('payroll_preparation'),'No authority changes');
}
await set(['recruiter'],false);await session(subject);await deny('select public.desktop_workspace_context()');
await set(['recruiter']);await db.exec('reset role');const employee=(await db.query('select employee_id from public.company_members where user_id=$1',[subject])).rows[0].employee_id;
await deny("update public.employees set status='archived'where id=$1",[employee],['23514']);
await session(owner);
const current=await rpc('desktop_workspace_context'),failed=request(current,all);
await db.exec('reset role');await db.exec("create function public.workspace_qa_failure()returns trigger language plpgsql as $$begin raise exception 'Fictional audit fault';end$$;create trigger workspace_qa_failure before insert on hris_private.desktop_workspace_events for each row execute function public.workspace_qa_failure();");
await session(owner);await deny('select public.save_desktop_workspaces($1::jsonb)',[JSON.stringify(failed)],['P0001']);eq(await rpc('desktop_workspace_context'),current,'Audit failure atomic rollback');
await db.exec('reset role');await db.exec('drop trigger workspace_qa_failure on hris_private.desktop_workspace_events;drop function public.workspace_qa_failure()');
await deny('update hris_private.desktop_workspace_events set actor_id=actor_id',[],['23514']);await deny('delete from hris_private.desktop_workspace_events',[],['23514']);
for(const role of['anon','service_role']){await session(null,role);await deny('select public.desktop_workspace_context()');await deny('select *from hris_private.desktop_workspace_settings');}
await db.exec('reset role');eq((await db.query("select count(*)::int n from public.companies")).rows[0].n,1,'Single-company invariant retained');
eq((await db.query("select bool_and(not p.prosecdef)ok from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='public'and proname in('desktop_workspace_context','save_desktop_workspaces')")).rows[0].ok,true,'Public wrappers invoker-only');
console.log(JSON.stringify({status:'PASS',assertions:passed,migrations:count,roleCases:fixture.length}));
}finally{await db.close()}
