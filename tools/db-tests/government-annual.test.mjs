// Local PostgreSQL evidence. Auth/Storage and source-limit stubs are fictional.
import{writeFile}from'node:fs/promises';
export async function runGovernmentAnnualTests({db,eq,deny,session,company,owner,unassigned}){
 const call=async(year)=>(await db.query('select public.government_annual_ledger($1)r',[year])).rows[0].r;
 const vectors=[];
 await session(owner);
 for(const year of[2025,2026]){
  const w=await call(year);vectors.push(w);
  eq(w.version,'government-annual-ledger-v1','Annual versioned projection');
  eq(w.company_id,company.id,'Annual protected company');eq(w.year,year,'Annual selected year');
  eq(w.through_month,year===2025?'2025-12-01':w.as_of.slice(0,7)+'-01','No future monthly query');
  const months=Number(w.through_month.slice(5,7)),facts=new Map();
  for(let m=1;m<=months;m++){
   const month=year+'-'+String(m).padStart(2,'0')+'-01';
   const r=(await db.query('select public.government_payroll_sources($1,$2)r',[month,'bir'])).rows[0].r;
   for(const f of r.facts){const k=f.kind+':'+f.id;const prior=facts.get(k);if(prior)eq(prior.fact,f,'Repeated actual monthly source is identical');else facts.set(k,{fact:f,observed_months:[]});facts.get(k).observed_months.push(month);}
  }
  eq(w.entries,[...facts.values()].sort((a,b)=>(a.fact.kind+':'+a.fact.id).localeCompare(b.fact.kind+':'+b.fact.id)),'Annual projection deduplicates actual monthly immutable sources exactly');
  eq(w.entries.every(e=>Object.keys(e).sort().join(',')==='fact,observed_months'),true,'No private review history in entries');
  eq(w.entries.every(e=>e.fact.state!=='received'||e.fact.received_on.startsWith(year+'-')),true,'Received facts follow actual receipt year');
  eq(new Set(w.employees.map(e=>e.employee_id)).size,w.employees.length,'Each employee label once; namesakes not collapsed');
  eq(w.employees.every(e=>Object.keys(e).sort().join(',')==='employee_code,employee_id,employee_name'),true,'No tax identifiers or HR evidence in labels');
  eq(JSON.stringify(w).includes('PRIVATE_'),false,'No fixture private sentinels');
 }
 for(const y of[null,2024,2200])await deny('select public.government_annual_ledger($1)',[y],['23514']);
 await db.exec('reset role');
 const actors=(await db.query("select user_id,role,active from public.company_members order by user_id")).rows;
 for(const role of['employee','manager']){const a=actors.find(a=>a.role===role&&a.active);eq(!!a,true,'Existing '+role+' fixture');await session(a.user_id);await deny('select public.government_annual_ledger($1)',[2026],['42501']);}
 const hr=actors.find(a=>a.role==='hr_generalist'&&a.active);eq(!!hr,true,'Existing HR fixture');await session(hr.user_id);eq((await call(2026)).company_id,company.id,'HR annual read positive');
 await session(unassigned);await deny('select public.government_annual_ledger($1)',[2026],['42501']);
 await db.exec('reset role');await db.exec('begin');
 try{await db.query("update auth.users set raw_user_meta_data='{"+'"role":"administrator","company_id":"'+company.id+'"'+"}'::jsonb where id=$1",[unassigned]);await session(unassigned);await deny('select public.government_annual_ledger($1)',[2026],['42501']);}
 finally{await db.exec('rollback');await db.exec('reset role');}
 const inactive=actors.find(a=>!a.active);if(inactive){await session(inactive.user_id);await deny('select public.government_annual_ledger($1)',[2026],['42501']);}
 await session('','anon');await deny('select public.government_annual_ledger($1)',[2026],['42501']);
 await db.exec('reset role');
 for(const schema of['public','hris_private']){
  eq((await db.query('select prosecdef from pg_proc where oid=$1::regprocedure',[schema+'.government_annual_ledger(integer)'])).rows[0].prosecdef,false,'Annual function is security invoker '+schema);
  eq((await db.query("select has_function_privilege('anon',$1,'execute')v",[schema+'.government_annual_ledger(integer)'])).rows[0].v,false,'Anonymous has no annual execution '+schema);
 }
 const eid=vectors[1].employees[0]?.employee_id||(await db.query('select id from public.employees where company_id=$1 limit 1',[company.id])).rows[0].id;
 const makeFact=(overrides={})=>({company_id:company.id,employee_id:eid,id:'00000000-0000-0000-0000-0000000a0057',agency:'bir',kind:'annual_tax',state:'received',starts_on:'2025-01-01',ends_on:'2025-12-31',contribution_month:null,received_on:'2026-01-12',parts:{employee_cents:-12345,employer_cents:0,ec_cents:0},...overrides});
 // Replace only the base projection INSIDE a rolled-back transaction to exercise
 // conflict/budget branches without fabricating or rewriting payroll histories.
 const stub=async(expression,check)=>{
  await db.exec('reset role');await db.exec('begin');
  try{
   await db.exec(`create or replace function hris_private.government_payroll_sources(p_month date,p_agency text)returns jsonb language plpgsql stable security definer set search_path=''as $stub$
   begin
    if p_month>date'2026-10-26'then raise exception 'Future month requested'using errcode='23514';end if;
    return jsonb_build_object('company_id','${company.id}','company_name','Fictional Annual Source Company','generated_at',transaction_timestamp(),'as_of','2026-10-26','facts',(${expression}));end;$stub$;`);
   await session(owner);await check();
  }finally{await db.exec('rollback');await db.exec('reset role');}
 };
 const literal=f=>`'${JSON.stringify(f).replaceAll("'","''")}'::jsonb`;
 const postdated=makeFact({state:'dated_after_as_of',received_on:'2026-10-28'});
 await stub(`case when extract(month from p_month)in(1,10)then jsonb_build_array(${literal(postdated)})else'[]'::jsonb end`,async()=>{
  const w=await call(2026);vectors.push(w);eq(w.entries.length,1,'Postdated duplicate only one annual source');eq(w.entries[0].observed_months,['2026-01-01','2026-10-01'],'Both observed months retained');eq(w.entries[0].fact.received_on,'2026-10-28','Postdated actual date preserved');
 });
 const signed=makeFact();
 await stub(`case when extract(month from p_month)=1 then jsonb_build_array(${literal(signed)})else'[]'::jsonb end`,async()=>{
  const w=await call(2026);vectors.push(w);eq(w.entries[0].fact.parts.employee_cents,-12345,'Signed centavos retained, never truncated or clamped');eq(w.entries[0].fact.starts_on,'2025-01-01','Prior service year remains separate from actual receipt year');
 });
 await stub(`case when extract(month from p_month)in(1,10)then jsonb_build_array(jsonb_set(${literal(postdated)},'{parts,employee_cents}',to_jsonb(extract(month from p_month)::integer)))else'[]'::jsonb end`,async()=>{
  await deny('select public.government_annual_ledger(2026)',[],['23514']);
 });
 const missing=makeFact({state:'receipt_missing',received_on:null});
 await stub(`jsonb_build_array(${literal(missing)})`,async()=>{
  const w=await call(2026);vectors.push(w);eq(w.entries.length,1,'Repeated missing-receipt fact not counted ten times');eq(w.entries[0].observed_months.length,10,'Missing receipt observation is not an invented payment date');
 });
 await stub(`'[]'::jsonb`,async()=>{const w=await call(2026);vectors.push(w);eq(w.entries,[],'Empty year remains no recorded sources');eq(w.employees,[],'No unrelated employee identities returned');});
 const many=n=>`(select jsonb_agg(jsonb_set(${literal(missing)},'{id}',to_jsonb(('00000000-0000-0000-0000-'||lpad(g::text,12,'0')))))from generate_series(1,${n})g)`;
 await stub(many(4001),async()=>{await deny('select public.government_annual_ledger(2026)',[],['23514']);});
 await stub(`case when extract(month from p_month)=1 then ${many(20000)} when extract(month from p_month)=2 then jsonb_build_array(jsonb_set(${literal(missing)},'{id}','"00000000-0000-0000-0000-999999999999"'))else'[]'::jsonb end`,async()=>{await deny('select public.government_annual_ledger(2026)',[],['23514']);});
 await session(owner);eq((await call(2026)).entries,vectors[1].entries,'Every temporary source replacement rolled back');
 await db.exec('reset role');
 await writeFile(new URL('../../work/annual057-db-vectors.json',import.meta.url),JSON.stringify(vectors));
 console.log('Annual recorded-source ledger: actual sources, scopes, signed dates, duplicate/conflict and whole-file budget cases passed.');
}
