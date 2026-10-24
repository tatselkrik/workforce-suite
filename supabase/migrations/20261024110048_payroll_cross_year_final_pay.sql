-- Reviewed2026 cash-year final pay, retaining actual prior-year service and immutable earlier slips.

begin;

-- Keep the legacy worksheet shape readable by older installed clients.
alter table public.payroll_final_cases add column cash_year_reviewed boolean not null default false;
alter table public.payroll_final_cases add column cash_year_note text not null default ''check(char_length(cash_year_note)<=2000);

create or replace function hris_private.annual_tax_dates(yr integer,kind text,service_end date,asof date)returns void language plpgsql immutable security invoker set search_path=''as $$
begin
 if yr is null or yr not between 2026 and 2200 or service_end is null or asof is null or extract(year from service_end)<1900 or extract(year from asof)<>yr or service_end>asof
 or kind is null or(kind<>'termination'and(kind<>'year_end'or service_end<>make_date(yr,12,31)or asof<>service_end))then raise exception 'Use a same-year service/payment period; year-end closes31December.'using errcode='23514';end if;
end;$$;

create or replace function hris_private.annual_tax_compute(s jsonb,w jsonb)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare maximum constant bigint:=100000000000;cap constant bigint:=9000000;
 st date;fin date;asof date;lo date;hi date;d date;gs date;ge date;covered date[]:='{}';prior_dates date[]:='{}';ids text[]:='{}';pid text;
 p jsonb;k text;gaps jsonb:='[]';v bigint;current_taxable bigint:=0;current_withheld bigint:=0;prior_taxable bigint:=0;prior_withheld bigint:=0;
 current_benefits bigint:=0;opening_benefits bigint:=0;prior_benefits bigint:=0;taxed_benefits bigint:=0;current_taxed_benefits bigint:=0;contributions bigint:=0;
 reclassified bigint;correction bigint;previous_adjustment bigint;gross bigint;exempt bigint;taxable_benefits bigint;benefit_adjustment bigint;taxable bigint;due bigint;adjustment bigint;additional bigint;refund bigint;available bigint;unreconciled bigint;
