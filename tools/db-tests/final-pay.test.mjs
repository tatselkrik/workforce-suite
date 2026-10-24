import {writeFile}from'node:fs/promises';
export async function runFinalPayEngineTests({db,eq,deny,session,owner,unassigned}){
 await db.exec('reset role');
 const row={id:'monthly:paid',starts_on:'2026-01-01',ends_on:'2026-08-31',taxable_cents:20000000,withholding_cents:1000000,benefit_gross_cents:0,benefit_exempt_cents:0,contribution_cents:0};
 const owed={...row,id:'cutoff:owed',starts_on:'2026-09-01',ends_on:'2026-09-30',taxable_cents:10000000};
 const s={employment_start:'2020-01-01',service_end:'2026-09-30',cash_date:'2026-10-15',annual:{starts_on:'2026-01-01',service_end:'2026-09-30',as_of:'2026-10-15',income:[row],prior_adjustment_cents:0,pending_payments:1,pending_adjustments:0,external_thirteenth_floor:0,other_benefits_floor:0},outstanding:[{id:owed.id,income:owed,net_cents:9000000}],previous_vacation_days_hundredths:0};
 const w=(await db.query('select hris_private.final_pay_empty()w')).rows[0].w;
 Object.assign(w,{entitlements_reviewed:true,source_scope_reviewed:true,contributions_reviewed:true,review_note:'PRIVATE_FINAL_REVIEW',contribution_note:'PRIVATE_FINAL_CONTRIBUTIONS'});
 Object.assign(w.tax,{final_compensation_reviewed:true,previous_employers_reviewed:true,benefits_reviewed:true,review_note:'PRIVATE_FINAL_TAX_REVIEW',settlement_note:'PRIVATE_FINAL_HANDLING'});
 const vectors=[];const calc=async(a,b)=>(await db.query('select hris_private.final_pay_compute($1,$2)c',[a,b])).rows[0].c;
 const add=async(a,b)=>{const c=await calc(a,b);vectors.push({source:structuredClone(a),worksheet:structuredClone(b),result:c});return c;};
 const bad=(a,b,fn='compute')=>deny(`select hris_private.final_pay_${fn}($1,$2)`,[a,b],['23514']);
 let c=await add(s,w);eq([c.source_net_cents,c.tax.taxable_cents,c.tax.current_withheld_cents,c.tax.refund_cents,c.net_cents],[9000000,30000000,2000000,1250000,10250000],'Final cash includes unpaid salary once and prospective annual refund');eq(c.due_on,'2026-10-30','Due date follows separation, not clearance');
 const v={days_hundredths:1300,daily_cents:100000,previous_days_hundredths:0,reference:'PRIVATE_VACATION'};
 c=await add(s,{...w,vacation:v});eq([c.vacation_gross_cents,c.vacation_exempt_cents,c.vacation_shared_benefit_cents,c.tax.taxable_cents,c.net_cents],[1300000,1200000,100000,30000000,11550000],'Twelve-day vacation exclusion and excess shared with annual90k');
 const cap={...row,id:'benefit:paid',starts_on:null,ends_on:null,taxable_cents:0,withholding_cents:0,benefit_gross_cents:9000000,benefit_exempt_cents:9000000};
 const capped={...s,annual:{...s.annual,income:[row,cap]}};
 c=await add(capped,{...w,vacation:v});eq([c.tax.taxable_cents,c.net_cents],[30100000,11535000],'Earlier13th uses same90k exclusion as excess vacation');
 c=await add(capped,{...w,vacation:{...v,previous_days_hundredths:1250}});eq([c.vacation_exempt_cents,c.tax.taxable_cents,c.net_cents],[0,31300000,11355000],'Earlier calendar-year conversion consumes de-minimis days');
 for(const days of[0,1,50,1199,1200,1201,36599,36600])await add(s,{...w,vacation:{...v,days_hundredths:days,daily_cents:123457}});
 const sep={cause:'retrenchment',monthly_basic_cents:3000000,regular_allowances_cents:500000,total_award_cents:12250000,previously_paid_cents:2250000,tax_exempt:true,reference:'PRIVATE_SEPARATION'};
 c=await add(s,{...w,separation:sep});eq([c.credited_service_years,c.separation_floor_cents,c.separation_cents,c.net_cents],[7,12250000,10000000,20250000],'Separation uses latest basic plus regular allowances and deducts earlier award payments');
 await bad(s,{...w,separation:{...sep,total_award_cents:12249999}});
 for(const cause of['redundancy','labor_saving_devices']){c=await add(s,{...w,separation:{...sep,cause,total_award_cents:24500000,previously_paid_cents:0}});eq(c.separation_floor_cents,24500000,'One-month per credited year for redundancy/labor saving');}
 for(const cause of['resignation','just_cause','company_policy']){await bad(s,{...w,separation:{...sep,cause}});await add(s,{...w,separation:{...sep,cause,tax_exempt:false}});}
 for(const[lo,hi,full,credited]of[['2026-01-01','2026-06-29',0,0],['2026-01-01','2026-06-30',0,1],['2020-02-29','2026-02-26',5,6],['2020-02-29','2026-02-27',6,6],['2021-01-01','2026-09-30',5,6]])eq((await db.query('select hris_private.final_pay_service($1,$2)s',[lo,hi])).rows[0].s,{full,credited},'Inclusive calendar anniversaries and exact six-month rounding');
 const ret={...w.retirement,method:'statutory',daily_basic_cents:100000,plan_award_cents:14000000,previously_paid_cents:500000,birth_date:'1966-01-01',ordinary_statutory_coverage:true,tax_treatment:'statutory_no_plan',reference:'PRIVATE_RETIREMENT'};
 c=await add({...s,employment_start:'2021-01-01'},{...w,retirement:ret});eq([c.retirement_floor_cents,c.retirement_cents,c.net_cents],[13500000,13500000,23750000],'Actual five-year service and22.5daily units per credited year');
 await bad({...s,employment_start:'2022-01-01'},{...w,retirement:ret});await bad(s,{...w,retirement:{...ret,employee_plan_covered:true}});
 await add(s,{...w,retirement:{...ret,employee_plan_covered:true,tax_treatment:'taxable'}});
 const qualified={...ret,method:'plan',employee_plan_covered:true,tax_treatment:'qualified_plan',qualified_plan_reviewed:true,first_qualified_availment:true,birth_date:'1976-01-01'};
 await add({...s,employment_start:'2010-01-01'},{...w,retirement:qualified});
 for(const change of[{first_qualified_availment:false},{qualified_plan_reviewed:false},{employee_plan_covered:false},{birth_date:'1977-01-01'}])await bad({...s,employment_start:'2010-01-01'},{...w,retirement:{...qualified,...change}});
 const short={...s,employment_start:'2026-08-01',annual:{...s.annual,starts_on:'2026-08-01',income:[{...row,starts_on:'2026-08-01'}]}};
 c=await add(short,{...w,separation:{...sep,total_award_cents:3500000,previously_paid_cents:0}});eq(c.separation_floor_cents,3500000,'At least one month even with less than six months service');
 const deficient={...s,annual:{...s.annual,income:[{...row,taxable_cents:40000000,withholding_cents:0}]},outstanding:[{id:owed.id,income:{...owed,withholding_cents:0},net_cents:100000}]};
 c=await add(deficient,w);eq([c.tax.annual_tax_cents,c.tax.available_deduction_cents,c.tax.employer_advance_cents,c.net_cents],[4250000,100000,4150000,0],'Insufficient final cash is separate employer handling, never automatic employee debt');
 const line={label:'Evidenced amount',kind:'deposit_return',amount_cents:1000000,reference:'PRIVATE_AMOUNT'};
 c=await add(deficient,{...w,lines:[line,{...line,kind:'lawful_deduction',amount_cents:100000}],sss_adjustment_cents:50000});eq([c.tax.taxable_cents,c.cash_before_tax_cents,c.tax.employer_advance_cents],[49950000,950000,3290000],'Contributions removed once, deposits not income and lawful deductions not tax offsets');
 for(const kind of['taxable','shared_benefit','lawful_deduction'])await add(s,{...w,lines:[{...line,kind}]});
 await bad(s,{...w,lines:[{...line,kind:'unknown'}]});await bad(s,{...w,lines:[{...line,reference:''}]});await bad(s,{...w,lines:[{...line,kind:'lawful_deduction',amount_cents:10000000}]});
 await bad({...s,annual:{...s.annual,pending_adjustments:1}},w,'ready');await bad({...s,outstanding:[]},w);await bad({...s,outstanding:[...s.outstanding,...s.outstanding],annual:{...s.annual,pending_payments:2}},w);
 await bad({...s,previous_vacation_days_hundredths:100},w);await bad(s,{...w,release_within_days:31});await bad(s,{...w,release_within_days:0});
 for(const k of['entitlements_reviewed','source_scope_reviewed','contributions_reviewed']){await bad(s,{...w,[k]:false},'ready');await bad(s,{...w,[k]:null});}
 for(const k of['final_compensation_reviewed','previous_employers_reviewed','benefits_reviewed'])await bad(s,{...w,tax:{...w.tax,[k]:false}},'ready');
 for(const changed of[{cash_date:'2027-01-01'},{service_end:'2025-12-31'},{employment_start:'2026-10-01'},{extra:true}])await bad({...s,...changed},w);
 await bad(s,{...w,extra:true});await bad(s,{...w,vacation:{...v,days_hundredths:1.5}});await bad(s,{...w,separation:{...sep,unknown:true}});await bad(s,{...w,retirement:{...ret,qualified_plan_reviewed:null}});
 c=await add({...s,cash_date:'2026-11-01',annual:{...s.annual,as_of:'2026-11-01'}},w);eq(c.planned_late,true,'Late planned release warns without refusing a correct late record');
 eq((await db.query('select hris_private.final_pay_ready($1,$2)c',[s,w])).rows[0].c,await calc(s,w),'Readiness keeps the exact prospective calculation');
 await writeFile(new URL('../../work/final-pay-engine-vectors.json',import.meta.url),JSON.stringify(vectors,null,2));
 for(const actor of[owner,unassigned]){await session(actor);for(const fn of['compute','ready'])await deny(`select hris_private.final_pay_${fn}($1,$2)`,[s,w],['42501']);await deny('select hris_private.final_pay_empty()',[],['42501']);}
 await db.exec('reset role');
}
