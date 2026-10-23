import {writeFile}from'node:fs/promises';
export async function runThirteenthEngineTests({db,eq,deny,session,owner,unassigned}){
 await db.exec('reset role');
 const id=n=>`00000000-0000-4000-8000-${String(150000+n).padStart(12,'0')}`;
 const s={starts_on:'2026-01-01',as_of:'2026-09-30',periods:[{id:id(1),starts_on:'2026-02-01',ends_on:'2026-09-30',basic_cents:24000000}],prior:[],external_thirteenth_floor:0,other_benefits_floor:0};
 const w={openings:[{starts_on:'2026-01-01',ends_on:'2026-01-31',basic_cents:3000000,reference:'Reviewed January ledger'}],classification:'rank_and_file',eligibility_note:'Actual coverage reviewed',served_one_month:true,basic_adjustment_cents:0,adjustment_note:'',policy_topup_cents:0,policy_note:'',external_thirteenth_cents:0,other_benefits_cents:0,opening_note:'',requested_gross_cents:null,withholding_cents:0,tax_reviewed:true,tax_note:'Supplementary withholding reviewed',sss_adjustment_cents:0,philhealth_adjustment_cents:0,pagibig_adjustment_cents:0,contributions_reviewed:true,contribution_note:'Monthly contribution reconciliation reviewed',reviewed:true,review_note:'Sources reconciled'};
 const compute=async(s,w)=>(await db.query('select hris_private.thirteenth_compute($1,$2)c',[JSON.stringify(s),JSON.stringify(w)])).rows[0].c;
 const ready=async(s,w)=>(await db.query('select hris_private.thirteenth_ready($1,$2)c',[JSON.stringify(s),JSON.stringify(w)])).rows[0].c;
 const no=(s,w,fn='compute')=>deny(`select hris_private.thirteenth_${fn}($1,$2)`,[JSON.stringify(s),JSON.stringify(w)],['23514']);
 const c=await ready(s,w);eq([c.basic_cents,c.entitlement_cents,c.net_cents],[27000000,2250000,2250000],'13th entitlement uses earned basic/openings, not salary projections');
 const missing=await compute(s,{...w,openings:[]});eq(missing.coverage_gaps,[{starts_on:'2026-01-01',ends_on:'2026-01-31'}],'Missing January is visible');await no(s,{...w,openings:[]},'ready');
 await no(s,{...w,openings:[{...w.openings[0],ends_on:'2026-02-01'}]});await no({...s,periods:[...s.periods,...s.periods]},w);
 await no(s,{...w,openings:[{...w.openings[0],reference:''}]});await no(s,{...w,requested_gross_cents:2250001});
 for(const bad of[{classification:'manager'},{classification:'rank_and_file',served_one_month:false},{classification:''},{tax_reviewed:false},{tax_note:''},{contributions_reviewed:false},{contribution_note:''},{reviewed:false},{eligibility_note:''}])await no(s,{...w,...bad},'ready');
 await no(s,{...w,basic_adjustment_cents:-1});await no(s,{...w,basic_adjustment_cents:-27000001,adjustment_note:'Invalid negative total'});await no(s,{...w,policy_topup_cents:1});await no(s,{...w,withholding_cents:2250001});await no(s,{...w,tax_reviewed:'true'});await no(s,{...w,unexpected:true});
 await no({...s,external_thirteenth_floor:100},w);await no({...s,other_benefits_floor:100},w);
 const prior={...s,prior:[{id:id(2),gross_cents:1000000,exempt_cents:1000000}]};const p=await ready(prior,w);eq([p.prior_company_cents,p.gross_cents],[1000000,1250000],'Earlier issued payment cannot pay twice');
 const shared=await ready(prior,{...w,other_benefits_cents:7500000,opening_note:'Reviewed other benefits'});eq([shared.gross_cents,shared.exempt_cents,shared.taxable_cents],[1250000,500000,750000],'One annual90k cap shared with prior payments and other benefits');
 const over={...s,prior:[{id:id(2),gross_cents:3000000,exempt_cents:3000000}]};eq((await compute(over,w)).overpayment_cents,750000,'Overpayment is visible without wage recovery');await no(over,w,'ready');
 const vectors=[];const add=async(source,worksheet)=>vectors.push({source,worksheet,result:await compute(source,worksheet)});
 await add(s,w);await add(s,{...w,openings:[]});await add(s,{...w,openings:[{...w.openings[0],basic_cents:0,reference:'Reviewed zero interval'}]});await add(prior,w);await add(prior,{...w,other_benefits_cents:7500000,opening_note:'Reviewed shared benefits'});await add(over,w);
 for(const cents of[0,1,5,6,11,12])await add(s,{...w,openings:[{...w.openings[0],basic_cents:3000000+cents}]});
 for(const benefits of[8999999,9000000,9000001])await add(s,{...w,other_benefits_cents:benefits,opening_note:'Boundary reviewed'});
 await add(s,{...w,requested_gross_cents:100000,withholding_cents:10000,sss_adjustment_cents:1000});await add(s,{...w,policy_topup_cents:10000,policy_note:'More favorable company policy'});await add(s,{...w,basic_adjustment_cents:-3000000,adjustment_note:'Documented basic correction'});
 await writeFile(new URL('../../work/thirteenth-engine-vectors.json',import.meta.url),JSON.stringify(vectors,null,2));
 for(const actor of[owner,unassigned]){await session(actor);for(const fn of['compute','ready'])await deny(`select hris_private.thirteenth_${fn}($1,$2)`,[JSON.stringify(s),JSON.stringify(w)],['42501']);}await db.exec('reset role');
 eq((await db.query("select count(*)::int n from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='hris_private'and p.proname like 'thirteenth_%'and(has_function_privilege('anon',p.oid,'EXECUTE')or has_function_privilege('authenticated',p.oid,'EXECUTE'))")).rows[0].n,0,'Private 13th calculation engine has no client execution grants');
}
