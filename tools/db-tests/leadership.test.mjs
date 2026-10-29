// Fresh chronological migration replay; managed Auth/Storage are local fixtures.
import{readFile,readdir,mkdir,writeFile}from'node:fs/promises';
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
 let preservation=null;
 const originals=async()=>{const tables=(await db.query("select schemaname,tablename from pg_tables where schemaname in('public','hris_private')order by schemaname,tablename")).rows;const facts={};for(const t of tables){assert(/^[a-z0-9_]+$/.test(t.tablename));facts[t.schemaname+'.'+t.tablename]=(await db.query('select to_jsonb(x)r from '+t.schemaname+'.'+t.tablename+' x order by to_jsonb(x)::text')).rows;}return{facts,auth:(await db.query('select to_jsonb(x)r from auth.users x order by id')).rows,functions:(await db.query("select p.oid::regprocedure::text identity,md5(pg_get_functiondef(p.oid))digest,p.proacl::text,p.proowner from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname in('public','hris_private')and p.prokind='f'order by 1")).rows};};
 for(const file of(await readdir(folder)).filter(f=>f.endsWith('.sql')).sort()){
  if(file.endsWith('_hr_leadership_reporting.sql'))preservation=await originals();
  try{await db.exec(await readFile(new URL(file,folder),'utf8'));}
  catch(e){throw Error('Fresh replay failed at '+file+': '+e.message,{cause:e});}
  if(file.endsWith('_hr_leadership_reporting.sql')){const after=await originals();for(const [name,rows]of Object.entries(preservation.facts))assert.deepEqual(after.facts[name],rows,name);assert.deepEqual(after.auth,preservation.auth);for(const f of preservation.functions)assert.deepEqual(after.functions.find(v=>v.identity===f.identity),f);}
  await db.query('insert into supabase_migrations.schema_migrations(version)values($1)',[file.split('_')[0]]);count++;
 }

 


let checks=0,n=1;const id=()=>'69000000-0000-4000-8000-'+String(n++).padStart(12,'0');
const eq=(a,b,label)=>{assert.deepEqual(a,b,label);checks++;};
const deny=async(sql,args=[],codes=['42501','23514','23505','22P02','54000','22007','22008'])=>{try{await db.query(sql,args)}catch(e){assert(codes.includes(e.code),e.code+': '+e.message);checks++;return}assert.fail('Expected denial '+sql)};
const owner=id(),hr=id(),manager=id(),person=id(),other=id(),specialist=id();
const email=u=>'leadership-'+u+'@example.invalid';
for(const u of[owner,hr,manager,person,other,specialist])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3)',[u,email(u),JSON.stringify({role:'administrator',staff_roles:['administrator']})]);
const company=(await db.query("insert into public.companies(name,owner_id)values('Fictional leadership oversight',$1)returning id",[owner])).rows[0].id;
const session=async(user,role='authenticated')=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user??'']);await db.exec('set role '+role)};
const rpc=async(name,arg)=>(await db.query('select public.'+name+'('+(arg===undefined?'':'$1::jsonb')+')r',arg===undefined?[]:[JSON.stringify(arg)])).rows[0].r;
await session(owner);for(const[u,r]of[[hr,'hr_generalist'],[manager,'manager'],[person,'employee'],[specialist,'employee']])await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[u,email(u),r]);
await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company]);
const employees=Object.fromEntries((await db.query('select user_id,employee_id from public.company_members where company_id=$1',[company])).rows.map(r=>[r.user_id,r.employee_id]));
const role=async(user,roles,active=true)=>{await session(owner);const real=(await db.query('select public.staff_role_admin_context(true)c')).rows[0].c;const a=real.accounts.find(x=>x.user_id===user);return rpc('set_staff_roles',{id:id(),user_id:user,expected_revision:a.revision,expected_base_role:a.base_role,expected_member_updated_at:a.member_updated_at,roles,active,reason:'Fictional services access review'});};


