-- DOLE 2024 handbook p.18: an eligible unworked double regular holiday is 200%.
-- True monthly pay already includes one day; add only the second. Do not rewrite
-- issued slips, events or deployed migrations. The earlier pack was pre-release.
begin;
do $$begin
 if exists(select 1 from public.payroll_runs where status in('draft','submitted') and rule_version='PH-private-2026-10-23-v1')then
  raise exception 'Review and cancel pre-release v1 draft registers before applying the corrected rule pack.';
 end if;
end;$$;
alter table public.payroll_runs alter column rule_version set default 'PH-private-2026-10-23-v2';
create or replace function hris_private.payroll_compute(p jsonb,w jsonb,mon date)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare d jsonb;c jsonb;k text;daydate date;dates date[]:='{}';is_monthly boolean;eligible boolean;dn numeric;dd numeric;f integer;ot integer;v bigint;
 regular_m bigint;ot_m bigint;night_m bigint;night_ot bigint;leave_m bigint;unpaid_m bigint;holiday_m bigint;
 basic bigint:=0;paid_leave bigint:=0;unworked_holiday bigint:=0;premium bigint:=0;overtime bigint:=0;night bigint:=0;allowances bigint:=0;exempt_allowances bigint:=0;rice bigint:=0;laundry bigint:=0;reimbursement bigint:=0;benefits bigint:=0;voluntary_deductions bigint:=0;advances bigint:=0;
 gross bigint;taxable bigint;sss_basis bigint;sss_msc bigint:=0;sss_employee bigint:=0;sss_employer bigint:=0;sss_regular_employee bigint:=0;sss_regular_employer bigint:=0;sss_mpf_employee bigint:=0;sss_mpf_employer bigint:=0;ec bigint:=0;ph bigint;ph_total bigint;philhealth_employee bigint:=0;philhealth_employer bigint:=0;pi bigint;pagibig_employee bigint:=0;pagibig_employer bigint:=0;statutory bigint;withholding bigint;net bigint;
