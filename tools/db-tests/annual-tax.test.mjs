import {writeFile}from'node:fs/promises';
export async function runAnnualTaxEngineTests({db,eq,deny,session,owner,unassigned}){
 await db.exec('reset role');
 const row={id:'monthly:synthetic-one',starts_on:'2026-07-01',ends_on:'2026-09-30',taxable_cents:30000000,withholding_cents:1500000,benefit_gross_cents:0,benefit_exempt_cents:0,contribution_cents:0};
 const s={starts_on:'2026-07-01',service_end:'2026-09-30',as_of:'2026-09-30',income:[row],prior_adjustment_cents:0,pending_payments:0,pending_adjustments:0,external_thirteenth_floor:0,other_benefits_floor:0};
 const w={openings:[],previous_employers:[],reclassified_benefit_cents:0,classification_note:'',taxable_adjustment_cents:0,adjustment_note:'',last_pay_available_cents:500000,settlement_note:'PRIVATE_TAX_SETTLEMENT',final_compensation_reviewed:true,previous_employers_reviewed:true,benefits_reviewed:true,review_note:'PRIVATE_TAX_REVIEW'};
 const prior={starts_on:'2026-01-01',ends_on:'2026-06-30',employer:'Synthetic former employer',taxable_cents:30000000,withholding_cents:1500000,benefit_gross_cents:0,benefit_exempt_cents:0,reference:'PRIVATE_TAX_2316'};
 const calc=async(a,b)=>(await db.query('select hris_private.annual_tax_compute($1,$2)c',[a,b])).rows[0].c;
 const bad=(a,b,fn='compute')=>deny(`select hris_private.annual_tax_${fn}($1,$2)`,[a,b],['23514']);
 const vectors=[];const add=async(a,b)=>{const c=await calc(a,b);vectors.push({source:structuredClone(a),worksheet:structuredClone(b),result:c});return c;};
 for(const[t,expected]of[[0,0],[24999999,0],[25000000,0],[25000001,0],[25000004,1],[39999999,2250000],[40000000,2250000],[40000005,2250001],[80000000,10250000],[80000002,10250001],[200000000,40250000],[200000002,40250001],[800000000,220250000],[800000002,220250001],[1000000000,290250000]]){
  eq((await db.query('select hris_private.annual_tax_amount($1)n',[t])).rows[0].n,expected,'Annual2023+brackets and once-only centavo rounding');
  await add({...s,income:[{...row,taxable_cents:t,withholding_cents:0}]},w);
 }
 let c=await add(s,w);eq([c.annual_tax_cents,c.refund_cents,c.additional_withholding_cents],[750000,750000,0],'Actual annual tax over-withholding is a refund, not zero-clamped');
 c=await add(s,{...w,previous_employers:[prior]});eq([c.taxable_cents,c.annual_tax_cents,c.additional_withholding_cents,c.available_deduction_cents,c.employer_advance_cents],[60000000,6250000,3250000,500000,2750000],'Prior-employer net2316tax/withholding and insufficient last cash');
 const benefit={...row,id:'thirteenth:synthetic-one',starts_on:null,ends_on:null,taxable_cents:0,withholding_cents:0,benefit_gross_cents:6000000,benefit_exempt_cents:6000000};
 c=await add({...s,income:[row,benefit]},{...w,previous_employers:[{...prior,benefit_gross_cents:6000000,benefit_exempt_cents:6000000}]});
 eq([c.benefit_exempt_cents,c.benefit_taxable_cents,c.taxable_cents,c.annual_tax_cents,c.additional_withholding_cents],[9000000,3000000,63000000,6850000,3850000],'One90kcap across employers and current13th; benefit income never added twice');
 c=await add(s,{...w,reclassified_benefit_cents:3000000,classification_note:'PRIVATE_TAX_BENEFIT_RECLASS'});eq([c.taxable_cents,c.refund_cents],[27000000,1200000],'Existing taxable benefits are reclassified without adding income');
 c=await add({...s,income:[{...row,taxable_cents:0,withholding_cents:0}, {...benefit,contribution_cents:50000}]},w);eq(c.taxable_cents,0,'MWE/exempt benefits do not create negative taxable compensation from mandatory contributions');
 c=await add({...s,prior_adjustment_cents:-750000},w);eq(c.adjustment_cents,0,'Earlier settled refund cannot be refunded twice');
 c=await add({...s,prior_adjustment_cents:100000},w);eq(c.refund_cents,850000,'Earlier additional withholding remains an annual credit');
 const open={starts_on:s.starts_on,ends_on:s.service_end,taxable_cents:30000000,withholding_cents:1500000,benefit_gross_cents:0,benefit_exempt_cents:0,reference:'PRIVATE_TAX_OPENING'};
 await add({...s,income:[]},{...w,openings:[open]});await add({...s,income:[]},w);
 await add({...s,income:[],external_thirteenth_floor:100000,other_benefits_floor:200000},{...w,openings:[{...open,benefit_gross_cents:300000,benefit_exempt_cents:300000}]});
 for(const correction of[-100000,100000])await add(s,{...w,taxable_adjustment_cents:correction,adjustment_note:'PRIVATE_TAX_CORRECTION'});
 await add({...s,income:[row,{...row,id:'cutoff:synthetic-one',starts_on:null,ends_on:null,taxable_cents:1,withholding_cents:0}]},w);
 c=await add({...s,income:[row,{...benefit,taxable_cents:6000000,benefit_gross_cents:15000000,benefit_exempt_cents:9000000,contribution_cents:50000}]},w);eq([c.taxable_cents,c.annual_tax_cents,c.additional_withholding_cents],[35950000,1642500,142500],'Additional mandatory13thcontributions reduce taxable exactly once');
 eq((await db.query('select hris_private.annual_tax_ready($1,$2)c',[s,w])).rows[0].c,await calc(s,w),'Readiness returns the exact approved calculation');
 await bad({...s,income:[]},w,'ready');await bad({...s,pending_payments:1},w,'ready');await bad({...s,pending_adjustments:1},w,'ready');await bad({...s,other_benefits_floor:1},w,'ready');
 for(const k of['final_compensation_reviewed','previous_employers_reviewed','benefits_reviewed']){await bad(s,{...w,[k]:false},'ready');await bad(s,{...w,[k]:null});}
 await bad(s,{...w,openings:[open]});await bad({...s,income:[row,row]},w);await bad(s,{...w,previous_employers:[prior,prior]});
 await bad(s,{...w,previous_employers:[{...prior,ends_on:'2026-07-01'}]});await bad(s,{...w,previous_employers:[{...prior,reference:''}]});await bad(s,{...w,extra:true});await bad({...s,extra:true},w);
 await bad(s,{...w,taxable_adjustment_cents:-30000001,adjustment_note:'Beyond all taxable income'});await bad(s,{...w,reclassified_benefit_cents:30000001,classification_note:'Invalid'});await bad(s,{...w,last_pay_available_cents:-1});await bad({...s,pending_payments:-1},w);
 await bad({...s,income:[]},{...w,openings:[{...open,starts_on:null}]});await bad({...s,income:[{...row,ends_on:null}]},w);await bad({...s,income:[{...row,taxable_cents:1.5}]},w);
 await writeFile(new URL('../../work/annual-tax-engine-vectors.json',import.meta.url),JSON.stringify(vectors,null,2));
 for(const actor of[owner,unassigned]){await session(actor);for(const fn of['compute','ready'])await deny(`select hris_private.annual_tax_${fn}($1,$2)`,[s,w],['42501']);await deny('select hris_private.annual_tax_amount(30000000)',[],['42501']);}
 await db.exec('reset role');
 eq((await db.query("select count(*)::int n from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='public'and p.prosecdef")).rows[0].n,0,'All exposedRPCs remain invoker functions');
}