const today=(await db.query("select to_char(statement_timestamp()at time zone'Asia/Manila','YYYY-MM-DD')d")).rows[0].d;
const day=async(n)=>(await db.query('select to_char($1::date+$2::integer,\'YYYY-MM-DD\')d',[today,n])).rows[0].d;
const start=await day(-29),end=today;
const read=async(a=start,b=end)=>(await db.query('select public.hr_leadership_report($1::date,$2::date)r',[a,b])).rows[0].r;
await session(owner);let c=await read();eq(c.user_id,owner,'Bound protected actor');eq(c.company_id,company,'Bound company');eq(c.workforce.active,5,'Initial recorded employee statuses');eq(c.months[0].starts_on,start,'Partial first month clipped');eq(c.months.at(-1).ends_on,end,'Partial final month clipped');eq(Object.keys(c.workload).length,15,'Complete typed workload metrics');
const vectors=[structuredClone(c)];
const employeeA=id(),employeeB=id(),employeeC=id();
await db.exec('reset role');
await db.query("insert into public.work_locations(company_id,name,active)values($1,'Inactive recorded location',true)",[company]);
const location=(await db.query("select id from public.work_locations where company_id=$1 and name='Inactive recorded location'",[company])).rows[0].id;
await db.query("insert into public.employment_types(company_id,name,active)values($1,'Inactive recorded type',true)",[company]);
const type=(await db.query("select id from public.employment_types where company_id=$1 and name='Inactive recorded type'",[company])).rows[0].id;
await db.query("insert into public.employees(id,company_id,employee_code,full_name,department,job_title,joined_on,status,annual_leave_days,work_location_id,employment_type_id)values($1,$2,'LEAD-A','PRIVATE_EMPLOYEE_NAME','Not set','Fictional role',$3,'active',15,$4,$5)",[employeeA,company,start,location,type]);
await db.query("insert into public.employees(id,company_id,employee_code,full_name,department,job_title,joined_on,status,annual_leave_days)values($1,$2,'LEAD-B','PRIVATE_FUTURE_NAME','','Fictional role',$3,'active',15),($4,$2,'LEAD-C','PRIVATE_ARCHIVED_NAME','Ops','Fictional role',null,'archived',15)",[employeeB,company,await day(3),employeeC]);
await db.query('update public.work_locations set active=false where id=$1',[location]);await db.query('update public.employment_types set active=false where id=$1',[type]);
await db.exec('reset role');
// Saved facts with private sentinels; the new projection must read aggregates only.
await session(owner);
await db.query('select public.correct_attendance($1::jsonb)',[JSON.stringify({employee_id:employeeA,work_date:start,checked_in_at:start+'T00:00:00Z',checked_out_at:start+'T09:00:00Z',reason:'Fictional leadership completed source fixture'})]);
await db.query('select public.correct_attendance($1::jsonb)',[JSON.stringify({employee_id:employees[person],work_date:await day(-1),checked_in_at:(await day(-1))+'T00:00:00Z',checked_out_at:null,reason:'Fictional leadership open source fixture'})]);
await db.exec('reset role');
await db.query("insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on,reason,status,type_code,type_name,charged_units,charged_dates,policy_revision,approval_steps,requester_name)values($1,$2,'sick',$3,$3,'PRIVATE_LEAVE_REASON','approved','sick','Sick',1,jsonb_build_array(jsonb_build_object('date',($3::date)::text,'units',1)),0,'[]','PRIVATE_REQUESTER')",[company,employeeA,await day(-5)]);
await db.query("insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on,reason,status,type_code,type_name,charged_units,charged_dates,policy_revision,approval_steps,requester_name)values($1,$2,'sick',$3,$3,'PRIVATE_PENDING_REASON','pending','sick','Sick',2,jsonb_build_array(jsonb_build_object('date',($3::date)::text,'units',2)),0,'[]','PRIVATE_REQUESTER')",[company,employees[person],await day(-3)]);
const exitId=id();await db.query("insert into public.exit_cases(id,company_id,employee_id,employee_name,employee_code,last_working_on,category,private_notes,status,birth_payload,created_by)values($1,$2,$3,'PRIVATE_EXIT_NAME','PRIVATE_CODE',$4,'other','PRIVATE_EXIT_NOTES','finalized','{}',$5)",[exitId,company,employeeA,await day(-2),owner]);
for(const [stage,archived]of[['applied',false],['offer',false],['rejected',false],['withdrawn',false],['interview',true]])await db.query("insert into public.applicants(id,company_id,full_name,position,applied_on,stage,archived,notes,created_by)values($1,$2,'PRIVATE_APPLICANT','Fictional position',$3,$4,$5,'PRIVATE_RESUME_NOTE',$6)",[id(),company,start,stage,archived,owner]);
for(const [status,due]of[['active',await day(-1)],['active',today],['completed',await day(-2)]])await db.query("insert into public.performance_goals(id,company_id,employee_id,employee_name,employee_code,title,starts_on,due_on,status,progress_note,created_by)values($1,$2,$3,'PRIVATE_GOAL_NAME','PRIVATE_CODE','Fictional goal',$4,$5,$6,'PRIVATE_GOAL_NOTE',$7)",[id(),company,employeeA,start,due,status,owner]);
for(const [status,due]of[['planned',await day(-1)],['in_progress',today],['planned',null],['cancelled',await day(-2)]])await db.query("insert into public.employee_training(id,company_id,employee_id,employee_name,employee_code,title,starts_on,due_on,status,progress_note,created_by)values($1,$2,$3,'PRIVATE_TRAINING_NAME','PRIVATE_CODE','Fictional training',$4,$5,$6,'PRIVATE_PROGRESS',$7)",[id(),company,employeeA,start,due,status,owner]);
await db.query("insert into public.employee_training(id,company_id,employee_id,employee_name,employee_code,title,starts_on,status,completed_on,outcome,created_by)values($1,$2,$3,'PRIVATE_COMPLETION_NAME','PRIVATE_CODE','Fictional completion',$4,'completed',$5,'PRIVATE_OUTCOME',$6)",[id(),company,employeeC,start,await day(-3),owner]);
for(const [issued,expires,status,archived]of[[start,await day(-1),'valid',false],[start,today,'valid',false],[start,await day(30),'valid',false],[start,await day(31),'valid',false],[start,await day(-1),'revoked',false],[start,await day(-1),'valid',true],[await day(1),await day(3),'valid',false],[start,null,'valid',false]])await db.query("insert into public.employee_certifications(id,company_id,employee_id,employee_name,employee_code,title,reference_number,issued_on,expires_on,status,archived,created_by)values($1,$2,$3,'PRIVATE_CERT_NAME','PRIVATE_CODE','Fictional certificate','PRIVATE_CERT_NUMBER',$4,$5,$6,$7,$8)",[id(),company,employeeA,issued,expires,status,archived,owner]);

