-- Independently approved final statements, atomic individual cash attribution and shared tax sources.
begin;
create table public.payroll_final_cases(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid not null,employment_start date not null,service_end date not null,cash_date date not null,
 title text not null check(char_length(btrim(title))between 1 and 120),employee_name text not null,employee_code text not null,department text not null,
 status text not null default 'draft'check(status in('draft','submitted','approved','settled','cancelled','withdrawn')),revision bigint not null default 0 check(revision>=0),
 created_by uuid not null references auth.users(id),prepared_by uuid not null references auth.users(id),submitted_by uuid references auth.users(id),approved_by uuid references auth.users(id),approved_at timestamptz,
 settled_on date,payment_reference text not null default '',handling_reference text not null default '',rule_version text not null default 'PH-final-pay-2026-10-24-v1',
 source_basis jsonb not null check(octet_length(source_basis::text)<=2097152),worksheet jsonb not null check(octet_length(worksheet::text)<=262144),totals jsonb not null,
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),unique(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id),
 check(employment_start<=service_end and service_end<=cash_date),check(char_length(payment_reference)<=500 and char_length(handling_reference)<=1000),
 check((status in('submitted','approved','settled','withdrawn')and submitted_by is not null)or(status in('draft','cancelled')and submitted_by is null)),
 check((status in('approved','settled','withdrawn')and approved_by is not null and approved_at is not null and approved_by<>submitted_by and approved_by<>prepared_by and approved_by<>created_by)or(status in('draft','submitted','cancelled')and approved_by is null and approved_at is null)),
 check((status='settled'and settled_on=cash_date and btrim(payment_reference)<>'')or(status<>'settled'and settled_on is null and payment_reference=''and handling_reference=''))
);
create index payroll_final_cases_employee_idx on public.payroll_final_cases(company_id,employee_id,cash_date,status);
create index payroll_final_cases_creator_idx on public.payroll_final_cases(created_by);
create index payroll_final_cases_preparer_idx on public.payroll_final_cases(prepared_by);
create index payroll_final_cases_submitter_idx on public.payroll_final_cases(submitted_by);
create index payroll_final_cases_approver_idx on public.payroll_final_cases(approved_by);
create table public.payroll_final_slips(
 id uuid primary key,company_id uuid not null,employee_id uuid not null,employment_start date not null,service_end date not null,cash_date date not null,
 company_name text not null,employee_name text not null,employee_code text not null,department text not null,totals jsonb not null,sources jsonb not null,components jsonb not null,
 approved_by uuid not null references auth.users(id),approved_at timestamptz not null,rule_version text not null,unique(company_id,id),
 foreign key(company_id,id)references public.payroll_final_cases(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_final_slips_employee_idx on public.payroll_final_slips(company_id,employee_id,cash_date);
create index payroll_final_slips_approver_idx on public.payroll_final_slips(approved_by);
-- Own terminal status contains no private bank/payment references or HR evidence.
create table public.payroll_final_status(
 id uuid primary key,company_id uuid not null,employee_id uuid not null,kind text not null check(kind in('settled','withdrawn')),recorded_on date not null,net_cents bigint not null check(net_cents>=0),
 foreign key(company_id,id)references public.payroll_final_slips(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_final_status_case_idx on public.payroll_final_status(company_id,id);
create index payroll_final_status_employee_idx on public.payroll_final_status(company_id,employee_id,recorded_on);
create trigger payroll_final_slips_immutable before update or delete on public.payroll_final_slips for each row execute function hris_private.employment_history_immutable();
create trigger payroll_final_status_immutable before update or delete on public.payroll_final_status for each row execute function hris_private.employment_history_immutable();
do $$declare t text;begin
 foreach t in array array['payroll_final_cases','payroll_final_slips','payroll_final_status']loop
  execute format('alter table public.%I enable row level security',t);execute format('revoke all on public.%I from public,anon,authenticated',t);execute format('grant select on public.%I to authenticated',t);
 end loop;
end;$$;
create policy payroll_final_cases_hr_read on public.payroll_final_cases for select to authenticated using(hris_private.payroll_hr(company_id));
create policy payroll_final_slips_read on public.payroll_final_slips for select to authenticated using(hris_private.payroll_hr(company_id)or hris_private.payroll_own(company_id,employee_id));
create policy payroll_final_status_read on public.payroll_final_status for select to authenticated using(hris_private.payroll_hr(company_id)or hris_private.payroll_own(company_id,employee_id));

create function hris_private.annual_tax_basis_before_final(cid uuid,eid uuid,yr integer,asof date,service_end date,exclude_id uuid)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare e public.employees;lo date;income jsonb;payments jsonb;previous jsonb;adjustment bigint;pending integer;pending_adjustments integer;ext bigint;other bigint;latest date;begin
 select * into e from public.employees where company_id=cid and id=eid;
 if e.id is null then raise exception 'Employee unavailable.'using errcode='42501';end if;
 lo:=greatest(make_date(yr,1,1),coalesce(e.joined_on,make_date(yr,1,1)));
 if lo>service_end then raise exception 'Employee joined after this service end.'using errcode='23514';end if;
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
create function hris_private.thirteenth_basis_before_final(cid uuid,eid uuid,yr integer,asof date)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare e public.employees;lo date;ps jsonb;prior jsonb;ext bigint;other bigint;latest date;begin
 select * into e from public.employees where company_id=cid and id=eid;
 if e.id is null then raise exception 'Employee unavailable.'using errcode='42501';end if;
 if yr not between 2026 and 2200 or extract(year from asof)<>yr then raise exception 'Use an as-of date in the benefit year.'using errcode='23514';end if;
 lo:=greatest(make_date(yr,1,1),coalesce(e.joined_on,make_date(yr,1,1)));
 if lo>asof then raise exception 'Employee joined after this benefit coverage.'using errcode='23514';end if;
 select coalesce(jsonb_agg(x.p order by x.p->>'starts_on',x.p->>'id'),'[]')into ps from(
  select jsonb_build_object('id',s.id,'starts_on',greatest(s.month,lo),'ends_on',(s.month+interval '1month -1day')::date,'basic_cents',(s.item#>>'{totals,basic}')::bigint)p
   from public.payroll_slips s where s.company_id=cid and s.employee_id=eid and extract(year from s.month)=yr and(s.month+interval '1month -1day')::date<=asof
  union all
  select jsonb_build_object('id',s.id,'starts_on',greatest(s.starts_on,lo),'ends_on',s.ends_on,'basic_cents',(s.item#>>'{totals,basic}')::bigint)p
   from public.payroll_cutoff_slips s where s.company_id=cid and s.employee_id=eid and extract(year from s.month)=yr and s.ends_on<=asof
 )x;
 select coalesce(jsonb_agg(jsonb_build_object('id',s.id,'gross_cents',(s.item#>>'{totals,gross_cents}')::bigint,'exempt_cents',(s.item#>>'{totals,exempt_cents}')::bigint)order by s.id),'[]'),coalesce(max((s.item#>>'{worksheet,external_thirteenth_cents}')::bigint),0),coalesce(max((s.item#>>'{worksheet,other_benefits_cents}')::bigint),0),max(s.as_of)
 into prior,ext,other,latest from public.payroll_thirteenth_slips s where s.company_id=cid and s.employee_id=eid and s.year=yr;
 if latest>asof then raise exception 'A later annual-benefit payout is already issued; reconcile through that coverage or later.'using errcode='23514';end if;
 return jsonb_build_object('employee',jsonb_build_object('id',e.id,'full_name',e.full_name,'employee_code',e.employee_code,'department',e.department,'joined_on',e.joined_on,'status',e.status),
  'calculation_source',jsonb_build_object('starts_on',lo,'as_of',asof,'periods',ps,'prior',prior,'external_thirteenth_floor',ext,'other_benefits_floor',other));
end;$$;

-- Full source nets can be discharged through final statutory withholding/offsets.
-- Only the final statement's net is the actual combined cash, not a second receipt payment.
alter table public.payroll_payment_receipts add column final_case_id uuid references public.payroll_final_cases(id);
create index payroll_payment_receipts_final_idx on public.payroll_payment_receipts(final_case_id);
alter table public.payroll_payment_receipts drop constraint payroll_payment_receipts_origin_check;
alter table public.payroll_payment_receipts add check(origin in('individual','batch','legacy_batch','final_settlement'));
alter table public.payroll_payment_receipts add check((origin='final_settlement')=(final_case_id is not null));

-- Read-only helpers retain the existing algorithms without changing installed clients.
create or replace function hris_private.thirteenth_basis(cid uuid,eid uuid,yr integer,asof date)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare b jsonb;external_other bigint;final_shared bigint;facts jsonb;begin
 b:=hris_private.thirteenth_basis_before_final(cid,eid,yr,asof);
 select coalesce(max(greatest((s.item#>>'{worksheet,other_benefits_cents}')::bigint-coalesce((i.source_basis->>'final_shared_benefits_cents')::bigint,0),0)),0)into external_other
 from public.payroll_thirteenth_slips s join public.payroll_thirteenth_items i on i.company_id=s.company_id and i.id=s.id where s.company_id=cid and s.employee_id=eid and s.year=yr;
 select coalesce(sum((p.totals->>'shared_extras_cents')::bigint),0)::bigint,coalesce(jsonb_agg(jsonb_build_object('id',p.id,'cash_date',p.cash_date,'shared_cents',p.totals->'shared_extras_cents')order by p.id),'[]')into final_shared,facts
 from public.payroll_final_cases p where p.company_id=cid and p.employee_id=eid and p.status='settled'and extract(year from p.cash_date)=yr;
 return jsonb_set(b,'{calculation_source,other_benefits_floor}',to_jsonb(external_other+final_shared))||jsonb_build_object('final_shared_benefits_cents',final_shared,'final_benefits',facts);
end;$$;
create or replace function hris_private.annual_tax_basis(cid uuid,eid uuid,yr integer,asof date,service_end date,exclude_id uuid)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare b jsonb;p public.payroll_final_cases;cs jsonb;income jsonb;facts jsonb:='[]';adjustment bigint;pending integer;latest date;external_other bigint;begin
 b:=hris_private.annual_tax_basis_before_final(cid,eid,yr,asof,service_end,exclude_id);cs:=b->'calculation_source';income:=cs->'income';adjustment:=(cs->>'prior_adjustment_cents')::bigint;pending:=(cs->>'pending_adjustments')::integer;
 for p in select *from public.payroll_final_cases x where x.company_id=cid and x.employee_id=eid and x.id is distinct from exclude_id and extract(year from x.cash_date)=yr and x.status in('approved','settled')order by x.id loop
  if p.cash_date>asof then raise exception 'Use a payment as-of date at least as late as the earlier final statement.'using errcode='23514';end if;
  if p.status='approved'then pending:=pending+1;else
   income:=income||jsonb_build_array(jsonb_build_object('id','final:'||p.id::text,'starts_on',null,'ends_on',null,'taxable_cents',(p.totals->>'taxable_extras_cents')::bigint+(p.totals->>'shared_extras_cents')::bigint,'withholding_cents',0,'benefit_gross_cents',p.totals->'shared_extras_cents','benefit_exempt_cents',0,'contribution_cents',p.totals->'additional_contributions_cents'));
   adjustment:=adjustment+(p.totals#>>'{tax,adjustment_cents}')::bigint;
  end if;
  facts:=facts||jsonb_build_array(jsonb_build_object('id',p.id,'status',p.status,'cash_date',p.cash_date,'revision',p.revision,'totals',p.totals));
 end loop;
 -- Known final benefits already occur as income; only off-system other-benefit facts need openings/reclassification.
 select coalesce(max(greatest((s.item#>>'{worksheet,other_benefits_cents}')::bigint-coalesce((i.source_basis->>'final_shared_benefits_cents')::bigint,0),0)),0)into external_other
 from public.payroll_thirteenth_slips s join public.payroll_thirteenth_items i on i.company_id=s.company_id and i.id=s.id
 join public.payroll_payment_receipts r on r.kind='thirteenth'and r.slip_id=s.id where s.company_id=cid and s.employee_id=eid and s.year=yr and s.as_of<=asof and extract(year from r.paid_on)=yr and r.paid_on<=asof;
 cs:=cs||jsonb_build_object('income',income,'prior_adjustment_cents',adjustment,'pending_adjustments',pending,'other_benefits_floor',external_other);
 return b||jsonb_build_object('calculation_source',cs,'final_payments',facts);
end;$$;
create view hris_private.final_pay_source_slips with(security_invoker=true)as
 select p.*,x.first_on,x.last_on,x.taxable_cents,x.withholding_cents,x.benefit_gross_cents,x.benefit_exempt_cents,x.contribution_cents,x.coverage,x.external_thirteenth_floor,x.other_benefits_floor
 from hris_private.payroll_issued_payments p join(
  select 'monthly'::text kind,s.id,s.month first_on,(s.month+interval '1month -1day')::date last_on,(s.item#>>'{totals,taxable}')::bigint taxable_cents,(s.item#>>'{totals,withholding}')::bigint withholding_cents,0::bigint benefit_gross_cents,0::bigint benefit_exempt_cents,0::bigint contribution_cents,true coverage,0::bigint external_thirteenth_floor,0::bigint other_benefits_floor from public.payroll_slips s
  union all
  select 'cutoff',s.id,s.starts_on,s.ends_on,(s.item#>>'{totals,taxable}')::bigint,(s.item#>>'{totals,withholding}')::bigint,0,0,0,true,0,0 from public.payroll_cutoff_slips s
  union all
  select 'thirteenth',s.id,s.as_of,s.as_of,(s.item#>>'{totals,taxable_cents}')::bigint,(s.item#>>'{totals,withholding_cents}')::bigint,(s.item#>>'{totals,gross_cents}')::bigint,(s.item#>>'{totals,exempt_cents}')::bigint,(s.item#>>'{totals,sss_adjustment_cents}')::bigint+(s.item#>>'{totals,philhealth_adjustment_cents}')::bigint+(s.item#>>'{totals,pagibig_adjustment_cents}')::bigint,false,(s.item#>>'{worksheet,external_thirteenth_cents}')::bigint,greatest((s.item#>>'{worksheet,other_benefits_cents}')::bigint-coalesce((i.source_basis->>'final_shared_benefits_cents')::bigint,0),0) from public.payroll_thirteenth_slips s join public.payroll_thirteenth_items i on i.company_id=s.company_id and i.id=s.id
 )x on x.kind=p.kind and x.id=p.slip_id;
revoke all on hris_private.final_pay_source_slips from public,anon,authenticated;
create function hris_private.final_pay_basis(cid uuid,eid uuid,fin date,cash date,exclude_id uuid)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare e public.employees;b jsonb;cs jsonb;outstanding jsonb;selected jsonb;payments jsonb;prior jsonb;vacation bigint;ext bigint;other bigint;lo date;begin
 select *into e from public.employees where company_id=cid and id=eid;
 if e.id is null then raise exception 'Employee unavailable.'using errcode='42501';end if;
 if e.joined_on is null then raise exception 'Set the evidenced employment start in the employee record before final pay.'using errcode='23514';end if;
 b:=hris_private.annual_tax_basis(cid,eid,extract(year from cash)::integer,cash,fin,exclude_id);cs:=b->'calculation_source';lo:=(cs->>'starts_on')::date;
 select coalesce(jsonb_agg(jsonb_build_object('id',p.kind||':'||p.slip_id::text,'net_cents',p.amount_cents,'income',jsonb_build_object('id',p.kind||':'||p.slip_id::text,'starts_on',case when p.coverage and greatest(p.first_on,lo)<=least(p.last_on,fin)then greatest(p.first_on,lo)end,'ends_on',case when p.coverage and greatest(p.first_on,lo)<=least(p.last_on,fin)then least(p.last_on,fin)end,'taxable_cents',p.taxable_cents,'withholding_cents',p.withholding_cents,'benefit_gross_cents',p.benefit_gross_cents,'benefit_exempt_cents',p.benefit_exempt_cents,'contribution_cents',p.contribution_cents))order by p.kind,p.slip_id),'[]'),
 coalesce(jsonb_agg(jsonb_build_object('kind',p.kind,'slip_id',p.slip_id,'run_id',p.run_id,'ends_on',p.last_on,'amount_cents',p.amount_cents)order by p.kind,p.slip_id),'[]'),coalesce(max(p.external_thirteenth_floor),0),coalesce(max(p.other_benefits_floor),0)
 into outstanding,selected,ext,other from hris_private.final_pay_source_slips p where p.company_id=cid and p.employee_id=eid and p.status='approved'and p.last_on<=cash and not exists(select 1 from public.payroll_payment_receipts r where r.kind=p.kind and r.slip_id=p.slip_id);
 cs:=cs||jsonb_build_object('external_thirteenth_floor',greatest((cs->>'external_thirteenth_floor')::bigint,ext),'other_benefits_floor',greatest((cs->>'other_benefits_floor')::bigint,other));
 -- A coworker's payment changes a batch revision, not this person's immutable income/receipt facts.
 select coalesce(jsonb_agg((p-'revision')order by p->>'id'),'[]')into payments from jsonb_array_elements(b->'payments')p;
 b:=b||jsonb_build_object('payments',payments,'employee',(b->'employee')-'status');
 select coalesce(jsonb_agg(jsonb_build_object('id',p.id,'status',p.status,'employment_start',p.employment_start,'service_end',p.service_end,'cash_date',p.cash_date)order by p.id),'[]'),
 coalesce(max((p.worksheet#>>'{vacation,previous_days_hundredths}')::bigint+(p.worksheet#>>'{vacation,days_hundredths}')::bigint)filter(where p.status='settled'and extract(year from p.cash_date)=extract(year from cash)),0)
 into prior,vacation from public.payroll_final_cases p where p.company_id=cid and p.employee_id=eid and p.id is distinct from exclude_id and p.status in('approved','settled');
 return jsonb_build_object('annual_basis',b,'prior_final',prior,'selected',selected,'calculation_source',jsonb_build_object('employment_start',e.joined_on,'service_end',fin,'cash_date',cash,'annual',cs,'outstanding',outstanding,'previous_vacation_days_hundredths',vacation));
end;$$;
create function hris_private.final_pay_check(r public.payroll_final_cases)returns void language plpgsql stable security invoker set search_path=''as $$
declare fresh jsonb;t jsonb;begin
 fresh:=hris_private.final_pay_basis(r.company_id,r.employee_id,r.service_end,r.cash_date,r.id);
 if fresh is distinct from r.source_basis then raise exception 'Final-pay income, payments or employment facts changed. Refresh a draft, or withdraw an unpaid issued statement and prepare a new review.'using errcode='PT409';end if;
 t:=hris_private.final_pay_ready(fresh->'calculation_source',r.worksheet);
 if t is distinct from r.totals or r.rule_version<>'PH-final-pay-2026-10-24-v1'then raise exception 'Final-pay totals/rule version changed. Refresh.'using errcode='PT409';end if;
 if exists(select 1 from public.payroll_final_cases p where p.company_id=r.company_id and p.employee_id=r.employee_id and p.id<>r.id and p.status in('approved','settled')and p.employment_start<=r.service_end and p.service_end>=r.employment_start)then raise exception 'An overlapping employment already has an issued final statement. Resolve its unpaid statement or use the separate amendment process.'using errcode='23514';end if;
end;$$;
create function hris_private.final_pay_case_immutable()returns trigger language plpgsql security invoker set search_path=''as $$
begin
 if tg_op='DELETE'or old.status in('settled','withdrawn','cancelled')then raise exception 'Retain final-pay history.'using errcode='42501';end if;
 if old.status='approved'and(new.status not in('settled','withdrawn')or(to_jsonb(new)-array['status','settled_on','payment_reference','handling_reference','revision','updated_at'])is distinct from(to_jsonb(old)-array['status','settled_on','payment_reference','handling_reference','revision','updated_at']))then raise exception 'An issued final statement remains immutable; only actual settlement or unpaid withdrawal may be appended.'using errcode='42501';end if;
 return new;
end;$$;
create trigger payroll_final_case_immutable before update or delete on public.payroll_final_cases for each row execute function hris_private.final_pay_case_immutable();
create function hris_private.final_pay_receipt_guard()returns trigger language plpgsql security definer set search_path=''as $$
begin
 if new.origin='final_settlement'and not exists(select 1 from public.payroll_final_cases p cross join lateral jsonb_array_elements(p.source_basis->'selected')s where p.id=new.final_case_id and p.company_id=new.company_id and p.employee_id=new.employee_id and p.status='settled'and p.cash_date=new.paid_on and s->>'kind'=new.kind and(s->>'slip_id')::uuid=new.slip_id)then raise exception 'A final source receipt requires its protected settled statement and exact cash date.'using errcode='23514';end if;
 if exists(select 1 from public.payroll_final_cases p cross join lateral jsonb_array_elements(p.source_basis->'selected')s where p.company_id=new.company_id and p.employee_id=new.employee_id and p.status='approved'and s->>'kind'=new.kind and(s->>'slip_id')::uuid=new.slip_id)then raise exception 'This unpaid payslip belongs to an approved final statement. Record its atomic final settlement, or withdraw that unpaid statement first.'using errcode='23514';end if;
 return new;
end;$$;
create trigger payroll_final_receipt_guard before insert on public.payroll_payment_receipts for each row execute function hris_private.final_pay_receipt_guard();
create function hris_private.record_final_pay_source(r public.payroll_final_cases,part jsonb)returns void language plpgsql security invoker set search_path=''as $$
declare p record;remaining integer;latest date;tbl text;begin
 select *into p from hris_private.payroll_issued_payments where company_id=r.company_id and employee_id=r.employee_id and kind=part->>'kind'and slip_id=(part->>'slip_id')::uuid;
 if p.slip_id is null or p.status<>'approved'or p.amount_cents is distinct from(part->>'amount_cents')::bigint then raise exception 'Final source changed. Refresh.'using errcode='PT409';end if;
 insert into public.payroll_payment_receipts(id,company_id,employee_id,kind,slip_id,run_id,amount_cents,paid_on,payment_reference,reason,origin,recorded_by,final_case_id)
 values(gen_random_uuid(),r.company_id,r.employee_id,p.kind,p.slip_id,p.run_id,p.amount_cents,r.cash_date,r.payment_reference,'Source net discharged in combined final settlement; actual cash is on the final statement.','final_settlement',auth.uid(),r.id);
 select count(*)into remaining from hris_private.payroll_issued_payments q where q.company_id=r.company_id and q.kind=p.kind and q.run_id=p.run_id and not exists(select 1 from public.payroll_payment_receipts pay where pay.kind=q.kind and pay.slip_id=q.slip_id);
 tbl:=case p.kind when'monthly'then'payroll_runs'when'cutoff'then'payroll_cutoff_runs'else'payroll_thirteenth_runs'end;
 if remaining=0 then
  select max(paid_on)into latest from public.payroll_payment_receipts where company_id=r.company_id and kind=p.kind and run_id=p.run_id;
  execute format('update public.%I set status=''paid'',paid_on=$1,payment_reference=''Per-employee payment records'',revision=revision+1,updated_at=clock_timestamp()where company_id=$2 and id=$3',tbl)using latest,r.company_id,p.run_id;
 else
  execute format('update public.%I set revision=revision+1,updated_at=clock_timestamp()where company_id=$1 and id=$2',tbl)using r.company_id,p.run_id;
 end if;
end;$$;
create function hris_private.manage_payroll_final(p_operation text,p_record jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
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
    w:=r.worksheet||'{"entitlements_reviewed":false,"source_scope_reviewed":false,"contributions_reviewed":false,"review_note":""}'::jsonb;
    w:=jsonb_set(w,'{tax}',(w->'tax')||'{"final_compensation_reviewed":false,"previous_employers_reviewed":false,"benefits_reviewed":false,"review_note":""}'::jsonb);
    w:=jsonb_set(w,'{vacation,previous_days_hundredths}',to_jsonb(greatest((w#>>'{vacation,previous_days_hundredths}')::bigint,(fresh#>>'{calculation_source,previous_vacation_days_hundredths}')::bigint)));
   end if;
   if(w#>>'{vacation,previous_days_hundredths}')::bigint>0 and btrim(w#>>'{vacation,reference}')=''then w:=jsonb_set(w,'{vacation,reference}',to_jsonb('Earlier recorded vacation conversion retained. Review the original evidence.'::text));end if;
   if p_operation='refresh'then begin t:=hris_private.final_pay_compute(fresh->'calculation_source',w);exception when check_violation then w:=hris_private.final_pay_seed(fresh->'calculation_source');t:=hris_private.final_pay_compute(fresh->'calculation_source',w);end;
   else t:=hris_private.final_pay_compute(fresh->'calculation_source',w);end if;
   select *into e from public.employees where company_id=m.company_id and id=r.employee_id;
   update public.payroll_final_cases set employment_start=e.joined_on,employee_name=e.full_name,employee_code=e.employee_code,department=e.department,source_basis=fresh,worksheet=w,totals=t,prepared_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
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
revoke all on function hris_private.annual_tax_basis_before_final(uuid,uuid,integer,date,date,uuid),hris_private.thirteenth_basis_before_final(uuid,uuid,integer,date),hris_private.final_pay_basis(uuid,uuid,date,date,uuid),hris_private.final_pay_check(public.payroll_final_cases),hris_private.final_pay_case_immutable(),hris_private.final_pay_receipt_guard(),hris_private.record_final_pay_source(public.payroll_final_cases,jsonb),hris_private.manage_payroll_final(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_payroll_final(text,jsonb)to authenticated;
create function public.manage_payroll_final(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_payroll_final(p_operation,p_record);$$;
revoke all on function public.manage_payroll_final(text,jsonb)from public,anon,authenticated;
grant execute on function public.manage_payroll_final(text,jsonb)to authenticated;
commit;
