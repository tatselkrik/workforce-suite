-- Additive Philippine monthly payroll; immutable previously deployed HR migrations.
begin;
create table public.payroll_profiles(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid not null,
 effective_on date not null check(extract(day from effective_on)=1 and effective_on between date '2026-02-01'and date '2200-12-01'),
 pay jsonb not null check(jsonb_typeof(pay)='object'and octet_length(pay::text)<=32768),revision bigint not null default 0 check(revision>=0),
 created_by uuid not null references auth.users(id),created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),unique(company_id,employee_id,effective_on),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_profiles_creator_idx on public.payroll_profiles(created_by);
create table public.payroll_holidays(
 id uuid primary key,company_id uuid not null references public.companies(id),date date not null check(date between date '2026-02-01'and date '2200-12-31'),
 name text not null check(char_length(btrim(name))between 1 and 120),kind text not null check(kind in('ordinary','special_working','special','double_special','regular','double_regular')),
 source text not null check(char_length(btrim(source))between 1 and 500),revision bigint not null default 0 check(revision>=0),unique(company_id,date)
);
create table public.payroll_runs(
 id uuid primary key,company_id uuid not null references public.companies(id),month date not null check(extract(day from month)=1 and month between date '2026-02-01'and date '2200-12-01'),
 title text not null check(char_length(btrim(title))between 1 and 120),status text not null default 'draft'check(status in('draft','submitted','approved','paid','cancelled')),
 revision bigint not null default 0 check(revision>=0),created_by uuid not null references auth.users(id),submitted_by uuid references auth.users(id),approved_by uuid references auth.users(id),
 paid_on date,payment_reference text not null default ''check(char_length(payment_reference)<=500),rule_version text not null default 'PH-private-2026-10-23-v1',
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),unique(company_id,id),
 check((status in('approved','paid')and approved_by is not null and submitted_by is not null and approved_by<>submitted_by)or(status not in('approved','paid')and approved_by is null)),
 check((status='paid'and paid_on is not null and btrim(payment_reference)<>'')or(status<>'paid'and paid_on is null and payment_reference=''))
);
create unique index payroll_runs_month_idx on public.payroll_runs(company_id,month)where status<>'cancelled';
create index payroll_runs_creator_idx on public.payroll_runs(created_by);
create index payroll_runs_submitter_idx on public.payroll_runs(submitted_by);
create index payroll_runs_approver_idx on public.payroll_runs(approved_by);
create table public.payroll_items(
 id uuid primary key,company_id uuid not null,run_id uuid not null,employee_id uuid not null,employee_name text not null,employee_code text not null,department text not null,
 profile jsonb not null,source_basis jsonb not null check(octet_length(source_basis::text)<=2097152),worksheet jsonb not null,totals jsonb not null,
 revision bigint not null default 0 check(revision>=0),unique(company_id,id),unique(company_id,run_id,employee_id),
 foreign key(company_id,run_id)references public.payroll_runs(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_items_employee_idx on public.payroll_items(company_id,employee_id);
create table public.payroll_slips(
 id uuid primary key,company_id uuid not null,run_id uuid not null,employee_id uuid not null,month date not null,company_name text not null,item jsonb not null,
 approved_by uuid not null references auth.users(id),approved_at timestamptz not null default clock_timestamp(),rule_version text not null,
 unique(company_id,run_id,employee_id),foreign key(company_id,run_id)references public.payroll_runs(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_slips_employee_idx on public.payroll_slips(company_id,employee_id,month);
create index payroll_slips_approver_idx on public.payroll_slips(approved_by);
create trigger payroll_slips_immutable before update or delete on public.payroll_slips for each row execute function hris_private.employment_history_immutable();
create table public.payroll_events(
 id uuid primary key default gen_random_uuid(),company_id uuid not null references public.companies(id),record_id uuid not null,kind text not null,
 reason text not null check(char_length(reason)between 1 and 500),actor_id uuid not null references auth.users(id),actor_name text not null,actor_role text not null,
 before_record jsonb,after_record jsonb not null,recorded_at timestamptz not null default clock_timestamp()
);
create index payroll_events_record_idx on public.payroll_events(company_id,record_id,recorded_at);
create index payroll_events_actor_idx on public.payroll_events(actor_id);
create trigger payroll_events_immutable before update or delete on public.payroll_events for each row execute function hris_private.employment_history_immutable();
create table hris_private.payroll_birth(company_id uuid not null references public.companies(id),operation text not null,record_id uuid not null,actor_id uuid not null references auth.users(id),payload jsonb not null,primary key(company_id,operation,record_id));
create index payroll_birth_actor_idx on hris_private.payroll_birth(actor_id);
alter table hris_private.payroll_birth enable row level security;
revoke all on hris_private.payroll_birth from public,anon,authenticated;
create function hris_private.payroll_hr(p_company uuid)returns boolean language sql stable security definer set search_path=''as $$
 select exists(select 1 from public.company_members where company_id=p_company and user_id=(select auth.uid())and active and role in('administrator','hr_generalist'));
$$;
create function hris_private.payroll_own(p_company uuid,p_employee uuid)returns boolean language sql stable security definer set search_path=''as $$
 select exists(select 1 from public.company_members where company_id=p_company and user_id=(select auth.uid())and active and employee_id=p_employee);
$$;
revoke all on function hris_private.payroll_hr(uuid),hris_private.payroll_own(uuid,uuid)from public,anon,authenticated;
grant execute on function hris_private.payroll_hr(uuid),hris_private.payroll_own(uuid,uuid)to authenticated;
do $$declare t text;begin
 foreach t in array array['payroll_profiles','payroll_holidays','payroll_runs','payroll_items','payroll_events','payroll_slips']loop
  execute format('alter table public.%I enable row level security',t);
  execute format('revoke all on public.%I from public,anon,authenticated',t);
  execute format('grant select on public.%I to authenticated',t);
  execute format('create policy %I on public.%I for select to authenticated using(hris_private.payroll_hr(company_id))',t||'_hr_read',t);
 end loop;
end;$$;
create policy payroll_slips_own_read on public.payroll_slips for select to authenticated using(hris_private.payroll_own(company_id,employee_id));

create function hris_private.payroll_text(j jsonb,k text,maxlen integer,required boolean default false)returns text language plpgsql immutable security invoker set search_path=''as $$
declare s text;begin
 if jsonb_typeof(j->k)is distinct from 'string'then raise exception 'Payroll text field % is required.',k using errcode='23514';end if;
 s:=btrim(j->>k);if char_length(s)>maxlen or(required and s='')or s~'[\x00-\x08\x0B\x0C\x0E-\x1F]'then raise exception 'Check payroll text field %.',k using errcode='23514';end if;return s;
end;$$;
create function hris_private.payroll_amount(j jsonb,k text,maximum bigint default 10000000000)returns bigint language plpgsql immutable security invoker set search_path=''as $$
declare v bigint;begin
 if jsonb_typeof(j->k)is distinct from 'number'or(j->>k)!~'^[0-9]+$'then raise exception 'Use integer non-negative centavos/minutes for %.',k using errcode='23514';end if;
 v:=(j->>k)::bigint;if v>maximum then raise exception 'Payroll amount % exceeds its bound.',k using errcode='23514';end if;return v;
end;$$;
create function hris_private.payroll_components(cs jsonb)returns void language plpgsql immutable security invoker set search_path=''as $$
declare c jsonb;k text;begin
 if jsonb_typeof(cs)is distinct from 'array'or jsonb_array_length(cs)>40 then raise exception 'Use at most40 payroll components.'using errcode='23514';end if;
 for c in select value from jsonb_array_elements(cs)loop
  if jsonb_typeof(c)is distinct from 'object'or c-array['label','kind','amount_cents','reference']<>'{}'::jsonb then raise exception 'Unsupported payroll component fields.'using errcode='23514';end if;
  perform hris_private.payroll_text(c,'label',100,true);k:=hris_private.payroll_text(c,'kind',40,true);perform hris_private.payroll_text(c,'reference',500);perform hris_private.payroll_amount(c,'amount_cents');
  if k not in('taxable_allowance','rice','laundry','reimbursement','deduction','advance','employer_benefit')or(k in('reimbursement','deduction','advance')and btrim(c->>'reference')='')then raise exception 'Use supported components and record evidence/consent or an advance reference.'using errcode='23514';end if;
 end loop;
end;$$;
create function hris_private.payroll_pay(p jsonb)returns void language plpgsql immutable security invoker set search_path=''as $$
declare k text;begin
 if jsonb_typeof(p)is distinct from 'object'or p-array['basis','rate_cents','annual_divisor','monthly_reference_cents','premium_eligible','minimum_wage_earner','sss','philhealth','pagibig','coverage_note','components','tin','sss_number','philhealth_number','pagibig_number']<>'{}'::jsonb then raise exception 'Unsupported salary fields.'using errcode='23514';end if;
 if hris_private.payroll_text(p,'basis',10,true)not in('monthly','daily')or hris_private.payroll_amount(p,'rate_cents')=0 or hris_private.payroll_amount(p,'annual_divisor',366)not in(365,366)then raise exception 'Use positive monthly/daily pay and a365/366 divisor for true monthly-paid salary.'using errcode='23514';end if;
 if hris_private.payroll_amount(p,'monthly_reference_cents')=0 and p->>'basis'='daily'then raise exception 'Daily pay needs a fixed monthly reference for PhilHealth.'using errcode='23514';end if;
 foreach k in array array['premium_eligible','minimum_wage_earner','sss','philhealth','pagibig']loop if jsonb_typeof(p->k)is distinct from 'boolean'then raise exception 'Coverage fields must be explicit booleans.'using errcode='23514';end if;end loop;
 if (not(p->>'premium_eligible')::boolean or(p->>'minimum_wage_earner')::boolean or not(p->>'sss')::boolean or not(p->>'philhealth')::boolean or not(p->>'pagibig')::boolean)and hris_private.payroll_text(p,'coverage_note',1000)=''then raise exception 'Document legal coverage exceptions or minimum-wage-earner status.'using errcode='23514';end if;
 perform hris_private.payroll_text(p,'coverage_note',1000);foreach k in array array['tin','sss_number','philhealth_number','pagibig_number']loop perform hris_private.payroll_text(p,k,40);end loop;
 perform hris_private.payroll_components(p->'components');
end;$$;
create function hris_private.payroll_factor(k text,rest boolean)returns integer language plpgsql immutable security invoker set search_path=''as $$
begin
 return case when k in('ordinary','special_working')then case when rest then 130 else 100 end when k='special'then case when rest then 150 else 130 end when k='double_special'then case when rest then 195 else 150 end when k='regular'then case when rest then 260 else 200 end when k='double_regular'then case when rest then 390 else 300 end else null end;
end;$$;
create function hris_private.payroll_tax(t bigint)returns bigint language sql immutable security invoker set search_path=''as $$
 select case when t<=2083300 then 0 when t<3333300 then round((t-2083300)*0.15)::bigint when t<6666700 then 187500+round((t-3333300)*0.20)::bigint when t<16666700 then 854180+round((t-6666700)*0.25)::bigint when t<66666700 then 3354180+round((t-16666700)*0.30)::bigint else 18354180+round((t-66666700)*0.35)::bigint end;
$$;
create function hris_private.payroll_compute(p jsonb,w jsonb,mon date)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
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
  if regular_m+leave_m+unpaid_m>480 or night_m>regular_m or night_ot>ot_m or(holiday_m>0 and(regular_m>0 or ot_m>0 or d->>'kind'not in('regular','double_regular')))then raise exception 'Check regular/OT/night minutes and unworked-holiday eligibility.'using errcode='23514';end if;
  if not eligible then f:=100;end if;ot:=case when f=100 then 125 else 130 end;
  if is_monthly then basic:=basic-round(dn*unpaid_m/(dd*480))::bigint;
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

create function hris_private.payroll_source_lock()returns trigger language plpgsql security definer set search_path=''as $$
begin
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended((case when tg_op='DELETE'then old.company_id else new.company_id end)::text,7));
 if tg_op='DELETE'then return old;end if;return new;
end;$$;
revoke all on function hris_private.payroll_source_lock()from public,anon,authenticated;
do $$declare t text;begin
 foreach t in array array['employees','attendance','attendance_breaks','leave_requests','overtime_requests','leave_policies','payroll_profiles','payroll_holidays']loop
  execute format('create trigger payroll_source_serialization before insert or update or delete on public.%I for each row execute function hris_private.payroll_source_lock()',t);
 end loop;
end;$$;
create function hris_private.payroll_basis(cid uuid,eid uuid,mon date)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare p public.payroll_profiles;e public.employees;zone text;lo timestamptz;hi timestamptz;result jsonb;begin
 select * into e from public.employees where company_id=cid and id=eid;
 select * into p from public.payroll_profiles where company_id=cid and employee_id=eid and effective_on<=mon order by effective_on desc limit 1;
 if e.joined_on >= (mon+interval '1 month')::date then raise exception 'This employee joined after the payroll month.'using errcode='23514';end if;
 if e.id is null or p.id is null then raise exception 'Save a salary profile effective on/before the month first.'using errcode='23514';end if;
 select timezone into zone from public.companies where id=cid;lo:=mon::timestamp at time zone zone;hi:=(mon+interval '1month')::timestamp at time zone zone;
 select jsonb_build_object('employee',jsonb_build_object('id',e.id,'full_name',e.full_name,'employee_code',e.employee_code,'department',e.department,'joined_on',e.joined_on,'status',e.status),'profile',to_jsonb(p),'timezone',zone,
 'attendance',coalesce((select jsonb_agg(jsonb_build_object('record',to_jsonb(a),'breaks',coalesce((select jsonb_agg(to_jsonb(b)order by b.id)from public.attendance_breaks b where b.attendance_id=a.id and not b.voided),'[]'::jsonb),'overtime',coalesce((select jsonb_agg(to_jsonb(r)order by r.id)from public.overtime_requests r where r.attendance_id=a.id),'[]'::jsonb),'effective_ot',coalesce((select sum(r.approved_minutes)from public.overtime_requests r where r.attendance_id=a.id and r.status='approved'and r.basis=hris_private.overtime_basis(a)),0))order by a.id)from public.attendance a where a.company_id=cid and a.employee_id=eid and((a.checked_in_at<hi and coalesce(a.checked_out_at,a.checked_in_at)>=lo)or(a.work_date>=mon and a.work_date<(mon+interval '1month')::date))),'[]'::jsonb),
 'leave',coalesce((select jsonb_agg(to_jsonb(l)order by l.id)from public.leave_requests l where l.company_id=cid and l.employee_id=eid and l.starts_on<(mon+interval '1month')::date and l.ends_on>=mon),'[]'::jsonb),
 'holidays',coalesce((select jsonb_agg(to_jsonb(h)order by h.date)from public.payroll_holidays h where h.company_id=cid and h.date>=mon and h.date<(mon+interval '1month')::date),'[]'::jsonb),
 'leave_holidays',coalesce((select jsonb_agg(h order by h->>'date')from public.leave_policies l cross join lateral jsonb_array_elements(l.holidays)h where l.company_id=cid and(h->>'date')::date>=mon and(h->>'date')::date<(mon+interval '1month')::date),'[]'::jsonb))into result;
 if octet_length(result::text)>2097152 then raise exception 'Payroll source facts exceed the review bound.'using errcode='23514';end if;return result;
end;$$;
create function hris_private.payroll_check_item(i public.payroll_items,mon date)returns void language plpgsql stable security invoker set search_path=''as $$
declare fresh jsonb;d jsonb;ot_minutes integer;cap integer;begin
 fresh:=hris_private.payroll_basis(i.company_id,i.employee_id,mon);
 if fresh is distinct from i.source_basis then raise exception 'Payroll source facts changed. Refresh each affected worksheet source and review again.'using errcode='PT409';end if;
 if coalesce((i.worksheet->>'reviewed')::boolean,false)is not true or btrim(coalesce(i.worksheet->>'review_note',''))=''then raise exception 'Review every employee worksheet before submitting payroll.'using errcode='23514';end if;
 if (fresh#>>'{employee,joined_on}')::date>mon and i.profile#>>'{pay,basis}'='monthly'and i.worksheet->'basic_override_cents'='null'::jsonb then raise exception 'A mid-month joiner needs reviewed basic proration.'using errcode='23514';end if;
 if exists(select 1 from jsonb_array_elements(fresh->'leave_holidays')h where not exists(select 1 from jsonb_array_elements(fresh->'holidays')p where p->>'date'=h->>'date'))then raise exception 'Classify the month''s leave-calendar holidays in Payroll holidays before approval.'using errcode='23514';end if;
 for d in select value from jsonb_array_elements(i.worksheet->'days')loop
  if exists(select 1 from jsonb_array_elements(fresh->'holidays')h where h->>'date'=d->>'date'and h->>'kind'<>d->>'kind')then raise exception 'A worksheet day differs from the payroll holiday classification.'using errcode='23514';end if;
 end loop;
 select coalesce(sum((dt.value->>'overtime_minutes')::integer),0)into ot_minutes from jsonb_array_elements(i.worksheet->'days')as dt(value);
 select coalesce(sum((a->>'effective_ot')::integer),0)into cap from jsonb_array_elements(fresh->'attendance')a;
 if ot_minutes>cap and btrim(i.worksheet->>'exception_note')=''then raise exception 'OT exceeds current authorizations. Record the payroll exception review before paying it.'using errcode='23514';end if;
 if(i.totals->>'net')::bigint<0 then raise exception 'Negative net pay requires correcting deductions/advances before approval.'using errcode='23514';end if;
 if i.totals is distinct from hris_private.payroll_compute(i.profile->'pay',i.worksheet,mon)then raise exception 'Payroll calculation does not reconcile.'using errcode='23514';end if;
end;$$;

-- Publish compensation facts, excluding HR-only review/coverage/evidence notes.
create function hris_private.payroll_published_item(i public.payroll_items)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare j jsonb:=to_jsonb(i)-'source_basis';cs jsonb;begin
 j:=jsonb_set(j,'{profile,pay,coverage_note}','""'::jsonb);
 j:=jsonb_set(jsonb_set(j,'{worksheet,review_note}','""'::jsonb),'{worksheet,exception_note}','""'::jsonb);
 select coalesce(jsonb_agg(jsonb_set(c,'{reference}','""'::jsonb)),'[]'::jsonb)into cs from jsonb_array_elements(j#>'{profile,pay,components}')c;
 j:=jsonb_set(j,'{profile,pay,components}',cs);
 select coalesce(jsonb_agg(jsonb_set(c,'{reference}','""'::jsonb)),'[]'::jsonb)into cs from jsonb_array_elements(j#>'{worksheet,adjustments}')c;
 return jsonb_set(j,'{worksheet,adjustments}',cs);
end;$$;
revoke all on function hris_private.payroll_published_item(public.payroll_items)from public,anon,authenticated;

create function hris_private.manage_payroll(p_operation text,p_record jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;actor_label text;today date;rid uuid;expected bigint;why text;old_record jsonb;result jsonb;birth hris_private.payroll_birth;
 p public.payroll_profiles;h public.payroll_holidays;r public.payroll_runs;i public.payroll_items;e public.employees;fresh jsonb;w jsonb;t jsonb;mon date;creating boolean:=false;allowed text[];
begin
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Active HR or Administrator access is required.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));perform 1 from public.companies where id=m.company_id for update;
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist')for update;
 if m.user_id is null then raise exception 'Your payroll access changed. Refresh.'using errcode='42501';end if;
 select full_name into actor_label from public.employees where company_id=m.company_id and id=m.employee_id;select(now()at time zone timezone)::date into today from public.companies where id=m.company_id;
 if jsonb_typeof(p_record)is distinct from 'object'or octet_length(p_record::text)>131072 then raise exception 'Enter bounded payroll details.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=hris_private.payroll_text(p_record,'reason',500,true);
 if rid is null then raise exception 'A payroll record identity is required.'using errcode='23514';end if;
 allowed:=case p_operation when'profile_save'then array['employee_id','effective_on','pay']when'holiday_save'then array['date','name','kind','source']when'run_create'then array['month','title']when'item_add'then array['run_id','employee_id']when'item_save'then array['worksheet']when'run_submit'then array['scope_reviewed']when'run_paid'then array['paid_on','payment_reference']else array[]::text[]end;
 if p_record-(allowed||array['id','expected_revision','reason'])<>'{}'::jsonb then raise exception 'Unsupported payroll request fields.'using errcode='23514';end if;
 if expected is null and p_operation in('profile_save','holiday_save','run_create','item_add')then
  select * into birth from hris_private.payroll_birth where company_id=m.company_id and operation=p_operation and record_id=rid;
  if birth.record_id is not null then
   if birth.actor_id<>m.user_id or birth.payload<>p_record then raise exception 'This payroll record ID already exists.'using errcode='PT409';end if;
   if p_operation='profile_save'then select to_jsonb(x)into result from public.payroll_profiles x where id=rid;
   elsif p_operation='holiday_save'then select to_jsonb(x)into result from public.payroll_holidays x where id=rid;
   elsif p_operation='run_create'then select to_jsonb(x)into result from public.payroll_runs x where id=rid;
   else select to_jsonb(x)into result from public.payroll_items x where id=rid;end if;
   if result is null then raise exception 'The prior record was removed. Refresh.'using errcode='PT409';end if;return result;
  end if;creating:=true;
 end if;
 if p_operation='profile_save'then
  select * into e from public.employees where company_id=m.company_id and id=(p_record->>'employee_id')::uuid;
  if e.id is null then raise exception 'Choose an employee in this company.'using errcode='42501';end if;
  perform hris_private.payroll_pay(p_record->'pay');mon:=(p_record->>'effective_on')::date;
  if creating then insert into public.payroll_profiles(id,company_id,employee_id,effective_on,pay,created_by)values(rid,m.company_id,e.id,mon,p_record->'pay',m.user_id)returning * into p;
  else select * into p from public.payroll_profiles where company_id=m.company_id and id=rid for update;
   if p.id is null then raise exception 'Salary profile unavailable.'using errcode='42501';end if;
   if p.revision is distinct from expected then raise exception 'Salary profile changed. Refresh.'using errcode='PT409';end if;
   if p.employee_id<>e.id or p.effective_on is distinct from mon then raise exception 'Keep this version''s employee and effective date; add a new salary version.'using errcode='23514';end if;
   old_record:=to_jsonb(p);update public.payroll_profiles set pay=p_record->'pay',revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into p;
  end if;result:=to_jsonb(p);
 elsif p_operation='holiday_save'then
  if creating then insert into public.payroll_holidays(id,company_id,date,name,kind,source)values(rid,m.company_id,(p_record->>'date')::date,hris_private.payroll_text(p_record,'name',120,true),p_record->>'kind',hris_private.payroll_text(p_record,'source',500,true))returning * into h;
  else select * into h from public.payroll_holidays where company_id=m.company_id and id=rid for update;
   if h.id is null then raise exception 'Payroll holiday unavailable.'using errcode='42501';end if;if h.revision is distinct from expected then raise exception 'Payroll holiday changed. Refresh.'using errcode='PT409';end if;
   old_record:=to_jsonb(h);update public.payroll_holidays set date=(p_record->>'date')::date,name=hris_private.payroll_text(p_record,'name',120,true),kind=p_record->>'kind',source=hris_private.payroll_text(p_record,'source',500,true),revision=revision+1 where id=rid returning * into h;
  end if;result:=to_jsonb(h);
 elsif p_operation='run_create'then
  if not creating then raise exception 'Use a new run ID.'using errcode='23514';end if;
  insert into public.payroll_runs(id,company_id,month,title,created_by)values(rid,m.company_id,(p_record->>'month')::date,hris_private.payroll_text(p_record,'title',120,true),m.user_id)returning * into r;result:=to_jsonb(r);
 elsif p_operation in('item_add','item_save','item_refresh','item_remove')then
  if p_operation='item_add'then select * into r from public.payroll_runs where company_id=m.company_id and id=(p_record->>'run_id')::uuid for update;
  else select * into i from public.payroll_items where company_id=m.company_id and id=rid for update;
   if i.id is null then raise exception 'Payroll worksheet unavailable.'using errcode='42501';end if;
   if i.revision is distinct from expected then raise exception 'Payroll worksheet changed. Refresh.'using errcode='PT409';end if;
   select * into r from public.payroll_runs where company_id=m.company_id and id=i.run_id for update;old_record:=to_jsonb(i);
  end if;
  if r.id is null then raise exception 'Payroll run unavailable.'using errcode='42501';end if;
  if r.status<>'draft'then raise exception 'Return submitted payroll to draft before changing worksheets. Approved payroll is immutable.'using errcode='23514';end if;
  if p_operation='item_remove'then delete from public.payroll_items where id=i.id;result:=old_record;
  else
   fresh:=hris_private.payroll_basis(m.company_id,case when p_operation='item_add'then(p_record->>'employee_id')::uuid else i.employee_id end,r.month);
   if p_operation='item_save'and fresh is distinct from i.source_basis then raise exception 'Sources changed. Use Refresh source, then review again.'using errcode='PT409';end if;
   if p_operation='item_add'then
    w:='{"days":[],"adjustments":[],"basic_override_cents":null,"sss_basis_override":null,"philhealth_basis_override":null,"pagibig_basis_override":null,"exception_note":"","review_note":"","reviewed":false}'::jsonb;
    insert into public.payroll_items(id,company_id,run_id,employee_id,employee_name,employee_code,department,profile,source_basis,worksheet,totals)values(rid,m.company_id,r.id,(fresh#>>'{employee,id}')::uuid,fresh#>>'{employee,full_name}',fresh#>>'{employee,employee_code}',coalesce(fresh#>>'{employee,department}',''),fresh->'profile',fresh,w,hris_private.payroll_compute(fresh#>'{profile,pay}',w,r.month))returning * into i;
   else
    w:=case when p_operation='item_save'then p_record->'worksheet'else jsonb_set(jsonb_set(i.worksheet,'{reviewed}','false'::jsonb),'{review_note}','""'::jsonb)end;
    t:=hris_private.payroll_compute(fresh#>'{profile,pay}',w,r.month);
    update public.payroll_items set employee_name=fresh#>>'{employee,full_name}',employee_code=fresh#>>'{employee,employee_code}',department=coalesce(fresh#>>'{employee,department}',''),profile=fresh->'profile',source_basis=fresh,worksheet=w,totals=t,revision=revision+1 where id=rid returning * into i;
   end if;result:=to_jsonb(i);
  end if;
  update public.payroll_runs set revision=revision+1,updated_at=clock_timestamp()where id=r.id;
 elsif p_operation in('run_submit','run_return','run_cancel','run_approve','run_paid')then
  select * into r from public.payroll_runs where company_id=m.company_id and id=rid for update;
  if r.id is null then raise exception 'Payroll run unavailable.'using errcode='42501';end if;
  if r.revision is distinct from expected then raise exception 'Payroll run changed. Refresh.'using errcode='PT409';end if;old_record:=to_jsonb(r);
  if p_operation in('run_submit','run_approve')then
   if (r.month+interval '1 month -1 day')::date>today or r.month>=date '2027-01-01'then raise exception 'Approve completed months only. A2027 register needs refreshed statutory rules.'using errcode='23514';end if;
   if not exists(select 1 from public.payroll_items where run_id=r.id)then raise exception 'Add at least one reviewed employee worksheet.'using errcode='23514';end if;
   for i in select * from public.payroll_items where company_id=m.company_id and run_id=r.id order by id loop perform hris_private.payroll_check_item(i,r.month);end loop;
  end if;
  if p_operation='run_submit'then
   if r.status<>'draft'or(p_record->>'scope_reviewed')::boolean is not true then raise exception 'Confirm which employees are included before submitting a draft.'using errcode='23514';end if;
   update public.payroll_runs set status='submitted',submitted_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='run_return'then
   if r.status<>'submitted'then raise exception 'Only submitted payroll can return to draft.'using errcode='23514';end if;
   update public.payroll_runs set status='draft',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='run_cancel'then
   if r.status not in('draft','submitted')then raise exception 'Approved payroll cannot be cancelled or overwritten. Use a later documented adjustment.'using errcode='23514';end if;
   update public.payroll_runs set status='cancelled',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='run_approve'then
   if m.role<>'administrator'or r.submitted_by=m.user_id then raise exception 'Another Administrator must approve submitted payroll.'using errcode='42501';end if;
   if r.status<>'submitted'then raise exception 'Submit the register before approval.'using errcode='23514';end if;
   insert into public.payroll_slips(id,company_id,run_id,employee_id,month,company_name,item,approved_by,rule_version)
    select pi.id,pi.company_id,pi.run_id,pi.employee_id,r.month,c.name,hris_private.payroll_published_item(pi),m.user_id,r.rule_version from public.payroll_items pi join public.companies c on c.id=pi.company_id where pi.company_id=m.company_id and pi.run_id=r.id;
   update public.payroll_runs set status='approved',approved_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  else
   if m.role<>'administrator'then raise exception 'Only Administrators record payroll payments.'using errcode='42501';end if;
   if r.status<>'approved'or(p_record->>'paid_on')::date>today or(p_record->>'paid_on')::date<r.month then raise exception 'Record an actual payment date for an approved run.'using errcode='23514';end if;
   update public.payroll_runs set status='paid',paid_on=(p_record->>'paid_on')::date,payment_reference=hris_private.payroll_text(p_record,'payment_reference',500,true),revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  end if;result:=to_jsonb(r);
 else raise exception 'Unknown payroll operation.'using errcode='23514';end if;
 if creating then insert into hris_private.payroll_birth(company_id,operation,record_id,actor_id,payload)values(m.company_id,p_operation,rid,m.user_id,p_record);end if;
 insert into public.payroll_events(company_id,record_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)values(m.company_id,rid,p_operation,why,m.user_id,actor_label,m.role,old_record,result);
 return result;
end;$$;
revoke all on function hris_private.payroll_text(jsonb,text,integer,boolean),hris_private.payroll_amount(jsonb,text,bigint),hris_private.payroll_components(jsonb),hris_private.payroll_pay(jsonb),hris_private.payroll_factor(text,boolean),hris_private.payroll_tax(bigint),hris_private.payroll_compute(jsonb,jsonb,date),hris_private.payroll_basis(uuid,uuid,date),hris_private.payroll_check_item(public.payroll_items,date)from public,anon,authenticated;
revoke all on function hris_private.manage_payroll(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_payroll(text,jsonb)to authenticated;
create function public.manage_payroll(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_payroll(p_operation,p_record);$$;
revoke all on function public.manage_payroll(text,jsonb)from public,anon,authenticated;
grant execute on function public.manage_payroll(text,jsonb)to authenticated;
commit;