begin
 if jsonb_typeof(s)is distinct from 'object'or s-array['starts_on','service_end','as_of','income','prior_adjustment_cents','pending_payments','pending_adjustments','external_thirteenth_floor','other_benefits_floor']<>'{}'
 or jsonb_typeof(w)is distinct from 'object'or octet_length(w::text)>131072 or w-array['openings','previous_employers','reclassified_benefit_cents','classification_note','taxable_adjustment_cents','adjustment_note','last_pay_available_cents','settlement_note','final_compensation_reviewed','previous_employers_reviewed','benefits_reviewed','review_note']<>'{}'then raise exception 'Use supported annual tax source/worksheet fields.'using errcode='23514';end if;
 st:=hris_private.thirteenth_date(s,'starts_on');fin:=hris_private.thirteenth_date(s,'service_end');asof:=hris_private.thirteenth_date(s,'as_of');
 if extract(year from asof)not between 2026 and 2200 or extract(year from st)<>extract(year from asof)or extract(year from fin)<1900 or(st>fin and extract(year from fin)>=extract(year from asof)) or fin>asof then raise exception 'Use a bounded current-year employment/payment period.'using errcode='23514';end if;
 if jsonb_typeof(s->'income')is distinct from 'array'or jsonb_array_length(s->'income')>120 or jsonb_typeof(w->'openings')is distinct from 'array'or jsonb_array_length(w->'openings')>32 or jsonb_typeof(w->'previous_employers')is distinct from 'array'or jsonb_array_length(w->'previous_employers')>8 then raise exception 'Too many annual tax source/opening/employer rows.'using errcode='23514';end if;
 foreach k in array array['classification_note','adjustment_note','settlement_note','review_note']loop perform hris_private.payroll_text(w,k,2000);end loop;
 foreach k in array array['final_compensation_reviewed','previous_employers_reviewed','benefits_reviewed']loop if jsonb_typeof(w->k)is distinct from 'boolean'then raise exception 'Explicit annual tax review booleans are required.'using errcode='23514';end if;end loop;
 foreach k in array array['external_thirteenth_floor','other_benefits_floor']loop perform hris_private.payroll_amount(s,k,maximum);end loop;
 foreach k in array array['pending_payments','pending_adjustments']loop perform hris_private.payroll_amount(s,k,4294967295);end loop;
 reclassified:=hris_private.payroll_amount(w,'reclassified_benefit_cents',maximum);perform hris_private.payroll_amount(w,'last_pay_available_cents',maximum);
 correction:=hris_private.annual_tax_signed(w,'taxable_adjustment_cents');previous_adjustment:=hris_private.annual_tax_signed(s,'prior_adjustment_cents');
 if(reclassified<>0 and btrim(w->>'classification_note')='')or(correction<>0 and btrim(w->>'adjustment_note')='')then raise exception 'Evidence is required for benefit reclassification and taxable corrections.'using errcode='23514';end if;
 for p in select value from jsonb_array_elements(s->'income')loop
  if jsonb_typeof(p)is distinct from 'object'or p-array['id','starts_on','ends_on','taxable_cents','withholding_cents','benefit_gross_cents','benefit_exempt_cents','contribution_cents']<>'{}'then raise exception 'Unsupported paid tax source fields.'using errcode='23514';end if;
  pid:=hris_private.payroll_text(p,'id',80,true);if pid=any(ids)then raise exception 'Duplicate/missing paid source identity.'using errcode='23514';end if;ids:=array_append(ids,pid);
  perform hris_private.annual_tax_income(p);contributions:=contributions+hris_private.payroll_amount(p,'contribution_cents',maximum);
  if not(p?'starts_on')or not(p?'ends_on')or(p->>'starts_on'is null)<>(p->>'ends_on'is null)then raise exception 'Paid coverage requires both dates, or neither.'using errcode='23514';end if;
  current_taxable:=current_taxable+(p->>'taxable_cents')::bigint;current_withheld:=current_withheld+(p->>'withholding_cents')::bigint;
  current_benefits:=current_benefits+(p->>'benefit_gross_cents')::bigint;taxed_benefits:=taxed_benefits+(p->>'benefit_gross_cents')::bigint-(p->>'benefit_exempt_cents')::bigint;
 end loop;
 for p in select value from jsonb_array_elements(w->'openings')loop
  if jsonb_typeof(p)is distinct from 'object'or p-array['starts_on','ends_on','taxable_cents','withholding_cents','benefit_gross_cents','benefit_exempt_cents','reference']<>'{}'then raise exception 'Unsupported current-employer opening fields.'using errcode='23514';end if;
  perform hris_private.thirteenth_date(p,'starts_on');perform hris_private.thirteenth_date(p,'ends_on');
  perform hris_private.payroll_text(p,'reference',500,true);perform hris_private.annual_tax_income(p);
  current_taxable:=current_taxable+(p->>'taxable_cents')::bigint;current_withheld:=current_withheld+(p->>'withholding_cents')::bigint;
  opening_benefits:=opening_benefits+(p->>'benefit_gross_cents')::bigint;taxed_benefits:=taxed_benefits+(p->>'benefit_gross_cents')::bigint-(p->>'benefit_exempt_cents')::bigint;
 end loop;
 current_taxed_benefits:=taxed_benefits;
 for p in select value from jsonb_array_elements((s->'income')||(w->'openings'))loop
  if p->>'starts_on'is null then continue;end if;
  lo:=hris_private.thirteenth_date(p,'starts_on');hi:=hris_private.thirteenth_date(p,'ends_on');
  if lo<st or hi>fin or lo>hi then raise exception 'Paid/opening intervals must fit the current employment period.'using errcode='23514';end if;
  for d in select g::date from pg_catalog.generate_series(lo::timestamp,hi::timestamp,interval '1day')g loop
   if d=any(covered)then raise exception 'Opening and paid payroll coverage cannot overlap.'using errcode='23514';end if;covered:=array_append(covered,d);
  end loop;
 end loop;
 for p in select value from jsonb_array_elements(w->'previous_employers')loop
  if jsonb_typeof(p)is distinct from 'object'or p-array['starts_on','ends_on','employer','taxable_cents','withholding_cents','benefit_gross_cents','benefit_exempt_cents','reference']<>'{}'then raise exception 'Unsupported prior-employer certificate fields.'using errcode='23514';end if;
  perform hris_private.payroll_text(p,'employer',120,true);perform hris_private.payroll_text(p,'reference',500,true);perform hris_private.annual_tax_income(p);
  lo:=hris_private.thirteenth_date(p,'starts_on');hi:=hris_private.thirteenth_date(p,'ends_on');
  if extract(year from lo)<>extract(year from asof)or lo>hi or(case when fin<st then hi>asof else hi>=st end) then raise exception 'Use same-year successive employment before the current hire; concurrent employers need separate review.'using errcode='23514';end if;
  for d in select g::date from pg_catalog.generate_series(lo::timestamp,hi::timestamp,interval '1day')g loop
   if d=any(prior_dates)then raise exception 'Prior-employer intervals cannot overlap.'using errcode='23514';end if;prior_dates:=array_append(prior_dates,d);
  end loop;
  prior_taxable:=prior_taxable+(p->>'taxable_cents')::bigint;prior_withheld:=prior_withheld+(p->>'withholding_cents')::bigint;
  prior_benefits:=prior_benefits+(p->>'benefit_gross_cents')::bigint;taxed_benefits:=taxed_benefits+(p->>'benefit_gross_cents')::bigint-(p->>'benefit_exempt_cents')::bigint;
 end loop;
 if reclassified>current_taxable-current_taxed_benefits then raise exception 'Reclassification exceeds current compensation still counted as regular taxable income.'using errcode='23514';end if;
 current_benefits:=current_benefits+opening_benefits+reclassified;gross:=current_benefits+prior_benefits;exempt:=least(gross,cap);taxable_benefits:=gross-exempt;
 benefit_adjustment:=taxable_benefits-taxed_benefits-reclassified;taxable:=current_taxable+prior_taxable+benefit_adjustment+correction;
 if taxable<0 then raise exception 'Corrections exceed annual taxable compensation.'using errcode='23514';end if;taxable:=greatest(taxable-contributions,0);due:=hris_private.annual_tax_amount(taxable);
 adjustment:=due-current_withheld-prior_withheld-previous_adjustment;additional:=greatest(adjustment,0);refund:=greatest(-adjustment,0);available:=least(additional,(w->>'last_pay_available_cents')::bigint);
 unreconciled:=greatest((s->>'external_thirteenth_floor')::bigint-opening_benefits,(s->>'external_thirteenth_floor')::bigint+(s->>'other_benefits_floor')::bigint-opening_benefits-prior_benefits-reclassified,0);
 foreach v in array array[current_taxable,prior_taxable,gross,current_withheld,prior_withheld,additional,refund,contributions]loop if v not between 0 and maximum then raise exception 'Annual tax totals exceed the supported amount.'using errcode='23514';end if;end loop;
 for d in select g::date from pg_catalog.generate_series(st::timestamp,fin::timestamp,interval '1day')g loop
  if not(d=any(covered))then if gs is null then gs:=d;end if;ge:=d;
  elsif gs is not null then gaps:=gaps||jsonb_build_array(jsonb_build_object('starts_on',gs,'ends_on',ge));gs:=null;end if;
 end loop;
 if gs is not null then gaps:=gaps||jsonb_build_array(jsonb_build_object('starts_on',gs,'ends_on',ge));end if;
 return jsonb_build_object('current_taxable_cents',current_taxable,'prior_taxable_cents',prior_taxable,'benefit_gross_cents',gross,'benefit_exempt_cents',exempt,'benefit_taxable_cents',taxable_benefits,'benefit_basis_adjustment_cents',benefit_adjustment,'taxable_adjustment_cents',correction,'taxable_cents',taxable,'annual_tax_cents',due,'current_withheld_cents',current_withheld,'prior_withheld_cents',prior_withheld,'prior_adjustment_cents',previous_adjustment,'adjustment_cents',adjustment,'additional_withholding_cents',additional,'refund_cents',refund,'available_deduction_cents',available,'employer_advance_cents',additional-available,'unreconciled_benefits_cents',unreconciled,'coverage_gaps',gaps);
