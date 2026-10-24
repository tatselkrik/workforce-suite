export async function runCrossYearTests({db,eq,deny,session,company,owner,unassigned}){
 await db.exec('reset role');
 const s={employment_start:'2020-01-01',service_end:'2025-12-31',cash_date:'2026-10-15',annual:{starts_on:'2026-01-01',service_end:'2025-12-31',as_of:'2026-10-15',income:[],prior_adjustment_cents:0,pending_payments:0,pending_adjustments:0,external_thirteenth_floor:0,other_benefits_floor:0},outstanding:[],previous_vacation_days_hundredths:0};
 const w=(await db.query('select hris_private.final_pay_empty()w')).rows[0].w;
 Object.assign(w,{entitlements_reviewed:true,source_scope_reviewed:true,contributions_reviewed:true,review_note:'PRIVATE_CROSS_REVIEW',contribution_note:'PRIVATE_CROSS_CONTRIBUTIONS',lines:[{label:'Delayed earned pay',kind:'taxable',amount_cents:30000000,reference:'PRIVATE_CROSS_RECEIPT'}]});
 Object.assign(w.tax,{final_compensation_reviewed:true,previous_employers_reviewed:true,benefits_reviewed:true,review_note:'PRIVATE_CROSS_TAX',settlement_note:'PRIVATE_CROSS_HANDLING'});
 const calc=async w=>(await db.query('select hris_private.final_pay_ready($1,$2)c',[s,w])).rows[0].c;
 await deny('select hris_private.final_pay_ready($1,$2)',[s,w],['23514']);
 w.cash_year_reviewed=true;w.cash_year_note='PRIVATE_CROSS_NO_PRIOR_CONSTRUCTIVE_RECEIPT';
 let c=await calc(w);
 eq([c.tax.taxable_cents,c.tax.annual_tax_cents,c.net_cents],[30000000,750000,29250000],'Delayed earned income taxed in supported receipt year');
 eq(c.tax.coverage_gaps,[],'Prior-year separation creates no fictional current-year service or gaps');
 eq(c.due_on,'2026-01-30','Delayed release deadline remains tied to actual separation');
 eq(c.planned_late,true,'Late cross-year release is flagged without blocking owed pay');
 await deny('select hris_private.final_pay_compute($1,$2)',[s,{...w,tax:{...w.tax,openings:[{starts_on:'2026-01-01',ends_on:'2026-01-01',taxable_cents:0,withholding_cents:0,benefit_gross_cents:0,benefit_exempt_cents:0,reference:'Invented coverage'}]}}],['23514']);
 w.tax.previous_employers=[{starts_on:'2026-01-01',ends_on:'2026-03-31',employer:'Fictional later employer',taxable_cents:20000000,withholding_cents:1000000,benefit_gross_cents:0,benefit_exempt_cents:0,reference:'PRIVATE_CROSS_CERT'}];
 c=await calc(w);eq([c.tax.taxable_cents,c.tax.annual_tax_cents,c.net_cents],[50000000,4250000,26750000],'Certified cash-year subsequent employer included without crediting prior-year withholding');
 await deny('select hris_private.final_pay_compute($1,$2)',[s,{...w,tax:{...w.tax,previous_employers:[{...w.tax.previous_employers[0],ends_on:'2026-12-31'}]}}],['23514']);
 await deny('select hris_private.final_pay_compute($1,$2)',[{...s,cash_date:'2027-01-01',annual:{...s.annual,as_of:'2027-01-01',starts_on:'2027-01-01'}},w],['23514']);
 await deny('select hris_private.final_pay_compute($1,$2)',[{...s,service_end:'2024-12-31',annual:{...s.annual,service_end:'2024-12-31'}},w],['23514']);
 await db.query("select hris_private.annual_tax_dates(2026,'termination','2025-12-31','2026-10-15')");
 await deny("select hris_private.annual_tax_dates(2026,'year_end','2025-12-31','2026-10-15')",[],['23514']);
 for(const actor of [owner,unassigned]){await session(actor);await deny('select hris_private.final_pay_ready($1,$2)',[s,w],['42501']);}
 await db.exec('reset role');
}
