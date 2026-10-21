export async function runEmploymentTests({db,eq,deny,session,employee,company,owner,unassigned}) {
  await session(owner);
  const types=(await db.query('select * from public.employment_types order by name')).rows;
  const locations=(await db.query('select * from public.work_locations order by name')).rows;
  eq(types.map(c=>c.name),['Contract','Full-time','Internship','Part-time'],'Bootstrap seeds editable generic employment types');
  eq(locations.map(c=>c.name),['Main office','Remote'],'Bootstrap seeds generic locations');
  const person=await employee(company.id,'EMPLOYMENT-QA');
  eq([person.employment_type_id,person.work_location_id],[null,null],'Existing/new unclassified employee remains not set');
  const save=async(kind,value)=>(await db.query('select public.save_employment_option($1,$2::jsonb) id',[kind,JSON.stringify(value)])).rows[0].id;
  for(const kind of ['employment_type','work_location']) {
    for(const value of [{name:' ',active:true},{name:'a'.repeat(81),active:true},{name:'bad\nname',active:true},{name:123,active:true},{name:'Name',active:'yes'},{name:'Name',active:null}]) {
      await deny('select public.save_employment_option($1,$2::jsonb)',[kind,JSON.stringify(value)],['23514']);
    }
  }
  await deny('select public.save_employment_option($1,$2::jsonb)',['unknown',JSON.stringify({name:'Invalid',active:true})],['23514']);
  const type=await save('employment_type',{name:'  QA Specialist  ',active:true});
  const location=await save('work_location',{name:'QA Branch',active:true});
  const row=async(table,id)=>(await db.query(`select * from public.${table} where id=$1`,[id])).rows[0];
  eq((await row('employment_types',type)).name,'QA Specialist','Setting names are trimmed on the server');
  eq((await row('employment_types',type)).revision,0,'New option revision starts at zero');
  await deny('select public.save_employment_option($1,$2::jsonb)',['employment_type',JSON.stringify({name:'qa SPECIALIST',active:true})],['23505']);
  await db.query('update public.employees set employment_type_id=$1,work_location_id=$2 where id=$3',[type,location,person.id]);
  eq((await db.query('select employment_type_id,work_location_id from public.employees where id=$1',[person.id])).rows[0],{employment_type_id:type,work_location_id:location},'HR fields store both canonical choice IDs');
  await save('employment_type',{id:type,name:'QA Specialist renamed',active:false,expected_revision:0});
  await save('work_location',{id:location,name:'QA Branch renamed',active:false,expected_revision:0});
  eq((await row('employment_types',type)).revision,1,'Rename/deactivation increments revision');
  eq((await db.query('select employment_type_id,work_location_id from public.employees where id=$1',[person.id])).rows[0],{employment_type_id:type,work_location_id:location},'Deactivation retains existing employee assignments');
  await deny('select public.save_employment_option($1,$2::jsonb)',['employment_type',JSON.stringify({id:type,name:'Stale',active:true,expected_revision:0})],['PT409']);
  await deny('select public.save_employment_option($1,$2::jsonb)',['work_location',JSON.stringify({id:type,name:'Wrong collection',active:true,expected_revision:1})],['PT409']);
  await deny('select public.save_employment_option($1,$2::jsonb)',['employment_type',JSON.stringify({id:unassigned,name:'Missing',active:true,expected_revision:0})],['PT409']);
  const another=await employee(company.id,'EMPLOYMENT-OTHER');
  for(const [column,id] of [['employment_type_id',type],['work_location_id',location],['employment_type_id',locations[0].id],['work_location_id',types[0].id]]) {
    await deny(`update public.employees set ${column}=$1 where id=$2`,[id,another.id],['23514','23503']);
  }
  // An old client does not know these fields; it must not clear them on save.
  await db.query(`select public.save_employee_record((to_jsonb(e)-'employment_type_id'-'work_location_id')||'{"department":"QA retained inactive"}'::jsonb,'{}'::jsonb) from public.employees e where id=$1`,[person.id]);
  eq((await db.query('select employment_type_id,work_location_id from public.employees where id=$1',[person.id])).rows[0],{employment_type_id:type,work_location_id:location},'Legacy employee RPC preserves omitted assignments, including inactive ones');
  await db.query(`select public.save_employee_record(to_jsonb(e)||'{"employment_type_id":null,"work_location_id":null}'::jsonb,'{}'::jsonb) from public.employees e where id=$1`,[person.id]);
  eq((await db.query('select employment_type_id,work_location_id from public.employees where id=$1',[person.id])).rows[0],{employment_type_id:null,work_location_id:null},'Explicit null clears employee choices');
  await deny('update public.employees set employment_type_id=$1 where id=$2',[type,person.id],['23514']);
  await save('employment_type',{id:type,name:'QA Specialist renamed',active:true,expected_revision:1});
  await save('work_location',{id:location,name:'QA Branch renamed',active:true,expected_revision:1});
  await db.query('update public.employees set employment_type_id=$1,work_location_id=$2 where id=$3',[type,location,person.id]);
  for(const table of ['employment_types','work_locations']) {
    await deny(`insert into public.${table}(company_id,name) values($1,'Forged direct write')`,[company.id],['42501']);
    await deny(`update public.${table} set active=false where company_id=$1`,[company.id],['42501']);
    await deny(`delete from public.${table} where company_id=$1`,[company.id],['42501']);
  }
  const hr='00000000-0000-0000-0000-000000000601';
  await db.exec('reset role');await db.query(`insert into auth.users(id,email,email_confirmed_at) values($1,'employment-hr@example.invalid',now())`,[hr]);
  await session(owner);await db.query(`select public.save_company_account($1,'employment-hr@example.invalid','hr_generalist',true,null,null)`,[hr]);
  await session(hr);
  eq((await db.query('select * from public.employment_types')).rows.length,5,'HR reads available/inactive company choices');
  await db.query('update public.employees set employment_type_id=$1,work_location_id=$2 where id=$3',[types[0].id,locations[0].id,another.id]);
  eq((await db.query('select employment_type_id from public.employees where id=$1',[another.id])).rows[0].employment_type_id,types[0].id,'HR can assign company choices');
  await deny('select public.save_employment_option($1,$2::jsonb)',['employment_type',JSON.stringify({name:'Forbidden HR option',active:true})],['42501']);
  const manager='00000000-0000-0000-0000-000000000102',staff='00000000-0000-0000-0000-000000000103';
  for(const uid of [manager,staff]) {
    await session(uid);
    eq((await db.query('select * from public.work_locations')).rows.length,3,'Active staff/portal scope reads labels without configuration access');
    eq((await db.query('update public.employees set employment_type_id=$1 returning id',[type])).rows.length,0,'Manager/Employee cannot edit employment assignments, including their own');
    await deny('select public.save_employment_option($1,$2::jsonb)',['work_location',JSON.stringify({name:'Forbidden',active:true})],['42501']);
  }
  await session(unassigned);
  for(const table of ['employment_types','work_locations'])eq((await db.query(`select * from public.${table}`)).rows.length,0,'Unassigned membership cannot read company choices');
  await session('', 'anon');
  for(const table of ['employment_types','work_locations'])await deny(`select * from public.${table}`,[],['42501']);
  await deny('select public.save_employment_option($1,$2::jsonb)',['work_location',JSON.stringify({name:'Anonymous',active:true})],['42501']);
  await session(owner);await db.query(`select public.save_company_account($1,'employment-hr@example.invalid','hr_generalist',false,null,null)`,[hr]);
  await session(hr);
  eq((await db.query('select * from public.employment_types')).rows.length,0,'Disabled membership loses choice reads immediately');
  await session(owner);
  // The combined staff editor carries the same assignments through its existing atomic RPC.
  await db.query(`select public.save_staff_account($1,'admin@example.invalid','administrator',true,to_jsonb(e)||jsonb_build_object('employment_type_id',$2::uuid,'work_location_id',$3::uuid),'{}'::jsonb)
    from public.employees e join public.company_members m on m.employee_id=e.id where m.user_id=$1`,[owner,type,location]);
  const assigned=(await db.query('select e.employment_type_id,e.work_location_id from public.employees e join public.company_members m on m.employee_id=e.id where m.user_id=$1',[owner])).rows[0];
  eq(assigned,{employment_type_id:type,work_location_id:location},'Existing staff-account RPC saves both assignment fields');
  const before=(await db.query('select phone from public.employee_contacts where employee_id=$1',[person.id])).rows[0];
  await deny(`select public.save_employee_record(to_jsonb(e)||jsonb_build_object('employment_type_id',$2::uuid),'{"phone":"000 111 2222"}'::jsonb) from public.employees e where id=$1`,[person.id,unassigned],['23514','23503']);
  eq((await db.query('select phone from public.employee_contacts where employee_id=$1',[person.id])).rows[0],before,'Invalid assignment rolls back the employment/contact save');
  await db.exec('reset role');
  eq((await db.query(`select count(*)::int n from pg_class where oid in ('public.employment_types'::regclass,'public.work_locations'::regclass) and relrowsecurity`)).rows[0].n,2,'Both new exposed tables enforce RLS');
  await session(owner);
}
