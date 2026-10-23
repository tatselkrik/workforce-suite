-- Private calculation foundation; no client writes or UI payout contract yet.
begin;
create function hris_private.thirteenth_date(j jsonb,k text)returns date language plpgsql immutable security invoker set search_path=''as $$
begin
 if jsonb_typeof(j->k)is distinct from 'string'or(j->>k)!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'then raise exception 'Use an explicit ISO date for %.',k using errcode='23514';end if;
 return(j->>k)::date;
end;$$;
create function hris_private.thirteenth_compute(s jsonb,w jsonb)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare maximum constant bigint:=100000000000;cap constant bigint:=9000000;
 st date;fin date;lo date;hi date;day date;gap_start date;gap_end date;covered date[]:='{}';ids uuid[]:='{}';pid uuid;
 p jsonb;k text;basic bigint;prior bigint;previous_exempt bigint:=0;entitlement bigint;remaining bigint;gross bigint;used bigint;exempt bigint;net bigint;gaps jsonb:='[]';v bigint;
begin
 if jsonb_typeof(s)is distinct from 'object'or s-array['starts_on','as_of','periods','prior','external_thirteenth_floor','other_benefits_floor']<>'{}'or jsonb_typeof(w)is distinct from 'object'or octet_length(w::text)>65536 or w-array['openings','classification','eligibility_note','served_one_month','basic_adjustment_cents','adjustment_note','policy_topup_cents','policy_note','external_thirteenth_cents','other_benefits_cents','opening_note','requested_gross_cents','withholding_cents','tax_reviewed','tax_note','sss_adjustment_cents','philhealth_adjustment_cents','pagibig_adjustment_cents','contributions_reviewed','contribution_note','reviewed','review_note']<>'{}'then raise exception 'Use supported annual-benefit source/worksheet fields.'using errcode='23514';end if;
 st:=hris_private.thirteenth_date(s,'starts_on');fin:=hris_private.thirteenth_date(s,'as_of');
 if extract(year from fin)not between 2026 and 2200 or extract(year from st)<>extract(year from fin)or st>fin then raise exception 'Use a bounded current-year earnings period.'using errcode='23514';end if;
 if jsonb_typeof(s->'periods')is distinct from 'array'or jsonb_array_length(s->'periods')>24 or jsonb_typeof(s->'prior')is distinct from 'array'or jsonb_array_length(s->'prior')>24 or jsonb_typeof(w->'openings')is distinct from 'array'or jsonb_array_length(w->'openings')>32 then raise exception 'Too many annual earning/opening/payment rows.'using errcode='23514';end if;
 k:=hris_private.payroll_text(w,'classification',30);if k not in('','rank_and_file','company_policy')then raise exception 'Review actual employment coverage, not account role.'using errcode='23514';end if;
 foreach k in array array['eligibility_note','adjustment_note','policy_note','opening_note','tax_note','contribution_note','review_note']loop perform hris_private.payroll_text(w,k,2000);end loop;
 foreach k in array array['served_one_month','tax_reviewed','contributions_reviewed','reviewed']loop if jsonb_typeof(w->k)is distinct from 'boolean'then raise exception 'Review fields require explicit booleans.'using errcode='23514';end if;end loop;
 foreach k in array array['policy_topup_cents','external_thirteenth_cents','other_benefits_cents','withholding_cents','sss_adjustment_cents','philhealth_adjustment_cents','pagibig_adjustment_cents']loop perform hris_private.payroll_amount(w,k,maximum);end loop;
 perform hris_private.payroll_amount(s,'external_thirteenth_floor',maximum);perform hris_private.payroll_amount(s,'other_benefits_floor',maximum);
 if jsonb_typeof(w->'basic_adjustment_cents')is distinct from 'number'or(w->>'basic_adjustment_cents')!~'^-?[0-9]+$'then raise exception 'Use integer centavos for basic adjustment.'using errcode='23514';end if;
 basic:=(w->>'basic_adjustment_cents')::bigint;
 if basic not between -maximum and maximum or(basic<>0 and btrim(w->>'adjustment_note')='')or((w->>'policy_topup_cents')::bigint<>0 and btrim(w->>'policy_note')='')then raise exception 'Document bounded basic/policy adjustments.'using errcode='23514';end if;
 if(w->>'external_thirteenth_cents')::bigint<(s->>'external_thirteenth_floor')::bigint or(w->>'other_benefits_cents')::bigint<(s->>'other_benefits_floor')::bigint then raise exception 'Earlier approved external payment/benefit facts cannot be reduced here.'using errcode='23514';end if;
 if((w->>'external_thirteenth_cents')::bigint<>0 or(w->>'other_benefits_cents')::bigint<>0)and btrim(w->>'opening_note')=''then raise exception 'Document prior payments and shared cap usage.'using errcode='23514';end if;
 for p in select value from jsonb_array_elements(s->'periods')loop
  if jsonb_typeof(p)is distinct from 'object'or p-array['id','starts_on','ends_on','basic_cents']<>'{}'then raise exception 'Unsupported issued earnings fields.'using errcode='23514';end if;
  pid:=(p->>'id')::uuid;if pid is null or pid=any(ids)then raise exception 'Duplicate/missing issued payroll identity.'using errcode='23514';end if;ids:=array_append(ids,pid);
 end loop;
 for p in select value from jsonb_array_elements(w->'openings')loop
  if jsonb_typeof(p)is distinct from 'object'or p-array['starts_on','ends_on','basic_cents','reference']<>'{}'then raise exception 'Unsupported opening fields.'using errcode='23514';end if;
  perform hris_private.payroll_text(p,'reference',500,true);
 end loop;
 for p in select value from jsonb_array_elements((s->'periods')||(w->'openings'))loop
  lo:=hris_private.thirteenth_date(p,'starts_on');hi:=hris_private.thirteenth_date(p,'ends_on');v:=hris_private.payroll_amount(p,'basic_cents',maximum);
  if lo<st or hi>fin or lo>hi then raise exception 'Earnings/opening dates must fit the current-employer period.'using errcode='23514';end if;
  for day in select g::date from pg_catalog.generate_series(lo::timestamp,hi::timestamp,interval '1day')g loop
   if day=any(covered)then raise exception 'Opening and issued earnings periods overlap.'using errcode='23514';end if;covered:=array_append(covered,day);
  end loop;basic:=basic+v;
 end loop;
 if basic not between 0 and maximum then raise exception 'Reviewed earned basic must be bounded and nonnegative.'using errcode='23514';end if;
 for day in select g::date from pg_catalog.generate_series(st::timestamp,fin::timestamp,interval '1day')g loop
  if not(day=any(covered))then
   if gap_start is null then gap_start:=day;end if;gap_end:=day;
  elsif gap_start is not null then gaps:=gaps||jsonb_build_array(jsonb_build_object('starts_on',gap_start,'ends_on',gap_end));gap_start:=null;end if;
 end loop;
 if gap_start is not null then gaps:=gaps||jsonb_build_array(jsonb_build_object('starts_on',gap_start,'ends_on',gap_end));end if;
 prior:=(w->>'external_thirteenth_cents')::bigint;ids:='{}';
 for p in select value from jsonb_array_elements(s->'prior')loop
  if jsonb_typeof(p)is distinct from 'object'or p-array['id','gross_cents','exempt_cents']<>'{}'then raise exception 'Unsupported prior payment fields.'using errcode='23514';end if;
  pid:=(p->>'id')::uuid;v:=hris_private.payroll_amount(p,'gross_cents',maximum);
  if pid is null or pid=any(ids)or hris_private.payroll_amount(p,'exempt_cents',maximum)>v then raise exception 'Prior issued benefit facts do not reconcile.'using errcode='23514';end if;
  ids:=array_append(ids,pid);prior:=prior+v;previous_exempt:=previous_exempt+(p->>'exempt_cents')::bigint;
 end loop;
 if prior>maximum or previous_exempt>cap then raise exception 'Prior payments/exemptions exceed their bounded annual amounts.'using errcode='23514';end if;
 entitlement:=round(basic::numeric/12)::bigint+(w->>'policy_topup_cents')::bigint;
 if entitlement>maximum then raise exception 'Annual entitlement exceeds the supported amount.'using errcode='23514';end if;
 remaining:=greatest(entitlement-prior,0);
 if not(w?'requested_gross_cents')then raise exception 'Choose an explicit nullable requested payout.'using errcode='23514';end if;
 gross:=case when w->>'requested_gross_cents'is null then remaining else hris_private.payroll_amount(w,'requested_gross_cents',maximum)end;
 if gross>remaining then raise exception 'Requested payout exceeds unpaid entitlement.'using errcode='23514';end if;
 used:=greatest(least(prior+(w->>'other_benefits_cents')::bigint,cap),least(previous_exempt+(w->>'external_thirteenth_cents')::bigint+(w->>'other_benefits_cents')::bigint,cap));exempt:=least(gross,cap-used);
 net:=gross-(w->>'withholding_cents')::bigint-(w->>'sss_adjustment_cents')::bigint-(w->>'philhealth_adjustment_cents')::bigint-(w->>'pagibig_adjustment_cents')::bigint;
 if net<0 then raise exception 'Reviewed withholding/contribution adjustments exceed this payout.'using errcode='23514';end if;
 return jsonb_build_object('basic_cents',basic,'entitlement_cents',entitlement,'prior_company_cents',prior,'remaining_cents',remaining,'overpayment_cents',greatest(prior-entitlement,0),'gross_cents',gross,'exempt_cents',exempt,'taxable_cents',gross-exempt,'withholding_cents',w->'withholding_cents','sss_adjustment_cents',w->'sss_adjustment_cents','philhealth_adjustment_cents',w->'philhealth_adjustment_cents','pagibig_adjustment_cents',w->'pagibig_adjustment_cents','net_cents',net,'coverage_gaps',gaps);
