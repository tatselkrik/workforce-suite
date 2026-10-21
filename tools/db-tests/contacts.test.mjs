export async function runContactTests({db,eq,deny,session,employee,company,owner,unassigned}) {
  const hr='00000000-0000-0000-0000-000000000101';
  const manager='00000000-0000-0000-0000-000000000102';
  const staff='00000000-0000-0000-0000-000000000103';
  const peer='00000000-0000-0000-0000-000000000104';
  await session(owner);
  const members=(await db.query('select user_id,employee_id from public.company_members')).rows;
  const person=id=>members.find(m=>m.user_id===id).employee_id;
  const add=async id=>(await db.query(`insert into public.employee_contacts(company_id,employee_id,phone,personal_email,address,
    emergency_name,emergency_relationship,emergency_phone) values($1,$2,' 000 000 0000 ',' sample@example.invalid ',
    ' Fictional sample address ',' Sample Contact ',' Sibling ','000 000 0001') returning *`,[company.id,id])).rows[0];
  const count=async()=> (await db.query('select employee_id from public.employee_contacts')).rows.map(r=>r.employee_id).sort();
  eq((await db.query("select relrowsecurity from pg_class where oid='public.employee_contacts'::regclass")).rows[0].relrowsecurity,true,'Contact table has RLS');
  const initial=await add(person(manager));
  eq(initial.phone,'000 000 0000','Phone is trimmed on the server');
  eq(initial.personal_email,'sample@example.invalid','Personal email is trimmed on the server');
  eq(initial.emergency_name,'Sample Contact','Emergency contact is trimmed on the server');
  await add(person(staff)); await add(person(peer));
  await deny('insert into public.employee_contacts(company_id,employee_id) values($1,$2)',[company.id,person(manager)],['23505']);
  for(const [field,value] of [['phone','call me'],['phone','12'],['phone','1'.repeat(41)],['phone','000\n000'],
      ['personal_email','invalid'],['personal_email','a@@example.invalid'],['personal_email','a@.invalid'],
      ['personal_email','a\u0001@example.invalid'],['address','界'.repeat(501)],['emergency_name','x'.repeat(121)],
      ['emergency_relationship','x'.repeat(81)],['emergency_phone',''],['emergency_name','']]) {
    await deny(`update public.employee_contacts set ${field}=$1 where employee_id=$2`,[value,person(staff)],['23514']);
  }
  for(const field of ['id','company_id','employee_id','updated_at']) {
    await deny(`update public.employee_contacts set ${field}=${field} where employee_id=$1`,[person(staff)],['42501']);
  }
  await deny('delete from public.employee_contacts',[],['42501']);
  await deny('insert into public.employee_contacts(company_id,employee_id) values($1,$2)',[company.id,'ffffffff-ffff-ffff-ffff-ffffffffffff'],['23503']);
  await deny("insert into public.employee_contacts(company_id,employee_id,emergency_alternate_phone) values($1,$2,'000 000 0002')",[company.id,person(hr)],['23514']);
  await session(hr);
  eq((await count()).length,3,'HR can read private contacts across the company');
  await add(person(hr));
  eq((await db.query("update public.employee_contacts set address='Updated fictional address' where employee_id=$1 returning address",[person(staff)])).rows[0].address,
    'Updated fictional address','HR can edit another employee contact record');
  await session(manager);
  eq(await count(),[person(manager)],'Manager reads only own private contacts, excluding direct report');
  eq((await db.query("update public.employee_contacts set address='Updated own fictional address' returning employee_id")).rows.map(r=>r.employee_id),[person(manager)],'Manager updates only own contacts, excluding team');
  await deny('insert into public.employee_contacts(company_id,employee_id) values($1,$2)',[company.id,person(manager)],['23505']);
  await session(staff);
  eq(await count(),[person(staff)],'Portal employee database scope reads own private contacts only');
  eq((await db.query("update public.employee_contacts set phone='000 000 0002' returning employee_id")).rows.map(r=>r.employee_id),[person(staff)],'Portal employee can update only own contacts');
  await session(owner);
  await db.query('select public.save_company_account($1,$2,$3,false,$4,$5)',[staff,'staff@example.invalid','employee',person(staff),null]);
  await session(staff);
  eq(await count(),[],'Disabled membership loses contacts despite existing token');
  await session(owner);
  await db.query('select public.save_company_account($1,$2,$3,true,$4,$5)',[staff,'staff@example.invalid','employee',person(staff),null]);
  eq((await db.query(`update public.employee_contacts set phone='',personal_email='',address='',emergency_name='',
    emergency_relationship='',emergency_phone='',emergency_alternate_phone='' where employee_id=$1 returning phone`,[person(staff)])).rows[0].phone,'','Optional details can be cleared together');
  const archived=await employee(company.id,'CONTACTS-ARCHIVED',15,'archived');
  await add(archived.id);
  eq((await db.query('select employee_id from public.employee_contacts where employee_id=$1',[archived.id])).rows.length,1,'Archived employee retains editable HR contact record');
  await session(unassigned);
  eq(await count(),[],'Unassigned account cannot read contacts');
  await deny('insert into public.employee_contacts(company_id,employee_id) values($1,$2)',[company.id,person(peer)],['42501']);
  await session('');
  eq(await count(),[],'Missing subject cannot read contacts');
  await session('', 'anon');
  await deny('select * from public.employee_contacts',[],['42501']);
  await deny('insert into public.employee_contacts(company_id,employee_id) values($1,$2)',[company.id,person(peer)],['42501']);
}
