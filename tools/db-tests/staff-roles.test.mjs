// Foundation only: fictional PostgreSQL Auth/membership fixtures, no hosted access.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {PGlite} from '@electric-sql/pglite';
const db=new PGlite();let checks=0;
const eq=(actual,expected,label)=>{assert.deepEqual(actual,expected,label);checks++;};
const deny=async(sql,args=[],codes=['42501','23514'])=>{
 try{await db.query(sql,args);}catch(e){assert(codes.includes(e.code),`${e.code}: ${e.message}`);checks++;return;}assert.fail('Expected denial: '+sql);
};
const id=n=>'00000000-0000-0000-0000-'+String(n).padStart(12,'0');
const company=id(900);const otherCompany=id(901);const owner=id(1);const hr=id(2);const manager=id(3);const employee=id(4);const foreignOwner=id(5);const disabled=id(6);const archived=id(7);const unknown=id(8);
const session=async(user,role='authenticated')=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user??'']);await db.exec('set role '+role);};
try{
 await db.exec(`create role anon nologin;create role authenticated nologin;create role service_role nologin bypassrls;
 create schema auth;create schema hris_private;grant usage on schema auth,hris_private to authenticated;
 create table auth.users(id uuid primary key,raw_user_meta_data jsonb not null default'{}'::jsonb);
 create function auth.uid()returns uuid language sql stable as $$select nullif(current_setting('request.jwt.claim.sub',true),'')::uuid$$;
 grant execute on function auth.uid()to authenticated;
 create table public.companies(id uuid primary key,owner_id uuid not null references auth.users(id));
 create table public.employees(id uuid primary key,company_id uuid not null references public.companies(id),employee_code text not null,full_name text not null,status text not null,unique(company_id,id));
 create table public.company_members(company_id uuid not null references public.companies(id),user_id uuid primary key references auth.users(id),employee_id uuid not null,role text not null check(role in('administrator','hr_generalist','manager','employee')),active boolean not null default true,unique(company_id,user_id),foreign key(company_id,employee_id)references public.employees(company_id,id));
 create function public.existing_contract()returns text language sql immutable as $$select'preserved'::text$$;`);
 for(let n=1;n<=8;n++)await db.query('insert into auth.users(id)values($1)',[id(n)]);
 await db.query('insert into public.companies values($1,$2),($3,$4)',[company,owner,otherCompany,foreignOwner]);
 const members=[[owner,'administrator',true,'active'],[hr,'hr_generalist',true,'active'],[manager,'manager',true,'active'],[employee,'employee',true,'active'],[foreignOwner,'administrator',true,'active'],[disabled,'hr_generalist',false,'active'],[archived,'employee',true,'archived']];
 for(const [user,role,active,status]of members){const c=user===foreignOwner?otherCompany:company;await db.query('insert into public.employees values($1,$2,$3,$4,$5)',[id(Number(user.slice(-12))+100),c,'EMP-'+user.slice(-3),'Fictional '+role,status]);await db.query('insert into public.company_members values($1,$2,$3,$4,$5)',[c,user,id(Number(user.slice(-12))+100),role,active]);}
 const before={};for(const t of ['companies','employees','company_members'])before[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
 const functionBefore=(await db.query("select md5(pg_get_functiondef('public.existing_contract()'::regprocedure))digest")).rows[0].digest;
 await db.exec(await readFile(new URL('../../supabase/migrations/20261027092804_staff_role_foundation.sql',import.meta.url),'utf8'));
 for(const t of Object.keys(before))eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,before[t],'Existing '+t+' unchanged');
 eq((await db.query("select md5(pg_get_functiondef('public.existing_contract()'::regprocedure))digest")).rows[0].digest,functionBefore,'Existing function preserved');
 eq((await db.query('select count(*)::int n from hris_private.staff_role_assignments')).rows[0].n,7,'All prior assignments captured');
 eq((await db.query('select count(*)::int n from hris_private.staff_role_events')).rows[0].n,7,'Compatibility mappings audited');
 eq((await db.query('select roles from hris_private.staff_role_assignments where user_id=$1',[hr])).rows[0].roles,['hr_generalist','recruiter','learning_performance_officer','payroll_officer'],'Prior HR rights retain explicit specialty mapping');
 const fixture=JSON.parse(await readFile(new URL('../../tests/fixtures/staff_role_capabilities.json',import.meta.url),'utf8'));
 for(const example of fixture){const actual=(await db.query('select hris_private.staff_role_capabilities($1::text[])c',[example.roles])).rows[0].c;eq([...actual].sort(),[...example.capabilities].sort(),'Capability contract '+example.roles.join('+'));}
 eq((await db.query("select hris_private.normalize_staff_roles(array['payroll_officer','recruiter'])r")).rows[0].r,['recruiter','payroll_officer'],'Canonical role order');
 for(const values of [null,[],['employee','manager'],['recruiter','recruiter'],['owner'],['learning_officer'],['performance_officer'],['administrator','hr_generalist','recruiter','learning_performance_officer','payroll_officer','manager','employee'],[null],['recruiter',null]])await deny('select hris_private.normalize_staff_roles($1::text[])',[values]);
 await deny("select hris_private.normalize_staff_roles(array[['manager','employee']])");
 await deny("select hris_private.legacy_staff_roles('owner')");
 await deny("update hris_private.staff_role_events set reason='changed'");
 await deny('delete from hris_private.staff_role_events');
 for(const [user,role]of members.filter(m=>m[2]&&m[3]==='active')){
  await session(user);const c=(await db.query('select public.staff_role_context()c')).rows[0].c;
  eq(c.user_id,user,'Actor-bound context');eq(c.company_id,user===foreignOwner?otherCompany:company,'Own company only');eq(c.employee_id,id(Number(user.slice(-12))+100),'Own employee binding');eq(c.base_role,role,'Current membership role');eq(c.runtime_ready,false,'Runtime gate remains closed');eq(c.schema_version,1,'Versioned context');eq(c.revision,1,'Initial revision');
  eq(Object.keys(c).sort(),['schema_version','company_id','user_id','employee_id','base_role','revision','runtime_ready','roles'].sort(),'Minimal role metadata only');
  for(const t of ['staff_role_assignments','staff_role_events']){
   await deny('select * from hris_private.'+t);await deny('delete from hris_private.'+t);
  }
  await deny("select hris_private.staff_role_capabilities(array['administrator'])");
  if(role!=='administrator')await deny('select public.staff_role_admin_context()');
 }
 await session(owner);let listing=(await db.query('select public.staff_role_admin_context()c')).rows[0].c;
 eq(listing.company_id,company,'Admin list company bound');eq(listing.accounts.length,5,'Disabled hidden by default');eq(listing.accounts.some(a=>a.user_id===foreignOwner),false,'Other company excluded');
 eq(listing.accounts.some(a=>a.user_id===disabled),false,'Disabled excluded');
 listing=(await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;eq(listing.accounts.length,6,'Explicit disabled history');
 eq(listing.accounts.some(a=>a.user_id===disabled),true,'Disabled metadata retained');
 eq(listing.accounts.some(a=>Object.keys(a).some(k=>/email|reason|reference|salary/.test(k))),false,'No private contact or financial fields');
 await deny('select public.staff_role_admin_context(null)');
 for(const user of [disabled,archived,unknown,null]){await session(user);await deny('select public.staff_role_context()');await deny('select public.staff_role_admin_context()');}
 await session(owner,'anon');await deny('select public.staff_role_context()');await deny('select public.staff_role_admin_context()');
 await session(owner,'service_role');await deny('select * from hris_private.staff_role_events');
 await db.exec('reset role');await db.query("update auth.users set raw_user_meta_data='{"+ '"role":"administrator","roles":["administrator"]' +"}'::jsonb where id=$1",[employee]);
 await session(employee);eq((await db.query('select public.staff_role_context()c')).rows[0].c.roles,['employee'],'Forged metadata does not grant roles');await deny('select public.staff_role_admin_context()');
 await db.exec('reset role');await db.query("update hris_private.staff_role_assignments set roles=array['recruiter']where user_id=$1",[employee]);
 await session(employee);let c=(await db.query('select public.staff_role_context()c')).rows[0].c;eq(c.roles,['recruiter'],'Private draft role can be represented');eq(c.runtime_ready,false,'Draft cannot activate new access');
 await db.exec('reset role');await db.query("update public.company_members set role='manager'where user_id=$1",[employee]);
 await session(employee);eq((await db.query('select public.staff_role_context()c')).rows[0].c.roles,['manager'],'Changed legacy role invalidates stale captured mapping');
 await db.exec('reset role');await db.query('update public.company_members set active=false where user_id=$1',[owner]);
 await session(owner);await deny('select public.staff_role_context()');await deny('select public.staff_role_admin_context()');
 await db.exec('reset role');await db.query('update public.company_members set active=true where user_id=$1',[owner]);
 await session(owner);eq((await db.query('select public.staff_role_context()c')).rows[0].c.base_role,'administrator','Owner restored in fixture only');
 await db.exec('reset role');await db.query("update public.employees set status='archived'where id=$1",[id(101)]);
 await session(owner);await deny('select public.staff_role_context()');await deny('select public.staff_role_admin_context()');
 await db.exec('reset role');await db.query("update public.employees set status='active'where id=$1",[id(101)]);
 await db.exec('begin');
 for(const n of [1001]){
  await db.exec("insert into auth.users select ('10000000-0000-0000-0000-'||lpad(i::text,12,'0'))::uuid,'{}'::jsonb from generate_series(1,"+n+")i");
  await db.query("insert into public.employees select ('20000000-0000-0000-0000-'||lpad(i::text,12,'0'))::uuid,$1,'BOUND-'||i,'Fictional bound fixture','active'from generate_series(1,$2::int)i",[company,n]);
  await db.query("insert into public.company_members select $1,('10000000-0000-0000-0000-'||lpad(i::text,12,'0'))::uuid,('20000000-0000-0000-0000-'||lpad(i::text,12,'0'))::uuid,'employee',true from generate_series(1,$2::int)i",[company,n]);
 }
 await session(owner);await deny('select public.staff_role_admin_context()');await db.exec('rollback');
 console.log('Passed '+checks+' staff-role foundation PostgreSQL assertions; specialist runtime remains disabled.');
}finally{await db.close();}
