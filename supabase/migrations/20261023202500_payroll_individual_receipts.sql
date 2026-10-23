-- Individual immutable payment records. Old batch clients record only outstanding slips.
begin;
create table public.payroll_payment_receipts(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid not null,
 kind text not null check(kind in('monthly','cutoff','thirteenth')),slip_id uuid not null,run_id uuid not null,
 monthly_id uuid generated always as(case when kind='monthly'then slip_id end)stored references public.payroll_slips(id),
 cutoff_id uuid generated always as(case when kind='cutoff'then slip_id end)stored references public.payroll_cutoff_slips(id),
 thirteenth_id uuid generated always as(case when kind='thirteenth'then slip_id end)stored references public.payroll_thirteenth_slips(id),
 amount_cents bigint not null check(amount_cents>=0),paid_on date not null,
 payment_reference text not null check(char_length(btrim(payment_reference))between 1 and 500),
 reason text not null check(char_length(btrim(reason))between 1 and 500),
 origin text not null check(origin in('individual','batch','legacy_batch')),
 recorded_by uuid references auth.users(id),recorded_at timestamptz not null default clock_timestamp(),
 unique(kind,slip_id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_payment_receipts_employee_idx on public.payroll_payment_receipts(company_id,employee_id,paid_on);
create index payroll_payment_receipts_run_idx on public.payroll_payment_receipts(company_id,kind,run_id);
create index payroll_payment_receipts_monthly_idx on public.payroll_payment_receipts(monthly_id);
create index payroll_payment_receipts_cutoff_idx on public.payroll_payment_receipts(cutoff_id);
create index payroll_payment_receipts_thirteenth_idx on public.payroll_payment_receipts(thirteenth_id);
create index payroll_payment_receipts_actor_idx on public.payroll_payment_receipts(recorded_by);
alter table public.payroll_payment_receipts enable row level security;
revoke all on public.payroll_payment_receipts from public,anon,authenticated;
grant select on public.payroll_payment_receipts to authenticated;
create policy payroll_payment_receipts_hr_read on public.payroll_payment_receipts for select to authenticated using(hris_private.payroll_hr(company_id));
create trigger payroll_payment_receipts_immutable before update or delete on public.payroll_payment_receipts for each row execute function hris_private.employment_history_immutable();

create view hris_private.payroll_issued_payments with(security_invoker=true)as
 select 'monthly'::text kind,s.id slip_id,s.company_id,s.employee_id,s.run_id,(s.item#>>'{totals,net}')::bigint amount_cents,
 (s.month+interval '1 month -1 day')::date earliest_on,r.month legacy_earliest_on,r.status,r.revision,r.paid_on,r.payment_reference,r.updated_at
 from public.payroll_slips s join public.payroll_runs r on r.company_id=s.company_id and r.id=s.run_id
 union all
 select 'cutoff',s.id,s.company_id,s.employee_id,s.run_id,(s.item#>>'{totals,net}')::bigint,s.ends_on,s.ends_on,r.status,r.revision,r.paid_on,r.payment_reference,r.updated_at
 from public.payroll_cutoff_slips s join public.payroll_cutoff_runs r on r.company_id=s.company_id and r.id=s.run_id
 union all
 select 'thirteenth',s.id,s.company_id,s.employee_id,s.run_id,(s.item#>>'{totals,net_cents}')::bigint,s.as_of,s.as_of,r.status,r.revision,r.paid_on,r.payment_reference,r.updated_at
 from public.payroll_thirteenth_slips s join public.payroll_thirteenth_runs r on r.company_id=s.company_id and r.id=s.run_id;
revoke all on hris_private.payroll_issued_payments from public,anon,authenticated;
create function hris_private.payroll_receipt_binding()returns trigger language plpgsql security definer set search_path=''as $$
declare s record;begin
 select * into s from hris_private.payroll_issued_payments where kind=new.kind and slip_id=new.slip_id;
 if s.slip_id is null or s.company_id<>new.company_id or s.employee_id<>new.employee_id or s.run_id<>new.run_id or s.amount_cents is distinct from new.amount_cents or s.status not in('approved','paid')then
 raise exception 'Payment must match its issued payslip and company.'using errcode='23514';end if;
 return new;
end;$$;
create trigger payroll_receipt_binding before insert on public.payroll_payment_receipts for each row execute function hris_private.payroll_receipt_binding();
-- Preserve earlier run/slip/audit rows. These receipts retain the original recorded cash date.
insert into public.payroll_payment_receipts(id,company_id,employee_id,kind,slip_id,run_id,amount_cents,paid_on,payment_reference,reason,origin,recorded_at)
 select gen_random_uuid(),company_id,employee_id,kind,slip_id,run_id,amount_cents,paid_on,payment_reference,'Imported existing batch payment record.','legacy_batch',updated_at
 from hris_private.payroll_issued_payments where status='paid';

create function hris_private.payroll_batch_receipts()returns trigger language plpgsql security definer set search_path=''as $$
declare k text;begin
 if new.status<>'paid'or old.status='paid'then return new;end if;
 k:=case tg_table_name when'payroll_runs'then'monthly'when'payroll_cutoff_runs'then'cutoff'else'thirteenth'end;
 insert into public.payroll_payment_receipts(id,company_id,employee_id,kind,slip_id,run_id,amount_cents,paid_on,payment_reference,reason,origin,recorded_by)
 select gen_random_uuid(),s.company_id,s.employee_id,s.kind,s.slip_id,s.run_id,s.amount_cents,new.paid_on,new.payment_reference,'Recorded with the remaining payroll batch.','batch',auth.uid()
 from hris_private.payroll_issued_payments s where s.company_id=new.company_id and s.run_id=new.id and s.kind=k
 and not exists(select 1 from public.payroll_payment_receipts p where p.kind=s.kind and p.slip_id=s.slip_id);
 return new;
end;$$;
create trigger payroll_batch_receipts after update on public.payroll_runs for each row execute function hris_private.payroll_batch_receipts();
create trigger payroll_batch_receipts after update on public.payroll_cutoff_runs for each row execute function hris_private.payroll_batch_receipts();
create trigger payroll_batch_receipts after update on public.payroll_thirteenth_runs for each row execute function hris_private.payroll_batch_receipts();

create function hris_private.manage_payroll_receipt(p_operation text,p_record jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;rid uuid;k text;sid uuid;why text;ref text;pd date;today date;actor_label text;birth hris_private.payroll_birth;
 s record;run jsonb;tbl text;expected bigint;result jsonb;remaining integer;latest date;
begin
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role='administrator';
 if m.user_id is null then raise exception 'Active Administrator access is required to record payments.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));perform 1 from public.companies where id=m.company_id for update;
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role='administrator'for update;
 if m.user_id is null then raise exception 'Your payroll access changed. Refresh.'using errcode='42501';end if;
 if p_operation is distinct from 'record'or jsonb_typeof(p_record)is distinct from 'object'or octet_length(p_record::text)>8192 or p_record-array['id','kind','slip_id','expected_revision','paid_on','payment_reference','reason']<>'{}'then raise exception 'Use a supported bounded payment request.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;sid:=(p_record->>'slip_id')::uuid;k:=hris_private.payroll_text(p_record,'kind',20,true);why:=hris_private.payroll_text(p_record,'reason',500,true);ref:=hris_private.payroll_text(p_record,'payment_reference',500,true);pd:=hris_private.thirteenth_date(p_record,'paid_on');expected:=(p_record->>'expected_revision')::bigint;
 if rid is null or rid='00000000-0000-0000-0000-000000000000'or sid is null or sid='00000000-0000-0000-0000-000000000000'or k not in('monthly','cutoff','thirteenth')or expected is null or expected<0 then raise exception 'Payment identity and current run revision required.'using errcode='23514';end if;
 select * into birth from hris_private.payroll_birth where company_id=m.company_id and operation='receipt_record'and record_id=rid;
 if birth.record_id is not null then
  if birth.actor_id<>m.user_id or birth.payload<>p_record then raise exception 'This payment creation identity was already used.'using errcode='PT409';end if;
  select to_jsonb(p)into result from public.payroll_payment_receipts p where p.company_id=m.company_id and p.id=rid;return result;
 end if;
 if exists(select 1 from public.payroll_payment_receipts where id=rid or(kind=k and slip_id=sid))then raise exception 'This payslip already has a payment record. Refresh.'using errcode='PT409';end if;
 select * into s from hris_private.payroll_issued_payments where company_id=m.company_id and kind=k and slip_id=sid;
 if s.slip_id is null then raise exception 'Issued payslip unavailable.'using errcode='42501';end if;
 tbl:=case k when'monthly'then'payroll_runs'when'cutoff'then'payroll_cutoff_runs'else'payroll_thirteenth_runs'end;
 execute format('select to_jsonb(r)from public.%I r where company_id=$1 and id=$2 for update',tbl)into run using m.company_id,s.run_id;
 if(run->>'revision')::bigint is distinct from expected then raise exception 'Payroll payment status changed. Refresh.'using errcode='PT409';end if;
 select(now()at time zone timezone)::date into today from public.companies where id=m.company_id;
 if run->>'status'<>'approved'or pd<s.earliest_on or pd>today then raise exception 'Record an actual full-slip payment after the completed period.'using errcode='23514';end if;
 insert into public.payroll_payment_receipts(id,company_id,employee_id,kind,slip_id,run_id,amount_cents,paid_on,payment_reference,reason,origin,recorded_by)
 values(rid,m.company_id,s.employee_id,k,sid,s.run_id,s.amount_cents,pd,ref,why,'individual',m.user_id)returning to_jsonb(payroll_payment_receipts)into result;
 select count(*)into remaining from hris_private.payroll_issued_payments q where q.company_id=m.company_id and q.kind=k and q.run_id=s.run_id and not exists(select 1 from public.payroll_payment_receipts p where p.kind=q.kind and p.slip_id=q.slip_id);
 if remaining=0 then
  select max(paid_on)into latest from public.payroll_payment_receipts where company_id=m.company_id and kind=k and run_id=s.run_id;
  execute format('update public.%I set status=''paid'',paid_on=$1,payment_reference=''Per-employee payment records'',revision=revision+1,updated_at=clock_timestamp()where company_id=$2 and id=$3',tbl)using latest,m.company_id,s.run_id;
 else
  execute format('update public.%I set revision=revision+1,updated_at=clock_timestamp()where company_id=$1 and id=$2',tbl)using m.company_id,s.run_id;
 end if;
 insert into hris_private.payroll_birth(company_id,operation,record_id,actor_id,payload)values(m.company_id,'receipt_record',rid,m.user_id,p_record);
 select full_name into actor_label from public.employees where company_id=m.company_id and id=m.employee_id;
 insert into public.payroll_events(company_id,record_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)values(m.company_id,rid,'receipt_record',why,m.user_id,actor_label,m.role,null,result);
 return result;
end;$$;
revoke all on function hris_private.payroll_receipt_binding(),hris_private.payroll_batch_receipts(),hris_private.manage_payroll_receipt(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_payroll_receipt(text,jsonb)to authenticated;
create function public.manage_payroll_receipt(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_payroll_receipt(p_operation,p_record);$$;
revoke all on function public.manage_payroll_receipt(text,jsonb)from public,anon,authenticated;
grant execute on function public.manage_payroll_receipt(text,jsonb)to authenticated;

create or replace function hris_private.annual_tax_basis(cid uuid,eid uuid,yr integer,asof date,service_end date,exclude_id uuid)returns jsonb language plpgsql stable security invoker set search_path=''as $$
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

commit;
