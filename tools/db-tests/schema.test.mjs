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
  const onboardingMigrations=[];
  const exitMigrations=[];
  const performanceMigrations=[];
  const learningMigrations=[];
  const careerMigrations=[];
  const payrollMigrations=[];
  const portalMigrations=[];
  const cutoffMigrations=[];
  const thirteenthMigrations=[];
  const annualTaxMigrations=[];
  const receiptMigrations=[];
  const finalPayMigrations=[];
  const wageMigrations=[];
  const payrollReportMigrations=[];
  const amendmentMigrations=[];
  const portalTimeMigrations=[];
  const portalInboxMigrations=[];
  const governmentMigrations=[];
  const governmentReconciliationMigrations=[];
  const governmentRegistrationMigrations=[];
  const governmentPreparationMigrations=[];
  const lifecycleMigrations=[];
  const creditMigrations=[];
  const delegationMigrations=[];
  const datedMigrations=[];
  const attendanceImportMigrations=[];
  const leaveImportMigrations=[];
  const exchangeMigrations=[];
  const annualLedgerMigrations=[];
  const birIdentityMigrations=[];
  const annualPayMigrations=[];
  const staffRoleMigrations=[];
  const staffTransactionMigrations=[];
  const staffProvisionMigrations=[];
  const staffCapabilityMigrations=[];
  const staffPermissionMigrations=[];
  const staffWorkflowMigrations=[];
  let companyA;
  const deferred=[];
  const shiftMigrations=[];
  const breakMigrations=[];
  const calculationMigrations=[];
  const requestMigrations=[];
  const overtimeMigrations=[];
  const importMigrations=[];
  const reportPreferenceMigrations=[];
  const notificationMigrations=[];
  const hiringMigrations=[];
  const recruitingMigrations=[];
  for (const filename of (await readdir(folder)).filter(f=>f.endsWith('.sql')).sort()) {
    if(filename.endsWith('_staff_workflow_people.sql')){staffWorkflowMigrations.push(filename);continue;}
    if(filename.endsWith('_staff_account_safeguards.sql')){staffPermissionMigrations.push(filename);continue;}
    if(filename.endsWith('_staff_account_history.sql')){staffPermissionMigrations.push(filename);continue;}
    if(/_staff_.*_permissions\.sql$/.test(filename)){staffPermissionMigrations.push(filename);continue;}
    if(filename.endsWith('_staff_permission_capabilities.sql')){staffCapabilityMigrations.push(filename);continue;}
    if(filename.endsWith('_staff_account_provisioning.sql')){staffProvisionMigrations.push(filename);continue;}
    if(filename.endsWith('_staff_role_transactions.sql')){staffTransactionMigrations.push(filename);continue;}
    if(filename.endsWith('_staff_role_foundation.sql')){staffRoleMigrations.push(filename);continue;}
    if(filename.endsWith('_government_annual_pay_details.sql')){annualPayMigrations.push(filename);continue;}
    if(filename.endsWith('_government_bir_identity_review.sql')){birIdentityMigrations.push(filename);continue;}
    if(filename.endsWith('_government_annual_recorded_sources.sql')){annualLedgerMigrations.push(filename);continue;}
    if(filename.endsWith('_payroll_exchange.sql')){exchangeMigrations.push(filename);continue;}
    if(filename.endsWith('_leave_request_imports.sql')){leaveImportMigrations.push(filename);continue;}
    if(filename.endsWith('_attendance_request_imports.sql')){attendanceImportMigrations.push(filename);continue;}
    if(filename.endsWith('_leave_opening_arithmetic.sql')||filename.endsWith('_leave_openings_and_policy_schedule.sql')){datedMigrations.push(filename);continue;}
    if(filename.endsWith('_approval_delegation.sql')){delegationMigrations.push(filename);continue;}
    if(filename.endsWith('_audited_leave_credits.sql')||filename.endsWith('_leave_credit_active_team_scope.sql')){creditMigrations.push(filename);continue;}
    if(filename.endsWith('_lifecycle_notifications.sql')){lifecycleMigrations.push(filename);continue;}
    if(filename.endsWith('_government_preparation_exports.sql')){governmentPreparationMigrations.push(filename);continue;}
    if(filename.endsWith('_government_registration_records.sql')){governmentRegistrationMigrations.push(filename);continue;}
    if(filename.endsWith('_government_reconciliation.sql')){governmentReconciliationMigrations.push(filename);continue;}
    if(filename.endsWith('_government_payroll_sources.sql')){governmentMigrations.push(filename);continue;}
    if(filename.endsWith('_employee_portal_notifications.sql')){portalInboxMigrations.push(filename);continue;}
    if(filename.endsWith('_employee_portal_attendance_context.sql')){portalTimeMigrations.push(filename);continue;}
    if(filename.includes('_payroll_cross_year_')||filename.includes('_payroll_amendments')){amendmentMigrations.push(filename);continue;}
    if(filename.includes('_expanded_payroll_reporting')){payrollReportMigrations.push(filename);continue;}
    if(filename.includes('_regional_wage_')){wageMigrations.push(filename);continue;}
    if(filename.includes('_payroll_final_pay_')){finalPayMigrations.push(filename);continue;}
    if(filename.endsWith('_payroll_individual_receipts.sql')){receiptMigrations.push(filename);continue;}
    if(filename.endsWith('_payroll_annual_tax_engine.sql')||filename.endsWith('_payroll_annual_tax_workflow.sql')||filename.endsWith('_payroll_annual_tax_receipt_index.sql')){annualTaxMigrations.push(filename);continue;}
    if(filename.endsWith('_payroll_thirteenth_engine.sql')||filename.endsWith('_payroll_thirteenth_workflow.sql')){thirteenthMigrations.push(filename);continue;}
    if(filename.endsWith('_payroll_cutoffs.sql')||filename.endsWith('_payroll_cutoff_read_policy.sql')||filename.endsWith('_payroll_cutoff_ot_reconciliation.sql')){cutoffMigrations.push(filename);continue;}
    if(filename.endsWith('_philippine_payroll_benefits.sql')||filename.endsWith('_payroll_payslip_read_policy.sql')||filename.endsWith('_payroll_double_holiday_monthly.sql')){payrollMigrations.push(filename);continue;}
    if(filename.endsWith('_employee_portal_leave.sql')){portalMigrations.push(filename);continue;}
    if(filename.endsWith('_career_development_plans.sql')){careerMigrations.push(filename);continue;}
    if(filename.endsWith('_employee_learning_certifications.sql')){learningMigrations.push(filename);continue;}
    if(filename.endsWith('_performance_goals_appraisals.sql')){performanceMigrations.push(filename);continue;}
    if(filename.endsWith('_employee_exit_clearance.sql')){exitMigrations.push(filename);continue;}
    if(filename.endsWith('_hiring_offers_onboarding.sql')) {onboardingMigrations.push(filename);continue;}
    if(filename.endsWith('_recruiting_attachments_interviews.sql')) {recruitingMigrations.push(filename);continue;}
    if(filename.endsWith('_applicant_pipeline.sql')) {hiringMigrations.push(filename);continue;}
    if(filename.endsWith('_notification_read_state.sql')) {notificationMigrations.push(filename);continue;}
    if(filename.endsWith('_report_preferences.sql')) {reportPreferenceMigrations.push(filename);continue;}
    if(filename.endsWith('_employee_csv_imports.sql')) {importMigrations.push(filename);continue;}
    if(filename.endsWith('_overtime_approvals.sql')) {overtimeMigrations.push(filename);continue;}
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
  console.log(`Passed ${checks} schema assertions before overtime migration.`);
  await db.exec('reset role');
  const beforeOvertime=(await db.query('select to_jsonb(a) original from public.attendance a order by id')).rows;
  const beforeOvertimeRequests=(await db.query('select to_jsonb(r) original from public.attendance_requests r order by id')).rows;
  for(const filename of overtimeMigrations) {await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);}
  eq((await db.query('select to_jsonb(a) original from public.attendance a order by id')).rows,beforeOvertime,'Overtime migration preserves every attendance fact and saved rule');
  eq((await db.query('select to_jsonb(r) original from public.attendance_requests r order by id')).rows,beforeOvertimeRequests,'Overtime migration preserves correction-request history');
  const {runOvertimeTests}=await import('./overtime.test.mjs');
  await runOvertimeTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const beforeImports=(await db.query('select id,employee_code,full_name from public.employees order by id')).rows;
  for(const filename of importMigrations) {await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);}
  eq((await db.query('select id,employee_code,full_name from public.employees order by id')).rows,beforeImports,'Import migration preserves all existing employee records');
  const {runImportTests}=await import('./imports.test.mjs');
  await runImportTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  console.log(`Passed ${checks} schema assertions before report preferences.`);
  await db.exec('reset role');
  const {runReportPreferenceTests,reportView}=await import('./report-preferences.test.mjs');
  const legacy=[reportView(companyA.id,'Synthetic legacy view')];
  await db.query("update auth.users set raw_user_meta_data=jsonb_set(coalesce(raw_user_meta_data,'{}'::jsonb),'{hris_report_views}',$2::jsonb) where id=$1",[uidA,JSON.stringify(legacy)]);
  const beforePreferenceAuth=(await db.query('select id,raw_user_meta_data from auth.users order by id')).rows;
  const beforePreferencePeople=(await db.query('select to_jsonb(e) original from public.employees e order by id')).rows;
  for(const filename of reportPreferenceMigrations) {await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);}
  eq((await db.query('select id,raw_user_meta_data from auth.users order by id')).rows,beforePreferenceAuth,'Preference migration preserves Auth metadata and legacy views');
  eq((await db.query('select to_jsonb(e) original from public.employees e order by id')).rows,beforePreferencePeople,'Preference migration preserves employee records');
  await runReportPreferenceTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB,legacy});
  console.log(`Passed ${checks} schema assertions before notifications.`);
  await db.exec('reset role');
  const beforeNotifications=(await db.query('select to_jsonb(e) original from public.employees e order by id')).rows;
  const beforeNotificationAuth=(await db.query('select id,raw_user_meta_data from auth.users order by id')).rows;
  for(const filename of notificationMigrations) {await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);}
  eq((await db.query('select to_jsonb(e) original from public.employees e order by id')).rows,beforeNotifications,'Notification migration preserves all employee records');
  eq((await db.query('select id,raw_user_meta_data from auth.users order by id')).rows,beforeNotificationAuth,'Notification migration preserves Auth metadata');
  const {runNotificationTests}=await import('./notifications.test.mjs');
  await runNotificationTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  console.log(`Passed ${checks} schema assertions before applicants.`);
  await db.exec('reset role');
  const preservedTables=['employees','company_members','attendance','leave_requests','report_preferences','notification_read_state'];
  const preserved=await Promise.all(preservedTables.map(t=>db.query(`select to_jsonb(t) original from public.${t} t order by to_jsonb(t)::text`)));
  const beforeHiringAuth=(await db.query('select to_jsonb(t) original from auth.users t order by id')).rows;
  for(const filename of hiringMigrations) {await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);}
  for(let i=0;i<preservedTables.length;i++) eq((await db.query(`select to_jsonb(t) original from public.${preservedTables[i]} t order by to_jsonb(t)::text`)).rows,preserved[i].rows,`Applicant migration preserves ${preservedTables[i]}`);
  eq((await db.query('select to_jsonb(t) original from auth.users t order by id')).rows,beforeHiringAuth,'Applicant migration preserves Auth');
  const {runHiringTests}=await import('./hiring.test.mjs');
  await runHiringTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  console.log(`Passed ${checks} schema assertions before recruiting attachments/interviews.`);
  await db.exec('reset role');
  const beforeRecruiting=(await db.query('select to_jsonb(t) r from public.applicants t order by id')).rows;
  for(const filename of recruitingMigrations) {await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version) values($1)',[filename.split('_')[0]]);}
  eq((await db.query('select to_jsonb(t) r from public.applicants t order by id')).rows,beforeRecruiting,'Recruiting migration preserves applicant records');
  const {runRecruitingTests}=await import('./recruiting.test.mjs');
  await runRecruitingTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const preservedHiring=(await db.query('select to_jsonb(t) r from public.applicants t order by id')).rows;
  for(const filename of onboardingMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  eq((await db.query('select to_jsonb(t) r from public.applicants t order by id')).rows,preservedHiring,'Offer migration preserves applications');
  const {runOnboardingTests}=await import('./onboarding.test.mjs');
  await runOnboardingTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  for(const filename of exitMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const {runExitTests}=await import('./exits.test.mjs');
  await runExitTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  for(const filename of performanceMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const {runPerformanceTests}=await import('./performance.test.mjs');
  await runPerformanceTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  for(const filename of learningMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const {runLearningTests}=await import('./learning.test.mjs');
  await runLearningTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  for(const filename of careerMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const {runCareerTests}=await import('./career.test.mjs');
  await runCareerTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  for(const filename of payrollMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const {runPayrollTests}=await import('./payroll.test.mjs');
  await runPayrollTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  for(const filename of portalMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const {runPortalTests}=await import('./portal.test.mjs');
  await runPortalTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  for(const filename of cutoffMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const {runCutoffTests}=await import('./cutoffs.test.mjs');
  await runCutoffTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  for(const filename of thirteenthMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const {runThirteenthEngineTests}=await import('./thirteenth.test.mjs');
  await runThirteenthEngineTests({db,eq,deny,session,owner:uidA,unassigned:uidB});
  const {runThirteenthWorkflowTests}=await import('./thirteenth-workflow.test.mjs');
  await runThirteenthWorkflowTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  for(const filename of annualTaxMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const {runAnnualTaxEngineTests}=await import('./annual-tax.test.mjs');
  await runAnnualTaxEngineTests({db,eq,deny,session,owner:uidA,unassigned:uidB});
  const {runAnnualTaxWorkflowTests}=await import('./annual-tax-workflow.test.mjs');
  await runAnnualTaxWorkflowTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const beforeReceipts={};for(const table of ['payroll_runs','payroll_slips','payroll_cutoff_runs','payroll_cutoff_slips','payroll_thirteenth_runs','payroll_thirteenth_slips','payroll_tax_reconciliations','payroll_tax_slips'])beforeReceipts[table]=(await db.query(`select to_jsonb(r) r from public.${table} r order by id`)).rows;
  for(const filename of receiptMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const[table,rows]of Object.entries(beforeReceipts))eq((await db.query(`select to_jsonb(r) r from public.${table} r order by id`)).rows,rows,'Individual receipt migration preserves '+table);
  const {runPaymentReceiptTests}=await import('./payment-receipts.test.mjs');
  await runPaymentReceiptTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const finalPreservedTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(x=>x.tablename);
  const finalPreserved={};for(const name of finalPreservedTables)finalPreserved[name]=(await db.query(`select to_jsonb(t)-'final_case_id' r from public.${name} t order by to_jsonb(t)::text`)).rows;
  for(const filename of finalPayMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const name of finalPreservedTables)eq((await db.query(`select to_jsonb(t)-'final_case_id' r from public.${name} t order by to_jsonb(t)::text`)).rows,finalPreserved[name],'Final-pay migrations preserve prior public '+name);
  const {runFinalPayEngineTests}=await import('./final-pay.test.mjs');
  await runFinalPayEngineTests({db,eq,deny,session,owner:uidA,unassigned:uidB});
  const {runFinalPayWorkflowTests}=await import('./final-pay-workflow.test.mjs');
  await runFinalPayWorkflowTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const wageTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(x=>x.tablename);
  const wagePreserved={};for(const t of wageTables)wagePreserved[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  for(const filename of wageMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of wageTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,wagePreserved[t],'Regional wages preserve prior public '+t);
  const {runWageEngineTests}=await import('./regional-wages.test.mjs');
  await runWageEngineTests({db,eq,deny,session,owner:uidA});
  const {runWageWorkflowTests}=await import('./regional-wage-workflow.test.mjs');
  await runWageWorkflowTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const reportTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(x=>x.tablename);
  const reportBefore={};for(const t of reportTables)reportBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  for(const filename of payrollReportMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const {runPayrollReportTests}=await import('./payroll-reporting.test.mjs');
  await runPayrollReportTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');for(const t of reportTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,reportBefore[t],'Read-only reporting preserves public '+t);

  await db.exec('reset role');const preAmendTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);const preAmend={};
  for(const t of preAmendTables)preAmend[t]=(await db.query('select to_jsonb(t)-array[\'cash_year_reviewed\',\'cash_year_note\']r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  for(const filename of amendmentMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of preAmendTables)eq((await db.query('select to_jsonb(t)-array[\'cash_year_reviewed\',\'cash_year_note\']r from public.'+t+' t order by to_jsonb(t)::text')).rows,preAmend[t],'Amendment migrations preserve prior '+t+' facts apart from additive private review columns');
  const {runCrossYearTests}=await import('./payroll-cross-year.test.mjs');
  await runCrossYearTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  const {runAmendmentTests}=await import('./payroll-amendments.test.mjs');
  await runAmendmentTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');const beforePortalTime={};const portalTimeTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  for(const t of portalTimeTables)beforePortalTime[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  for(const filename of portalTimeMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of portalTimeTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,beforePortalTime[t],'Own portal context preserves prior '+t+' records');
  const{runPortalAttendanceTests}=await import('./portal-attendance.test.mjs');await runPortalAttendanceTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');const inboxBefore={};const inboxTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  for(const t of inboxTables)inboxBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  for(const filename of portalInboxMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of inboxTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,inboxBefore[t],'Portal inbox migration preserves prior '+t+' records');
  const{runPortalNotificationTests}=await import('./portal-notifications.test.mjs');await runPortalNotificationTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');const governmentBefore={};const governmentTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  for(const t of governmentTables)governmentBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  for(const filename of governmentMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const{runGovernmentSourceTests}=await import('./government-payroll.test.mjs');await runGovernmentSourceTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');for(const t of governmentTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,governmentBefore[t],'Government read-only migration preserves '+t);
  const beforeGovernmentWorkflow={};for(const t of governmentTables)beforeGovernmentWorkflow[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  for(const filename of governmentReconciliationMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of governmentTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,beforeGovernmentWorkflow[t],'Government workflow migration preserves '+t);
  const{runGovernmentReconciliationTests}=await import('./government-reconciliation.test.mjs');await runGovernmentReconciliationTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');const registrationBefore={};const registrationTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  for(const t of registrationTables)registrationBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  for(const filename of governmentRegistrationMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of registrationTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,registrationBefore[t],'Registration migration preserves '+t);
  const{runGovernmentRegistrationTests}=await import('./government-registration.test.mjs');await runGovernmentRegistrationTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');const preparationBefore={};const preparationTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  for(const t of preparationTables)preparationBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  for(const filename of governmentPreparationMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  const{runGovernmentPreparationTests}=await import('./government-preparation.test.mjs');await runGovernmentPreparationTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');for(const t of preparationTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,preparationBefore[t],'Read-only agency preparation preserves '+t);
  await db.exec('reset role');const lifecycleBefore={};const lifecycleTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  for(const t of lifecycleTables)lifecycleBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const priorFunctions=(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  for(const filename of lifecycleMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of lifecycleTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,lifecycleBefore[t],'Lifecycle migration preserves '+t);
  for(const f of priorFunctions)eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Lifecycle leaves prior function unchanged '+f.identity);
  const{runLifecycleTests}=await import('./lifecycle-notifications.test.mjs');await runLifecycleTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');const creditBefore={};const creditTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const creditPriorFunctions=(await db.query("select n.nspname schema,p.proname name,p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1,2,3")).rows;
  for(const t of creditTables)creditBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  for(const filename of creditMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of creditTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,creditBefore[t],'Credit migration preserves prior '+t+' records');
  const changed=[];for(const f of creditPriorFunctions){const now=(await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest;if(['hris_private.leave_balance','public.submit_leave'].includes(f.schema+'.'+f.name)){if(now!==f.digest)changed.push(f.schema+'.'+f.name);}else eq(now,f.digest,'Credits preserve unrelated function '+f.identity);}
  eq(changed.sort(),['hris_private.leave_balance','public.submit_leave'],'Credit migration changes exactly the balance integration and native compatibility wrapper');
  const{runLeaveCreditTests}=await import('./leave-credits.test.mjs');await runLeaveCreditTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');const delegationBefore={};const delegationTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const delegationPriorFunctions=(await db.query("select n.nspname schema,p.proname name,p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1,2,3")).rows;
  for(const t of delegationTables)delegationBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  for(const filename of delegationMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of delegationTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,delegationBefore[t],'Delegation migration preserves prior '+t+' records');
  const allowedDelegationChanges=['hris_private.can_act_leave','hris_private.review_leave','hris_private.attendance_request','hris_private.overtime_request','hris_private.can_apply_attendance_request','hris_private.record_attendance_calculation','public.review_leave','public.attendance_request','public.overtime_request'];
  for(const f of delegationPriorFunctions)if(!allowedDelegationChanges.includes(f.schema+'.'+f.name))eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Delegation preserves unrelated function '+f.identity);
  const{runDelegationTests}=await import('./approval-delegation.test.mjs');await runDelegationTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const datedTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const datedBefore={};for(const t of datedTables)datedBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const datedPriorFunctions=(await db.query("select n.nspname schema,p.proname name,p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1,2,3")).rows;
  for(const filename of datedMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of datedTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,datedBefore[t],'Dated leave preserves prior '+t+' records');
  const datedChanged=['leave_balance','minimum_annual_allowance','save_leave_policy','submit_leave','review_leave_without_delegation_receipt','require_credit_preview','portal_leave_context','portal_submit_leave','payroll_basis','shift_working','exit_readiness','post_leave_credit'];
  for(const f of datedPriorFunctions)if(f.schema!=='hris_private'||!datedChanged.includes(f.name))eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Dated leave preserves unrelated function '+f.identity);
  await db.exec('reset role');
  const importTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const importBefore={};for(const t of importTables)importBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const importFunctions=(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  for(const filename of attendanceImportMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of importTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,importBefore[t],'Attendance imports preserve prior '+t+' records');
  for(const f of importFunctions)eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Attendance imports preserve every existing function '+f.identity);
  const leaveImportTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const leaveImportBefore={};for(const t of leaveImportTables)leaveImportBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const leaveImportFunctions=(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  for(const filename of leaveImportMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of leaveImportTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,leaveImportBefore[t],'Leave imports preserve prior '+t+' records');
  for(const f of leaveImportFunctions)eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Leave imports preserve every existing function '+f.identity);
  await db.exec('reset role');
  const exchangeTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const exchangeBefore={};for(const t of exchangeTables)exchangeBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const exchangeFunctions=(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  for(const filename of exchangeMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of exchangeTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,exchangeBefore[t],'Exchange migration preserves '+t);
  for(const f of exchangeFunctions)eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Exchange migration preserves every existing function '+f.identity);
  const{runPayrollExchangeTests}=await import('./payroll-exchange.test.mjs');await runPayrollExchangeTests({db,eq,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const annualTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const annualBefore={};for(const t of annualTables)annualBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const annualFunctions=(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  for(const filename of annualLedgerMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of annualTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,annualBefore[t],'Annual ledger migration preserves '+t);
  for(const f of annualFunctions)eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Annual ledger migration preserves existing function '+f.identity);
  const{runGovernmentAnnualTests}=await import('./government-annual.test.mjs');await runGovernmentAnnualTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const identityTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const identityBefore={};for(const t of identityTables)identityBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const identityFunctions=(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  for(const filename of birIdentityMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of identityTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,identityBefore[t],'BIR identity migration preserves '+t);
  for(const f of identityFunctions)eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'BIR identity migration preserves existing function '+f.identity);
  const{runBirIdentityTests}=await import('./government-bir-identities.test.mjs');await runBirIdentityTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const payTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const payBefore={};for(const t of payTables)payBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const payFunctions=(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  for(const filename of annualPayMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of payTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,payBefore[t],'Pay detail migration preserves '+t);
  for(const f of payFunctions)eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Pay detail migration preserves existing function '+f.identity);
  const{runAnnualPayTests}=await import('./government-annual-pay.test.mjs');await runAnnualPayTests({db,eq,deny,session,company:companyA,owner:uidA,unassigned:uidB});
  await db.exec('reset role');
  const staffTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const staffBefore={};for(const t of staffTables)staffBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const staffFunctions=(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  for(const filename of staffRoleMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of staffTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,staffBefore[t],'Role foundation preserves '+t);
  for(const f of staffFunctions)eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Role foundation preserves existing function '+f.identity);
  eq((await db.query('select count(*)::int n from hris_private.staff_role_assignments')).rows[0].n,(await db.query('select count(*)::int n from public.company_members')).rows[0].n,'Every current membership receives an audited compatibility role set');
  eq((await db.query('select count(*)::int n from hris_private.staff_role_events')).rows[0].n,(await db.query('select count(*)::int n from public.company_members')).rows[0].n,'Every compatibility assignment has immutable history');
  await session(uidA);
  const staffContext=(await db.query('select public.staff_role_context()c')).rows[0].c;
  eq(staffContext.user_id,uidA,'Protected role context retains current actor');
  eq(staffContext.runtime_ready,false,'Foundation does not activate incomplete specialist paths');
  await db.exec('reset role');
  const transactionTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const transactionBefore={};for(const t of transactionTables)transactionBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const transactionFunctions=(await db.query("select p.oid::regprocedure::text identity,p.prosrc source,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  const changedRoleFunctions=new Set(['hris_private.my_hris_access()','hris_private.save_company_account(uuid,text,text,boolean,uuid,uuid)','hris_private.staff_role_context()','hris_private.staff_role_admin_context(boolean)']);
  for(const filename of staffTransactionMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of transactionTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,transactionBefore[t],'Role transaction migration preserves '+t);
  for(const f of transactionFunctions){if(!changedRoleFunctions.has(f.identity))eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Role transaction migration preserves unrelated function '+f.identity);}
  for(const [original,copy]of[['hris_private.my_hris_access()','hris_private.my_hris_access_legacy()'],['hris_private.save_company_account(uuid,text,text,boolean,uuid,uuid)','hris_private.save_company_account_legacy(uuid,text,text,boolean,uuid,uuid)']])eq((await db.query('select prosrc source from pg_proc where oid=$1::regprocedure',[copy])).rows[0].source,transactionFunctions.find(f=>f.identity===original).source,'Original protected function body retained '+original);
  await session(uidA);
  eq((await db.query('select public.my_hris_access()c')).rows[0].c.staff_contract_version,1,'Native role contract is explicitly advertised');
  eq((await db.query('select public.staff_role_context()c')).rows[0].c.runtime_ready,false,'Transaction contract does not enable incomplete module paths');
  const transactionContext=(await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;
  eq(transactionContext.account_count,transactionContext.accounts.length,'Admin context does not silently omit member identities');
  await deny('select public.set_staff_roles($1::jsonb)',[JSON.stringify({id:'00000000-0000-0000-0000-000000999999',user_id:uidB,expected_revision:1,expected_base_role:'employee',expected_member_updated_at:'2026-10-27T00:00:00Z',active:true,roles:['recruiter'],reason:'Fictional rollout gate test'})],['42501']);
  await db.exec('reset role');
  const provisionTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const provisionBefore={};for(const t of provisionTables)provisionBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const provisionFunctions=(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  for(const filename of staffProvisionMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of provisionTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,provisionBefore[t],'Provisioning migration preserves '+t);
  for(const f of provisionFunctions)eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Provisioning migration preserves prior function '+f.identity);
  await session(uidA);eq((await db.query('select public.pending_staff_provisions()p')).rows[0].p.entry_count,0,'Additive provisioning has no synthetic hosted-style intents');
  eq((await db.query('select public.staff_role_context()c')).rows[0].c.runtime_ready,false,'Provisioning migration keeps specialist rollout disabled');
  await db.exec('reset role');
  const capTables=(await db.query("select tablename from pg_tables where schemaname='public'order by tablename")).rows.map(r=>r.tablename);
  const capBefore={};for(const t of capTables)capBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const capFunctions=(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  for(const filename of staffCapabilityMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of capTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,capBefore[t],'Capability kernel preserves '+t);
  for(const f of capFunctions)eq((await db.query('select md5(pg_get_functiondef($1::regprocedure))digest',[f.identity])).rows[0].digest,f.digest,'Capability kernel preserves existing function '+f.identity);
  await session(uidA);eq((await db.query("select hris_private.has_staff_capability($1,'account_administration')c",[companyA.id])).rows[0].c,true,'Existing owner retains capability');
  eq((await db.query('select public.staff_role_context()c')).rows[0].c.runtime_ready,false,'Capability kernel does not activate incomplete permission paths');
  await db.exec('reset role');
  const permissionBefore={};for(const t of capTables)permissionBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
  const permissionAcl=(await db.query("select p.oid::regprocedure::text identity,p.prosecdef,p.provolatile,p.proconfig,p.proacl::text from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows;
  for(const filename of staffPermissionMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const filename of staffWorkflowMigrations){await db.exec(await readFile(new URL(filename,folder),'utf8'));await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[filename.split('_')[0]]);}
  for(const t of capTables)eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,permissionBefore[t],'Permission rollout preserves '+t);
  for(const f of permissionAcl)eq((await db.query('select p.oid::regprocedure::text identity,p.prosecdef,p.provolatile,p.proconfig,p.proacl::text from pg_proc p where p.oid=$1::regprocedure',[f.identity])).rows[0],f,'Permission rollout preserves signature, owner execution and grants '+f.identity);
  await session(uidA);eq((await db.query('select public.staff_role_context()c')).rows[0].c.runtime_ready,false,'Module permission patches do not activate incomplete roles');
  console.log(`Passed ${checks} local PostgreSQL migration, authorization, and workflow assertions.`);
} finally {
  await db.close();
}