begin
 perform hris_private.payroll_pay(p);
 if mon is null or extract(day from mon)<>1 or mon<date '2026-02-01'then raise exception 'This rule pack supports months from February2026.'using errcode='23514';end if;
 if jsonb_typeof(w)is distinct from 'object'or w-array['days','adjustments','basic_override_cents','sss_basis_override','philhealth_basis_override','pagibig_basis_override','exception_note','review_note','reviewed']<>'{}'::jsonb then raise exception 'Unsupported worksheet fields.'using errcode='23514';end if;
 if jsonb_typeof(w->'days')is distinct from 'array'or jsonb_array_length(w->'days')>31 or jsonb_typeof(w->'reviewed')is distinct from 'boolean'then raise exception 'Use explicit reviewed status and at most31 calendar-date rows.'using errcode='23514';end if;
 perform hris_private.payroll_text(w,'exception_note',1000);perform hris_private.payroll_text(w,'review_note',2000,(w->>'reviewed')::boolean);perform hris_private.payroll_components(w->'adjustments');
 foreach k in array array['basic_override_cents','sss_basis_override','philhealth_basis_override','pagibig_basis_override']loop
  if w->k is null then raise exception 'Supply all override fields explicitly (null when unused).'using errcode='23514';end if;
  if w->k<>'null'::jsonb then perform hris_private.payroll_amount(w,k);if btrim(w->>'exception_note')=''then raise exception 'Overrides need a documented payroll review.'using errcode='23514';end if;end if;
 end loop;
 is_monthly:=p->>'basis'='monthly';eligible:=(p->>'premium_eligible')::boolean;
 dn:=(p->>'rate_cents')::numeric*case when is_monthly then 12 else 1 end;dd:=case when is_monthly then(p->>'annual_divisor')::numeric else 1 end;
 if w->'basic_override_cents'<>'null'::jsonb and(not is_monthly or(w->>'basic_override_cents')::bigint>(p->>'rate_cents')::bigint)then raise exception 'Basic override is a monthly proration within salary.'using errcode='23514';end if;
 if is_monthly then basic:=coalesce((w->>'basic_override_cents')::bigint,(p->>'rate_cents')::bigint);end if;
 for d in select value from jsonb_array_elements(w->'days')loop
  if jsonb_typeof(d)is distinct from 'object'or d-array['date','kind','rest','regular_minutes','overtime_minutes','night_regular','night_overtime','paid_leave_minutes','unworked_holiday_minutes','unpaid_minutes']<>'{}'::jsonb or jsonb_typeof(d->'rest')is distinct from 'boolean'then raise exception 'Unsupported day fields.'using errcode='23514';end if;
  daydate:=(d->>'date')::date;f:=hris_private.payroll_factor(d->>'kind',(d->>'rest')::boolean);
  if daydate is null or daydate<mon or daydate>=(mon+interval '1month')::date or daydate=any(dates)or f is null then raise exception 'Use unique dates inside the month and supported holiday classifications.'using errcode='23514';end if;dates:=array_append(dates,daydate);
  regular_m:=hris_private.payroll_amount(d,'regular_minutes',480);ot_m:=hris_private.payroll_amount(d,'overtime_minutes',960);night_m:=hris_private.payroll_amount(d,'night_regular',480);night_ot:=hris_private.payroll_amount(d,'night_overtime',960);leave_m:=hris_private.payroll_amount(d,'paid_leave_minutes',480);unpaid_m:=hris_private.payroll_amount(d,'unpaid_minutes',480);holiday_m:=hris_private.payroll_amount(d,'unworked_holiday_minutes',480);
  if regular_m+leave_m+unpaid_m>480 or night_m>regular_m or night_ot>ot_m or(holiday_m>0 and(regular_m>0 or ot_m>0 or leave_m>0 or unpaid_m>0 or d->>'kind'not in('regular','double_regular')))then raise exception 'Check regular/OT/night minutes and unworked-holiday eligibility.'using errcode='23514';end if;
  if not eligible then f:=100;end if;ot:=case when f=100 then 125 else 130 end;
  if is_monthly then basic:=basic-round(dn*unpaid_m/(dd*480))::bigint;
   if eligible and d->>'kind'='double_regular'then unworked_holiday:=unworked_holiday+round(dn*holiday_m/(dd*480))::bigint;end if;
  else basic:=basic+round(dn*regular_m/(dd*480))::bigint;paid_leave:=paid_leave+round(dn*leave_m/(dd*480))::bigint;
   if eligible then unworked_holiday:=unworked_holiday+round(dn*holiday_m*(case when d->>'kind'='double_regular'then 2 else 1 end)/(dd*480))::bigint;end if;
  end if;
  premium:=premium+round(dn*regular_m*(f-100)/(dd*480*100))::bigint;
  if eligible then overtime:=overtime+round(dn*ot_m*f*ot/(dd*480*10000))::bigint;night:=night+round(dn*night_m*f/(dd*480*1000))::bigint+round(dn*night_ot*f*ot/(dd*480*100000))::bigint;end if;
 end loop;
 if basic<0 then raise exception 'Unpaid time exceeds basic pay.'using errcode='23514';end if;
 for c in select value from jsonb_array_elements((p->'components')||(w->'adjustments'))loop
  v:=(c->>'amount_cents')::bigint;case c->>'kind'when'taxable_allowance'then allowances:=allowances+v;when'rice'then rice:=rice+v;allowances:=allowances+v;when'laundry'then laundry:=laundry+v;allowances:=allowances+v;when'reimbursement'then reimbursement:=reimbursement+v;when'deduction'then voluntary_deductions:=voluntary_deductions+v;when'advance'then advances:=advances+v;when'employer_benefit'then benefits:=benefits+v;else null;end case;
 end loop;
 exempt_allowances:=least(rice,250000)+least(laundry,40000);gross:=basic+paid_leave+unworked_holiday+premium+overtime+night+allowances;
 sss_basis:=coalesce((w->>'sss_basis_override')::bigint,gross);
 if(p->>'sss')::boolean and sss_basis>0 then
  sss_msc:=greatest(500000,least(3500000,((sss_basis+25000)/50000)*50000));sss_employee:=round(sss_msc*0.05)::bigint;sss_employer:=round(sss_msc*0.10)::bigint;
  sss_regular_employee:=round(least(sss_msc,2000000)*0.05)::bigint;sss_regular_employer:=round(least(sss_msc,2000000)*0.10)::bigint;
  sss_mpf_employee:=sss_employee-sss_regular_employee;sss_mpf_employer:=sss_employer-sss_regular_employer;ec:=case when sss_msc<1500000 then 1000 else 3000 end;
 end if;
 ph:=coalesce((w->>'philhealth_basis_override')::bigint,case when is_monthly then(p->>'rate_cents')::bigint else(p->>'monthly_reference_cents')::bigint end);
 if(p->>'philhealth')::boolean and ph>0 then ph_total:=round(greatest(1000000,least(10000000,ph))*0.05)::bigint;philhealth_employee:=round(ph_total/2.0)::bigint;philhealth_employer:=ph_total-philhealth_employee;end if;
 pi:=least(1000000,coalesce((w->>'pagibig_basis_override')::bigint,basic+paid_leave+allowances));
 if(p->>'pagibig')::boolean and pi>0 then pagibig_employee:=round(pi*(case when pi<=150000 then 0.01 else 0.02 end))::bigint;pagibig_employer:=round(pi*0.02)::bigint;end if;
 statutory:=sss_employee+philhealth_employee+pagibig_employee;
 taxable:=greatest(0,(case when(p->>'minimum_wage_earner')::boolean then allowances-exempt_allowances else gross-exempt_allowances end)-statutory);
 withholding:=hris_private.payroll_tax(taxable);net:=gross+reimbursement-statutory-withholding-voluntary_deductions-advances;
 return jsonb_build_object('basic',basic,'paid_leave',paid_leave,'unworked_holiday',unworked_holiday,'premium',premium,'overtime',overtime,'night',night,'allowances',allowances,'exempt_allowances',exempt_allowances,'reimbursement',reimbursement,'benefits',benefits,'voluntary_deductions',voluntary_deductions,'advances',advances,'gross',gross,'taxable',taxable,'sss_basis',sss_basis,'sss_msc',sss_msc,'sss_employee',sss_employee,'sss_employer',sss_employer,'sss_regular_employee',sss_regular_employee,'sss_regular_employer',sss_regular_employer,'sss_mpf_employee',sss_mpf_employee,'sss_mpf_employer',sss_mpf_employer,'ec',ec,'philhealth_employee',philhealth_employee,'philhealth_employer',philhealth_employer,'pagibig_employee',pagibig_employee,'pagibig_employer',pagibig_employer,'withholding',withholding,'net',net,'thirteenth_accrual',round(basic/12.0)::bigint);
end;$$;

commit;
