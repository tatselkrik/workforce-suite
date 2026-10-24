import{writeFile}from'node:fs/promises';
export async function runPayrollReportTests({db,eq,deny,session,company,owner,unassigned}){
 const filter={starts_on:'2026-01-01',ends_on:'2026-12-31',axis:'issued',kind:'all',employee_id:null,search:''},vectors=[];
 const call=async f=>(await db.query('select public.payroll_report($1)r',[f])).rows[0].r;
 await session(owner);const all=await call(filter);eq(all.company_id,company.id,'Report binds protected company');eq(all.version,'payroll-report-v1','Report contract version');
 eq(all.rows.length>0,true,'Mixed existing financial facts reported');
 eq([...new Set(all.rows.map(r=>r.kind))].sort(),['annual_tax','cutoff','final','monthly','thirteenth'],'All five financial sources');
 const text=JSON.stringify(all);eq(text.includes('PRIVATE_'),false,'Report excludes private evidence and notes');
 for(const name of['source_basis','worksheet','payment_reference','handling_reference','settlement_reference','tin','created_by','approved_by'])eq(all.rows.some(r=>Object.hasOwn(r,name)),false,'No private raw field '+name);
 const sums={};for(const key of['cash_cents','offset_cents','reserved_cents'])sums[key]=all.rows.reduce((n,r)=>n+r[key],0);
 await db.exec('reset role');
 const direct=(await db.query("select coalesce(sum(amount_cents),0)::bigint n from public.payroll_payment_receipts where company_id=$1 and origin<>'final_settlement'",[company.id])).rows[0].n;
 const final=(await db.query("select coalesce(sum(st.net_cents),0)::bigint n from public.payroll_final_status st where company_id=$1 and kind='settled'",[company.id])).rows[0].n;
 eq(sums.cash_cents,direct+final,'Recorded cash is direct receipts plus combined final cash once');
 eq(sums.offset_cents,(await db.query("select coalesce(sum(amount_cents),0)::bigint n from public.payroll_payment_receipts where company_id=$1 and origin='final_settlement'",[company.id])).rows[0].n,'Final source offsets retained separately from cash');
 eq(all.rows.filter(r=>r.kind==='annual_tax').every(r=>r.cash_cents===0&&r.net_cents===null),true,'Tax advice/handling is not payroll cash');
 eq(all.rows.filter(r=>r.payment_state==='final_offset').every(r=>r.cash_cents===0&&r.reserved_case_id!==null&&r.outstanding_cents===0),true,'Discharged source cannot imply another cash payment');
 eq(all.rows.filter(r=>r.kind==='final'&&r.status==='withdrawn').every(r=>r.cash_cents===0&&r.received_on===null),true,'Withdrawn statement retains history without a receipt');
 vectors.push(all);
 await session(owner);
 for(const axis of['issued','received'])for(const kind of['all','monthly','cutoff','thirteenth','annual_tax','final']){
  const f={...filter,axis,kind};const r=await call(f);eq(r.rows.every(x=>(kind==='all'||kind===x.kind)&&(axis!=='received'||x.received_on!==null)),true,'Source/date filter '+axis+' '+kind);vectors.push(r);
 }
 const first=all.rows[0];const person=await call({...filter,employee_id:first.employee_id});eq(person.rows.every(r=>r.employee_id===first.employee_id),true,'Employee filter never expands scope');vectors.push(person);
 const found=await call({...filter,search:first.employee_code});eq(found.rows.every(r=>(r.employee_code+' '+r.employee_name+' '+r.department).toLowerCase().includes(first.employee_code.toLowerCase())),true,'Search saved labels');vectors.push(found);
 const empty=await call({...filter,search:'__no_such_person_40__'});eq(empty.rows,[],'Empty report is explicit');vectors.push(empty);
 const prior=await call({...filter,starts_on:'2026-01-01',ends_on:'2026-01-01',axis:'received'});eq(prior.rows.every(r=>r.received_on==='2026-01-01'),true,'Receipt date never uses approval or batch date');vectors.push(prior);
 for(const change of[{starts_on:null},{ends_on:null},{axis:null},{kind:null},{search:null},{axis:'period'},{kind:'raw'},{search:'x'.repeat(81)},{search:'x\n'},{starts_on:'2025-12-31'},{ends_on:'2027-01-02'},{starts_on:'2026-02-01',ends_on:'2026-01-31'},{company_id:company.id},{employee_id:'00000000-0000-4000-8000-000000999999'}]){
  await deny('select public.payroll_report($1)',[{...filter,...change}],['23514','42501']);
 }
 for(const actor of[unassigned,'00000000-0000-4000-8000-000000390002','00000000-0000-4000-8000-000000390003','00000000-0000-4000-8000-000000390001']){
  await session(actor);await deny('select public.payroll_report($1)',[filter],['42501']);
 }
 await session(owner);await deny('select *from hris_private.payroll_report_rows($1)',[company.id],['42501']);
 await session('','anon');await deny('select public.payroll_report($1)',[filter],['42501']);
 await db.exec('reset role');eq((await db.query("select prosecdef from pg_proc where oid='public.payroll_report(jsonb)'::regprocedure")).rows[0].prosecdef,false,'Public report wrapper is invoker');
 eq((await db.query("select bool_and(provolatile='s')stable from pg_proc where oid in('public.payroll_report(jsonb)'::regprocedure,'hris_private.payroll_report(jsonb)'::regprocedure,'hris_private.payroll_report_rows(uuid)'::regprocedure)")).rows[0].stable,true,'All report readers use the statement snapshot');
 const original=(await db.query("select pg_get_functiondef('hris_private.payroll_report_rows(uuid)'::regprocedure)f")).rows[0].f;
 const sample=JSON.stringify(all.rows[0]).replaceAll("'","''");
 try{
  await db.exec("create or replace function hris_private.payroll_report_rows(cid uuid)returns setof jsonb language sql stable security invoker set search_path=''as $$select jsonb_set('"+sample+"'::jsonb,'{id}',to_jsonb(md5(n::text)::uuid::text))from generate_series(1,5000)n;$$;");
  await session(owner);eq((await call(filter)).rows.length,5000,'Exactly5000complete rows allowed');
  await db.exec('reset role');await db.exec("create or replace function hris_private.payroll_report_rows(cid uuid)returns setof jsonb language sql stable security invoker set search_path=''as $$select jsonb_set('"+sample+"'::jsonb,'{id}',to_jsonb(md5(n::text)::uuid::text))from generate_series(1,5001)n;$$;");
  await session(owner);await deny('select public.payroll_report($1)',[filter],['23514']);
 }finally{await db.exec('reset role');await db.exec(original);}
 eq((await db.query("select pg_get_functiondef('hris_private.payroll_report_rows(uuid)'::regprocedure)f")).rows[0].f,original,'Boundary fixture restores exact protected row projection');
 await writeFile(new URL('../../work/payroll-report40-vectors.json',import.meta.url),JSON.stringify(vectors,null,2)+'\n');
}
