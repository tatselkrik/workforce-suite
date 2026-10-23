-- Annual compensation tax, BIR RR11-2018 schedule2023+; integer centavos.
begin;
create function hris_private.annual_tax_amount(t bigint)returns bigint language plpgsql immutable security invoker set search_path=''as $$
begin
 if t not between 0 and 100000000000 then raise exception 'Annual taxable compensation is outside the supported amount.'using errcode='23514';end if;
 return case when t<=25000000 then 0 when t<=40000000 then round((t-25000000)::numeric*15/100)::bigint
 when t<=80000000 then 2250000+round((t-40000000)::numeric*20/100)::bigint
 when t<=200000000 then 10250000+round((t-80000000)::numeric*25/100)::bigint
 when t<=800000000 then 40250000+round((t-200000000)::numeric*30/100)::bigint
 else 220250000+round((t-800000000)::numeric*35/100)::bigint end;
end;$$;
create function hris_private.annual_tax_income(j jsonb)returns void language plpgsql immutable security invoker set search_path=''as $$
declare t bigint;g bigint;e bigint;begin
 t:=hris_private.payroll_amount(j,'taxable_cents',100000000000);perform hris_private.payroll_amount(j,'withholding_cents',100000000000);
 g:=hris_private.payroll_amount(j,'benefit_gross_cents',100000000000);e:=hris_private.payroll_amount(j,'benefit_exempt_cents',100000000000);
 if e>g or e>9000000 or t<g-e then raise exception 'Certified taxable and shared-benefit figures do not reconcile.'using errcode='23514';end if;
end;$$;
create function hris_private.annual_tax_signed(j jsonb,k text)returns bigint language plpgsql immutable security invoker set search_path=''as $$
declare v bigint;begin
 if jsonb_typeof(j->k)is distinct from 'number'or(j->>k)!~'^-?[0-9]+$'then raise exception 'Use integer centavos for %.',k using errcode='23514';end if;
 v:=(j->>k)::bigint;if v not between -100000000000 and 100000000000 then raise exception 'Annual adjustment exceeds the supported amount.'using errcode='23514';end if;return v;
end;$$;
create function hris_private.annual_tax_dates(yr integer,kind text,service_end date,asof date)returns void language plpgsql immutable security invoker set search_path=''as $$
begin
 if yr is null or yr not between 2026 and 2200 or service_end is null or asof is null or extract(year from service_end)<>yr or extract(year from asof)<>yr or service_end>asof
 or kind is null or(kind<>'termination'and(kind<>'year_end'or service_end<>make_date(yr,12,31)or asof<>service_end))then raise exception 'Use a same-year service/payment period; year-end closes31December.'using errcode='23514';end if;
end;$$;
create function hris_private.annual_tax_compute(s jsonb,w jsonb)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare maximum constant bigint:=100000000000;cap constant bigint:=9000000;
 st date;fin date;asof date;lo date;hi date;d date;gs date;ge date;covered date[]:='{}';prior_dates date[]:='{}';ids text[]:='{}';pid text;
 p jsonb;k text;gaps jsonb:='[]';v bigint;current_taxable bigint:=0;current_withheld bigint:=0;prior_taxable bigint:=0;prior_withheld bigint:=0;
 current_benefits bigint:=0;opening_benefits bigint:=0;prior_benefits bigint:=0;taxed_benefits bigint:=0;current_taxed_benefits bigint:=0;contributions bigint:=0;
 reclassified bigint;correction bigint;previous_adjustment bigint;gross bigint;exempt bigint;taxable_benefits bigint;benefit_adjustment bigint;taxable bigint;due bigint;adjustment bigint;additional bigint;refund bigint;available bigint;unreconciled bigint;
begin
 if jsonb_typeof(s)is distinct from 'object'or s-array['starts_on','service_end','as_of','income','prior_adjustment_cents','pending_payments','pending_adjustments','external_thirteenth_floor','other_benefits_floor']<>'{}'
 or jsonb_typeof(w)is distinct from 'object'or octet_length(w::text)>131072 or w-array['openings','previous_employers','reclassified_benefit_cents','classification_note','taxable_adjustment_cents','adjustment_note','last_pay_available_cents','settlement_note','final_compensation_reviewed','previous_employers_reviewed','benefits_reviewed','review_note']<>'{}'then raise exception 'Use supported annual tax source/worksheet fields.'using errcode='23514';end if;
 st:=hris_private.thirteenth_date(s,'starts_on');fin:=hris_private.thirteenth_date(s,'service_end');asof:=hris_private.thirteenth_date(s,'as_of');
 if extract(year from asof)not between 2026 and 2200 or extract(year from st)<>extract(year from asof)or extract(year from fin)<>extract(year from asof)or st>fin or fin>asof then raise exception 'Use a bounded current-year employment/payment period.'using errcode='23514';end if;
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
  if extract(year from lo)<>extract(year from asof)or lo>hi or hi>=st then raise exception 'Use same-year successive employment before the current hire; concurrent employers need separate review.'using errcode='23514';end if;
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
create function hris_private.annual_tax_ready(s jsonb,w jsonb)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare c jsonb;begin
 c:=hris_private.annual_tax_compute(s,w);
 if jsonb_array_length(c->'coverage_gaps')<>0 then raise exception 'Evidence is required for every missing paid/opening interval, including zero.'using errcode='23514';end if;
 if(s->>'pending_payments')::bigint<>0 or(s->>'pending_adjustments')::bigint<>0 then raise exception 'Record outstanding issued payments and earlier tax settlements before closing.'using errcode='23514';end if;
 if(c->>'unreconciled_benefits_cents')::bigint<>0 then raise exception 'Reconcile external/shared benefits already used by approved13th-month pay.'using errcode='23514';end if;
 if(w->>'final_compensation_reviewed')::boolean is not true or(w->>'previous_employers_reviewed')::boolean is not true or(w->>'benefits_reviewed')::boolean is not true or btrim(w->>'review_note')=''or btrim(w->>'settlement_note')=''then raise exception 'Review final actual compensation, prior employers, shared benefits and settlement evidence, including zero.'using errcode='23514';end if;
 return c;
end;$$;
create function hris_private.annual_tax_empty()returns jsonb language sql immutable security invoker set search_path=''as $$
 select '{"openings":[],"previous_employers":[],"reclassified_benefit_cents":0,"classification_note":"","taxable_adjustment_cents":0,"adjustment_note":"","last_pay_available_cents":0,"settlement_note":"","final_compensation_reviewed":false,"previous_employers_reviewed":false,"benefits_reviewed":false,"review_note":""}'::jsonb;
$$;
revoke all on function hris_private.annual_tax_amount(bigint),hris_private.annual_tax_income(jsonb),hris_private.annual_tax_signed(jsonb,text),hris_private.annual_tax_dates(integer,text,date,date),hris_private.annual_tax_compute(jsonb,jsonb),hris_private.annual_tax_ready(jsonb,jsonb),hris_private.annual_tax_empty()from public,anon,authenticated;
commit;
