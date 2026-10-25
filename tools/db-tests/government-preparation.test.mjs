import{writeFile}from'node:fs/promises';
export async function runGovernmentPreparationTests({db,eq,deny,session,company,owner,unassigned}){
 const vectors=[];await session(owner);
 for(const agency of['sss','philhealth','pagibig','bir']){
  const r=(await db.query('select public.government_preparation($1,$2)r',['2026-01-01',agency])).rows[0].r;
  eq(r.version,'government-preparation-v1','Versioned preparation envelope');eq(r.review.source.company_id,company.id,'Single protected company');eq(r.review.source.agency,agency,'Selected agency only');
  for(const identity of r.registrations){
   eq(identity.company_id,company.id,'Identity stays within selected company');
   eq(Object.hasOwn(identity,'reference'),false,'No supporting identity reference');eq(Object.hasOwn(identity,'note'),false,'No private identity note');eq(Object.hasOwn(identity,'created_by'),false,'No actor identity');
   const person=identity.employee_id!==null;
   const fields=[...(person?['last_name','first_name','middle_name','name_suffix']:['registered_name']),...(agency==='bir'?['bir_tin','bir_branch']:[{sss:'sss_number',philhealth:'philhealth_number',pagibig:'pagibig_number'}[agency]]),...(person&&agency==='philhealth'?['birth_date','sex']:person&&agency==='bir'?['nationality']:[])];
   eq(Object.keys(identity.details).sort(),fields.sort(),'Only agency-purpose identity fields');
   if(person)eq(r.review.employees.some(e=>e.employee_id===identity.employee_id),true,'No unrelated directory registration');
  }
  eq(JSON.stringify(r.registrations).includes('PRIVATE_REGISTRATION_NOTE'),false,'Private evidence excluded');vectors.push(r);
 }
 await deny('select public.government_preparation($1,$2)',['2026-01-02','sss']);await deny('select public.government_preparation($1,$2)',['2026-01-01','foreign']);
 await db.exec('reset role');const rows=(await db.query("select m.user_id,m.role,m.active from public.company_members m join auth.users u on u.id=m.user_id where u.email in('registration-staff@example.invalid','registration-manager@example.invalid','registration-hr@example.invalid')")).rows;
 eq(rows.length,3,'Retained synthetic authorization fixtures');
 for(const r of rows){await session(r.user_id);await deny('select public.government_preparation($1,$2)',['2026-01-01','sss'],['42501']);}
 await session(unassigned);await deny('select public.government_preparation($1,$2)',['2026-01-01','sss'],['42501']);await session('','anon');await deny('select public.government_preparation($1,$2)',['2026-01-01','sss'],['42501']);
 await writeFile(new URL('../../work/government-preparation-vectors.json',import.meta.url),JSON.stringify(vectors,null,2)+'\n');
}
