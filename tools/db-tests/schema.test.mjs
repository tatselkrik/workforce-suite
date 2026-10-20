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
    create schema auth;
    create table auth.users(id uuid primary key);
    create function auth.uid() returns uuid language sql stable as
      $$ select nullif(current_setting('request.jwt.claim.sub',true),'')::uuid $$;
    grant usage on schema auth to authenticated;
    grant execute on function auth.uid() to authenticated;
    insert into auth.users values ('${uidA}'),('${uidB}');
  `);
  const folder=new URL('../../supabase/migrations/',import.meta.url);
  for (const filename of (await readdir(folder)).filter(f=>f.endsWith('.sql')).sort()) {
    await db.exec(await readFile(new URL(filename,folder),'utf8'));
  }
  eq((await db.query(`select count(*)::int as n from pg_class where relname in ('companies','employees','attendance','leave_requests') and relrowsecurity`)).rows[0].n,4,'RLS enabled on every HR table');
  eq((await db.query(`select count(*)::int as n from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='hris_private' and p.prosecdef`)).rows[0].n,0,'No security-definer bypass');

  await session(uidA);
  await deny(`insert into public.companies(owner_id,name) values ($1,'Unapproved first company')`,[uidA],['42501']);
  await session(uidB);
  await deny(`insert into public.companies(owner_id,name) values ($1,'Unapproved other company')`,[uidB],['42501']);
  await db.exec('reset role');
  const companyA=(await db.query(`insert into public.companies(owner_id,name,timezone) values ($1,'Demo Company A','Asia/Manila') returning *`,[uidA])).rows[0];
  await deny(`insert into public.companies(owner_id,name) values ($1,'Second installation company')`,[uidB],['23505']);
  eq((await db.query('select count(*)::int as n from public.companies')).rows[0].n,1,'Database permits exactly one company');
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
  console.log(`Passed ${checks} local PostgreSQL schema, authorization, and workflow assertions.`);
} finally {
  await db.close();
}
