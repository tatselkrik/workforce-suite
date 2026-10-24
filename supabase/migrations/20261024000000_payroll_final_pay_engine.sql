-- Prospective final pay, reviewed2026 private-sector rule pack. Integer centavos.
begin;
create function hris_private.final_pay_object(j jsonb,keys text[])returns void language plpgsql immutable security invoker set search_path=''as $$
begin
 if jsonb_typeof(j)is distinct from 'object'or j-keys<>'{}'or not(j?&keys)then raise exception 'Use the complete supported final-pay fields.'using errcode='23514';end if;
end;$$;
create function hris_private.final_pay_bool(j jsonb,k text)returns boolean language plpgsql immutable security invoker set search_path=''as $$
begin if jsonb_typeof(j->k)is distinct from 'boolean'then raise exception 'Explicit final-pay review booleans are required.'using errcode='23514';end if;return(j->>k)::boolean;end;$$;
create function hris_private.final_pay_service(st date,fin date)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare full_years integer;credited integer;exclusive date;begin
 if st is null or fin is null or st>fin or extract(year from st)<1900 or extract(year from fin)>2200 then raise exception 'Review actual employment start and last working day.'using errcode='23514';end if;
 exclusive:=fin+1;full_years:=extract(year from fin)::integer-extract(year from st)::integer;
 while(st+make_interval(months=>full_years*12))::date>exclusive loop full_years:=full_years-1;end loop;
 if(st+make_interval(months=>(full_years+1)*12))::date<=exclusive then full_years:=full_years+1;end if;
 credited:=full_years+case when(st+make_interval(months=>full_years*12+6))::date<=exclusive then 1 else 0 end;
 return jsonb_build_object('full',full_years,'credited',credited);
end;$$;
create function hris_private.final_pay_compute(s jsonb,w jsonb)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare maximum constant bigint:=100000000000;
 st date;fin date;cash date;due date;birth date;full_years integer;credited integer;years_old integer;v jsonb;p jsonb;r jsonb;l jsonb;pending jsonb;annual jsonb;tw jsonb;tax jsonb;income jsonb;service jsonb;
 k text;ids text[]:='{}';pid text;days bigint;previous_days bigint;daily bigint;release_days bigint;factor integer;base bigint;award bigint;ordinary boolean;active boolean;exempt boolean;
 vacation bigint:=0;vacation_exempt bigint:=0;vacation_shared bigint:=0;separation_floor bigint:=0;separation bigint:=0;separation_overpaid bigint:=0;
 retirement_floor bigint:=0;retirement bigint:=0;retirement_overpaid bigint:=0;taxable bigint:=0;shared bigint:=0;non_taxable bigint:=0;deposits bigint:=0;deductions bigint:=0;contributions bigint:=0;source_net bigint:=0;extras bigint;before_tax bigint;net bigint;n bigint;
begin
 if octet_length(w::text)>262144 or octet_length(s::text)>524288 then raise exception 'Final-pay source/worksheet exceeds its limit.'using errcode='23514';end if;
 perform hris_private.final_pay_object(s,array['employment_start','service_end','cash_date','annual','outstanding','previous_vacation_days_hundredths']);
 perform hris_private.final_pay_object(w,array['tax','vacation','separation','retirement','lines','sss_adjustment_cents','philhealth_adjustment_cents','pagibig_adjustment_cents','contribution_note','release_within_days','entitlements_reviewed','source_scope_reviewed','contributions_reviewed','review_note']);
 st:=hris_private.thirteenth_date(s,'employment_start');fin:=hris_private.thirteenth_date(s,'service_end');cash:=hris_private.thirteenth_date(s,'cash_date');
 if cash<date'2026-01-06'or extract(year from cash)<>2026 or extract(year from fin)<>2026 or fin>cash or st>fin then raise exception 'Use same-year2026 separation and cash dates from6January; cross-year payments need a separate rule review.'using errcode='23514';end if;
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
create function hris_private.final_pay_ready(s jsonb,w jsonb)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare c jsonb;k text;begin
 c:=hris_private.final_pay_compute(s,w);
 if(s#>>'{annual,pending_adjustments}')::bigint<>0 or jsonb_array_length(c#>'{tax,coverage_gaps}')<>0 or(c#>>'{tax,unreconciled_benefits_cents}')::bigint<>0 then raise exception 'Resolve earlier tax advice and review all annual coverage/shared benefits.'using errcode='23514';end if;
 if(c->>'separation_overpaid_cents')::bigint<>0 or(c->>'retirement_overpaid_cents')::bigint<>0 then raise exception 'Resolve previous award overpayment without automatic wage recovery.'using errcode='23514';end if;
 foreach k in array array['entitlements_reviewed','source_scope_reviewed','contributions_reviewed']loop if not(w->>k)::boolean then raise exception 'Review all final-pay entitlements, source scope and additional contributions, including zero.'using errcode='23514';end if;end loop;
 perform hris_private.payroll_text(w,'review_note',2000,true);perform hris_private.payroll_text(w,'contribution_note',2000,true);
 foreach k in array array['final_compensation_reviewed','previous_employers_reviewed','benefits_reviewed']loop if(w#>>array['tax',k])::boolean is not true then raise exception 'Review prospective annual compensation, net prior-employer certificates and shared benefits.'using errcode='23514';end if;end loop;
 perform hris_private.payroll_text(w->'tax','review_note',2000,true);perform hris_private.payroll_text(w->'tax','settlement_note',2000,true);
 return c;
end;$$;
create function hris_private.final_pay_empty()returns jsonb language sql immutable security invoker set search_path=''as $$
 select jsonb_build_object('tax',hris_private.annual_tax_empty(),'vacation',jsonb_build_object('days_hundredths',0,'daily_cents',0,'previous_days_hundredths',0,'reference',''),'separation',jsonb_build_object('cause','none','monthly_basic_cents',0,'regular_allowances_cents',0,'total_award_cents',0,'previously_paid_cents',0,'tax_exempt',false,'reference',''),'retirement',jsonb_build_object('method','none','daily_basic_cents',0,'plan_award_cents',0,'previously_paid_cents',0,'birth_date',null,'ordinary_statutory_coverage',false,'employee_plan_covered',false,'tax_treatment','taxable','qualified_plan_reviewed',false,'first_qualified_availment',false,'reference',''),'lines','[]'::jsonb,'sss_adjustment_cents',0,'philhealth_adjustment_cents',0,'pagibig_adjustment_cents',0,'contribution_note','','release_within_days',30,'entitlements_reviewed',false,'source_scope_reviewed',false,'contributions_reviewed',false,'review_note','');
$$;
create function hris_private.final_pay_seed(s jsonb)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare w jsonb;n bigint;begin w:=hris_private.final_pay_empty();n:=hris_private.payroll_amount(s,'previous_vacation_days_hundredths',36600);w:=jsonb_set(w,'{vacation,previous_days_hundredths}',to_jsonb(n));if n>0 then w:=jsonb_set(w,'{vacation,reference}',to_jsonb('Earlier recorded vacation conversion retained. Review the original evidence.'::text));end if;return w;end;$$;
revoke all on function hris_private.final_pay_object(jsonb,text[]),hris_private.final_pay_bool(jsonb,text),hris_private.final_pay_service(date,date),hris_private.final_pay_compute(jsonb,jsonb),hris_private.final_pay_ready(jsonb,jsonb),hris_private.final_pay_empty(),hris_private.final_pay_seed(jsonb)from public,anon,authenticated;
commit;
