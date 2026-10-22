// Local PostgreSQL tests. auth.uid() and auth.users are simulated, not hosted Auth.
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { PGlite } from '@electric-sql/pglite';
import { btree_gist } from '@electric-sql/pglite/contrib/btree_gist';

const db = new PGlite({ extensions: { btree_gist } });
let checks = 0;
const eq = (actual, expected, label) => { assert.deepEqual(actual, expected, label); checks++; };
const deny = async (sql, params = [], codes = ['42501','23514','23505','23P01']) => {
  try { await db.query(sql,params); } catch (error) {
    assert.ok(codes.includes(error.code), `Expected denial, got ${error.code}: ${error.message}`);
    checks++; return;
  }
  assert.fail(`Operation should have been denied: ${sql}`);
};
const uidA = '00000000-0000-0000-0000-000000000011';
const uidB = '00000000-0000-0000-0000-000000000012';
const session = async (uid,role='authenticated') => {
  await db.exec('reset role');
  await db.query("select set_config('request.jwt.claim.sub',$1,false)",[uid]);
  await db.query("select set_config('request.headers',$1,false)",[JSON.stringify({'x-hris-leave-contract':'2'})]);
  await db.exec(`set role ${role}`);
};
const employee = async (company,code='EMP-001',allowance=15,status='active') => (await db.query(`
  insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days,status)
  values ($1,$2,'Demo Employee','Operations','Associate','2026-01-01',$3,$4) returning *`,[company,code,allowance,status])).rows[0];
const request = async (company,person,start='2027-01-04',end='2027-01-05') => (await db.query(`
  insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on)
  values ($1,$2,'annual',$3,$4) returning *`,[company,person,start,end])).rows[0];