const selectedApplicants=(await db.query('select id from public.applicants where company_id=$1 order by id',[company])).rows;
const offers=[];
for(const[i,status]of['sent','accepted','accepted'].entries()){const oid=id();offers.push(oid);await db.query("insert into public.hiring_offers(id,company_id,applicant_id,applicant_name,job_title,starts_on,amount_minor,currency,pay_period,terms,status,birth_payload,approved_by,approved_at,created_by)values($1,$2,$3,'PRIVATE_OFFER_NAME','Fictional role',$4,123456,'PHP','monthly','PRIVATE_OFFER_TERMS',$5,'{}',$6,now(),$6)",[oid,company,selectedApplicants[i].id,start,status,owner]);}
const conversion=id();await db.query("insert into public.hire_conversions(id,company_id,applicant_id,offer_id,employee_id,birth_payload,created_by)values($1,$2,$3,$4,$5,'{}',$6)",[conversion,company,selectedApplicants[2].id,offers[2],employeeA,owner]);
await db.query("insert into public.onboarding_cases(id,company_id,employee_id,conversion_id,employee_name,employee_code,starts_on,status)values($1,$2,$3,$4,'PRIVATE_ONBOARD_NAME','PRIVATE_CODE',$5,'open')",[id(),company,employeeA,conversion,start]);
const cycle=id();await db.query("insert into public.performance_cycles(id,company_id,name,starts_on,ends_on,due_on,created_by)values($1,$2,'Fictional leadership cycle',$3,$4,$4,$5)",[cycle,company,start,await day(-2),owner]);
for(const[personId,status,due]of[[employeeA,'draft',await day(-1)],[employeeC,'submitted',today]])await db.query("insert into public.performance_reviews(id,company_id,cycle_id,employee_id,employee_name,employee_code,cycle_name,starts_on,ends_on,due_on,reviewer_employee_id,reviewer_name,status,assessment,created_by)values($1,$2,$3,$4,'PRIVATE_REVIEW_NAME','PRIVATE_CODE','Fictional cycle',$5,$6,$7,$8,'PRIVATE_REVIEWER',$9,'PRIVATE_ASSESSMENT',$10)",[id(),company,cycle,personId,start,await day(-2),due,employees[manager],status,owner]);
await db.query("insert into public.career_plans(id,company_id,employee_id,employee_name,employee_code,title,shared_notes,starts_on,created_by)values($1,$2,$3,'PRIVATE_CAREER_NAME','PRIVATE_CODE','Fictional career plan','PRIVATE_CAREER_NOTES',$4,$5)",[id(),company,employeeA,start,owner]);
await db.exec('reset role');const factsBefore=await originals();await session(owner);c=await read();await db.exec('reset role');const factsAfter=await originals();eq(factsAfter,factsBefore,'Pure report changes no original facts/Auth/functions');
eq(c.workforce.active,7,'Active includes future joining date');eq(c.workforce.archived,1,'Archive retained separately');eq(c.workforce.future_join,1,'Future join explicit');eq(c.workforce.joined_in_period,1,'Only known current joining date in period');eq(c.workforce.finalized_exits_in_period,1,'Only finalized exit case, not archived record');eq(c.workforce.unknown_join,6,'Unknown joining dates explicit');
eq(c.groups.find(g=>g.by==='location'&&g.name==='Inactive recorded location').active,1,'Inactive option label retained');eq(c.groups.filter(g=>g.by==='department'&&g.unassigned).length,1,'Explicit unassigned group');eq(c.groups.find(g=>g.by==='department'&&g.name==='Not set').unassigned,false,'Literal Not set label distinct');
const sums=k=>c.months.reduce((n,m)=>n+m[k],0);eq(sums('joined'),1,'Monthly joining counts reconcile');eq(sums('attendance_completed'),1,'Recorded completed employee-day');eq(sums('attendance_open'),1,'Recorded open employee-day');eq(sums('approved_leave_units'),1,'Half-day saved charge retained');eq(sums('pending_leave_units'),2,'Pending saved charge distinct');eq(c.workload.offers_sent,1,'Sent offers counted without amounts');eq(c.workload.accepted_unconverted,1,'Converted accepted offer excluded');eq(c.workload.onboarding_open,1,'Open onboarding counted');eq(c.workload.reviews_unpublished,2,'Draft and submitted reviews counted without scores');eq(c.workload.reviews_overdue,1,'Review due today not overdue');eq(c.workload.career_active,1,'Active career state counted');eq(c.workload.applicants_pipeline,2,'Six-stage applicant contract respected');eq(c.workload.goals_active,2,'Current goal states');eq(c.workload.goals_overdue,1,'Due today not overdue');eq(c.workload.training_open,3,'Open training includes unknown due date');eq(c.workload.training_overdue,1,'Unknown due date not inferred overdue');eq(c.workload.training_completed_in_period,1,'HR-recorded completion, retained archived employment');eq(c.workload.certificates_expired,1,'Expired valid non-archived already-issued only');eq(c.workload.certificates_expiring30,2,'Today and exact 30-day expiry boundaries included');
for(const sentinel of['PRIVATE_','LEAD-A','LEAD-B','LEAD-C',employeeA,employeeB,employeeC,exitId])eq(JSON.stringify(c).includes(sentinel),false,'No private source field '+sentinel);
vectors.push(structuredClone(c));
for(const[user,roles]of[[hr,['hr_generalist']],[manager,['manager']],[person,['employee']],[specialist,['payroll_officer']],[specialist,['recruiter']],[specialist,['learning_performance_officer']],[specialist,['hr_generalist','recruiter','payroll_officer','learning_performance_officer','manager']]]){await role(user,roles);await session(user);await deny('select public.hr_leadership_report($1,$2)',[start,end]);await deny('select hris_private.hr_leadership_report($1,$2)',[start,end]);}
await session(null,'anon');await deny('select public.hr_leadership_report($1,$2)',[start,end]);
await session(owner);for(const[a,b]of[[null,end],[start,null],[end,start],[start,await day(1)],[await day(-366),end]])await deny('select public.hr_leadership_report($1,$2)',[a,b]);
let leap=await read('2024-01-01','2024-12-31');eq(leap.months.length,12,'366-day leap-year accepted');eq(leap.months[1].ends_on,'2024-02-29','Leap February boundary');vectors.push(leap);
// A different company is never included and disabled/archived Admin access is denied.
await db.exec('reset role');await deny("insert into public.companies(name,owner_id)values('Foreign company',$1)",[other],['23505']);
await session(owner);eq((await read()).workforce,c.workforce,'Different company cannot affect totals');
await role(hr,['administrator'],false);await db.exec('reset role');await db.query("update public.employees set status='archived'where id=$1",[employees[hr]]);await session(hr);await deny('select public.hr_leadership_report($1,$2)',[start,end]);
await session(owner);await db.exec('reset role');await db.query("update public.employees set status='active'where id=$1",[employees[hr]]);await role(hr,['hr_generalist']);
// Read-only transaction proves the projection cannot conceal a write.
await session(owner);await db.exec('begin read only');eq((await read()).workforce,c.workforce,'Read-only report path');await db.exec('rollback');
// Group overflow is explicit and leaves originals unchanged.
await db.exec('reset role');await db.exec('begin');await db.query("insert into public.employees(company_id,employee_code,full_name,department,job_title,status,annual_leave_days)select $1,'BUDGET-'||n,'Fictional budget employee','Group '||n,'Fictional role','active',15 from generate_series(1,601)n",[company]);await session(owner);await deny('select public.hr_leadership_report($1,$2)',[start,end],['54000']);await db.exec('rollback');
await session(owner);eq((await read()).workforce,c.workforce,'Rejected overflow preserves complete source totals');

