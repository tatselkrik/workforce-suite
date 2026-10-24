import{writeFile}from'node:fs/promises';
export async function runGovernmentSourceTests({db,eq,deny,session,company,owner,unassigned}){
 const call=async(month,agency)=>(await db.query('select public.government_payroll_sources($1,$2)r',[month,agency])).rows[0].r;
 const agencies=['bir','sss','philhealth','pagibig'],vectors=[];
 await session(owner);
 for(const agency of agencies){
  const r=await call('2026-09-01',agency);vectors.push(r);
  eq(r.version,'government-payroll-sources-v1','Explicit source contract');eq(r.company_id,company.id,'Protected company binding');
  eq(r.month,'2026-09-01','Exact contribution/report month');eq(r.agency,agency,'Agency projection');
  eq(r.facts.every(f=>f.company_id===company.id&&f.agency===agency),true,'All facts stay in current company/agency');
  eq(new Set(r.facts.map(f=>f.kind+':'+f.id+':'+f.agency)).size,r.facts.length,'No duplicate immutable source');
  eq(JSON.stringify(r).includes('PRIVATE_'),false,'Source projection omits private sentinel/evidence');
  eq(r.facts.every(f=>Object.keys(f).sort().join(',')==='agency,company_id,contribution_month,employee_id,ends_on,id,kind,parts,received_on,starts_on,state'),true,'Only reviewed fact fields are projected');
  eq(r.facts.every(f=>Object.keys(f.parts).sort().join(',')==='ec_cents,employee_cents,employer_cents'),true,'Shares remain separate');
  if(agency!=='sss')eq(r.facts.every(f=>f.parts.ec_cents===0),true,'EC belongs to SSS only');
  if(agency==='bir')eq(r.facts.every(f=>f.parts.employer_cents===0&&f.contribution_month===null),true,'Withholding has no invented employer contribution or coverage month');
  else{
   eq(r.facts.filter(f=>['monthly','cutoff'].includes(f.kind)).every(f=>f.contribution_month==='2026-09-01'),true,'Regular contributions use earned month, not pay date');
   eq(r.facts.filter(f=>['thirteenth','amendment','final_contribution'].includes(f.kind)&&f.state!=='review_only').every(f=>f.parts.employer_cents===null),true,'Unavailable employer adjustment is unknown, never silently zero');
  }
  eq(r.facts.filter(f=>f.state==='received').every(f=>f.received_on!==null&&f.received_on<=r.as_of),true,'Received facts have an actual nonfuture date');
  eq(r.facts.filter(f=>f.state==='dated_after_as_of').every(f=>f.received_on>r.as_of),true,'Postdated receipts retain an explicit review state');
  eq(r.facts.filter(f=>!['received','dated_after_as_of'].includes(f.state)).every(f=>f.received_on===null),true,'Approval/advice/missing batch receipt never supplies a received date');
 }
 // Month-by-month BIR queries exercise direct, final-offset, supplementary and
 // settled-tax dates using the existing independent workflow fixture histories.
 for(let month=1;month<=10;month++){
  const r=await call('2026-'+String(month).padStart(2,'0')+'-01','bir');vectors.push(r);
  eq(r.facts.filter(f=>f.state==='received').every(f=>f.received_on.slice(0,7)===r.month.slice(0,7)),true,'BIR facts follow per-person actual received month '+month);
 }
 await db.exec('reset role');
 const originals=(await db.query("select s.id,s.item#>>'{totals,withholding}'amount,p.paid_on::text paid_on from public.payroll_slips s join public.payroll_payment_receipts p on p.company_id=s.company_id and p.employee_id=s.employee_id and p.kind='monthly'and p.slip_id=s.id where s.company_id=$1 and p.paid_on between date'2026-01-01'and date'2026-10-31'",[company.id])).rows;
 for(const row of originals){const r=vectors.find(v=>v.agency==='bir'&&v.month.slice(0,7)===row.paid_on.slice(0,7));eq(r.facts.find(f=>f.kind==='monthly'&&f.id===row.id)?.parts.employee_cents,Number(row.amount),'Original withholding retained once, including final-offset source');}
 const finals=(await db.query("select s.id,(s.totals#>>'{tax,adjustment_cents}')::bigint amount,st.recorded_on::text recorded_on from public.payroll_final_slips s join public.payroll_final_status st on st.company_id=s.company_id and st.id=s.id and st.kind='settled'where s.company_id=$1 and st.recorded_on between date'2026-01-01'and date'2026-10-31'",[company.id])).rows;
 for(const row of finals){const r=vectors.find(v=>v.agency==='bir'&&v.month.slice(0,7)===row.recorded_on.slice(0,7));eq(r.facts.find(f=>f.kind==='final_adjustment'&&f.id===row.id)?.parts.employee_cents,row.amount,'Final exposes only additional signed tax adjustment, not combined cash or source tax twice');}
 await session(owner);
 for(const args of[[null,'sss'],['2026-09-01',null],['2026-09-02','sss'],['2024-12-01','sss'],['2200-01-01','sss'],['2026-09-01','raw']])await deny('select public.government_payroll_sources($1,$2)',args,['23514']);
 await session(unassigned);await deny('select public.government_payroll_sources($1,$2)',['2026-09-01','sss'],['42501']);
 await db.exec('reset role');const actors=(await db.query("select user_id,role,active from public.company_members where role in('employee','manager','hr_generalist')order by user_id")).rows;
 for(const role of['employee','manager']){const a=actors.find(a=>a.role===role&&a.active);eq(!!a,true,'Existing '+role+' fixture available');await session(a.user_id);await deny('select public.government_payroll_sources($1,$2)',['2026-09-01','sss'],['42501']);}
 const hr=actors.find(a=>a.role==='hr_generalist'&&a.active);eq(!!hr,true,'Existing HR fixture available');await session(hr.user_id);eq((await call('2026-09-01','sss')).company_id,company.id,'HR can review own company only');
 const disabled=actors.find(a=>!a.active);if(disabled){await session(disabled.user_id);await deny('select public.government_payroll_sources($1,$2)',['2026-09-01','sss'],['42501']);}
 await session('','anon');await deny('select public.government_payroll_sources($1,$2)',['2026-09-01','sss'],['42501']);
 await db.exec('reset role');
 eq((await db.query("select prosecdef from pg_proc where oid='public.government_payroll_sources(date,text)'::regprocedure")).rows[0].prosecdef,false,'Public wrapper remains invoker');
 eq((await db.query("select bool_and(provolatile='s')stable from pg_proc where oid in('public.government_payroll_sources(date,text)'::regprocedure,'hris_private.government_payroll_sources(date,text)'::regprocedure)")).rows[0].stable,true,'Reader uses one statement snapshot');
 await writeFile(new URL('../../work/government-source-vectors.json',import.meta.url),JSON.stringify(vectors,null,2)+'\n');
}