try {
  await db.exec(`
    create role anon nologin;
    create role authenticated nologin;
    create role service_role nologin bypassrls;
    create schema supabase_migrations;
    create table supabase_migrations.schema_migrations(version text primary key);
    create schema auth;
    create table auth.users(id uuid primary key, email text unique, email_confirmed_at timestamptz,
      raw_user_meta_data jsonb default '{}'::jsonb);
    create function auth.uid() returns uuid language sql stable as
      $$ select nullif(current_setting('request.jwt.claim.sub',true),'')::uuid $$;
    grant usage on schema auth to authenticated;
    grant execute on function auth.uid() to authenticated;
    insert into auth.users(id,email,email_confirmed_at) values ('${uidA}','admin@example.invalid',now()),('${uidB}','unassigned@example.invalid',now());
    -- Minimal managed-Storage contract for local policy tests; no S3 service is simulated.
    create schema storage;
    create table storage.buckets(id text primary key, name text, public boolean default false,
      file_size_limit bigint, allowed_mime_types text[]);
    create table storage.objects(id uuid primary key default gen_random_uuid(),
      bucket_id text references storage.buckets(id), name text, owner_id text, metadata jsonb,
      unique(bucket_id,name));
    alter table storage.objects enable row level security;
    grant usage on schema storage to authenticated,anon;
    grant select,insert,update,delete on storage.objects to authenticated,anon;
  `);
  const folder=new URL('../../supabase/migrations/',import.meta.url);
  let companyA;
  const deferred=[];
  const shiftMigrations=[];
  const breakMigrations=[];
  const calculationMigrations=[];
  const requestMigrations=[];
  for (const filename of (await readdir(folder)).filter(f=>f.endsWith('.sql')).sort()) {
    if(filename.endsWith('_attendance_correction_requests.sql')||filename.endsWith('_attendance_request_context_index.sql')) {requestMigrations.push(filename);continue;}
    if(filename.endsWith('_attendance_calculations.sql')) {calculationMigrations.push(filename);continue;}
    if(filename.endsWith('_attendance_breaks.sql')) {breakMigrations.push(filename);continue;}
    if(filename.endsWith('_shift_schedules.sql')) {shiftMigrations.push(filename);continue;}
    if(filename.endsWith('_leave_policies_and_approval_chains.sql')||filename.endsWith('_leave_policy_tracking_start.sql')||filename.endsWith('_leave_client_compatibility.sql')) {deferred.push(filename);continue;}
    if(filename.endsWith('_employment_history.sql')) {
      companyA=(await db.query(`insert into public.companies(owner_id,name,timezone) values ($1,'Demo Company A','Asia/Manila') returning *`,[uidA])).rows[0];
    }
    await db.exec(await readFile(new URL(filename,folder),'utf8'));
    if(filename.endsWith('_employment_history.sql')) {
      const initial=(await db.query('select * from public.employment_history')).rows;
      eq(initial.length,1,'Migration captures one initial snapshot for each existing employee');
      eq(initial[0].kind,'starting_snapshot','Existing employee baseline is never a fabricated creation event');
      eq([initial[0].before_record,initial[0].actor_id,initial[0].actor_name],[null,null,'System'],'Migration baseline has no invented actor or prior record');
    }
    await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);
  }
  eq((await db.query(`select count(*)::int as n from pg_class where relname in ('companies','employees','attendance','leave_requests') and relrowsecurity`)).rows[0].n,4,'RLS enabled on every HR table');
  eq((await db.query(`select count(*)::int as n from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='public' and p.prosecdef`)).rows[0].n,0,'Exposed RPCs use invoker security; protected definer helpers stay private');

  await session(uidA);
  await deny(`insert into public.companies(owner_id,name) values ($1,'Unapproved first company')`,[uidA],['42501']);
  await session(uidB);
  await deny(`insert into public.companies(owner_id,name) values ($1,'Unapproved other company')`,[uidB],['42501']);
  await db.exec('reset role');
  if(!companyA)companyA=(await db.query(`insert into public.companies(owner_id,name,timezone) values ($1,'Demo Company A','Asia/Manila') returning *`,[uidA])).rows[0];
  await deny(`insert into public.companies(owner_id,name) values ($1,'Second installation company')`,[uidB],['23505']);
  eq((await db.query('select count(*)::int as n from public.companies')).rows[0].n,1,'Database permits exactly one company');
  const photoPath=uidA+'/avatar.png';
  await session(uidA);
  await db.query(`insert into storage.objects(bucket_id,name,owner_id) values ('profile-photos',$1,$2)`,[photoPath,uidA]);
  eq((await db.query('select * from storage.objects')).rows.length,1,'Owner can read own private photo');
  eq((await db.query(`update storage.objects set metadata='{"version":2}' where name=$1 returning id`,[photoPath])).rows.length,1,'Photo replacement can update its own object');
  await deny(`update storage.objects set owner_id=$1 where name=$2`,[uidB,photoPath],['42501']);
  await deny(`update storage.objects set name=$1 where name=$2`,[uidB+'/avatar.png',photoPath],['42501']);
  await deny(`insert into storage.objects(bucket_id,name,owner_id) values ('profile-photos',$1,$2)`,[uidA+'/other.png',uidA],['42501']);
  await session(uidB);
  eq((await db.query('select * from storage.objects')).rows.length,0,'Another account cannot read the photo');
  eq((await db.query(`delete from storage.objects where name=$1 returning id`,[photoPath])).rows.length,0,'Another account cannot delete the photo');
  await deny(`insert into storage.objects(bucket_id,name,owner_id) values ('profile-photos',$1,$2)`,[photoPath,uidB],['42501']);
  await deny(`insert into storage.objects(bucket_id,name,owner_id) values ('profile-photos',$1,$2)`,[uidB+'/avatar.png',uidB],['42501']);
  await session('', 'anon');
  eq((await db.query('select * from storage.objects')).rows.length,0,'Anonymous reads cannot see private photos');
  await deny(`insert into storage.objects(bucket_id,name,owner_id) values ('profile-photos',$1,$2)`,[photoPath,uidA],['42501']);
  await session(uidA);
  eq((await db.query(`delete from storage.objects where name=$1 returning id`,[photoPath])).rows.length,1,'Owner can remove own photo');
  await db.exec('reset role');
  const photoBucket=(await db.query(`select public,file_size_limit::int,allowed_mime_types from storage.buckets where id='profile-photos'`)).rows[0];
  eq(photoBucket,{public:false,file_size_limit:2097152,allowed_mime_types:['image/png']},'Private bucket restricts size and MIME type');
  const unrelatedCompany='00000000-0000-0000-0000-000000000101';
  const unrelatedEmployee='00000000-0000-0000-0000-000000000102';
  await session(uidA);
  eq((await db.query('select * from public.companies')).rows.length,1,'Owner reads own workspace');
  const personA=await employee(companyA.id);
  eq(personA.employee_code,'EMP-001','Employee insert succeeds');
  const archivedA=await employee(companyA.id,'ARCHIVED',15,'archived');
  await deny(`insert into public.companies(owner_id,name) values ($1,'Forged owner')`,[uidB],['42501']);
  await deny(`update public.companies set owner_id=$1 where id=$2`,[uidB,companyA.id],['42501']);
  await deny(`update public.companies set timezone='Invalid/Zone' where id=$1`,[companyA.id],['23514']);
  await deny(`insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days) values ($1,' emp-001 ','Duplicate','Operations','Associate','2026-01-01',15)`,[companyA.id],['23505']);

  await session(uidB);
  eq((await db.query('select * from public.companies')).rows.length,0,'Unrelated account sees no companies');
  eq((await db.query('select * from public.employees')).rows.length,0,'Unrelated account sees no employees');
  await deny(`insert into public.companies(owner_id,name) values ($1,'Other account company')`,[uidB],['42501']);
  eq((await db.query(`update public.employees set full_name='Forbidden' where id=$1 returning id`,[personA.id])).rows.length,0,'Cross-company update affects no rows');
  await deny(`insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days) values ($1,'FORGED','Demo','Ops','Associate','2026-01-01',15)`,[companyA.id],['42501']);
  await deny(`insert into public.attendance(company_id,employee_id) values ($1,$2)`,[companyA.id,personA.id]);
  await deny(`insert into public.attendance(company_id,employee_id) values ($1,$2)`,[unrelatedCompany,personA.id]);

  await session(uidA);
  const attendance=(await db.query(`insert into public.attendance(company_id,employee_id) values ($1,$2) returning *`,[companyA.id,personA.id])).rows[0];
  eq(attendance.checked_out_at,null,'New check-in is open');
  eq(Math.abs(new Date(attendance.checked_in_at).getTime()-Date.now())<10000,true,'Check-in is server-timed');
  await deny(`insert into public.attendance(company_id,employee_id) values ($1,$2)`,[companyA.id,personA.id],['23505']);
  await deny(`insert into public.attendance(company_id,employee_id,checked_in_at) values ($1,$2,'1900-01-01')`,[companyA.id,personA.id],['42501']);
  await deny(`insert into public.attendance(company_id,employee_id) values ($1,$2)`,[companyA.id,archivedA.id],['23514']);
  const closed=(await db.query(`update public.attendance set checked_out_at='1900-01-01' where id=$1 returning *`,[attendance.id])).rows[0];
  eq(new Date(closed.checked_out_at)>=new Date(closed.checked_in_at),true,'Check-out ignores forged client timestamp');
  await deny(`update public.attendance set checked_out_at=now() where id=$1`,[attendance.id],['23514']);
  await deny(`update public.attendance set employee_id=$1 where id=$2`,[unrelatedEmployee,attendance.id],['42501']);
  await deny(`delete from public.attendance where id=$1`,[attendance.id],['42501']);

  const limited=await employee(companyA.id,'LIMITED',2);
  const first=await request(companyA.id,limited.id);
  eq(first.status,'pending','New leave starts pending');
  await deny(`insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on) values ($1,$2,'annual','2027-01-05','2027-01-06')`,[companyA.id,limited.id],['23P01']);
  await deny(`insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on,status) values ($1,$2,'annual','2027-02-01','2027-02-01','approved')`,[companyA.id,limited.id],['42501']);
  const approved=(await db.query(`update public.leave_requests set status='approved' where id=$1 returning *`,[first.id])).rows[0];
  eq(approved.status,'approved','Owner can approve pending leave');
  await deny(`update public.leave_requests set status='rejected' where id=$1`,[first.id],['23514']);
  const extra=await request(companyA.id,limited.id,'2027-01-06','2027-01-06');
  await deny(`update public.leave_requests set status='approved' where id=$1`,[extra.id],['23514']);
  eq((await db.query(`select status from public.leave_requests where id=$1`,[extra.id])).rows[0].status,'pending','Failed approval preserves pending state');
  await db.query(`update public.leave_requests set status='rejected' where id=$1`,[extra.id]);
  const retry=await request(companyA.id,limited.id,'2027-01-06','2027-01-06');
  eq(retry.status,'pending','Rejected dates can be requested again');
  await deny(`update public.leave_requests set employee_id=$1 where id=$2`,[unrelatedEmployee,retry.id],['42501']);
  await deny(`insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on) values ($1,$2,'annual','2027-01-09','2027-01-10')`,[companyA.id,personA.id],['23514']);
  await deny(`insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on) values ($1,$2,'annual','2026-12-31','2027-01-04')`,[companyA.id,personA.id],['23514']);
  await deny(`insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on) values ($1,$2,'annual','2027-02-01','2027-02-01')`,[companyA.id,archivedA.id],['23514']);
  await deny(`delete from public.employees where id=$1`,[personA.id],['42501']);

  await session(uidB);
  eq((await db.query('select * from public.attendance')).rows.length,0,'Attendance isolated across companies');
  eq((await db.query('select * from public.leave_requests')).rows.length,0,'Leave isolated across companies');
  eq((await db.query(`update public.leave_requests set status='rejected' where id=$1 returning id`,[retry.id])).rows.length,0,'Other owner cannot review leave');

  await session('');
  eq((await db.query('select * from public.companies')).rows.length,0,'No subject grants no workspace access');
  await session('', 'anon');
  for (const table of ['companies','employees','attendance','leave_requests']) {
    await deny(`select * from public.${table}`,[],['42501']);
    await deny(`delete from public.${table}`,[],['42501']);
  }
  await deny(`insert into public.companies(owner_id,name) values ($1,'Anonymous')`,[uidA],['42501']);
  await deny(`update public.employees set full_name='Anonymous'`,[],['42501']);
  const {runAccessTests}=await import('./accounts.test.mjs');
  await runAccessTests({db,eq,deny,session,employee,request,company:companyA,owner:uidA,unassigned:uidB});
  const {runContactTests}=await import('./contacts.test.mjs');
  await runContactTests({db,eq,deny,session,employee,company:companyA,owner:uidA,unassigned:uidB});
  const {runRefinementTests}=await import('./refinements.test.mjs');
  await runRefinementTests({db,eq,deny,session,employee,company:companyA,owner:uidA,unassigned:uidB});
  const {runProfileTests}=await import('./profile.test.mjs');
  await runProfileTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  const {runCorrectionTests}=await import('./corrections.test.mjs');
  await runCorrectionTests({db,eq,deny,session,employee,company:companyA,owner:uidA,unassigned:uidB});
  const {runEmploymentTests}=await import('./employment.test.mjs');
  await runEmploymentTests({db,eq,deny,session,employee,company:companyA,owner:uidA,unassigned:uidB});
  const {runHistoryTests}=await import('./history.test.mjs');
  await runHistoryTests({db,eq,deny,session,employee,company:companyA,owner:uidA,unassigned:uidB});
  const {runDocumentTests}=await import('./documents.test.mjs');
  await runDocumentTests({db,eq,deny,session,employee,company:companyA,owner:uidA,unassigned:uidB});
  console.log(`Passed ${checks} legacy schema assertions before upgrading leave writes.`);
  await db.exec('reset role');
  const beforeLeave=(await db.query('select id,company_id,employee_id,kind,starts_on,ends_on,reason,status from public.leave_requests order by id')).rows;
  for(const filename of deferred) {await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);}
  eq((await db.query('select id,company_id,employee_id,kind,starts_on,ends_on,reason,status from public.leave_requests order by id')).rows,beforeLeave,'Leave migration preserves existing identity, dates, notes and final decisions');
  const {runLeaveTests}=await import('./leave.test.mjs');
  await runLeaveTests({db,eq,deny,session,employee,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const beforeShifts=(await db.query('select id,company_id,employee_id,work_date,checked_in_at,checked_out_at,revision from public.attendance order by id')).rows;
  for(const filename of shiftMigrations) {await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);}
  eq((await db.query('select id,company_id,employee_id,work_date,checked_in_at,checked_out_at,revision from public.attendance order by id')).rows,beforeShifts,'Shift migration preserves all existing attendance');
  const {runShiftTests}=await import('./shifts.test.mjs');
  await runShiftTests({db,eq,deny,session,employee,company:companyA,owner:uidA,unassigned:uidB});
  console.log(`Passed ${checks} schema assertions before break migration.`);
  await db.exec('reset role');
  const beforeBreaks=(await db.query('select id,employee_id,work_date,checked_in_at,checked_out_at,revision,planned_shift from public.attendance order by id')).rows;
  for(const filename of breakMigrations) {await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);}
  eq((await db.query('select id,employee_id,work_date,checked_in_at,checked_out_at,revision,planned_shift from public.attendance order by id')).rows,beforeBreaks,'Break migration preserves existing attendance and saved plans');
  const {runBreakTests}=await import('./breaks.test.mjs');
  await runBreakTests({db,eq,deny,session,employee,company:companyA,owner:uidA,unassigned:uidB});
  console.log(`Passed ${checks} schema assertions before calculation migration.`);
  await db.exec('reset role');
  const beforeCalculations=(await db.query('select to_jsonb(a) as original from public.attendance a order by id')).rows.map(r=>r.original);
  for(const filename of calculationMigrations) {await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);}
  eq((await db.query("select to_jsonb(a)-'calculation_snapshot' as original from public.attendance a order by id")).rows.map(r=>r.original),beforeCalculations,'Calculation migration preserves all previous attendance values');
  eq((await db.query('select count(*)::int n from public.attendance where calculation_snapshot is not null')).rows[0].n,0,'No historical calculation snapshot is invented');
  const {runCalculationTests}=await import('./calculations.test.mjs');
  await runCalculationTests({db,eq,deny,session,employee,company:companyA,owner:uidA,unassigned:uidB});
  console.log(`Passed ${checks} schema assertions before request migration.`);
  await db.exec('reset role');
  const beforeRequests=(await db.query('select to_jsonb(a) as original from public.attendance a order by id')).rows;
  const beforeCorrectionRequests=(await db.query('select to_jsonb(c) as original from public.attendance_corrections c order by id')).rows;
  for(const filename of requestMigrations) {await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);}
  eq((await db.query('select to_jsonb(a) as original from public.attendance a order by id')).rows,beforeRequests,'Request migration preserves actual attendance and snapshots');
  eq((await db.query("select to_jsonb(c)-'request_id' as original from public.attendance_corrections c order by id")).rows,beforeCorrectionRequests,'Request migration preserves all earlier corrections');
  const {runRequestTests}=await import('./requests.test.mjs');
  await runRequestTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  console.log(`Passed ${checks} local PostgreSQL migration, authorization, and workflow assertions.`);
} finally {
  await db.close();
}