// Reject inconsistent retained charge facts without recalculating policies or silently omitting a row.
for(const bad of[
 [{date:await day(-5),units:3}],
 [{date:await day(-5),units:1},{date:await day(-5),units:1}],
 [{date:'1900-01-01',units:1}],
 Array.from({length:367},()=>({date:today,units:1}))
]){
 await db.exec('reset role');await db.exec('begin');await db.exec('alter table public.leave_requests disable trigger all');
 await db.query("update public.leave_requests set charged_dates=$1::jsonb where company_id=$2 and employee_id=$3",[JSON.stringify(bad),company,employeeA]);
 await db.exec('alter table public.leave_requests enable trigger all');await session(owner);await deny('select public.hr_leadership_report($1,$2)',[start,end],['23514']);await db.exec('rollback');
}
await db.exec('reset role');await db.exec('begin');
await db.query("insert into public.applicants(id,company_id,full_name,position,applied_on,created_by)select gen_random_uuid(),$1,'Fictional bound applicant','Fictional position',$2,$3 from generate_series(1,50001)",[company,start,owner]);
await session(owner);await deny('select public.hr_leadership_report($1,$2)',[start,end],['54000']);await db.exec('rollback');
// Role removal takes effect on the next call; metadata never grants authority.
await role(hr,['administrator']);await session(hr);eq((await read()).user_id,hr,'Protected Admin assignment grants aggregate access');
await role(hr,['hr_generalist']);await session(hr);await deny('select public.hr_leadership_report($1,$2)',[start,end]);
await session(owner);const functions=(await db.query("select proname,provolatile,prosecdef,proconfig from pg_proc where proname='hr_leadership_report'order by prosecdef")).rows;eq(functions.length,2,'Only protected implementation and public wrapper added');eq(functions.every(f=>f.provolatile==='s'&&f.proconfig.some(v=>v.startsWith('search_path='))),true,'Stable statement snapshot and pinned search_path');
const vectorFolder=new URL('../../work/leadership-db-vectors/',import.meta.url);await mkdir(vectorFolder,{recursive:true});for(const[i,v]of vectors.entries())await writeFile(new URL('report-'+i+'.json',vectorFolder),JSON.stringify(v));
console.log(JSON.stringify({status:'PASS',assertions:checks,migrations:count,original_record_sets_preserved:Object.keys(preservation.facts).length,original_functions_preserved:preservation.functions.length,vectors:vectors.length}));
}catch(e){console.error(JSON.stringify({status:'FAIL',message:e.message,code:e.code,where:e.where,query:e.query,stack:e.stack?.split('\n').slice(0,3)}));process.exitCode=1;}finally{await db.close()}
