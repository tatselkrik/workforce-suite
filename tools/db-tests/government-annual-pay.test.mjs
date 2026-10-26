// Local PostgreSQL only. Immutable fictional source fixtures, simulated Auth.
import{writeFile}from'node:fs/promises';
export async function runAnnualPayTests({db,eq,deny,session,company,owner,unassigned}){
 const call=async(y=2026)=>(await db.query('select public.government_annual_pay_details($1)r',[y])).rows[0].r;
 const source={monthly:['payroll_slips',"item->'totals'"],cutoff:['payroll_cutoff_slips',"item->'totals'"],thirteenth:['payroll_thirteenth_slips',"item->'totals'"],amendment:['payroll_amendment_slips','totals'],final_adjustment:['payroll_final_slips','totals'],annual_tax:['payroll_tax_slips','totals']};
 const keys={
  monthly:'basic paid_leave unworked_holiday premium overtime night allowances exempt_allowances reimbursement gross taxable sss_employee philhealth_employee pagibig_employee',
  thirteenth:'gross_cents exempt_cents taxable_cents sss_adjustment_cents philhealth_adjustment_cents pagibig_adjustment_cents',
  amendment:'basic_cents gross_cents taxable_cents contribution_cents',
  final_adjustment:'vacation_gross_cents vacation_exempt_cents vacation_shared_benefit_cents separation_cents retirement_cents taxable_extras_cents shared_extras_cents exempt_extras_cents deposit_returns_cents additional_contributions_cents extras_gross_cents',annual_tax:''};keys.cutoff=keys.monthly;
 await session(owner);const vectors=[];const covered=new Set();
 for(const year of[2025,2026]){
  const w=await call(year);vectors.push(w);eq(w.version,'government-annual-pay-v1','Versioned pay detail contract');
  const old=(await db.query('select public.government_annual_ledger($1)r',[year])).rows[0].r;
  // Different statements may have different generated timestamps.
  eq({...w.ledger,generated_at:''},{...old,generated_at:''},'Existing annual source contract unchanged');
  eq(w.details.length,w.ledger.entries.length,'Exact one detail per original source');
  const parent=new Map(w.ledger.entries.map(e=>[e.fact.kind+':'+e.fact.id,e.fact]));
  for(const d of w.details){const f=parent.get(d.kind+':'+d.id);eq(!!f,true,'Bound to actual annual source');covered.add(d.kind);
   eq([d.company_id,d.employee_id],[f.company_id,f.employee_id],'Exact source company and employee');
   eq(Object.keys(d).sort(),['company_id','employee_id','id','kind','source_found','values'],'No HR evidence or raw source bundle');
   const [table,path]=source[d.kind];const raw=(await db.query(`select ${path} r from public.${table} where company_id=$1 and employee_id=$2 and id=$3`,[d.company_id,d.employee_id,d.id])).rows;
   eq(d.source_found,raw.length===1,'Source existence explicit');
   const allowed=f.state==='review_only'?[]:keys[d.kind].split(' ').filter(Boolean);
   eq(Object.keys(d.values).sort(),[...allowed].sort(),'Exact minimal per-kind whitelist');
   for(const k of allowed)eq(d.values[k],raw[0]?.r?.[k]??null,'Exact issued value or explicit unknown '+d.kind+' '+k);
   eq('source_net_cents'in d.values,false,'Contained original final pay not repeated');
   eq('entitlement_cents'in d.values,false,'Annual entitlement not new payout');
   if(d.kind==='annual_tax'||f.state==='review_only')eq(d.values,{},'Settlement/review has no new income');
  }
 }
 for(const kind of Object.keys(source))eq(covered.has(kind),true,'Actual fixture coverage '+kind);
 for(const y of[null,2024,2200])await deny('select public.government_annual_pay_details($1)',[y],['23514']);
 await db.exec('reset role');const actors=(await db.query('select user_id,role,active from public.company_members order by user_id')).rows;
 for(const role of['employee','manager']){const a=actors.find(a=>a.role===role&&a.active);eq(!!a,true,'Fixture '+role);await session(a.user_id);await deny('select public.government_annual_pay_details(2026)',[],['42501']);}
 const hr=actors.find(a=>a.role==='hr_generalist'&&a.active);await session(hr.user_id);eq((await call()).ledger.company_id,company.id,'HR allowed current protected details');
 await session(unassigned);await deny('select public.government_annual_pay_details(2026)',[],['42501']);
 await db.exec('reset role');await db.exec('begin');try{
  await db.query('update auth.users set raw_user_meta_data=$1 where id=$2',[{role:'administrator',company_id:company.id},unassigned]);
  await session(unassigned);await deny('select public.government_annual_pay_details(2026)',[],['42501']);
 }finally{await db.exec('rollback');}
 const inactive=actors.find(a=>!a.active);if(inactive){await session(inactive.user_id);await deny('select public.government_annual_pay_details(2026)',[],['42501']);}
 await session('','anon');await deny('select public.government_annual_pay_details(2026)',[],['42501']);
 await db.exec('reset role');
 for(const schema of['public','hris_private']){
  const name=schema+'.government_annual_pay_details(integer)';
  const f=(await db.query('select prosecdef,provolatile,proconfig from pg_proc where oid=$1::regprocedure',[name])).rows[0];
  eq(f.prosecdef,false,'RLS invoker '+schema);eq(f.provolatile,'s','Stable single statement '+schema);eq(f.proconfig,['search_path=""'],'Empty search path '+schema);
  eq((await db.query("select has_function_privilege('anon',$1,'execute')v",[name])).rows[0].v,false,'Anonymous denied '+schema);
 }
 // Rolled-back ledger substitutes test missing/foreign bindings without altering
 // immutable pay, receipt or audit fixtures. Their parent guards were tested above.
 const stub=async(ledger,check)=>{await db.exec('reset role');await db.exec('begin');try{
  await db.exec(`create or replace function hris_private.government_annual_ledger(p_year integer)returns jsonb language sql stable security invoker set search_path=''as $stub$select '${JSON.stringify(ledger).replaceAll("'","''")}'::jsonb;$stub$;`);
  await session(owner);await check(await call());
 }finally{await db.exec('rollback');await db.exec('reset role');}};
 const one=structuredClone(vectors[1].ledger);one.entries=one.entries.filter(e=>e.fact.kind==='monthly').slice(0,1);eq(one.entries.length,1,'Monthly missing-source fixture');one.employees=one.employees.filter(e=>e.employee_id===one.entries[0].fact.employee_id);
 const missing=structuredClone(one);missing.entries[0].fact.id='00000000-0000-0000-0000-0000000a0060';
 await stub(missing,async w=>{vectors.push(w);eq(w.details[0].source_found,false,'Absent source is explicit');eq(Object.values(w.details[0].values).every(v=>v===null),true,'Missing source has unknown fields, never zero');});
 const foreign=structuredClone(one);foreign.entries[0].fact.employee_id='00000000-0000-0000-0000-0000000b0060';
 await stub(foreign,async w=>{eq(w.details[0].source_found,false,'Same source ID cannot cross employee binding');eq(Object.values(w.details[0].values).every(v=>v===null),true,'No foreign issued amounts');});
 const empty=structuredClone(one);empty.entries=[];empty.employees=[];await stub(empty,async w=>{vectors.push(w);eq(w.details,[],'Empty source set is explicit');});
 await session(owner);eq((await call()).details,vectors[1].details,'Temporary replacements rolled back; original details unchanged');
 await db.exec('reset role');await writeFile(new URL('../../work/paydetails060-db-vectors.json',import.meta.url),JSON.stringify(vectors));
 console.log('Annual pay details: issued fields, unknown sources, no duplicated underlying income, scopes and preservation passed.');
}
