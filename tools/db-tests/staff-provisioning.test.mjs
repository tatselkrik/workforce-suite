// Fictional local Auth/Storage, actual migration chain; no hosted writes or emails.
import assert from'node:assert/strict';
import{readFile,readdir}from'node:fs/promises';
import{PGlite}from'@electric-sql/pglite';
import{btree_gist}from'@electric-sql/pglite/contrib/btree_gist';
const db=new PGlite({extensions:{btree_gist}});let checks=0,serial=1000,company;
const id=n=>'10000000-0000-4000-8000-'+String(n).padStart(12,'0');
const owner=id(1),otherAdmin=id(2),hr=id(3),manager=id(4),employee=id(5);
const eq=(a,b,label)=>{assert.deepEqual(a,b,label);checks++;};
const deny=async(sql,args=[],codes=['42501','23514','23505','23502','22P02','22023','54000'])=>{try{await db.query(sql,args);}catch(e){assert(codes.includes(e.code),e.code+': '+e.message);checks++;return;}assert.fail('Expected denial: '+sql);};
const session=async(user,role='authenticated')=>{await db.exec('reset role');await db.query("select set_config('request.jwt.claim.sub',$1,false)",[user??'']);await db.exec('set role '+role);};
const rpc=async(name,args)=> (await db.query('select public.'+name+'($1::jsonb)r',[JSON.stringify(args)])).rows[0].r;
const begin=p=>rpc('begin_staff_provision',p);const commit=p=>rpc('commit_staff_provision',p);
const pending=async()=>(await db.query('select public.pending_staff_provisions()r')).rows[0].r;
const source=async person=>(await db.query('select public.staff_employee_source($1)r',[person])).rows[0].r;
const make=()=>{const n=serial++;return{id:id(n),email:'fictional-'+n+'@example.invalid',roles:['recruiter','learning_performance_officer'],active:true,reason:'Reviewed staff responsibilities',employee:{id:null,company_id:company,employee_code:'QA-'+n,full_name:'Fictional Person '+n,department:'Prototype',job_title:'Specialist',joined_on:null,annual_leave_days:15,status:'active',reports_to:null,employment_type_id:null,work_location_id:null},contacts:{company_id:company,employee_id:'00000000-0000-0000-0000-000000000000',phone:'000 123 4567',personal_email:'',address:'Fictional address',emergency_name:'Fictional Contact',emergency_relationship:'Sibling',emergency_phone:'000 123 4568',emergency_alternate_phone:'',updated_at:null},expected_source:null,supersedes:null};};
const auth=async intent=>{await db.exec('reset role');await db.query('insert into auth.users(id,email,raw_user_meta_data)values($1,$2,$3)',[intent.user_id,intent.email,JSON.stringify({role:'administrator',roles:['administrator']})]);await session(owner);};
const facts=async()=>{await db.exec('reset role');const result={};for(const t of['employees','employee_contacts','employment_history','company_members','account_access_events']){
 result[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
}await session(owner);return result;};
try{
 await db.exec(`create role anon nologin;create role authenticated nologin;create role service_role nologin bypassrls;
 create schema supabase_migrations;create table supabase_migrations.schema_migrations(version text primary key);
 create schema auth;create table auth.users(id uuid primary key,email text unique,email_confirmed_at timestamptz,raw_user_meta_data jsonb default'{}'::jsonb);
 create function auth.uid()returns uuid language sql stable as $$select nullif(current_setting('request.jwt.claim.sub',true),'')::uuid$$;
 grant usage on schema auth to authenticated;grant execute on function auth.uid()to authenticated;
 create schema storage;create table storage.buckets(id text primary key,name text,public boolean default false,file_size_limit bigint,allowed_mime_types text[]);
 create table storage.objects(id uuid primary key default gen_random_uuid(),bucket_id text references storage.buckets(id),name text,owner_id text,metadata jsonb,unique(bucket_id,name));
 alter table storage.objects enable row level security;grant usage on schema storage to authenticated,anon;grant select,insert,update,delete on storage.objects to authenticated,anon;`);
 const folder=new URL('../../supabase/migrations/',import.meta.url);
 for(const f of(await readdir(folder)).filter(f=>f.endsWith('.sql')).sort()){
  try{await db.exec(await readFile(new URL(f,folder),'utf8'));}catch(e){throw Error('Migration '+f+': '+e.message,{cause:e});}
  await db.query('insert into supabase_migrations.schema_migrations values($1)',[f.split('_')[0]]);
 }
 for(const u of[owner,otherAdmin,hr,manager,employee])await db.query('insert into auth.users(id,email,email_confirmed_at,raw_user_meta_data)values($1,$2,now(),$3)',[u,'fixture-'+u.slice(-3)+'@example.invalid',JSON.stringify({full_name:'Fictional '+u.slice(-3)})]);
 company=(await db.query("insert into public.companies(name,owner_id)values('Fictional provisioning company',$1)returning id",[owner])).rows[0].id;
 await session(owner);for(const[u,r]of[[otherAdmin,'administrator'],[hr,'hr_generalist'],[manager,'manager'],[employee,'employee']])await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[u,'fixture-'+u.slice(-3)+'@example.invalid',r]);
 const paused=make();await deny('select public.begin_staff_provision($1::jsonb)',[JSON.stringify(paused)],['42501']);
 await deny('update hris_private.staff_role_runtime_policy set runtime_ready=true');
 await deny('select hris_private.staff_provision_admin(false)');await deny('select *from hris_private.staff_provision_intents');
 await db.exec('reset role');await db.query('insert into hris_private.staff_role_runtime_policy values($1,true)on conflict(company_id)do update set runtime_ready=true',[company]);
 for(const u of[hr,manager,employee]){await session(u);await deny('select public.begin_staff_provision($1::jsonb)',[JSON.stringify(paused)],['42501']);await deny('select public.pending_staff_provisions()');}
 await session(null,'anon');await deny('select public.begin_staff_provision($1::jsonb)',[JSON.stringify(paused)]);
 await session(owner);const baseline=await facts();const request=make(),intent=await begin(request);
 eq(intent.state,'reserved','Begin reserves without granting access');eq(intent.actor_id,owner,'Actor is protected current membership');eq(intent.email,request.email,'Reserved email is explicit');eq(await facts(),baseline,'Successful rehearsal preserves every business record');
 eq(await begin(request),intent,'Exact begin retry reuses one reserved Auth identity');eq((await pending()).entry_count,1,'Pending intent is recoverable without credentials');
 await deny('select public.begin_staff_provision($1::jsonb)',[JSON.stringify({...request,reason:'Different retry'})],['42501']);
 await session(otherAdmin);await deny('select public.begin_staff_provision($1::jsonb)',[JSON.stringify(request)],['42501']);eq((await pending()).entry_count,0,'Another Admin cannot enumerate another actor pending payloads');
 await session(owner);await deny('select public.commit_staff_provision($1::jsonb)',[JSON.stringify(request)],['23514']);
 await auth(intent);await session(intent.user_id);await deny('select public.my_hris_access()');await deny('select public.pending_staff_provisions()');
 await session(owner);const receipt=await commit(request);eq(receipt.user_id,intent.user_id,'Commit uses its pre-reserved Auth identity');eq(receipt.roles,['recruiter','learning_performance_officer'],'Combined specialist roles');eq(receipt.base_role,'employee','No unrelated legacy HR powers');eq(receipt.revision,1,'First explicit assignment is revision one');eq(receipt.active,true,'Initial access is explicit');
 eq(await commit(request),receipt,'Exact commit retry returns immutable receipt');eq((await begin(request)).receipt,receipt,'Post-commit begin retry does not recreate Auth');eq((await pending()).entry_count,0,'Committed intent leaves pending view');
 await db.exec('reset role');const rows=(await db.query('select e.employee_code,c.emergency_name,u.email_confirmed_at from public.employees e join public.employee_contacts c on c.employee_id=e.id join auth.users u on u.id=$1 where e.id=$2',[intent.user_id,receipt.employee_id])).rows[0];
 eq(rows,{employee_code:request.employee.employee_code,emergency_name:request.contacts.emergency_name,email_confirmed_at:null},'One employee with private contacts; Auth remains unconfirmed');
 eq((await db.query("select count(*)::int n from hris_private.staff_role_events where user_id=$1 and kind='account_provisioned'",[intent.user_id])).rows[0].n,1,'One immutable provisioning role event');
 await session(intent.user_id);eq((await db.query('select public.staff_role_context()c')).rows[0].c.roles,receipt.roles,'Forged Auth metadata grants only protected assigned roles');await deny('select public.staff_employee_source($1)',[receipt.employee_id]);await deny('select *from hris_private.staff_provision_intents');
 await session(owner);let old=make();old.employee.full_name='Fictional existing employee';const existing=(await db.query('select public.save_employee_record($1,$2)r',[old.employee,old.contacts])).rows[0].r;let savedSource=await source(existing);
 const linked=make();linked.employee={...savedSource.employee,job_title:'Explicitly reviewed new title'};linked.contacts={...savedSource.contacts};linked.expected_source=savedSource.source_hash;
 const linkIntent=await begin(linked);await auth(linkIntent);
 await db.exec('reset role');await db.query("update public.employee_contacts set phone='000 999 9999'where employee_id=$1",[existing]);await session(owner);
 const changed=await facts();await deny('select public.commit_staff_provision($1::jsonb)',[JSON.stringify(linked)],['23514']);eq(await facts(),changed,'Stale source rejection changes no employee, contact, membership or audit facts');
 savedSource=await source(existing);const replacement={...linked,id:id(serial++),contacts:savedSource.contacts,expected_source:savedSource.source_hash,supersedes:linked.id,reason:'Reviewed changed employee contact source'};
 const revised=await begin(replacement);eq(revised.user_id,linkIntent.user_id,'Replacement preserves Auth identity and existing credentials');await deny('select public.commit_staff_provision($1::jsonb)',[JSON.stringify(linked)],['23514']);
 const linkedReceipt=await commit(replacement);eq(linkedReceipt.employee_id,existing,'Existing employee gets one account without a duplicate person');
 await db.exec('reset role');eq((await db.query('select state from hris_private.staff_provision_intents where id=$1',[linked.id])).rows[0].state,'superseded','Original immutable attempt remains linked');await session(owner);
 const ownSource=await source(receipt.employee_id);const duplicatePerson={...make(),employee:ownSource.employee,contacts:ownSource.contacts,expected_source:ownSource.source_hash};await deny('select public.begin_staff_provision($1::jsonb)',[JSON.stringify(duplicatePerson)],['23505']);
 const invalid=make();const beforeInvalid=await facts();
 for(const patch of[{password:'must never be stored'},{roles:['employee','recruiter']},{roles:['recruiter','recruiter']},{roles:['learning_officer']},{reason:''},{active:'true'},{expected_source:'pretend'},{employee:{...invalid.employee,company_id:id(999)}},{contacts:{...invalid.contacts,company_id:id(999)}},{contacts:{...invalid.contacts,employee_id:id(888)}},{employee:{...invalid.employee,full_name:''}},{email:'invalid'},{supersedes:linked.id}])await deny('select public.begin_staff_provision($1::jsonb)',[JSON.stringify({...invalid,...patch})]);
 eq(await facts(),beforeInvalid,'Invalid rehearsal/request preserves business records');
 const collision={...make(),email:request.email};await deny('select public.begin_staff_provision($1::jsonb)',[JSON.stringify(collision)],['23505']);
 const cancelRequest=make(),cancelIntent=await begin(cancelRequest);await auth(cancelIntent);
 const cancelSql='select public.cancel_staff_provision($1,$2)r';const cancelled=(await db.query(cancelSql,[cancelRequest.id,'Reviewed cancellation'])).rows[0].r;
 eq(cancelled,{id:cancelRequest.id,state:'cancelled'},'Reasoned cancellation closes pending reservation');eq((await db.query(cancelSql,[cancelRequest.id,'Reviewed cancellation'])).rows[0].r,cancelled,'Exact cancellation retry');
 await deny(cancelSql,[cancelRequest.id,'Different reason'],['23514']);await deny('select public.commit_staff_provision($1::jsonb)',[JSON.stringify(cancelRequest)],['23514']);
 const resume={...cancelRequest,id:id(serial++),supersedes:cancelRequest.id,reason:'Explicit reviewed resumption'};eq((await begin(resume)).user_id,cancelIntent.user_id,'Cancelled attempt can resume through linked explicit review');
 await db.exec('reset role');eq((await db.query('select count(*)::int n from auth.users where id=$1',[cancelIntent.user_id])).rows[0].n,1,'Cancellation never deletes a possibly created Auth identity');await session(owner);await commit(resume);
 const fault=make(),faultIntent=await begin(fault);await auth(faultIntent);const faultBefore=await facts();
 await db.exec('reset role');await db.exec("create function hris_private.provision_test_fault()returns trigger language plpgsql as $$begin if new.kind='account_provisioned'then raise exception'fictional audit failure';end if;return new;end;$$;create trigger provision_test_fault before insert on hris_private.staff_role_events for each row execute function hris_private.provision_test_fault();");await session(owner);
 await deny('select public.commit_staff_provision($1::jsonb)',[JSON.stringify(fault)],['P0001']);eq(await facts(),faultBefore,'Audit failure rolls back employee/contact/membership/access history together');
 await db.exec('reset role');eq((await db.query('select state,receipt from hris_private.staff_provision_intents where id=$1',[fault.id])).rows[0],{state:'reserved',receipt:null},'Failure leaves no false success receipt');
 await db.exec('drop trigger provision_test_fault on hris_private.staff_role_events;drop function hris_private.provision_test_fault();');await session(owner);eq((await commit(fault)).user_id,faultIntent.user_id,'Exact failed commit succeeds after fault resolution');
 // Revocation applies before replay as well as before a first commit.
 await session(otherAdmin);const revoked=make(),revokedIntent=await begin(revoked);await auth(revokedIntent);await session(owner);
 await db.query('select public.save_company_account($1,$2,$3,false,null,null)',[otherAdmin,'fixture-'+otherAdmin.slice(-3)+'@example.invalid','administrator']);await session(otherAdmin);
 await deny('select public.commit_staff_provision($1::jsonb)',[JSON.stringify(revoked)],['42501']);await deny('select public.begin_staff_provision($1::jsonb)',[JSON.stringify(revoked)],['42501']);
 await session(owner);await db.exec('reset role');
 for(const table of['staff_provision_intents','staff_provision_events']){await deny('update hris_private.'+table+" set created_at=now()");await deny('delete from hris_private.'+table);}
 await session(owner);const publicBeforeCap=await facts();for(let i=(await pending()).entry_count;i<30;i++)await begin(make());
 await deny('select public.begin_staff_provision($1::jsonb)',[JSON.stringify(make())],['54000']);eq((await pending()).entry_count,30,'Pending quota is explicit without history truncation');eq(await facts(),publicBeforeCap,'Reservation quota rehearsals preserve all business data');
 console.log('Passed '+checks+' staff provisioning assertions against75actual migrations. Auth creation is fictional; specialist runtime enabled only inside this fixture.');
}catch(error){console.error(JSON.stringify({checks,message:error.message,code:error.code,where:error.where,frame:error.stack?.split('\n').filter(s=>s.includes('staff-provisioning.test.mjs')).slice(-3)}));process.exitCode=1;}finally{await db.close();}