end;$$;

create or replace function hris_private.annual_tax_basis_before_final(cid uuid,eid uuid,yr integer,asof date,service_end date,exclude_id uuid)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare e public.employees;lo date;income jsonb;payments jsonb;previous jsonb;adjustment bigint;pending integer;pending_adjustments integer;ext bigint;other bigint;latest date;begin
 select * into e from public.employees where company_id=cid and id=eid;
 if e.id is null then raise exception 'Employee unavailable.'using errcode='42501';end if;
 lo:=greatest(make_date(yr,1,1),coalesce(e.joined_on,make_date(yr,1,1)));
 if e.joined_on>service_end then raise exception 'Employee joined after this service end.'using errcode='23514';end if;
 with raw as(
  select 'monthly:'||s.id::text id,s.month first,(s.month+interval '1month' - interval '1day')::date last,case when pay.id is not null then 'paid'else r.status end status,coalesce(pay.paid_on,r.paid_on) paid_on,r.revision,
   (s.item#>>'{totals,taxable}')::bigint taxable,(s.item#>>'{totals,withholding}')::bigint withheld,0::bigint gross,0::bigint exempt,0::bigint contribution,true coverage
  from public.payroll_slips s join public.payroll_runs r on r.company_id=s.company_id and r.id=s.run_id left join public.payroll_payment_receipts pay on pay.kind='monthly'and pay.slip_id=s.id where s.company_id=cid and s.employee_id=eid
  union all
  select 'cutoff:'||s.id::text,s.starts_on,s.ends_on,case when pay.id is not null then 'paid'else r.status end status,coalesce(pay.paid_on,r.paid_on) paid_on,r.revision,(s.item#>>'{totals,taxable}')::bigint,(s.item#>>'{totals,withholding}')::bigint,0,0,0,true
  from public.payroll_cutoff_slips s join public.payroll_cutoff_runs r on r.company_id=s.company_id and r.id=s.run_id left join public.payroll_payment_receipts pay on pay.kind='cutoff'and pay.slip_id=s.id where s.company_id=cid and s.employee_id=eid
  union all
  select 'thirteenth:'||s.id::text,s.as_of,s.as_of,case when pay.id is not null then 'paid'else r.status end status,coalesce(pay.paid_on,r.paid_on) paid_on,r.revision,(s.item#>>'{totals,taxable_cents}')::bigint,(s.item#>>'{totals,withholding_cents}')::bigint,(s.item#>>'{totals,gross_cents}')::bigint,(s.item#>>'{totals,exempt_cents}')::bigint,
   (s.item#>>'{totals,sss_adjustment_cents}')::bigint+(s.item#>>'{totals,philhealth_adjustment_cents}')::bigint+(s.item#>>'{totals,pagibig_adjustment_cents}')::bigint,false
  from public.payroll_thirteenth_slips s join public.payroll_thirteenth_runs r on r.company_id=s.company_id and r.id=s.run_id left join public.payroll_payment_receipts pay on pay.kind='thirteenth'and pay.slip_id=s.id where s.company_id=cid and s.employee_id=eid
 ),included as(select * from raw where last<=asof)
 select coalesce(jsonb_agg(jsonb_build_object('id',id,'starts_on',case when coverage and greatest(first,lo)<=least(last,service_end)then greatest(first,lo)end,'ends_on',case when coverage and greatest(first,lo)<=least(last,service_end)then least(last,service_end)end,'taxable_cents',taxable,'withholding_cents',withheld,'benefit_gross_cents',gross,'benefit_exempt_cents',exempt,'contribution_cents',contribution)order by id)filter(where status='paid'and extract(year from paid_on)=yr and paid_on<=asof),'[]'),
 coalesce(jsonb_agg(jsonb_build_object('id',id,'paid_on',paid_on,'status',status,'revision',revision)order by id),'[]'),count(*)filter(where status='approved')::integer
 into income,payments,pending from included;
 select coalesce(max((s.item#>>'{worksheet,external_thirteenth_cents}')::bigint),0),coalesce(max((s.item#>>'{worksheet,other_benefits_cents}')::bigint),0)into ext,other from public.payroll_thirteenth_slips s join public.payroll_thirteenth_runs r on r.company_id=s.company_id and r.id=s.run_id left join public.payroll_payment_receipts pay on pay.kind='thirteenth'and pay.slip_id=s.id where s.company_id=cid and s.employee_id=eid and s.year=yr and s.as_of<=asof and(pay.id is not null or r.status='paid')and extract(year from coalesce(pay.paid_on,r.paid_on))=yr and coalesce(pay.paid_on,r.paid_on)<=asof;
 select coalesce(jsonb_agg(jsonb_build_object('id',p.id,'revision',p.revision,'status',p.status,'as_of',p.as_of,'adjustment_cents',(p.totals->>'adjustment_cents')::bigint,'settled_on',p.settled_on)order by p.id),'[]'),
 coalesce(sum((p.totals->>'adjustment_cents')::bigint)filter(where p.status='settled'),0)::bigint,count(*)filter(where p.status='approved')::integer,max(greatest(p.as_of,coalesce(p.settled_on,p.as_of)))
 into previous,adjustment,pending_adjustments,latest from public.payroll_tax_reconciliations p where p.company_id=cid and p.employee_id=eid and p.year=yr and p.id<>exclude_id and p.status in('approved','settled');
 if latest>asof then raise exception 'Use an as-of date at least as late as the earlier issued tax close.'using errcode='23514';end if;
 return jsonb_build_object('employee',jsonb_build_object('id',e.id,'full_name',e.full_name,'employee_code',e.employee_code,'department',e.department,'joined_on',e.joined_on,'status',e.status),'payments',payments,'previous',previous,
 'calculation_source',jsonb_build_object('starts_on',lo,'service_end',service_end,'as_of',asof,'income',income,'prior_adjustment_cents',adjustment,'pending_payments',pending,'pending_adjustments',pending_adjustments,'external_thirteenth_floor',ext,'other_benefits_floor',other));
end;$$;

create or replace function hris_private.final_pay_compute(s jsonb,w jsonb)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare maximum constant bigint:=100000000000;
 st date;fin date;cash date;due date;birth date;full_years integer;credited integer;years_old integer;v jsonb;p jsonb;r jsonb;l jsonb;pending jsonb;annual jsonb;tw jsonb;tax jsonb;income jsonb;service jsonb;
 k text;ids text[]:='{}';pid text;days bigint;previous_days bigint;daily bigint;release_days bigint;factor integer;base bigint;award bigint;ordinary boolean;active boolean;exempt boolean;
 vacation bigint:=0;vacation_exempt bigint:=0;vacation_shared bigint:=0;separation_floor bigint:=0;separation bigint:=0;separation_overpaid bigint:=0;
 retirement_floor bigint:=0;retirement bigint:=0;retirement_overpaid bigint:=0;taxable bigint:=0;shared bigint:=0;non_taxable bigint:=0;deposits bigint:=0;deductions bigint:=0;contributions bigint:=0;source_net bigint:=0;extras bigint;before_tax bigint;net bigint;n bigint;
begin
 if octet_length(w::text)>262144 or octet_length(s::text)>524288 then raise exception 'Final-pay source/worksheet exceeds its limit.'using errcode='23514';end if;
 perform hris_private.final_pay_object(s,array['employment_start','service_end','cash_date','annual','outstanding','previous_vacation_days_hundredths']);
 perform hris_private.final_pay_object(w-array['cash_year_reviewed','cash_year_note'],array['tax','vacation','separation','retirement','lines','sss_adjustment_cents','philhealth_adjustment_cents','pagibig_adjustment_cents','contribution_note','release_within_days','entitlements_reviewed','source_scope_reviewed','contributions_reviewed','review_note']);
 st:=hris_private.thirteenth_date(s,'employment_start');fin:=hris_private.thirteenth_date(s,'service_end');cash:=hris_private.thirteenth_date(s,'cash_date');
 if cash<date'2026-01-06'or extract(year from cash)<>2026 or fin<date'2025-01-01'or fin>cash or st>fin then raise exception 'Use2025/2026 separation and reviewed2026 cash dates from6January; other historical or later cash years need review.'using errcode='23514';end if;
 if w?'cash_year_reviewed'then perform hris_private.final_pay_bool(w,'cash_year_reviewed');end if;
 if w?'cash_year_note'then perform hris_private.payroll_text(w,'cash_year_note',2000);end if;
 annual:=s->'annual';
 if(annual->>'as_of')::date is distinct from cash or(annual->>'service_end')::date is distinct from fin or(annual->>'starts_on')::date is distinct from greatest(st,date'2026-01-01')or jsonb_typeof(s->'outstanding')is distinct from 'array'or jsonb_array_length(s->'outstanding')>120 or hris_private.payroll_amount(annual,'pending_payments',4294967295)<>jsonb_array_length(s->'outstanding')then raise exception 'Refresh received income and outstanding final-pay scope.'using errcode='23514';end if;
 if jsonb_typeof(w->'lines')is distinct from 'array'or jsonb_array_length(w->'lines')>32 then raise exception 'Use at most32 reviewed final-pay amounts.'using errcode='23514';end if;
 release_days:=hris_private.payroll_amount(w,'release_within_days',30);if release_days<1 then raise exception 'Use a release deadline within30days of separation.'using errcode='23514';end if;due:=fin+release_days::integer;
 foreach k in array array['contribution_note','review_note']loop perform hris_private.payroll_text(w,k,2000);end loop;
 foreach k in array array['entitlements_reviewed','source_scope_reviewed','contributions_reviewed']loop perform hris_private.final_pay_bool(w,k);end loop;
 service:=hris_private.final_pay_service(st,fin);full_years:=(service->>'full')::integer;credited:=(service->>'credited')::integer;
 v:=w->'vacation';perform hris_private.final_pay_object(v,array['days_hundredths','daily_cents','previous_days_hundredths','reference']);
 days:=hris_private.payroll_amount(v,'days_hundredths',36600);previous_days:=hris_private.payroll_amount(v,'previous_days_hundredths',36600);daily:=hris_private.payroll_amount(v,'daily_cents',maximum);
 if previous_days<hris_private.payroll_amount(s,'previous_vacation_days_hundredths',36600)or(days>0 and daily=0)then raise exception 'Review the vacation daily rate and retain earlier calendar-year conversions.'using errcode='23514';end if;
 perform hris_private.payroll_text(v,'reference',1000,days>0 or previous_days>0);
 vacation:=round(daily::numeric*days/100)::bigint;vacation_exempt:=round(daily::numeric*least(days,greatest(1200-previous_days,0))/100)::bigint;vacation_shared:=vacation-vacation_exempt;shared:=vacation_shared;non_taxable:=vacation_exempt;
 p:=w->'separation';perform hris_private.final_pay_object(p,array['cause','monthly_basic_cents','regular_allowances_cents','total_award_cents','previously_paid_cents','tax_exempt','reference']);
 k:=hris_private.payroll_text(p,'cause',40,true);factor:=case when k in('redundancy','labor_saving_devices')then 2 when k in('retrenchment','closure_without_serious_losses','disease')then 1 when k in('none','resignation','just_cause','closure_serious_losses','death_or_disability','company_policy')then 0 else -1 end;
 if factor=-1 then raise exception 'Review the separation cause; an account role is not a legal classification.'using errcode='23514';end if;
 base:=hris_private.payroll_amount(p,'monthly_basic_cents',maximum)+hris_private.payroll_amount(p,'regular_allowances_cents',maximum);
 if base>maximum or(factor>0 and base=0)then raise exception 'Review latest basic plus regular allowances for separation pay.'using errcode='23514';end if;
 separation_floor:=case when factor=0 then 0 else greatest(base,round(base::numeric*credited*factor/2)::bigint)end;
 award:=hris_private.payroll_amount(p,'total_award_cents',maximum);n:=hris_private.payroll_amount(p,'previously_paid_cents',maximum);exempt:=hris_private.final_pay_bool(p,'tax_exempt');
 if award<separation_floor or(k='none'and(award<>0 or n<>0 or exempt))then raise exception 'The classified separation award must meet its statutory floor.'using errcode='23514';end if;
 if exempt and k not in('redundancy','labor_saving_devices','retrenchment','closure_without_serious_losses','closure_serious_losses','disease','death_or_disability')then raise exception 'Resignation, just cause and policy awards do not automatically qualify for separation tax exemption.'using errcode='23514';end if;
 perform hris_private.payroll_text(p,'reference',1000,k<>'none');separation:=greatest(award-n,0);separation_overpaid:=greatest(n-award,0);
 if exempt then non_taxable:=non_taxable+separation;else taxable:=taxable+separation;end if;
 r:=w->'retirement';perform hris_private.final_pay_object(r,array['method','daily_basic_cents','plan_award_cents','previously_paid_cents','birth_date','ordinary_statutory_coverage','employee_plan_covered','tax_treatment','qualified_plan_reviewed','first_qualified_availment','reference']);
 k:=hris_private.payroll_text(r,'method',20,true);if k not in('none','statutory','plan')or r->>'tax_treatment'not in('taxable','statutory_no_plan','qualified_plan')then raise exception 'Review the private-sector retirement method and tax treatment.'using errcode='23514';end if;
 daily:=hris_private.payroll_amount(r,'daily_basic_cents',maximum);award:=hris_private.payroll_amount(r,'plan_award_cents',maximum);n:=hris_private.payroll_amount(r,'previously_paid_cents',maximum);
 foreach pid in array array['ordinary_statutory_coverage','employee_plan_covered','qualified_plan_reviewed','first_qualified_availment']loop perform hris_private.final_pay_bool(r,pid);end loop;
 birth:=null;years_old:=null;
 if r->>'birth_date'is not null then birth:=hris_private.thirteenth_date(r,'birth_date');if birth>fin or extract(year from birth)<1900 then raise exception 'Review retirement birth details.'using errcode='23514';end if;years_old:=extract(year from age(fin,birth))::integer;end if;
 active:=k<>'none';ordinary:=(r->>'ordinary_statutory_coverage')::boolean and full_years>=5 and coalesce(years_old>=60,false);
 perform hris_private.payroll_text(r,'reference',1000,active);
 if active then
  if birth is null or(k='statutory'and not ordinary)then raise exception 'Ordinary statutory retirement needs reviewed coverage, age60 and actual five-year service; monetary rounding does not grant eligibility.'using errcode='23514';end if;
  if ordinary then if daily=0 then raise exception 'Review latest daily retirement basic.'using errcode='23514';end if;retirement_floor:=round(daily::numeric*credited*45/2)::bigint;end if;
 else
  if daily<>0 or award<>0 or n<>0 or r->>'tax_treatment'<>'taxable'then raise exception 'Classify retirement amounts before entering pay or exemptions.'using errcode='23514';end if;
 end if;
 if r->>'tax_treatment'='statutory_no_plan'and(not active or not ordinary or(r->>'employee_plan_covered')::boolean)then raise exception 'RA7641 exemption needs an eligible employee not covered by an employer plan; ineligibility under a covered plan is not a fallback.'using errcode='23514';end if;
 if r->>'tax_treatment'='qualified_plan'and(not active or not(r->>'employee_plan_covered')::boolean or not(r->>'qualified_plan_reviewed')::boolean or not(r->>'first_qualified_availment')::boolean or full_years<10 or not coalesce(years_old>=50,false))then raise exception 'Qualified-plan exemption requires evidenced coverage/qualification, age50, actual ten-year service and first qualifying availment.'using errcode='23514';end if;
 award:=case when active then greatest(award,retirement_floor)else 0 end;retirement:=greatest(award-n,0);retirement_overpaid:=greatest(n-award,0);
 if r->>'tax_treatment'='taxable'then taxable:=taxable+retirement;else non_taxable:=non_taxable+retirement;end if;
 for l in select value from jsonb_array_elements(w->'lines')loop
  perform hris_private.final_pay_object(l,array['label','kind','amount_cents','reference']);perform hris_private.payroll_text(l,'label',120,true);perform hris_private.payroll_text(l,'reference',1000,true);n:=hris_private.payroll_amount(l,'amount_cents',maximum);k:=hris_private.payroll_text(l,'kind',30,true);
  case k when'taxable'then taxable:=taxable+n;when'shared_benefit'then shared:=shared+n;when'deposit_return'then deposits:=deposits+n;when'lawful_deduction'then deductions:=deductions+n;else raise exception 'Use taxable/shared-benefit/deposit-return/lawful-deduction amounts.'using errcode='23514';end case;
 end loop;
 foreach k in array array['sss_adjustment_cents','philhealth_adjustment_cents','pagibig_adjustment_cents']loop contributions:=contributions+hris_private.payroll_amount(w,k,maximum);end loop;
 extras:=taxable+shared+non_taxable+deposits;income:=annual->'income';
 if jsonb_typeof(income)is distinct from 'array'then raise exception 'Refresh annual income sources.'using errcode='23514';end if;
 for l in select value from jsonb_array_elements(income)loop ids:=array_append(ids,l->>'id');end loop;
 for pending in select value from jsonb_array_elements(s->'outstanding')loop
  perform hris_private.final_pay_object(pending,array['id','income','net_cents']);pid:=hris_private.payroll_text(pending,'id',80,true);
  if pid is distinct from pending#>>'{income,id}'or pid=any(ids)or pid='final:additional'then raise exception 'Outstanding and received pay must have unique matching source identities.'using errcode='23514';end if;ids:=array_append(ids,pid);
  source_net:=source_net+hris_private.payroll_amount(pending,'net_cents',maximum);income:=income||jsonb_build_array(pending->'income');
 end loop;
 if 'final:additional'=any(ids)then raise exception 'Final pay cannot include its own additional source.'using errcode='23514';end if;
 income:=income||jsonb_build_array(jsonb_build_object('id','final:additional','starts_on',null,'ends_on',null,'taxable_cents',taxable+shared,'withholding_cents',0,'benefit_gross_cents',shared,'benefit_exempt_cents',0,'contribution_cents',contributions));
 before_tax:=source_net+extras-contributions-deductions;
 if before_tax not between 0 and maximum then raise exception 'Review cash, lawful deductions and additional contributions; no negative automatic employee debt.'using errcode='23514';end if;
 tw:=(w->'tax')||jsonb_build_object('last_pay_available_cents',before_tax);annual:=annual||jsonb_build_object('income',income,'pending_payments',0);tax:=hris_private.annual_tax_compute(annual,tw);
 net:=before_tax-(tax->>'available_deduction_cents')::bigint+(tax->>'refund_cents')::bigint;
 foreach n in array array[vacation,separation_floor,retirement_floor,taxable,shared,non_taxable,deposits,contributions,deductions,source_net,extras,net]loop if n not between 0 and maximum then raise exception 'Final-pay totals exceed the supported amount.'using errcode='23514';end if;end loop;
 return jsonb_build_object('credited_service_years',credited,'vacation_gross_cents',vacation,'vacation_exempt_cents',vacation_exempt,'vacation_shared_benefit_cents',vacation_shared,'separation_floor_cents',separation_floor,'separation_cents',separation,'separation_overpaid_cents',separation_overpaid,'retirement_floor_cents',retirement_floor,'retirement_cents',retirement,'retirement_overpaid_cents',retirement_overpaid,'taxable_extras_cents',taxable,'shared_extras_cents',shared,'exempt_extras_cents',non_taxable,'deposit_returns_cents',deposits,'additional_contributions_cents',contributions,'lawful_deductions_cents',deductions,'source_net_cents',source_net,'extras_gross_cents',extras,'cash_before_tax_cents',before_tax,'tax',tax,'net_cents',net,'due_on',due,'planned_late',cash>due);
end;$$;

create or replace function hris_private.final_pay_ready(s jsonb,w jsonb)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare c jsonb;k text;begin
 if extract(year from(s->>'service_end')::date)<>extract(year from(s->>'cash_date')::date)and(not coalesce((w->>'cash_year_reviewed')::boolean,false)or btrim(coalesce(w->>'cash_year_note',''))='')then raise exception 'Review actual/constructive receipt, earlier-year certificates and other employment before issuing delayed final pay.'using errcode='23514';end if;
 c:=hris_private.final_pay_compute(s,w);
 if(s#>>'{annual,pending_adjustments}')::bigint<>0 or jsonb_array_length(c#>'{tax,coverage_gaps}')<>0 or(c#>>'{tax,unreconciled_benefits_cents}')::bigint<>0 then raise exception 'Resolve earlier tax advice and review all annual coverage/shared benefits.'using errcode='23514';end if;
 if(c->>'separation_overpaid_cents')::bigint<>0 or(c->>'retirement_overpaid_cents')::bigint<>0 then raise exception 'Resolve previous award overpayment without automatic wage recovery.'using errcode='23514';end if;
 foreach k in array array['entitlements_reviewed','source_scope_reviewed','contributions_reviewed']loop if not(w->>k)::boolean then raise exception 'Review all final-pay entitlements, source scope and additional contributions, including zero.'using errcode='23514';end if;end loop;
 perform hris_private.payroll_text(w,'review_note',2000,true);perform hris_private.payroll_text(w,'contribution_note',2000,true);
 foreach k in array array['final_compensation_reviewed','previous_employers_reviewed','benefits_reviewed']loop if(w#>>array['tax',k])::boolean is not true then raise exception 'Review prospective annual compensation, net prior-employer certificates and shared benefits.'using errcode='23514';end if;end loop;
 perform hris_private.payroll_text(w->'tax','review_note',2000,true);perform hris_private.payroll_text(w->'tax','settlement_note',2000,true);
 return c;
end;$$;

create or replace function hris_private.final_pay_check(r public.payroll_final_cases)returns void language plpgsql stable security invoker set search_path=''as $$
declare fresh jsonb;t jsonb;begin
 fresh:=hris_private.final_pay_basis(r.company_id,r.employee_id,r.service_end,r.cash_date,r.id);
 if fresh is distinct from r.source_basis then raise exception 'Final-pay income, payments or employment facts changed. Refresh a draft, or withdraw an unpaid issued statement and prepare a new review.'using errcode='PT409';end if;
 t:=hris_private.final_pay_ready(fresh->'calculation_source',r.worksheet||jsonb_build_object('cash_year_reviewed',r.cash_year_reviewed,'cash_year_note',r.cash_year_note));
 if t is distinct from r.totals or (r.rule_version<>'PH-final-pay-2026-10-24-v2'and not(r.rule_version='PH-final-pay-2026-10-24-v1'and extract(year from r.service_end)=extract(year from r.cash_date)))then raise exception 'Final-pay totals/rule version changed. Refresh.'using errcode='PT409';end if;
 if exists(select 1 from public.payroll_final_cases p where p.company_id=r.company_id and p.employee_id=r.employee_id and p.id<>r.id and p.status in('approved','settled')and p.employment_start<=r.service_end and p.service_end>=r.employment_start)then raise exception 'An overlapping employment already has an issued final statement. Resolve its unpaid statement or use the separate amendment process.'using errcode='23514';end if;
end;$$;

create or replace function hris_private.manage_payroll_final(p_operation text,p_record jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;r public.payroll_final_cases;e public.employees;birth hris_private.payroll_birth;rid uuid;eid uuid;expected bigint;why text;allowed text[];actor_label text;today date;zone text;fin date;cash date;w jsonb;t jsonb;fresh jsonb;old_record jsonb;result jsonb;part jsonb;p record;components jsonb;on_date date;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Active HR/Administrator access is required for final pay.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));perform 1 from public.companies where id=m.company_id for update;
 select *into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist')for update;
 if m.user_id is null then raise exception 'Your payroll access changed. Refresh.'using errcode='42501';end if;
 select timezone,(now()at time zone timezone)::date into zone,today from public.companies where id=m.company_id;select full_name into actor_label from public.employees where company_id=m.company_id and id=m.employee_id;
 if p_operation is null or p_operation not in('create','save','refresh','submit','return','cancel','approve','settle','withdraw')or jsonb_typeof(p_record)is distinct from 'object'or octet_length(p_record::text)>262144 then raise exception 'Use a supported bounded final-pay operation.'using errcode='23514';end if;
 allowed:=case p_operation when'create'then array['employee_id','service_end','cash_date','title']when'save'then array['worksheet']when'settle'then array['settled_on','payment_reference','handling_reference','cash_received_confirmed','employer_handling_confirmed']when'withdraw'then array['unpaid_confirmed']else array[]::text[]end;
 if p_record-(allowed||array['id','expected_revision','reason'])<>'{}'then raise exception 'Unsupported final-pay fields.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=hris_private.payroll_text(p_record,'reason',500,true);
 if rid is null or rid='00000000-0000-0000-0000-000000000000'then raise exception 'Final-pay identity required.'using errcode='23514';end if;
 if p_operation in('create','settle','withdraw')then
  select *into birth from hris_private.payroll_birth where company_id=m.company_id and operation='final_'||p_operation and record_id=rid;
  if birth.record_id is not null then
   if birth.actor_id<>m.user_id or birth.payload<>p_record then raise exception 'This final-pay operation identity was already used.'using errcode='PT409';end if;
   if p_operation<>'create'and m.role<>'administrator'then raise exception 'Only Administrators record settlement/withdrawal.'using errcode='42501';end if;
   select to_jsonb(x)into result from public.payroll_final_cases x where x.company_id=m.company_id and x.id=rid;return result;
  end if;
 end if;
 if p_operation='create'then
  if expected is not null then raise exception 'New final-pay cases do not have a revision.'using errcode='23514';end if;
  eid:=(p_record->>'employee_id')::uuid;select *into e from public.employees where company_id=m.company_id and id=eid;
  if e.id is null then raise exception 'Employee unavailable.'using errcode='42501';end if;
  fin:=hris_private.thirteenth_date(p_record,'service_end');cash:=hris_private.thirteenth_date(p_record,'cash_date');fresh:=hris_private.final_pay_basis(m.company_id,eid,fin,cash,rid);
  w:=hris_private.final_pay_seed(fresh->'calculation_source');
  t:=hris_private.final_pay_compute(fresh->'calculation_source',w);
  insert into public.payroll_final_cases(id,company_id,employee_id,employment_start,service_end,cash_date,title,employee_name,employee_code,department,created_by,prepared_by,source_basis,worksheet,totals)
  values(rid,m.company_id,eid,e.joined_on,fin,cash,hris_private.payroll_text(p_record,'title',120,true),e.full_name,e.employee_code,e.department,m.user_id,m.user_id,fresh,w,t)returning *into r;
 else
  select *into r from public.payroll_final_cases where company_id=m.company_id and id=rid for update;
  if r.id is null then raise exception 'Final-pay case unavailable.'using errcode='42501';end if;
  if expected is null or expected is distinct from r.revision then raise exception 'Final-pay record changed. Refresh.'using errcode='PT409';end if;old_record:=to_jsonb(r);
  if p_operation in('save','refresh')then
   if r.status<>'draft'then raise exception 'Only draft final-pay worksheets may change.'using errcode='23514';end if;
   fresh:=hris_private.final_pay_basis(m.company_id,r.employee_id,r.service_end,r.cash_date,r.id);
   if p_operation='save'then
    if fresh is distinct from r.source_basis then raise exception 'Final-pay sources changed. Refresh and review again.'using errcode='PT409';end if;w:=p_record->'worksheet';
   else
    w:=r.worksheet||jsonb_build_object('cash_year_note',r.cash_year_note)||'{"entitlements_reviewed":false,"source_scope_reviewed":false,"contributions_reviewed":false,"review_note":"","cash_year_reviewed":false}'::jsonb;
    w:=jsonb_set(w,'{tax}',(w->'tax')||'{"final_compensation_reviewed":false,"previous_employers_reviewed":false,"benefits_reviewed":false,"review_note":""}'::jsonb);
    w:=jsonb_set(w,'{vacation,previous_days_hundredths}',to_jsonb(greatest((w#>>'{vacation,previous_days_hundredths}')::bigint,(fresh#>>'{calculation_source,previous_vacation_days_hundredths}')::bigint)));
   end if;
   if(w#>>'{vacation,previous_days_hundredths}')::bigint>0 and btrim(w#>>'{vacation,reference}')=''then w:=jsonb_set(w,'{vacation,reference}',to_jsonb('Earlier recorded vacation conversion retained. Review the original evidence.'::text));end if;
   if p_operation='refresh'then begin t:=hris_private.final_pay_compute(fresh->'calculation_source',w);exception when check_violation then w:=hris_private.final_pay_seed(fresh->'calculation_source');t:=hris_private.final_pay_compute(fresh->'calculation_source',w);end;
   else t:=hris_private.final_pay_compute(fresh->'calculation_source',w);end if;
   select *into e from public.employees where company_id=m.company_id and id=r.employee_id;
   update public.payroll_final_cases set employment_start=e.joined_on,employee_name=e.full_name,employee_code=e.employee_code,department=e.department,source_basis=fresh,worksheet=w-array['cash_year_reviewed','cash_year_note'],cash_year_reviewed=coalesce((w->>'cash_year_reviewed')::boolean,false),cash_year_note=coalesce(w->>'cash_year_note',''),totals=t,prepared_by=m.user_id,rule_version='PH-final-pay-2026-10-24-v2',revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
  elsif p_operation in('submit','approve')then
   if r.service_end>today then raise exception 'Review completed employment and actual issued pay only.'using errcode='23514';end if;
   perform hris_private.final_pay_check(r);
   if p_operation='submit'then
    if r.status<>'draft'then raise exception 'Submit a draft final-pay review.'using errcode='23514';end if;
    update public.payroll_final_cases set status='submitted',submitted_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
   else
    if m.role<>'administrator'or m.user_id in(r.created_by,r.prepared_by,r.submitted_by)then raise exception 'An independent Administrator must approve final pay.'using errcode='42501';end if;
    if r.status<>'submitted'then raise exception 'Submit final pay before approval.'using errcode='23514';end if;
    update public.payroll_final_cases set status='approved',approved_by=m.user_id,approved_at=clock_timestamp(),revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
    select coalesce(jsonb_agg(jsonb_build_object('label',x->'label','kind',x->'kind','amount_cents',x->'amount_cents')order by n),'[]')into components from jsonb_array_elements(r.worksheet->'lines')with ordinality l(x,n);
    insert into public.payroll_final_slips(id,company_id,employee_id,employment_start,service_end,cash_date,company_name,employee_name,employee_code,department,totals,sources,components,approved_by,approved_at,rule_version)
    select r.id,r.company_id,r.employee_id,r.employment_start,r.service_end,r.cash_date,c.name,r.employee_name,r.employee_code,r.department,r.totals,coalesce((select jsonb_agg(x-array['run_id']order by x->>'kind',x->>'slip_id')from jsonb_array_elements(r.source_basis->'selected')x),'[]'),components,r.approved_by,r.approved_at,r.rule_version from public.companies c where c.id=m.company_id;
   end if;
  elsif p_operation='return'then
   if r.status<>'submitted'then raise exception 'Return a submitted final-pay review.'using errcode='23514';end if;
   update public.payroll_final_cases set status='draft',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
  elsif p_operation='cancel'then
   if r.status not in('draft','submitted')then raise exception 'Issued final statements remain immutable; unpaid withdrawal is a separate action.'using errcode='23514';end if;
   update public.payroll_final_cases set status='cancelled',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
  elsif p_operation='withdraw'then
   if m.role<>'administrator'or r.status<>'approved'then raise exception 'Only Administrators withdraw an unpaid approved statement.'using errcode='42501';end if;
   if not hris_private.final_pay_bool(p_record,'unpaid_confirmed')then raise exception 'Confirm that no final cash or employer tax settlement has occurred.'using errcode='23514';end if;
   update public.payroll_final_cases set status='withdrawn',revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
   insert into public.payroll_final_status(id,company_id,employee_id,kind,recorded_on,net_cents)values(rid,m.company_id,r.employee_id,'withdrawn',today,0);
  else
   if m.role<>'administrator'or r.status<>'approved'then raise exception 'Only Administrators record approved final settlements.'using errcode='42501';end if;
   on_date:=hris_private.thirteenth_date(p_record,'settled_on');
   if on_date<>r.cash_date or on_date>today or on_date<(r.approved_at at time zone zone)::date or not hris_private.final_pay_bool(p_record,'cash_received_confirmed')or not hris_private.final_pay_bool(p_record,'employer_handling_confirmed')then raise exception 'Confirm actual full final cash and tax handling on the approved cash date, after approval; reschedule by withdrawing an unpaid statement.'using errcode='23514';end if;
   perform hris_private.final_pay_check(r);
   update public.payroll_final_cases set status='settled',settled_on=on_date,payment_reference=hris_private.payroll_text(p_record,'payment_reference',500,true),handling_reference=hris_private.payroll_text(p_record,'handling_reference',1000,(r.totals#>>'{tax,employer_advance_cents}')::bigint>0),revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
   for part in select value from jsonb_array_elements(r.source_basis->'selected')loop
    perform hris_private.record_final_pay_source(r,part);
   end loop;
   insert into public.payroll_final_status(id,company_id,employee_id,kind,recorded_on,net_cents)values(rid,m.company_id,r.employee_id,'settled',on_date,(r.totals->>'net_cents')::bigint);
  end if;
 end if;
 result:=to_jsonb(r);
 if p_operation in('create','settle','withdraw')then insert into hris_private.payroll_birth(company_id,operation,record_id,actor_id,payload)values(m.company_id,'final_'||p_operation,rid,m.user_id,p_record);end if;
 insert into public.payroll_events(company_id,record_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)values(m.company_id,rid,'final_'||p_operation,why,m.user_id,actor_label,m.role,old_record,result);
 return result;
end;$$;

alter table public.payroll_final_cases alter column rule_version set default 'PH-final-pay-2026-10-24-v2';

-- A termination tax review may receive wages after the service year ended.
alter table public.payroll_tax_reconciliations drop constraint payroll_tax_reconciliations_check;
alter table public.payroll_tax_reconciliations add constraint payroll_tax_cash_service_dates
 check(extract(year from as_of)=year and service_end>=date'1900-01-01'and service_end<=as_of);

commit;
