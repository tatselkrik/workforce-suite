// Local PGlite authorization/projection tests. No live records or files are used.
import{writeFile}from'node:fs/promises';
export async function runBirIdentityTests({db,eq,deny,session,company,owner,unassigned}){
 const call=async(year)=>(await db.query('select public.government_bir_identities($1)r',[year])).rows[0].r;
 const vectors=[];
 await session(owner);
 for(const year of[2025,2026]){
  const w=await call(year);vectors.push(w);eq(w.version,'government-bir-identities-v1','Explicit minimal BIR identity version');
  const comparison=(await db.query('select public.government_annual_ledger($1)r',[year])).rows[0].r;
  eq({...w.ledger,generated_at:null},{...comparison,generated_at:null},'Annual facts preserved; separate reads have separate generation times');
  eq(w.people.every(p=>Object.keys(p).sort().join(',')==='label,registration'),true,'Only label and selected registration binding');
  eq(w.people.every(p=>Object.keys(p.label).sort().join(',')==='employee_code,employee_id,employee_name'),true,'Minimal current labels only');
  eq(new Set(w.people.map(p=>p.label.employee_id)).size,w.people.length,'Unique current employees; no name collapse');
  eq(w.ledger.employees.every(e=>w.people.some(p=>p.label.employee_id===e.employee_id&&p.label.employee_code===e.employee_code&&p.label.employee_name===e.employee_name)),true,'Source and current labels share the same statement scope');
  for(const b of[w.employer,...w.people.map(p=>p.registration)].filter(Boolean)){
   eq(Object.keys(b).sort().join(','),'branch,company_id,employee_id,id,revision,tin','Only BIR pair and revision binding');eq(b.company_id,company.id,'Current company binding');
   eq(typeof b.tin,'string','Missing TIN stays blank text');eq(typeof b.branch,'string','Missing branch stays blank text');
  }
  await db.exec('reset role');
  const all=(await db.query('select id,employee_code,full_name,status from public.employees where company_id=$1 order by id',[company.id])).rows;
  eq(w.people.map(p=>p.label.employee_id),all.map(p=>p.id),'Roster includes every current/archived company employee only');
  const registrations=(await db.query('select id,employee_id,revision,details from public.government_registration_records where company_id=$1',[company.id])).rows;
  for(const r of registrations){const b=r.employee_id?w.people.find(p=>p.label.employee_id===r.employee_id)?.registration:w.employer;
   eq(b?.id,r.id,'No invented or reassigned registration identity');eq(b?.revision,Number(r.revision),'Exact stored revision');eq(b?.tin,r.details.bir_tin||'','Only selected BIR TIN');eq(b?.branch,r.details.bir_branch||'','Only selected BIR branch');}
  await session(owner);
 }
 for(const year of[null,2024,2200])await deny('select public.government_bir_identities($1)',[year],['23514']);
 await db.exec('reset role');const actors=(await db.query('select user_id,role,active from public.company_members')).rows;
 for(const role of['employee','manager']){const a=actors.find(a=>a.role===role&&a.active);eq(!!a,true,'Existing '+role+' fixture');await session(a.user_id);await deny('select public.government_bir_identities(2026)',[],['42501']);}
 await session(unassigned);await deny('select public.government_bir_identities(2026)',[],['42501']);
 const hr=actors.find(a=>a.role==='hr_generalist'&&a.active);await session(hr.user_id);eq((await call(2026)).ledger.company_id,company.id,'HR protected roster positive');
 await session('','anon');await deny('select public.government_bir_identities(2026)',[],['42501']);
 await db.exec('reset role');await db.exec('begin');
 try{await db.query("update auth.users set raw_user_meta_data=$1::jsonb where id=$2",[JSON.stringify({role:'administrator',company_id:company.id}),unassigned]);await session(unassigned);await deny('select public.government_bir_identities(2026)',[],['42501']);}
 finally{await db.exec('rollback');}
 await session(owner);await db.exec('reset role');await db.exec('begin');
 try{await db.query('update public.company_members set active=false where user_id=$1',[hr.user_id]);await session(hr.user_id);await deny('select public.government_bir_identities(2026)',[],['42501']);}
 finally{await db.exec('rollback');}
 await db.exec('reset role');
 for(const schema of['public','hris_private']){
  eq((await db.query('select prosecdef from pg_proc where oid=$1::regprocedure',[schema+'.government_bir_identities(integer)'])).rows[0].prosecdef,false,'No new security definer '+schema);
  eq((await db.query("select has_function_privilege('anon',$1,'execute')v",[schema+'.government_bir_identities(integer)'])).rows[0].v,false,'No anonymous execute '+schema);
 }
 const triggerState=(await db.query("select tgname,tgenabled from pg_trigger where tgrelid='public.employees'::regclass order by tgname")).rows;
 // A rolled-back local inert fixture exercises the real SQL roster cap without
 // manufacturing employment events. No production trigger is ever disabled.
 await db.exec('begin');
 try{
  await db.exec('alter table public.employees disable trigger user');
  await db.query("insert into public.employees(company_id,employee_code,full_name,department,job_title,joined_on,annual_leave_days)select $1,'QA-BIR-CAP-'||g,'Fictional capacity fixture','QA','Fixture',date'2026-01-01',15 from generate_series(1,20001)g",[company.id]);
  await session(owner);await deny('select public.government_bir_identities(2026)',[],['23514']);
 }finally{await db.exec('rollback');}
 await db.exec('reset role');
 eq((await db.query("select tgname,tgenabled from pg_trigger where tgrelid='public.employees'::regclass order by tgname")).rows,triggerState,'All local employee triggers restored by rollback');
 await session(owner);const after=await call(2026);eq(after.people,vectors[1].people,'No capacity rows retained');
 await db.exec('reset role');
 await writeFile(new URL('../../work/identities058-db-vectors.json',import.meta.url),JSON.stringify(vectors));
 console.log('BIR current identity projection: minimal fields, archived/current scopes, revocation and whole-roster capacity cases passed.');
}