end;$$;
create function hris_private.thirteenth_ready(s jsonb,w jsonb)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare c jsonb;begin
 c:=hris_private.thirteenth_compute(s,w);
 if jsonb_array_length(c->'coverage_gaps')<>0 then raise exception 'Review opening basic for every missing payroll period, including zero-paid intervals.'using errcode='23514';end if;
 if(c->>'overpayment_cents')::bigint<>0 or(c->>'gross_cents')::bigint<=0 then raise exception 'Review earlier payments; there is no positive unpaid entitlement.'using errcode='23514';end if;
 if(w->>'classification')=''or btrim(w->>'eligibility_note')=''or(w->>'classification'='rank_and_file'and(w->>'served_one_month')::boolean is not true)then raise exception 'Review actual coverage/one-month service, independently of account role.'using errcode='23514';end if;
 if(w->>'reviewed')::boolean is not true or btrim(w->>'review_note')=''then raise exception 'Review the annual benefit worksheet.'using errcode='23514';end if;
 if(w->>'tax_reviewed')::boolean is not true or btrim(w->>'tax_note')=''then raise exception 'Record reviewed supplementary/cumulative withholding and evidence, including zero.'using errcode='23514';end if;
 if(w->>'contributions_reviewed')::boolean is not true or btrim(w->>'contribution_note')=''then raise exception 'Review month-level contribution reconciliation, without a second full monthly charge.'using errcode='23514';end if;
 return c;
end;$$;
revoke all on function hris_private.thirteenth_date(jsonb,text),hris_private.thirteenth_compute(jsonb,jsonb),hris_private.thirteenth_ready(jsonb,jsonb)from public,anon,authenticated;
commit;
