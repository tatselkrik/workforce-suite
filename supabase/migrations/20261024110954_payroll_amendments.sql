-- Linked supplemental corrections and independent overpayment reviews. Never rewrite an issued original.
begin;
create table public.payroll_amendment_cases(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid not null,
 target_kind text not null check(target_kind in('monthly','cutoff','thirteenth','final')),target_id uuid not null,
 starts_on date not null,ends_on date not null,cash_date date not null,title text not null check(char_length(btrim(title))between 1 and 120),
 employee_name text not null,employee_code text not null,department text not null,
 status text not null default 'draft'check(status in('draft','submitted','approved','paid','withdrawn','cancelled','reviewed')),revision bigint not null default 0 check(revision>=0),
 created_by uuid not null references auth.users(id),prepared_by uuid not null references auth.users(id),submitted_by uuid references auth.users(id),approved_by uuid references auth.users(id),approved_at timestamptz,
 paid_on date,payment_reference text not null default ''check(char_length(payment_reference)<=500),
 source_basis jsonb not null check(octet_length(source_basis::text)<=2097152),worksheet jsonb not null check(octet_length(worksheet::text)<=65536),totals jsonb not null,
 rule_version text not null default 'PH-payroll-amendment-2026-10-24-v1',created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id),check(starts_on<=ends_on and ends_on<=cash_date),
 check((status in('submitted','approved','paid','withdrawn','reviewed')and submitted_by is not null)or(status in('draft','cancelled')and submitted_by is null)),
 check((status in('approved','paid','withdrawn','reviewed')and approved_by is not null and approved_at is not null and approved_by<>created_by and approved_by<>prepared_by and approved_by<>submitted_by)or(status in('draft','submitted','cancelled')and approved_by is null and approved_at is null)),
 check((status='paid'and paid_on=cash_date and btrim(payment_reference)<>'')or(status<>'paid'and paid_on is null and payment_reference=''))
);
create index payroll_amendment_cases_employee_idx on public.payroll_amendment_cases(company_id,employee_id,cash_date,status);
create index payroll_amendment_cases_target_idx on public.payroll_amendment_cases(company_id,target_kind,target_id,status);
create index payroll_amendment_creator_idx on public.payroll_amendment_cases(created_by);
create index payroll_amendment_preparer_idx on public.payroll_amendment_cases(prepared_by);
create index payroll_amendment_submitter_idx on public.payroll_amendment_cases(submitted_by);
create index payroll_amendment_approver_idx on public.payroll_amendment_cases(approved_by);
create unique index payroll_amendment_one_pending_idx on public.payroll_amendment_cases(company_id,employee_id)where status='approved';
create table public.payroll_amendment_slips(
 id uuid primary key,company_id uuid not null,employee_id uuid not null,target_kind text not null,target_id uuid not null,
 starts_on date not null,ends_on date not null,cash_date date not null,title text not null,company_name text not null,employee_name text not null,employee_code text not null,department text not null,
 purpose text not null check(purpose in('payout','review_only')),totals jsonb not null,approved_by uuid not null references auth.users(id),approved_at timestamptz not null,rule_version text not null,
 unique(company_id,id),foreign key(company_id,id)references public.payroll_amendment_cases(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_amendment_slips_employee_idx on public.payroll_amendment_slips(company_id,employee_id,cash_date);
create index payroll_amendment_slips_approver_idx on public.payroll_amendment_slips(approved_by);
create table public.payroll_amendment_status(
 id uuid primary key,company_id uuid not null,employee_id uuid not null,kind text not null check(kind in('paid','withdrawn')),recorded_on date not null,net_cents bigint not null check(net_cents>=0),
 foreign key(company_id,id)references public.payroll_amendment_slips(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_amendment_status_case_idx on public.payroll_amendment_status(company_id,id);
create index payroll_amendment_status_employee_idx on public.payroll_amendment_status(company_id,employee_id,recorded_on);
create trigger payroll_amendment_slips_immutable before update or delete on public.payroll_amendment_slips for each row execute function hris_private.employment_history_immutable();
create trigger payroll_amendment_status_immutable before update or delete on public.payroll_amendment_status for each row execute function hris_private.employment_history_immutable();
do $$declare t text;begin foreach t in array array['payroll_amendment_cases','payroll_amendment_slips','payroll_amendment_status']loop
 execute format('alter table public.%I enable row level security',t);execute format('revoke all on public.%I from public,anon,authenticated',t);execute format('grant select on public.%I to authenticated',t);
end loop;end;$$;
create policy payroll_amendment_cases_hr_read on public.payroll_amendment_cases for select to authenticated using(hris_private.payroll_hr(company_id));
create policy payroll_amendment_slips_read on public.payroll_amendment_slips for select to authenticated using(hris_private.payroll_hr(company_id)or hris_private.payroll_own(company_id,employee_id));
create policy payroll_amendment_status_read on public.payroll_amendment_status for select to authenticated using(hris_private.payroll_hr(company_id)or hris_private.payroll_own(company_id,employee_id));

create function hris_private.amendment_compute(s jsonb,w jsonb,ready boolean default false)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare maximum constant bigint:=100000000000;k text;purpose text;kind text;cash date;paid date;basic bigint;taxable bigint;shared bigint;other bigint;exempt bigint;sss bigint;ph bigint;pi bigint;wh bigint;deduction bigint;review bigint;contribution bigint;gross bigint;net bigint;benefit_exempt bigint;begin
 perform hris_private.final_pay_object(s,array['kind','slip_id','employee_id','starts_on','ends_on','paid_on','cash_date','original_net_cents','sss_refundable_cents','philhealth_refundable_cents','pagibig_refundable_cents','withholding_refundable_cents','shared_floor_cents']);
 perform hris_private.final_pay_object(w,array['purpose','basic_cents','taxable_cents','shared_cents','other_benefits_cents','exempt_cents','sss_cents','philhealth_cents','pagibig_cents','withholding_cents','lawful_deduction_cents','review_amount_cents','evidence','tax_note','contribution_note','review_note','reviewed','tax_reviewed','contributions_reviewed']);
 kind:=hris_private.payroll_text(s,'kind',20,true);purpose:=hris_private.payroll_text(w,'purpose',20,true);
 cash:=hris_private.thirteenth_date(s,'cash_date');paid:=hris_private.thirteenth_date(s,'paid_on');
 if kind not in('monthly','cutoff','thirteenth','final')or coalesce((s->>'slip_id')::uuid,'00000000-0000-0000-0000-000000000000')='00000000-0000-0000-0000-000000000000'or coalesce((s->>'employee_id')::uuid,'00000000-0000-0000-0000-000000000000')='00000000-0000-0000-0000-000000000000'or hris_private.thirteenth_date(s,'starts_on')>hris_private.thirteenth_date(s,'ends_on')or hris_private.thirteenth_date(s,'ends_on')>cash or paid>cash or cash<date'2026-01-06'or extract(year from cash)<>2026 then raise exception 'Link an actually paid original and use reviewed2026 payment dates from6January.'using errcode='23514';end if;
 foreach k in array array['original_net_cents','sss_refundable_cents','philhealth_refundable_cents','pagibig_refundable_cents','withholding_refundable_cents','shared_floor_cents']loop perform hris_private.payroll_amount(s,k,maximum);end loop;
 foreach k in array array['evidence','tax_note','contribution_note','review_note']loop perform hris_private.payroll_text(w,k,2000);end loop;
 foreach k in array array['reviewed','tax_reviewed','contributions_reviewed']loop perform hris_private.final_pay_bool(w,k);end loop;
 basic:=hris_private.payroll_amount(w,'basic_cents',maximum);taxable:=hris_private.payroll_amount(w,'taxable_cents',maximum);shared:=hris_private.payroll_amount(w,'shared_cents',maximum);other:=hris_private.payroll_amount(w,'other_benefits_cents',maximum);exempt:=hris_private.payroll_amount(w,'exempt_cents',maximum);deduction:=hris_private.payroll_amount(w,'lawful_deduction_cents',maximum);
 sss:=hris_private.annual_tax_signed(w,'sss_cents');ph:=hris_private.annual_tax_signed(w,'philhealth_cents');pi:=hris_private.annual_tax_signed(w,'pagibig_cents');wh:=hris_private.annual_tax_signed(w,'withholding_cents');review:=hris_private.annual_tax_signed(w,'review_amount_cents');
 if purpose not in('payout','review_only')then raise exception 'Choose supplemental payout or overpayment review.'using errcode='23514';end if;
 if ready and(not(w->>'reviewed')::boolean or btrim(w->>'evidence')=''or btrim(w->>'review_note')='')then raise exception 'Review the original, prior amendments, owed difference and evidence.'using errcode='23514';end if;
 if purpose='review_only'then
  if review>0 or review<-(s->>'original_net_cents')::bigint or basic<>0 or taxable<>0 or shared<>0 or other<>0 or exempt<>0 or sss<>0 or ph<>0 or pi<>0 or wh<>0 or deduction<>0 or(ready and review=0)then raise exception 'An overpayment review contains only a negative reviewed amount. It creates no collectible debt, wage recovery or tax correction.'using errcode='23514';end if;
  return jsonb_build_object('basic_cents',0,'gross_cents',0,'taxable_cents',0,'shared_cents',0,'benefit_exempt_cents',0,'contribution_cents',0,'withholding_cents',0,'deduction_cents',0,'net_cents',0,'review_amount_cents',review);
 end if;
 if review<>0 or basic>taxable or(basic<>0 and kind not in('monthly','cutoff'))or(kind='thirteenth'and taxable<>0)then raise exception 'Basic belongs in additional salary earnings; additional13th-month earnings use the shared benefit amount.'using errcode='23514';end if;
 if sss<-(s->>'sss_refundable_cents')::bigint or ph<-(s->>'philhealth_refundable_cents')::bigint or pi<-(s->>'pagibig_refundable_cents')::bigint or wh<-(s->>'withholding_refundable_cents')::bigint then raise exception 'Refund exceeds retained original and linked charges or remaining paid-year tax credit.'using errcode='23514';end if;
 if least(sss,ph,pi,wh)<0 and extract(year from paid)<>extract(year from cash)then raise exception 'Prior-year refunds need a separate year-specific correction; never offset current income automatically.'using errcode='23514';end if;
 if shared>0 and other<(s->>'shared_floor_cents')::bigint then raise exception 'Retain known annual shared benefits before allocating another exclusion.'using errcode='23514';end if;
 contribution:=sss+ph+pi;if taxable<greatest(contribution,0)then raise exception 'Additional contributions cannot exceed additional regular taxable earnings.'using errcode='23514';end if;
 benefit_exempt:=least(shared,greatest(9000000-other,0));gross:=taxable+shared+exempt;net:=gross-contribution-wh-deduction;
 if gross not between 0 and maximum or net not between 0 and maximum or taxable+shared-benefit_exempt-contribution not between 0 and maximum or contribution not between -maximum and maximum or(ready and taxable=0 and shared=0 and exempt=0 and sss=0 and ph=0 and pi=0 and wh=0 and deduction=0)then raise exception 'Use a nonempty bounded correction with nonnegative cash. Overpayment recovery is a separate review.'using errcode='23514';end if;
 if ready and(not(w->>'tax_reviewed')::boolean or not(w->>'contributions_reviewed')::boolean or btrim(w->>'tax_note')=''or btrim(w->>'contribution_note')='')then raise exception 'Review supplementary withholding, shared classification and original-month contribution reconciliation, including zero.'using errcode='23514';end if;
 return jsonb_build_object('basic_cents',basic,'gross_cents',gross,'taxable_cents',taxable+shared-benefit_exempt-contribution,'shared_cents',shared,'benefit_exempt_cents',benefit_exempt,'contribution_cents',contribution,'withholding_cents',wh,'deduction_cents',deduction,'net_cents',net,'review_amount_cents',0);
end;$$;

create view hris_private.payroll_amendment_originals with(security_invoker=true)as
 select 'monthly'::text kind,s.id slip_id,s.company_id,s.employee_id,s.month starts_on,(s.month+interval '1month -1day')::date ends_on,r.paid_on,
 (s.item#>>'{totals,net}')::bigint net_cents,(s.item#>>'{totals,sss_employee}')::bigint sss_cents,(s.item#>>'{totals,philhealth_employee}')::bigint philhealth_cents,(s.item#>>'{totals,pagibig_employee}')::bigint pagibig_cents,(s.item#>>'{totals,withholding}')::bigint withholding_cents,
 s.item->>'employee_name'employee_name,s.item->>'employee_code'employee_code,s.item->>'department'department,s.item->'totals'totals
 from public.payroll_slips s join public.payroll_payment_receipts r on r.kind='monthly'and r.slip_id=s.id
 union all
 select 'cutoff',s.id,s.company_id,s.employee_id,s.starts_on,s.ends_on,r.paid_on,(s.item#>>'{totals,net}')::bigint,(s.item#>>'{totals,sss_employee}')::bigint,(s.item#>>'{totals,philhealth_employee}')::bigint,(s.item#>>'{totals,pagibig_employee}')::bigint,(s.item#>>'{totals,withholding}')::bigint,s.item->>'employee_name',s.item->>'employee_code',s.item->>'department',s.item->'totals'
 from public.payroll_cutoff_slips s join public.payroll_payment_receipts r on r.kind='cutoff'and r.slip_id=s.id
 union all
 select 'thirteenth',s.id,s.company_id,s.employee_id,s.as_of,s.as_of,r.paid_on,(s.item#>>'{totals,net_cents}')::bigint,(s.item#>>'{totals,sss_adjustment_cents}')::bigint,(s.item#>>'{totals,philhealth_adjustment_cents}')::bigint,(s.item#>>'{totals,pagibig_adjustment_cents}')::bigint,(s.item#>>'{totals,withholding_cents}')::bigint,s.item->>'employee_name',s.item->>'employee_code',s.item->>'department',s.item->'totals'
 from public.payroll_thirteenth_slips s join public.payroll_payment_receipts r on r.kind='thirteenth'and r.slip_id=s.id
 union all
 select 'final',s.id,s.company_id,s.employee_id,s.employment_start,s.service_end,s.cash_date,(s.totals->>'net_cents')::bigint,(c.worksheet->>'sss_adjustment_cents')::bigint,(c.worksheet->>'philhealth_adjustment_cents')::bigint,(c.worksheet->>'pagibig_adjustment_cents')::bigint,greatest((s.totals#>>'{tax,adjustment_cents}')::bigint,0),s.employee_name,s.employee_code,s.department,s.totals
 from public.payroll_final_slips s join public.payroll_final_cases c on c.company_id=s.company_id and c.id=s.id join public.payroll_final_status p on p.company_id=s.company_id and p.id=s.id and p.kind='settled';
revoke all on hris_private.payroll_amendment_originals from public,anon,authenticated;

create function hris_private.amendment_benefits(cid uuid,eid uuid,yr integer,exclude_id uuid)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare gross bigint;ext bigint;other bigint;fin bigint;amend bigint;outside bigint;facts jsonb;begin
 select coalesce(sum((s.item#>>'{totals,gross_cents}')::bigint),0)::bigint,coalesce(max((s.item#>>'{worksheet,external_thirteenth_cents}')::bigint),0),
 coalesce(max(greatest((s.item#>>'{worksheet,other_benefits_cents}')::bigint-coalesce((i.source_basis->>'final_shared_benefits_cents')::bigint,0)-coalesce((i.source_basis->>'amendment_shared_benefits_cents')::bigint,0),0)),0)
 into gross,ext,other from public.payroll_thirteenth_slips s join public.payroll_thirteenth_items i on i.company_id=s.company_id and i.id=s.id where s.company_id=cid and s.employee_id=eid and s.year=yr;
 select coalesce(sum((p.totals->>'shared_extras_cents')::bigint),0)::bigint into fin from public.payroll_final_cases p where p.company_id=cid and p.employee_id=eid and extract(year from p.cash_date)=yr and p.status in('approved','settled');
 select coalesce(sum((p.totals->>'shared_cents')::bigint),0)::bigint into amend from public.payroll_amendment_cases p where p.company_id=cid and p.employee_id=eid and p.id is distinct from exclude_id and extract(year from p.cash_date)=yr and p.status in('approved','paid')and p.worksheet->>'purpose'='payout';
 select coalesce(max(greatest((p.worksheet->>'other_benefits_cents')::bigint-coalesce((p.source_basis#>>'{benefits,issued_thirteenth_cents}')::bigint,0)-coalesce((p.source_basis#>>'{benefits,final_cents}')::bigint,0)-coalesce((p.source_basis#>>'{benefits,amendment_cents}')::bigint,0),0)),0)into outside from public.payroll_amendment_cases p where p.company_id=cid and p.employee_id=eid and p.id is distinct from exclude_id and extract(year from p.cash_date)=yr and p.status in('approved','paid');
 other:=greatest(other,outside-ext);
 return jsonb_build_object('floor_cents',gross+ext+other+fin+amend,'issued_thirteenth_cents',gross,'external_thirteenth_cents',ext,'external_other_cents',other,'final_cents',fin,'amendment_cents',amend);
end;$$;
create function hris_private.amendment_basis(cid uuid,kind text,target uuid,cash date,exclude_id uuid)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare p record;sss bigint;ph bigint;pi bigint;wh bigint;prior jsonb;benefits jsonb;credits bigint;fincredits bigint;taxcredits bigint;amendcredits bigint;creditfacts jsonb;begin
 select *into p from hris_private.payroll_amendment_originals s where s.company_id=cid and s.kind=amendment_basis.kind and s.slip_id=target;
 if p.slip_id is null then raise exception 'Select an actually paid original payslip or settled final statement.'using errcode='23514';end if;
 select coalesce(sum((r.worksheet->>'sss_cents')::bigint)filter(where r.status in('approved','paid')and r.worksheet->>'purpose'='payout'),0)::bigint,
 coalesce(sum((r.worksheet->>'philhealth_cents')::bigint)filter(where r.status in('approved','paid')and r.worksheet->>'purpose'='payout'),0)::bigint,coalesce(sum((r.worksheet->>'pagibig_cents')::bigint)filter(where r.status in('approved','paid')and r.worksheet->>'purpose'='payout'),0)::bigint,coalesce(sum((r.worksheet->>'withholding_cents')::bigint)filter(where r.status in('approved','paid')and r.worksheet->>'purpose'='payout'),0)::bigint,
 coalesce(jsonb_agg(jsonb_build_object('id',r.id,'status',r.status,'revision',r.revision,'totals',r.totals,'sss_cents',r.worksheet->'sss_cents','philhealth_cents',r.worksheet->'philhealth_cents','pagibig_cents',r.worksheet->'pagibig_cents','withholding_cents',r.worksheet->'withholding_cents')order by r.id),'[]')
 into sss,ph,pi,wh,prior from public.payroll_amendment_cases r where r.company_id=cid and r.target_kind=kind and r.target_id=target and r.id is distinct from exclude_id and r.status in('approved','paid','reviewed');
 benefits:=hris_private.amendment_benefits(cid,p.employee_id,extract(year from cash)::integer,exclude_id);
 select coalesce(sum(s.withholding_cents),0)::bigint into credits from hris_private.payroll_amendment_originals s where s.company_id=cid and s.employee_id=p.employee_id and s.kind<>'final'and extract(year from s.paid_on)=extract(year from cash);
 select coalesce(sum((r.totals#>>'{tax,adjustment_cents}')::bigint),0)::bigint into fincredits from public.payroll_final_cases r where r.company_id=cid and r.employee_id=p.employee_id and r.status='settled'and extract(year from r.cash_date)=extract(year from cash);
 select coalesce(sum((r.totals->>'adjustment_cents')::bigint),0)::bigint into taxcredits from public.payroll_tax_reconciliations r where r.company_id=cid and r.employee_id=p.employee_id and r.status='settled'and r.year=extract(year from cash);
 select coalesce(sum((r.totals->>'withholding_cents')::bigint),0)::bigint into amendcredits from public.payroll_amendment_cases r where r.company_id=cid and r.employee_id=p.employee_id and r.id is distinct from exclude_id and r.status='paid'and extract(year from r.cash_date)=extract(year from cash);
 credits:=greatest(credits+fincredits+taxcredits+amendcredits,0);
 return jsonb_build_object('calculation_source',jsonb_build_object('kind',kind,'slip_id',p.slip_id,'employee_id',p.employee_id,'starts_on',p.starts_on,'ends_on',p.ends_on,'paid_on',p.paid_on,'cash_date',cash,'original_net_cents',p.net_cents,'sss_refundable_cents',p.sss_cents+sss,'philhealth_refundable_cents',p.philhealth_cents+ph,'pagibig_refundable_cents',p.pagibig_cents+pi,'withholding_refundable_cents',least(p.withholding_cents+wh,credits),'shared_floor_cents',benefits->'floor_cents'),
 'original_totals',p.totals,'employee',jsonb_build_object('id',p.employee_id,'full_name',p.employee_name,'employee_code',p.employee_code,'department',p.department),'prior',prior,'benefits',benefits,'remaining_tax_credit_cents',credits);
end;$$;
create function hris_private.amendment_check(r public.payroll_amendment_cases)returns void language plpgsql stable security invoker set search_path=''as $$
declare b jsonb;t jsonb;begin
 b:=hris_private.amendment_basis(r.company_id,r.target_kind,r.target_id,r.cash_date,r.id);
 if b is distinct from r.source_basis then raise exception 'Original, earlier amendments, remaining tax credits or benefit facts changed. Refresh a draft or withdraw an unpaid amendment.'using errcode='PT409';end if;
 t:=hris_private.amendment_compute(b->'calculation_source',r.worksheet,true);
 if t is distinct from r.totals or r.rule_version<>'PH-payroll-amendment-2026-10-24-v1'then raise exception 'Amendment totals/rule changed. Refresh.'using errcode='PT409';end if;
end;$$;
create function hris_private.amendment_case_immutable()returns trigger language plpgsql security invoker set search_path=''as $$begin
 if tg_op='DELETE'then raise exception 'Retain amendment history.'using errcode='42501';end if;
 if old.status in('paid','reviewed','withdrawn','cancelled')or(old.status='approved'and(new.status not in('paid','withdrawn')or(to_jsonb(new)-array['status','paid_on','payment_reference','revision','updated_at'])is distinct from(to_jsonb(old)-array['status','paid_on','payment_reference','revision','updated_at'])))then raise exception 'An issued amendment stays immutable; only actual payment or unpaid withdrawal may be appended.'using errcode='42501';end if;return new;
end;$$;
create trigger payroll_amendment_case_immutable before update or delete on public.payroll_amendment_cases for each row execute function hris_private.amendment_case_immutable();

create function hris_private.amendment_seed(purpose text,floor_cents bigint)returns jsonb language sql immutable security invoker set search_path=''as $$
 select jsonb_build_object('purpose',purpose,'basic_cents',0,'taxable_cents',0,'shared_cents',0,'other_benefits_cents',case when purpose='payout'then floor_cents else 0 end,'exempt_cents',0,'sss_cents',0,'philhealth_cents',0,'pagibig_cents',0,'withholding_cents',0,'lawful_deduction_cents',0,'review_amount_cents',0,'evidence','','tax_note','','contribution_note','','review_note','','reviewed',false,'tax_reviewed',false,'contributions_reviewed',false);
$$;
revoke all on function hris_private.amendment_seed(text,bigint)from public,anon,authenticated;
create function hris_private.manage_payroll_amendment(p_operation text,p_record jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;r public.payroll_amendment_cases;birth hris_private.payroll_birth;rid uuid;expected bigint;why text;allowed text[];zone text;today date;cash date;on_date date;kind text;target uuid;purpose text;fresh jsonb;w jsonb;t jsonb;old_record jsonb;result jsonb;actor_label text;begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Active HR/Administrator access is required for amendments.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));perform 1 from public.companies where id=m.company_id for update;
 select *into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist')for update;
 if m.user_id is null then raise exception 'Your payroll access changed. Refresh.'using errcode='42501';end if;
 select c.timezone into zone from public.companies c where c.id=m.company_id;today:=(now()at time zone zone)::date;
 select e.full_name into actor_label from public.employees e where e.company_id=m.company_id and e.id=m.employee_id;
 if jsonb_typeof(p_record)is distinct from 'object'or octet_length(p_record::text)>65536 then raise exception 'Use bounded amendment details.'using errcode='23514';end if;
 allowed:=case p_operation when'create'then array['target_kind','target_id','cash_date','title','purpose']when'save'then array['worksheet']when'pay'then array['paid_on','payment_reference','received_confirmed']when'withdraw'then array['unpaid_confirmed']when'refresh'then array[]::text[]when'submit'then array[]::text[]when'return'then array[]::text[]when'cancel'then array[]::text[]when'approve'then array[]::text[]else null end;
 if allowed is null or p_record-(allowed||array['id','expected_revision','reason'])<>'{}'then raise exception 'Unsupported amendment operation/fields.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=hris_private.payroll_text(p_record,'reason',500,true);
 if rid is null or rid='00000000-0000-0000-0000-000000000000'then raise exception 'Amendment identity required.'using errcode='23514';end if;
 if p_operation in('approve','pay','withdraw')and m.role<>'administrator'then raise exception 'Only Administrators approve or record payment/withdrawal.'using errcode='42501';end if;
 if p_operation in('create','pay','withdraw')then
  select *into birth from hris_private.payroll_birth where company_id=m.company_id and operation='amendment_'||p_operation and record_id=rid;
  if birth.record_id is not null then if birth.actor_id<>m.user_id or birth.payload<>p_record then raise exception 'This amendment operation identity was already used.'using errcode='PT409';end if;select to_jsonb(x)into result from public.payroll_amendment_cases x where x.company_id=m.company_id and x.id=rid;return result;end if;
 end if;
 if p_operation='create'then
  if expected is not null then raise exception 'New amendments do not have a revision.'using errcode='23514';end if;
  kind:=hris_private.payroll_text(p_record,'target_kind',20,true);target:=(p_record->>'target_id')::uuid;cash:=hris_private.thirteenth_date(p_record,'cash_date');purpose:=hris_private.payroll_text(p_record,'purpose',20,true);
  fresh:=hris_private.amendment_basis(m.company_id,kind,target,cash,rid);
  w:=jsonb_build_object('purpose',purpose,'basic_cents',0,'taxable_cents',0,'shared_cents',0,'other_benefits_cents',case when purpose='payout'then(fresh#>>'{calculation_source,shared_floor_cents}')::bigint else 0 end,'exempt_cents',0,'sss_cents',0,'philhealth_cents',0,'pagibig_cents',0,'withholding_cents',0,'lawful_deduction_cents',0,'review_amount_cents',0,'evidence','','tax_note','','contribution_note','','review_note','','reviewed',false,'tax_reviewed',false,'contributions_reviewed',false);
  t:=hris_private.amendment_compute(fresh->'calculation_source',w);
  insert into public.payroll_amendment_cases(id,company_id,employee_id,target_kind,target_id,starts_on,ends_on,cash_date,title,employee_name,employee_code,department,created_by,prepared_by,source_basis,worksheet,totals)
  values(rid,m.company_id,(fresh#>>'{calculation_source,employee_id}')::uuid,kind,target,(fresh#>>'{calculation_source,starts_on}')::date,(fresh#>>'{calculation_source,ends_on}')::date,cash,hris_private.payroll_text(p_record,'title',120,true),fresh#>>'{employee,full_name}',fresh#>>'{employee,employee_code}',fresh#>>'{employee,department}',m.user_id,m.user_id,fresh,w,t)returning *into r;
 else
  select *into r from public.payroll_amendment_cases where company_id=m.company_id and id=rid for update;
  if r.id is null then raise exception 'Amendment unavailable.'using errcode='42501';end if;
  if expected is null or expected is distinct from r.revision then raise exception 'Amendment changed. Refresh.'using errcode='PT409';end if;old_record:=to_jsonb(r);
  if p_operation in('save','refresh')then
   if r.status<>'draft'then raise exception 'Only draft amendments may change.'using errcode='23514';end if;
   fresh:=hris_private.amendment_basis(r.company_id,r.target_kind,r.target_id,r.cash_date,r.id);
   if p_operation='save'then if fresh is distinct from r.source_basis then raise exception 'Amendment sources changed. Refresh.'using errcode='PT409';end if;w:=p_record->'worksheet';if w->>'purpose'is distinct from r.worksheet->>'purpose'then raise exception 'Amendment purpose cannot change.'using errcode='23514';end if;
   else w:=r.worksheet||'{"reviewed":false,"tax_reviewed":false,"contributions_reviewed":false,"review_note":""}'::jsonb;if w->>'purpose'='payout'then w:=jsonb_set(w,'{other_benefits_cents}',to_jsonb(greatest((w->>'other_benefits_cents')::bigint,(fresh#>>'{calculation_source,shared_floor_cents}')::bigint)));end if;end if;
   if p_operation='refresh'then begin t:=hris_private.amendment_compute(fresh->'calculation_source',w);exception when check_violation then w:=hris_private.amendment_seed(r.worksheet->>'purpose',(fresh#>>'{calculation_source,shared_floor_cents}')::bigint);t:=hris_private.amendment_compute(fresh->'calculation_source',w);end;else t:=hris_private.amendment_compute(fresh->'calculation_source',w);end if;
   update public.payroll_amendment_cases set source_basis=fresh,worksheet=w,totals=t,prepared_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
  elsif p_operation in('submit','approve')then
   perform hris_private.amendment_check(r);
   if p_operation='submit'then if r.status<>'draft'then raise exception 'Submit a draft amendment.'using errcode='23514';end if;update public.payroll_amendment_cases set status='submitted',submitted_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
   else
    if r.status<>'submitted'then raise exception 'Submit before approval.'using errcode='23514';end if;
    if m.user_id in(r.created_by,r.prepared_by,r.submitted_by)then raise exception 'An independent Administrator must approve this amendment.'using errcode='42501';end if;
    if r.worksheet->>'purpose'='payout'and(exists(select 1 from public.payroll_amendment_cases p where p.company_id=r.company_id and p.employee_id=r.employee_id and p.id<>r.id and p.status='approved')or exists(select 1 from public.payroll_final_cases p where p.company_id=r.company_id and p.employee_id=r.employee_id and p.status='approved')or exists(select 1 from public.payroll_tax_reconciliations p where p.company_id=r.company_id and p.employee_id=r.employee_id and p.status='approved'))then raise exception 'Settle or withdraw the earlier pending amendment, final statement or tax advice before issuing supplemental pay.'using errcode='23514';end if;
    update public.payroll_amendment_cases set status=case when worksheet->>'purpose'='review_only'then'reviewed'else'approved'end,approved_by=m.user_id,approved_at=clock_timestamp(),revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
    insert into public.payroll_amendment_slips(id,company_id,employee_id,target_kind,target_id,starts_on,ends_on,cash_date,title,company_name,employee_name,employee_code,department,purpose,totals,approved_by,approved_at,rule_version)
    select r.id,r.company_id,r.employee_id,r.target_kind,r.target_id,r.starts_on,r.ends_on,r.cash_date,r.title,c.name,r.employee_name,r.employee_code,r.department,r.worksheet->>'purpose',r.totals,r.approved_by,r.approved_at,r.rule_version from public.companies c where c.id=r.company_id;
   end if;
  elsif p_operation='return'then if r.status<>'submitted'then raise exception 'Return a submitted amendment.'using errcode='23514';end if;update public.payroll_amendment_cases set status='draft',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
  elsif p_operation='cancel'then if r.status not in('draft','submitted')then raise exception 'Issued amendments remain immutable.'using errcode='23514';end if;update public.payroll_amendment_cases set status='cancelled',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
  elsif p_operation='withdraw'then
   if r.status<>'approved'or not hris_private.final_pay_bool(p_record,'unpaid_confirmed')then raise exception 'Confirm that the approved supplemental payout was never paid.'using errcode='23514';end if;
   update public.payroll_amendment_cases set status='withdrawn',revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
   insert into public.payroll_amendment_status(id,company_id,employee_id,kind,recorded_on,net_cents)values(r.id,r.company_id,r.employee_id,'withdrawn',today,0);
  else
   if r.status<>'approved'then raise exception 'Record an approved supplemental payout.'using errcode='23514';end if;on_date:=hris_private.thirteenth_date(p_record,'paid_on');
   if on_date<>r.cash_date or on_date>today or on_date<(r.approved_at at time zone zone)::date or not hris_private.final_pay_bool(p_record,'received_confirmed')then raise exception 'Confirm actual full supplemental cash on the approved date after approval; withdraw an unpaid amendment to reschedule.'using errcode='23514';end if;
   perform hris_private.amendment_check(r);
   update public.payroll_amendment_cases set status='paid',paid_on=on_date,payment_reference=hris_private.payroll_text(p_record,'payment_reference',500,true),revision=revision+1,updated_at=clock_timestamp()where id=rid returning *into r;
   insert into public.payroll_amendment_status(id,company_id,employee_id,kind,recorded_on,net_cents)values(r.id,r.company_id,r.employee_id,'paid',on_date,(r.totals->>'net_cents')::bigint);
  end if;
 end if;
 result:=to_jsonb(r);if p_operation in('create','pay','withdraw')then insert into hris_private.payroll_birth(company_id,operation,record_id,actor_id,payload)values(m.company_id,'amendment_'||p_operation,rid,m.user_id,p_record);end if;
 insert into public.payroll_events(company_id,record_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)values(m.company_id,rid,'amendment_'||p_operation,why,m.user_id,actor_label,m.role,old_record,result);return result;
end;$$;
revoke all on function hris_private.amendment_compute(jsonb,jsonb,boolean),hris_private.amendment_benefits(uuid,uuid,integer,uuid),hris_private.amendment_basis(uuid,text,uuid,date,uuid),hris_private.amendment_check(public.payroll_amendment_cases),hris_private.amendment_case_immutable(),hris_private.manage_payroll_amendment(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_payroll_amendment(text,jsonb)to authenticated;
create function public.manage_payroll_amendment(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_payroll_amendment(p_operation,p_record);$$;
revoke all on function public.manage_payroll_amendment(text,jsonb)from public,anon;
grant execute on function public.manage_payroll_amendment(text,jsonb)to authenticated;
-- Read-only source projections include linked corrections without altering original earned/pay facts.
alter function hris_private.annual_tax_basis(uuid,uuid,integer,date,date,uuid)rename to annual_tax_basis_before_amendments;
create function hris_private.annual_tax_basis(cid uuid,eid uuid,yr integer,asof date,service_end date,exclude_id uuid)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare b jsonb;cs jsonb;income jsonb;facts jsonb:='[]';r public.payroll_amendment_cases;credit bigint;pending integer;other bigint;outside bigint;begin
 b:=hris_private.annual_tax_basis_before_amendments(cid,eid,yr,asof,service_end,exclude_id);cs:=b->'calculation_source';income:=cs->'income';credit:=(cs->>'prior_adjustment_cents')::bigint;pending:=(cs->>'pending_adjustments')::integer;
 for r in select *from public.payroll_amendment_cases p where p.company_id=cid and p.employee_id=eid and p.status in('approved','paid')and extract(year from p.cash_date)=yr order by p.id loop
  if r.cash_date>asof then raise exception 'Use an as-of date at least as late as the earlier issued amendment.'using errcode='23514';end if;
  if r.status='approved'then pending:=pending+1;else
   income:=income||jsonb_build_array(jsonb_build_object('id','amendment:'||r.id::text,'starts_on',null,'ends_on',null,'taxable_cents',r.totals->'taxable_cents','withholding_cents',greatest((r.totals->>'withholding_cents')::bigint,0),'benefit_gross_cents',r.totals->'shared_cents','benefit_exempt_cents',r.totals->'benefit_exempt_cents','contribution_cents',0));
   credit:=credit+least((r.totals->>'withholding_cents')::bigint,0);
  end if;
  facts:=facts||jsonb_build_array(jsonb_build_object('id',r.id,'status',r.status,'revision',r.revision,'cash_date',r.cash_date,'totals',r.totals));
 end loop;
 -- Later13th worksheets retain known amendment benefits. They are not invented external income.
 select coalesce(max(greatest((s.item#>>'{worksheet,other_benefits_cents}')::bigint-coalesce((i.source_basis->>'final_shared_benefits_cents')::bigint,0)-coalesce((i.source_basis->>'amendment_shared_benefits_cents')::bigint,0),0)),0)into other
 from public.payroll_thirteenth_slips s join public.payroll_thirteenth_items i on i.company_id=s.company_id and i.id=s.id join public.payroll_payment_receipts p on p.kind='thirteenth'and p.slip_id=s.id
 where s.company_id=cid and s.employee_id=eid and s.year=yr and s.as_of<=asof and extract(year from p.paid_on)=yr and p.paid_on<=asof;
 select coalesce(max(greatest((p.worksheet->>'other_benefits_cents')::bigint-coalesce((p.source_basis#>>'{benefits,issued_thirteenth_cents}')::bigint,0)-coalesce((p.source_basis#>>'{benefits,final_cents}')::bigint,0)-coalesce((p.source_basis#>>'{benefits,amendment_cents}')::bigint,0),0)),0)into outside from public.payroll_amendment_cases p where p.company_id=cid and p.employee_id=eid and p.status='paid'and extract(year from p.cash_date)=yr and p.cash_date<=asof;
 cs:=cs||jsonb_build_object('income',income,'prior_adjustment_cents',credit,'pending_adjustments',pending,'other_benefits_floor',greatest(other,outside-(cs->>'external_thirteenth_floor')::bigint));
 b:=b||jsonb_build_object('calculation_source',cs);if jsonb_array_length(facts)>0 then b:=b||jsonb_build_object('amendments',facts);end if;return b;
end;$$;
alter function hris_private.thirteenth_basis(uuid,uuid,integer,date)rename to thirteenth_basis_before_amendments;
create function hris_private.thirteenth_basis(cid uuid,eid uuid,yr integer,asof date)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare b jsonb;cs jsonb;periods jsonb:='[]';prior jsonb;facts jsonb;part jsonb;delta bigint;other bigint;known bigint;begin
 b:=hris_private.thirteenth_basis_before_amendments(cid,eid,yr,asof);cs:=b->'calculation_source';prior:=cs->'prior';
 for part in select value from jsonb_array_elements(cs->'periods')loop
  select coalesce(sum((p.totals->>'basic_cents')::bigint),0)::bigint into delta from public.payroll_amendment_cases p where p.company_id=cid and p.employee_id=eid and p.target_kind in('monthly','cutoff')and p.target_id=(part->>'id')::uuid and p.status in('approved','paid');
  if delta<>0 then part:=jsonb_set(part,'{basic_cents}',to_jsonb((part->>'basic_cents')::bigint+delta));end if;periods:=periods||jsonb_build_array(part);
 end loop;
 select coalesce(jsonb_agg(jsonb_build_object('id',p.id,'gross_cents',p.totals->'shared_cents','exempt_cents',p.totals->'benefit_exempt_cents')order by p.id),'[]')into facts
 from public.payroll_amendment_cases p join public.payroll_thirteenth_slips s on p.target_kind='thirteenth'and s.company_id=p.company_id and s.id=p.target_id where p.company_id=cid and p.employee_id=eid and s.year=yr and s.as_of<=asof and p.status in('approved','paid');prior:=prior||facts;
 select coalesce(max(greatest((s.item#>>'{worksheet,other_benefits_cents}')::bigint-coalesce((i.source_basis->>'final_shared_benefits_cents')::bigint,0)-coalesce((i.source_basis->>'amendment_shared_benefits_cents')::bigint,0),0)),0)into other
 from public.payroll_thirteenth_slips s join public.payroll_thirteenth_items i on i.company_id=s.company_id and i.id=s.id where s.company_id=cid and s.employee_id=eid and s.year=yr;
 select coalesce(sum((p.totals->>'shared_cents')::bigint),0)::bigint,coalesce(jsonb_agg(jsonb_build_object('id',p.id,'status',p.status,'target_kind',p.target_kind,'target_id',p.target_id,'totals',p.totals)order by p.id),'[]')into known,facts
 from public.payroll_amendment_cases p where p.company_id=cid and p.employee_id=eid and p.status in('approved','paid')and p.target_kind<>'thirteenth'and extract(year from p.cash_date)=yr;
 other:=(hris_private.amendment_benefits(cid,eid,yr,null)->>'external_other_cents')::bigint;
 cs:=cs||jsonb_build_object('periods',periods,'prior',prior,'other_benefits_floor',other+coalesce((b->>'final_shared_benefits_cents')::bigint,0)+known);
 b:=b||jsonb_build_object('calculation_source',cs);if jsonb_array_length(facts)>0 then b:=b||jsonb_build_object('amendment_shared_benefits_cents',known,'amendments',facts);end if;return b;
end;$$;
revoke all on function hris_private.annual_tax_basis_before_amendments(uuid,uuid,integer,date,date,uuid),hris_private.annual_tax_basis(uuid,uuid,integer,date,date,uuid),hris_private.thirteenth_basis_before_amendments(uuid,uuid,integer,date),hris_private.thirteenth_basis(uuid,uuid,integer,date)from public,anon,authenticated;
commit;
