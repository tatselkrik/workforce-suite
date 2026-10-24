import{writeFile}from'node:fs/promises';
export async function runGovernmentRegistrationTests({db,eq,deny,session,company,owner,unassigned}){
 const id=n=>`00000000-0000-4000-8000-${String(820000+n).padStart(12,'0')}`,users={};
 await db.exec('reset role');const originalCompany=(await db.query('select to_jsonb(c)r from public.companies c where id=$1',[company.id])).rows[0].r;
 const payroll={};for(const{tablename:t}of(await db.query("select tablename from pg_tables where schemaname='public'and tablename like'payroll_%'order by tablename")).rows)payroll[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
 for(const[k,role]of[['hr','hr_generalist'],['staff','employee'],['manager','manager']]){
  const uid=id(Object.keys(users).length+1);await db.exec('reset role');await db.query('insert into auth.users(id,email,email_confirmed_at)values($1,$2,now())',[uid,`registration-${k}@example.invalid`]);await session(owner);await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[uid,`registration-${k}@example.invalid`,role]);
  users[k]={uid,employee:(await db.query('select employee_id from public.company_members where user_id=$1',[uid])).rows[0].employee_id};
 }
 const call=async record=>(await db.query('select public.save_government_registration($1)r',[record])).rows[0].r;
 const no=(record,codes=['23514','42501','PT409','23505'])=>deny('select public.save_government_registration($1)',[record],codes);
 const snap=async(eid=null)=>(await db.query('select public.government_registration($1)r',[eid])).rows[0].r;
 const companyPayload={id:id(20),operation_id:id(21),employee_id:null,expected_revision:null,details:{registered_name:'Fictional Statutory Registration Company',bir_tin:'012-345-678',bir_branch:'0000',sss_number:'01-2345678-9',philhealth_number:'01-234567890-1',pagibig_number:'0123-4567-8901',country:'ph',email:'registration@example.invalid'},reference:'Synthetic registration certificate',note:'PRIVATE_REGISTRATION_NOTE',reason:'Record fictional registration facts',reviewed:true};
 await session(users.hr.uid);let empty=await snap();eq(empty.record,null,'New company registration stays unknown');eq(empty.history,[],'No invented legal history');
 for(const field of['employee_id','expected_revision','details','reviewed']){const bad={...companyPayload};delete bad[field];await no(bad);}
 for(const reviewed of[null,false,'true'])await no({...companyPayload,reviewed});
 for(const details of[null,[],{registered_name:2},{registered_name:'Bad\nname'},{birth_date:'1990-01-01'},{bir_tin:'000000000'},{bir_tin:'01234567'},{bir_tin:'O12345678'},{bir_branch:'000'},{philhealth_number:'000000000000'},{sss_number:'012345678'},{country:'PHL'},{email:'not-an-email'},{employer_type:'employee'}])await no({...companyPayload,details});
 await no({...companyPayload,company_id:company.id});await no({...companyPayload,reference:''});
 let receipt=await call(companyPayload);eq(receipt,{operation_id:companyPayload.operation_id,record_id:companyPayload.id,saved_revision:0},'Save returns a pinned operation receipt');
 eq(await call(companyPayload),receipt,'Exact registration retry creates no duplicate revision');let companyView=await snap();let original=companyView.record;
 eq(original.details.bir_tin,'012345678','TIN leading zeroes retained after separator normalization');eq(original.details.bir_branch,'0000','Documented main branch remains explicit');eq(original.details.country,'PH','Country normalization is explicit');eq(companyView.history.length,1,'One immutable creation event');
 await no({...companyPayload,note:'Changed retry'});await no({...companyPayload,operation_id:id(22)},['PT409']);
 const edit={...companyPayload,operation_id:id(23),expected_revision:0,details:{...original.details,registered_name:'Fictional Corrected Registration'},reason:'Correct the registered legal name'};
 await no({...edit,expected_revision:0.1});await no({...edit,expected_revision:-1});
 receipt=await call(edit);eq(receipt.saved_revision,1,'Audited revision changes exactly once');eq((await snap()).history[1].before_record,original,'History retains original legal facts');
 eq((await call(companyPayload)).saved_revision,0,'A creation retry retains its original receipt after later edits');eq((await snap()).record.revision,1,'Old retry never restores prior details');
 await session(owner);await no(companyPayload,['PT409']);
 await session(users.hr.uid);
 const employeePayload={id:id(30),operation_id:id(31),employee_id:users.staff.employee,expected_revision:null,details:{last_name:'Dela Cruz',first_name:'Fictional',middle_name:'Ángel',name_suffix:'Jr.',birth_date:'1990-02-28',sex:'M',nationality:'FILIPINO',bir_tin:'123456789',bir_branch:'0000',bir_region:'0006',sss_number:'0123456789',philhealth_number:'012345678901',pagibig_number:'012345678901'},reference:'Synthetic member registration forms',note:'PRIVATE_EMPLOYEE_REGISTRATION',reason:'Record fictional legal identity without changing employment',reviewed:true};
 for(const details of[{...employeePayload.details,registered_name:'Wrong scope'},{birth_date:'2200-01-01'},{birth_date:'1900-02-29'},{birth_date:'1990-2-28'},{birth_date:'1899-01-01'},{sex:'Other'},{pagibig_number:'123'},{bir_region:'006'},{last_name:'a'.repeat(101)}])await no({...employeePayload,details});
 await no({...employeePayload,employee_id:id(999)},['42501']);await call(employeePayload);let personView=await snap(users.staff.employee);
 eq(personView.record.details.middle_name,'Ángel','Legal diacritics preserved without name guessing');eq(personView.record.details.first_name,'Fictional','Explicit legal components retained');
 await db.exec('reset role');const canonical=(await db.query('select full_name from public.employees where id=$1',[users.staff.employee])).rows[0].full_name;eq(canonical==='Fictional',false,'Legal-name entry does not rename canonical employment');
 eq((await db.query('select to_jsonb(c)r from public.companies c where id=$1',[company.id])).rows[0].r,originalCompany,'Legal employer name never changes display branding/defaults');
 await deny('update public.government_registration_events set reason=$1 where id=$2',['Tamper',employeePayload.operation_id],['42501']);await deny('delete from public.government_registration_records where id=$1',[employeePayload.id],['42501']);
 await session(users.staff.uid);await deny('select public.government_registration($1)',[users.staff.employee],['42501']);eq((await db.query('select count(*)::int n from public.government_registration_records')).rows[0].n,0,'Active Employee cannot read even own HR-only registration');
 await session(owner);await db.query('select public.save_company_account($1,$2,$3,false,$4,null)',[users.staff.uid,'registration-staff@example.invalid','employee',users.staff.employee]);await db.query("update public.employees set status='archived'where id=$1",[users.staff.employee]);eq((await snap(users.staff.employee)).record.id,employeePayload.id,'Archived staff retain HR registration history');
 const vectors=[await snap(),await snap(users.staff.employee),await snap(users.manager.employee)];
 for(const key of['staff','manager']){
  await session(users[key].uid);for(const table of['government_registration_records','government_registration_events'])eq((await db.query('select count(*)::int n from public.'+table)).rows[0].n,0,'No '+key+' registration table visibility');
  await deny('select public.government_registration($1)',[users[key].employee],['42501']);await no(employeePayload,['42501']);
 }
 await session(unassigned);await deny('select public.government_registration(null)',[],['42501']);await session('','anon');await deny('select public.government_registration(null)',[],['42501']);
 await session(users.hr.uid);for(const table of['government_registration_records','government_registration_events']){await deny('delete from public.'+table,[],['42501']);await deny('update public.'+table+' set company_id=$1',[company.id],['42501']);}
 await session(owner);await db.query('select public.save_company_account($1,$2,$3,false,$4,null)',[users.hr.uid,'registration-hr@example.invalid','hr_generalist',users.hr.employee]);await session(users.hr.uid);await deny('select public.government_registration(null)',[],['42501']);await no(edit,['42501']);
 await db.exec('reset role');for(const t of Object.keys(payroll))eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,payroll[t],'Registration never rewrites '+t);
 await writeFile(new URL('../../work/government-registration-vectors.json',import.meta.url),JSON.stringify(vectors,null,2)+'\n');
}
