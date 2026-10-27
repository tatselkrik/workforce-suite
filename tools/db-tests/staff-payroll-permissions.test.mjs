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
 const denied=async(sql,params=[])=>{try{await db.query(sql,params);}catch(e){eq(e.code,'42501','Permission denied before changing financial facts: '+sql);return;}assert.fail('Expected authorization rejection: '+sql);};
 let serial=1;const id=()=>`31000000-0000-4000-8000-${String(serial++).padStart(12,'0')}`;
 const owner=id(),subject=id(),recipient=id();
 for(const user of[owner,subject,recipient])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3)',[user,`fictional-${user}@example.invalid`,JSON.stringify({roles:['administrator'],role:'administrator'})]);
 const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional payroll scope company',$1)returning id",[owner])).rows[0].id;
 const session=async user=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user]);await db.exec('set role authenticated');};
 await session(owner);for(const user of[subject,recipient])await db.query("select public.save_company_account($1,$2,'employee',true,null,null)",[user,`fictional-${user}@example.invalid`]);
 await db.exec('reset role');const people=(await db.query('select user_id,employee_id from public.company_members where company_id=$1',[company])).rows;const employee=people.find(p=>p.user_id===recipient).employee_id;
 await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company]);
 const call=async(op,r)=>(await db.query('select public.manage_payroll($1,$2::jsonb)r',[op,JSON.stringify(r)])).rows[0].r;
 const pay={basis:'monthly',rate_cents:3000000,annual_divisor:365,monthly_reference_cents:0,premium_eligible:true,minimum_wage_earner:false,sss:true,philhealth:true,pagibig:true,coverage_note:'',components:[],tin:'',sss_number:'',philhealth_number:'',pagibig_number:''};
 const profile={id:id(),employee_id:employee,effective_on:'2026-02-01',pay,reason:'Fictional protected salary fixture'};
 await session(owner);await call('profile_save',profile);
 const filter={starts_on:'2026-02-01',ends_on:'2026-09-30',axis:'issued',kind:'all',employee_id:null,search:''};
 const cases=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
 const sources=[['select public.government_payroll_sources($1,$2)',['2026-09-01','sss']],['select public.government_registration(null)',[]],['select public.payroll_report($1::jsonb)',[JSON.stringify(filter)]],['select public.payroll_report_v2($1::jsonb)',[JSON.stringify(filter)]],['select public.payroll_exchange_state()',[]]];
 const mutable=['manage_payroll','manage_payroll_cutoff','manage_payroll_thirteenth','manage_payroll_tax','manage_payroll_final','manage_payroll_amendment','manage_payroll_wage','manage_government_allocation'];
 const invariant=(await db.query('select to_jsonb(p)r from public.payroll_profiles p order by id')).rows;
 let nextMonth=2;
 for(const [index,test]of cases.entries()){
  await session(owner);const context=(await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;const current=context.accounts.find(a=>a.user_id===subject);
  const change={id:id(),user_id:subject,active:true,expected_revision:current.revision,expected_base_role:current.base_role,expected_member_updated_at:current.member_updated_at,roles:test.roles,reason:'Reviewed fictional payroll permission assignment'};
  await db.query('select public.set_staff_roles($1::jsonb)',[JSON.stringify(change)]);await session(subject);
  const payroll=test.capabilities.includes('payroll_preparation');
  eq((await db.query('select count(*)::int n from public.payroll_profiles')).rows[0].n,payroll?1:0,'Private salary RLS for '+test.roles);
  eq((await db.query('select hris_private.payroll_hr($1)c',[company])).rows[0].c,payroll,'Payroll table helper matches explicit assignment '+test.roles);
  eq((await db.query('select hris_private.payroll_hr($1)c',[owner])).rows[0].c,false,'No cross-company payroll selection');
  for(const [sql,params]of sources){if(payroll){await db.query(sql,params);checks++;}else await denied(sql,params);}
  if(payroll){
   const run=await call('run_create',{id:id(),month:`2026-${String(nextMonth++).padStart(2,'0')}-01`,title:'Fictional role scope run',reason:'Reviewed preparation permission'});
   eq(run.status,'draft','Protected specialist can prepare real payroll');
   if(!test.roles.includes('administrator')){
    await denied('select public.manage_payroll($1,$2::jsonb)',['run_paid',JSON.stringify({id:run.id,expected_revision:run.revision,reason:'Attempt restricted payment'})]);
    await denied('select public.manage_government_remittance($1,$2::jsonb)',['record','{}']);
   }
  }else{
   for(const name of mutable)await denied(`select public.${name}($1,$2::jsonb)`,['run_create','{}']);
   await denied('select public.save_government_registration($1::jsonb)',['{}']);
  }
 }
 await session(owner);await db.exec('reset role');eq((await db.query('select to_jsonb(p)r from public.payroll_profiles p order by id')).rows,invariant,'Role changes/preparation never rewrite existing salary facts');
 const current=(await db.query('select *from public.company_members where user_id=$1',[subject])).rows[0];
 await db.query('update public.company_members set active=false where user_id=$1',[subject]);await session(subject);
 await denied('select public.payroll_report($1::jsonb)',[JSON.stringify(filter)]);eq((await db.query('select count(*)::int n from public.payroll_profiles')).rows[0].n,0,'Revocation removes salary access');
 console.log('Passed '+checks+' actual payroll/government/RLS authorization checks against '+count+' migrations. No financial rates or issued facts changed.');
}catch(error){console.error(JSON.stringify({message:error.message,code:error.code,where:error.where,stack:error.stack}));process.exitCode=1;}finally{await db.close();}
