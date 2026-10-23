-- Additive semi-monthly payroll; installed monthly clients retain their schema.
begin;
create table public.payroll_cutoff_runs(
 id uuid primary key,company_id uuid not null references public.companies(id),
 month date not null check(extract(day from month)=1 and month between date '2026-02-01'and date '2200-12-01'),
 part smallint not null check(part in(1,2)),
 starts_on date generated always as(case when part=1 then month else month+15 end)stored,
 ends_on date generated always as(case when part=1 then month+14 else(month+interval '1 month -1 day')::date end)stored,
 pay_date date not null,title text not null check(char_length(btrim(title))between 1 and 120),
 status text not null default 'draft'check(status in('draft','submitted','approved','paid','cancelled')),
 revision bigint not null default 0 check(revision>=0),created_by uuid not null references auth.users(id),submitted_by uuid references auth.users(id),approved_by uuid references auth.users(id),
 paid_on date,payment_reference text not null default ''check(char_length(payment_reference)<=500),rule_version text not null default 'PH-semi-2026-10-23-v1',
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),unique(company_id,id),
 check(pay_date>=ends_on and pay_date<=ends_on+16),
 check((status in('approved','paid')and approved_by is not null and submitted_by is not null and approved_by<>submitted_by)or(status not in('approved','paid')and approved_by is null)),
 check((status='paid'and paid_on is not null and btrim(payment_reference)<>'')or(status<>'paid'and paid_on is null and payment_reference=''))
);
create unique index payroll_cutoff_runs_period_idx on public.payroll_cutoff_runs(company_id,month,part)where status<>'cancelled';
create index payroll_cutoff_runs_creator_idx on public.payroll_cutoff_runs(created_by);
create index payroll_cutoff_runs_submitter_idx on public.payroll_cutoff_runs(submitted_by);
create index payroll_cutoff_runs_approver_idx on public.payroll_cutoff_runs(approved_by);
create table public.payroll_cutoff_items(
 id uuid primary key,company_id uuid not null,run_id uuid not null,employee_id uuid not null,
 employee_name text not null,employee_code text not null,department text not null,
 profile jsonb not null,source_basis jsonb not null check(octet_length(source_basis::text)<=2097152),worksheet jsonb not null,totals jsonb not null,calculation jsonb not null,
 revision bigint not null default 0 check(revision>=0),unique(company_id,id),unique(company_id,run_id,employee_id),
 foreign key(company_id,run_id)references public.payroll_cutoff_runs(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_cutoff_items_employee_idx on public.payroll_cutoff_items(company_id,employee_id);
create table public.payroll_cutoff_slips(
 id uuid primary key,company_id uuid not null,run_id uuid not null,employee_id uuid not null,
 month date not null,part smallint not null check(part in(1,2)),starts_on date not null,ends_on date not null,pay_date date not null,
 company_name text not null,item jsonb not null,calculation jsonb not null,
 approved_by uuid not null references auth.users(id),approved_at timestamptz not null default clock_timestamp(),rule_version text not null,
 unique(company_id,month,part,employee_id),foreign key(company_id,run_id)references public.payroll_cutoff_runs(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index payroll_cutoff_slips_employee_idx on public.payroll_cutoff_slips(company_id,employee_id,month);
create index payroll_cutoff_slips_run_idx on public.payroll_cutoff_slips(company_id,run_id);
create index payroll_cutoff_slips_approver_idx on public.payroll_cutoff_slips(approved_by);
create trigger payroll_cutoff_slips_immutable before update or delete on public.payroll_cutoff_slips for each row execute function hris_private.employment_history_immutable();
do $$declare t text;begin
 foreach t in array array['payroll_cutoff_runs','payroll_cutoff_items','payroll_cutoff_slips']loop
  execute format('alter table public.%I enable row level security',t);
  execute format('revoke all on public.%I from public,anon,authenticated',t);
  execute format('grant select on public.%I to authenticated',t);
  execute format('create policy %I on public.%I for select to authenticated using(hris_private.payroll_hr(company_id))',t||'_hr_read',t);
 end loop;
end;$$;
create policy payroll_cutoff_slips_own_read on public.payroll_cutoff_slips for select to authenticated using(hris_private.payroll_own(company_id,employee_id));

create function hris_private.cutoff_tax(t bigint)returns bigint language sql immutable security invoker set search_path=''as $$
 select case when t<=1041700 then 0 when t<1666700 then round((t-1041700)*0.15)::bigint when t<3333300 then 93750+round((t-1666700)*0.20)::bigint when t<8333300 then 427070+round((t-3333300)*0.25)::bigint when t<33333300 then 1677070+round((t-8333300)*0.30)::bigint else 9177070+round((t-33333300)*0.35)::bigint end;
$$;
create function hris_private.cutoff_allocated(n bigint,part integer)returns bigint language sql immutable security invoker set search_path=''as $$select case when part=1 then n/2 else n-n/2 end;$$;
create function hris_private.cutoff_compute(p jsonb,w jsonb,mon date,part integer,previous jsonb)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare q jsonb;pw jsonb;t jsonb;s jsonb;prior jsonb;rice bigint;laundry bigint;pr bigint;pl bigint;cs jsonb;lo date;hi date;k text;stat bigint;taxable bigint;begin
 if part is null or part not in(1,2)or mon is null or extract(day from mon)<>1 then raise exception 'Choose a month first and cutoff 1 or 2.'using errcode='23514';end if;
 perform hris_private.payroll_compute(p,w,mon); -- original profile/worksheet validation
 lo:=case when part=1 then mon else mon+15 end;hi:=case when part=1 then mon+14 else(mon+interval '1month -1day')::date end;
 if exists(select 1 from jsonb_array_elements(w->'days')d where(d->>'date')::date not between lo and hi)then raise exception 'Worksheet dates must be inside this cutoff.'using errcode='23514';end if;
 if previous='null'::jsonb then previous:=null;end if;
 if previous is not null and(part<>2 or(previous->>'part')::integer<>1 or(previous->>'month')::date<>mon)then raise exception 'Use only this month''s issued first cutoff.'using errcode='23514';end if;
 if part=1 and(w->>'sss_basis_override'is not null or w->>'philhealth_basis_override'is not null or w->>'pagibig_basis_override'is not null)then raise exception 'Monthly contribution basis overrides belong to the second cutoff.'using errcode='23514';end if;
 if(w->>'basic_override_cents')::bigint>hris_private.cutoff_allocated((p->>'rate_cents')::bigint,part)then raise exception 'Cutoff basic proration exceeds the allocated half salary.'using errcode='23514';end if;
 select coalesce(jsonb_agg(jsonb_set(c,'{amount_cents}',to_jsonb(hris_private.cutoff_allocated((c->>'amount_cents')::bigint,part)))),'[]')into cs from jsonb_array_elements(p->'components')c;
 q:=p||jsonb_build_object('components',cs,'sss',false,'philhealth',false,'pagibig',false,'coverage_note','Cutoff allocation; monthly contributions reconciled on cutoff 2');
 pw:=w||jsonb_build_object('sss_basis_override',null,'philhealth_basis_override',null,'pagibig_basis_override',null,'exception_note','Internal cutoff allocation; original HR review retained separately');
 if p->>'basis'='monthly'then pw:=pw||jsonb_build_object('basic_override_cents',coalesce((w->>'basic_override_cents')::bigint,hris_private.cutoff_allocated((p->>'rate_cents')::bigint,part)));end if;
 t:=hris_private.payroll_compute(q,pw,mon);
 select coalesce(sum((c->>'amount_cents')::bigint)filter(where c->>'kind'='rice'),0),coalesce(sum((c->>'amount_cents')::bigint)filter(where c->>'kind'='laundry'),0)into rice,laundry from jsonb_array_elements(cs||(w->'adjustments'))c;
 prior:=coalesce(previous->'totals','{}');pr:=coalesce((previous->>'rice_cents')::bigint,0);pl:=coalesce((previous->>'laundry_cents')::bigint,0);
 t:=t||jsonb_build_object('exempt_allowances',least(rice+pr,250000)-least(pr,250000)+least(laundry+pl,40000)-least(pl,40000));
 if part=2 then
  pw:='{"days":[],"adjustments":[],"basic_override_cents":null,"reviewed":false,"review_note":"","exception_note":"Monthly contribution reconciliation from issued first cutoff and current reviewed cutoff"}'::jsonb||jsonb_build_object(
   'sss_basis_override',coalesce((w->>'sss_basis_override')::bigint,coalesce((prior->>'gross')::bigint,0)+(t->>'gross')::bigint),
   'philhealth_basis_override',coalesce((w->>'philhealth_basis_override')::bigint,case when p->>'basis'='monthly'then(p->>'rate_cents')::bigint else(p->>'monthly_reference_cents')::bigint end),
   'pagibig_basis_override',coalesce((w->>'pagibig_basis_override')::bigint,coalesce((prior->>'basic')::bigint,0)+coalesce((prior->>'paid_leave')::bigint,0)+coalesce((prior->>'allowances')::bigint,0)+(t->>'basic')::bigint+(t->>'paid_leave')::bigint+(t->>'allowances')::bigint));
  s:=hris_private.payroll_compute(p||'{"components":[]}'::jsonb,pw,mon);
  foreach k in array array['sss_basis','sss_msc','sss_employee','sss_employer','sss_regular_employee','sss_regular_employer','sss_mpf_employee','sss_mpf_employer','ec','philhealth_employee','philhealth_employer','pagibig_employee','pagibig_employer']loop t:=jsonb_set(t,array[k],s->k);end loop;
  t:=t||jsonb_build_object('thirteenth_accrual',round((coalesce((prior->>'basic')::bigint,0)+(t->>'basic')::bigint)::numeric/12)::bigint-coalesce((prior->>'thirteenth_accrual')::bigint,0));
 else t:=t||'{"sss_basis":0}'::jsonb;end if;
 stat:=(t->>'sss_employee')::bigint+(t->>'philhealth_employee')::bigint+(t->>'pagibig_employee')::bigint;
 taxable:=greatest((case when(p->>'minimum_wage_earner')::boolean then(t->>'allowances')::bigint else(t->>'gross')::bigint end)-(t->>'exempt_allowances')::bigint-stat,0);
 t:=t||jsonb_build_object('taxable',taxable,'withholding',hris_private.cutoff_tax(taxable));
 t:=t||jsonb_build_object('net',(t->>'gross')::bigint+(t->>'reimbursement')::bigint-stat-(t->>'withholding')::bigint-(t->>'voluntary_deductions')::bigint-(t->>'advances')::bigint);
 return jsonb_build_object('month',mon,'part',part,'totals',t,'rice_cents',rice,'laundry_cents',laundry);
end;$$;

create function hris_private.cutoff_basis(cid uuid,eid uuid,mon date,part integer)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare b jsonb;s public.payroll_cutoff_slips;lo date;hi date;zone text;begin
 b:=hris_private.payroll_basis(cid,eid,mon);zone:=b->>'timezone';
 lo:=case when part=1 then mon else mon+15 end;hi:=case when part=1 then mon+14 else(mon+interval '1month -1day')::date end;
 if(b#>>'{employee,joined_on}')::date>hi then raise exception 'Employee joined after this cutoff.'using errcode='23514';end if;
 b:=b||jsonb_build_object('attendance',coalesce((select jsonb_agg(a order by a#>>'{record,id}')from jsonb_array_elements(b->'attendance')a where(a#>>'{record,work_date}')::date between lo and hi or((a#>>'{record,checked_in_at}')::timestamptz<((hi+1)::timestamp at time zone zone)and coalesce(a#>>'{record,checked_out_at}',a#>>'{record,checked_in_at}')::timestamptz>=(lo::timestamp at time zone zone))),'[]'),
 'leave',coalesce((select jsonb_agg(l order by l->>'id')from jsonb_array_elements(b->'leave')l where(l->>'starts_on')::date<=hi and(l->>'ends_on')::date>=lo),'[]'),
 'holidays',coalesce((select jsonb_agg(h order by h->>'date')from jsonb_array_elements(b->'holidays')h where(h->>'date')::date between lo and hi),'[]'),
 'leave_holidays',coalesce((select jsonb_agg(h order by h->>'date')from jsonb_array_elements(b->'leave_holidays')h where(h->>'date')::date between lo and hi),'[]'));
 if part=2 then select ps.* into s from public.payroll_cutoff_slips ps where ps.company_id=cid and ps.employee_id=eid and ps.month=mon and ps.part=1;end if;
 return b||jsonb_build_object('first_cutoff',case when s.id is null then null else to_jsonb(s)end);
end;$$;

create function hris_private.cutoff_check_item(i public.payroll_cutoff_items,r public.payroll_cutoff_runs)returns void language plpgsql stable security invoker set search_path=''as $$
declare fresh jsonb;d jsonb;cap integer;ot integer;begin
 fresh:=hris_private.cutoff_basis(i.company_id,i.employee_id,r.month,r.part);
 if fresh is distinct from i.source_basis then raise exception 'Cutoff source facts changed. Refresh source and review again.'using errcode='PT409';end if;
 if coalesce((i.worksheet->>'reviewed')::boolean,false)is not true or btrim(coalesce(i.worksheet->>'review_note',''))=''then raise exception 'Review every cutoff worksheet before submitting.'using errcode='23514';end if;
 if r.part=2 and fresh->>'first_cutoff'is null and coalesce((fresh#>>'{employee,joined_on}')::date,r.month)<=r.month+14 then raise exception 'Issue this employee''s first cutoff before reconciling the second.'using errcode='23514';end if;
 if(fresh#>>'{employee,joined_on}')::date>r.starts_on and i.profile#>>'{pay,basis}'='monthly'and i.worksheet->>'basic_override_cents'is null then raise exception 'A mid-cutoff joiner needs reviewed basic proration.'using errcode='23514';end if;
 if exists(select 1 from jsonb_array_elements(fresh->'leave_holidays')h where not exists(select 1 from jsonb_array_elements(fresh->'holidays')p where p->>'date'=h->>'date'))then raise exception 'Classify this cutoff''s leave-calendar holidays in Payroll holidays.'using errcode='23514';end if;
 for d in select value from jsonb_array_elements(i.worksheet->'days')loop
  if(d->>'date')::date<(fresh#>>'{employee,joined_on}')::date then raise exception 'Worksheet dates cannot precede employment.'using errcode='23514';end if;
  if exists(select 1 from jsonb_array_elements(fresh->'holidays')h where h->>'date'=d->>'date'and h->>'kind'<>d->>'kind')then raise exception 'A worksheet day differs from the payroll holiday classification.'using errcode='23514';end if;
 end loop;
 select coalesce(sum((wd.value->>'overtime_minutes')::integer),0)into ot from jsonb_array_elements(i.worksheet->'days')as wd(value);
 select coalesce(sum((a->>'effective_ot')::integer),0)into cap from jsonb_array_elements(fresh->'attendance')a;
 if ot>cap and btrim(i.worksheet->>'exception_note')=''then raise exception 'Review and document overtime exceeding current authorizations.'using errcode='23514';end if;
 if(i.totals->>'net')::bigint<0 then raise exception 'Correct deductions or advances before approving negative net pay.'using errcode='23514';end if;
 if i.calculation is distinct from hris_private.cutoff_compute(i.profile->'pay',i.worksheet,r.month,r.part,fresh#>'{first_cutoff,calculation}')or i.totals is distinct from i.calculation->'totals'then raise exception 'Cutoff calculation does not reconcile.'using errcode='23514';end if;
 if exists(select 1 from public.payroll_slips where company_id=i.company_id and employee_id=i.employee_id and month=r.month)then raise exception 'This employee already has an issued monthly register. Do not issue overlapping cutoff pay.'using errcode='23514';end if;
end;$$;

create function hris_private.cutoff_published_item(i public.payroll_cutoff_items,part integer)returns jsonb language plpgsql immutable security invoker set search_path=''as $$
declare j jsonb:=to_jsonb(i)-'source_basis';cs jsonb;begin
 j:=jsonb_set(j,'{profile,pay,coverage_note}','""');j:=jsonb_set(jsonb_set(j,'{worksheet,review_note}','""'),'{worksheet,exception_note}','""');
 select coalesce(jsonb_agg(c||jsonb_build_object('amount_cents',hris_private.cutoff_allocated((c->>'amount_cents')::bigint,part),'reference','')),'[]')into cs from jsonb_array_elements(j#>'{profile,pay,components}')c;j:=jsonb_set(j,'{profile,pay,components}',cs);
 select coalesce(jsonb_agg(c||'{"reference":""}'::jsonb),'[]')into cs from jsonb_array_elements(j#>'{worksheet,adjustments}')c;return jsonb_set(j,'{worksheet,adjustments}',cs);
end;$$;

create function hris_private.monthly_cutoff_overlap_guard()returns trigger language plpgsql security definer set search_path=''as $$
begin
 if new.status='approved'and old.status is distinct from new.status then
  perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(new.company_id::text,7));
  if exists(select 1 from public.payroll_items i join public.payroll_cutoff_slips s on s.company_id=i.company_id and s.employee_id=i.employee_id and s.month=new.month where i.company_id=new.company_id and i.run_id=new.id)then raise exception 'An employee already has issued cutoff pay for this month. Do not issue an overlapping monthly register.'using errcode='23514';end if;
 end if;return new;
end;$$;
create trigger payroll_monthly_cutoff_overlap before update of status on public.payroll_runs for each row execute function hris_private.monthly_cutoff_overlap_guard();

create function hris_private.manage_payroll_cutoff(p_operation text,p_record jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;actor_label text;today date;rid uuid;expected bigint;why text;old_record jsonb;result jsonb;birth hris_private.payroll_birth;
 r public.payroll_cutoff_runs;i public.payroll_cutoff_items;fresh jsonb;w jsonb;t jsonb;creating boolean:=false;allowed text[];begin
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Active HR or Administrator access is required.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));perform 1 from public.companies where id=m.company_id for update;
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist')for update;
 if m.user_id is null then raise exception 'Your payroll access changed. Refresh.'using errcode='42501';end if;
 select full_name into actor_label from public.employees where company_id=m.company_id and id=m.employee_id;select(now()at time zone timezone)::date into today from public.companies where id=m.company_id;
 if p_operation is null or p_operation not in('run_create','run_update','item_add','item_save','item_refresh','item_remove','run_submit','run_return','run_cancel','run_approve','run_paid')or jsonb_typeof(p_record)is distinct from 'object'or octet_length(p_record::text)>131072 then raise exception 'Use a supported bounded cutoff request.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=hris_private.payroll_text(p_record,'reason',500,true);
 if rid is null or rid='00000000-0000-0000-0000-000000000000'then raise exception 'A cutoff record identity is required.'using errcode='23514';end if;
 allowed:=case p_operation when'run_create'then array['month','part','pay_date','title']when'run_update'then array['pay_date','title']when'item_add'then array['run_id','employee_id']when'item_save'then array['worksheet']when'run_submit'then array['scope_reviewed']when'run_paid'then array['paid_on','payment_reference']else array[]::text[]end;
 if p_record-(allowed||array['id','expected_revision','reason'])<>'{}'::jsonb then raise exception 'Unsupported cutoff fields.'using errcode='23514';end if;
 if expected is null and p_operation in('run_create','item_add')then
  select * into birth from hris_private.payroll_birth where company_id=m.company_id and operation='cutoff_'||p_operation and record_id=rid;
  if birth.record_id is not null then
   if birth.actor_id<>m.user_id or birth.payload<>p_record then raise exception 'This cutoff record ID already exists.'using errcode='PT409';end if;
   if p_operation='run_create'then select to_jsonb(x)into result from public.payroll_cutoff_runs x where id=rid and company_id=m.company_id;else select to_jsonb(x)into result from public.payroll_cutoff_items x where id=rid and company_id=m.company_id;end if;
   if result is null then raise exception 'The prior record was removed. Refresh.'using errcode='PT409';end if;return result;
  end if;creating:=true;
 end if;
 if p_operation='run_create'then
  if not creating then raise exception 'Use a new run ID.'using errcode='23514';end if;
  insert into public.payroll_cutoff_runs(id,company_id,month,part,pay_date,title,created_by)values(rid,m.company_id,(p_record->>'month')::date,(p_record->>'part')::smallint,(p_record->>'pay_date')::date,hris_private.payroll_text(p_record,'title',120,true),m.user_id)returning * into r;result:=to_jsonb(r);
 elsif p_operation in('item_add','item_save','item_refresh','item_remove')then
  if p_operation='item_add'then select * into r from public.payroll_cutoff_runs where company_id=m.company_id and id=(p_record->>'run_id')::uuid for update;
  else select * into i from public.payroll_cutoff_items where company_id=m.company_id and id=rid for update;
   if i.id is null then raise exception 'Cutoff worksheet unavailable.'using errcode='42501';end if;
   if i.revision is distinct from expected then raise exception 'Cutoff worksheet changed. Refresh.'using errcode='PT409';end if;
   select * into r from public.payroll_cutoff_runs where company_id=m.company_id and id=i.run_id for update;old_record:=to_jsonb(i);
  end if;
  if r.id is null then raise exception 'Cutoff run unavailable.'using errcode='42501';end if;
  if r.status<>'draft'then raise exception 'Only draft cutoff worksheets can change. Issued payroll stays immutable.'using errcode='23514';end if;
  if p_operation='item_remove'then delete from public.payroll_cutoff_items where id=i.id;result:=old_record;
  else
   fresh:=hris_private.cutoff_basis(m.company_id,case when p_operation='item_add'then(p_record->>'employee_id')::uuid else i.employee_id end,r.month,r.part);
   if p_operation='item_save'and fresh is distinct from i.source_basis then raise exception 'Sources changed. Refresh source and review again.'using errcode='PT409';end if;
   w:=case when p_operation='item_add'then '{"days":[],"adjustments":[],"basic_override_cents":null,"sss_basis_override":null,"philhealth_basis_override":null,"pagibig_basis_override":null,"exception_note":"","review_note":"","reviewed":false}'::jsonb when p_operation='item_save'then p_record->'worksheet'else jsonb_set(jsonb_set(i.worksheet,'{reviewed}','false'),'{review_note}','""')end;
   t:=hris_private.cutoff_compute(fresh#>'{profile,pay}',w,r.month,r.part,fresh#>'{first_cutoff,calculation}');
   if p_operation='item_add'then
    insert into public.payroll_cutoff_items(id,company_id,run_id,employee_id,employee_name,employee_code,department,profile,source_basis,worksheet,totals,calculation)values(rid,m.company_id,r.id,(fresh#>>'{employee,id}')::uuid,fresh#>>'{employee,full_name}',fresh#>>'{employee,employee_code}',coalesce(fresh#>>'{employee,department}',''),fresh->'profile',fresh,w,t->'totals',t)returning * into i;
   else
    update public.payroll_cutoff_items set employee_name=fresh#>>'{employee,full_name}',employee_code=fresh#>>'{employee,employee_code}',department=coalesce(fresh#>>'{employee,department}',''),profile=fresh->'profile',source_basis=fresh,worksheet=w,totals=t->'totals',calculation=t,revision=revision+1 where id=rid returning * into i;
   end if;result:=to_jsonb(i);
  end if;
  update public.payroll_cutoff_runs set revision=revision+1,updated_at=clock_timestamp()where id=r.id;
 else
  select * into r from public.payroll_cutoff_runs where company_id=m.company_id and id=rid for update;
  if r.id is null then raise exception 'Cutoff run unavailable.'using errcode='42501';end if;
  if r.revision is distinct from expected then raise exception 'Cutoff run changed. Refresh.'using errcode='PT409';end if;old_record:=to_jsonb(r);
  if p_operation in('run_submit','run_approve')then
   if r.ends_on>today or r.month>=date '2027-01-01'then raise exception 'Approve completed cutoffs only. A2027 cutoff needs refreshed statutory rules.'using errcode='23514';end if;
   if not exists(select 1 from public.payroll_cutoff_items where run_id=r.id)then raise exception 'Add at least one reviewed worksheet.'using errcode='23514';end if;
   for i in select * from public.payroll_cutoff_items where company_id=m.company_id and run_id=r.id order by id loop perform hris_private.cutoff_check_item(i,r);end loop;
  end if;
  if p_operation='run_update'then
   if r.status<>'draft'then raise exception 'Only a draft cutoff title/pay date can change.'using errcode='23514';end if;
   update public.payroll_cutoff_runs set title=hris_private.payroll_text(p_record,'title',120,true),pay_date=(p_record->>'pay_date')::date,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='run_submit'then
   if r.status<>'draft'or(p_record->>'scope_reviewed')::boolean is not true then raise exception 'Confirm included/omitted employees before submitting a draft.'using errcode='23514';end if;
   update public.payroll_cutoff_runs set status='submitted',submitted_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='run_return'then
   if r.status<>'submitted'then raise exception 'Only submitted cutoff pay can return to draft.'using errcode='23514';end if;
   update public.payroll_cutoff_runs set status='draft',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='run_cancel'then
   if r.status not in('draft','submitted')then raise exception 'Issued cutoff pay cannot be cancelled or overwritten.'using errcode='23514';end if;
   update public.payroll_cutoff_runs set status='cancelled',submitted_by=null,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  elsif p_operation='run_approve'then
   if m.role<>'administrator'or r.submitted_by=m.user_id then raise exception 'Another Administrator must approve cutoff payroll.'using errcode='42501';end if;
   if r.status<>'submitted'then raise exception 'Submit this cutoff before approval.'using errcode='23514';end if;
   insert into public.payroll_cutoff_slips(id,company_id,run_id,employee_id,month,part,starts_on,ends_on,pay_date,company_name,item,calculation,approved_by,rule_version)
    select pi.id,pi.company_id,pi.run_id,pi.employee_id,r.month,r.part,r.starts_on,r.ends_on,r.pay_date,c.name,hris_private.cutoff_published_item(pi,r.part),pi.calculation,m.user_id,r.rule_version from public.payroll_cutoff_items pi join public.companies c on c.id=pi.company_id where pi.company_id=m.company_id and pi.run_id=r.id;
   update public.payroll_cutoff_runs set status='approved',approved_by=m.user_id,revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  else
   if m.role<>'administrator'then raise exception 'Only Administrators record payments.'using errcode='42501';end if;
   if r.status<>'approved'or(p_record->>'paid_on')is null or(p_record->>'paid_on')::date>today or(p_record->>'paid_on')::date<r.ends_on then raise exception 'Record an actual payment date after this cutoff ends.'using errcode='23514';end if;
   update public.payroll_cutoff_runs set status='paid',paid_on=(p_record->>'paid_on')::date,payment_reference=hris_private.payroll_text(p_record,'payment_reference',500,true),revision=revision+1,updated_at=clock_timestamp()where id=rid returning * into r;
  end if;result:=to_jsonb(r);
 end if;
 if creating then insert into hris_private.payroll_birth(company_id,operation,record_id,actor_id,payload)values(m.company_id,'cutoff_'||p_operation,rid,m.user_id,p_record);end if;
 insert into public.payroll_events(company_id,record_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)values(m.company_id,rid,'cutoff_'||p_operation,why,m.user_id,actor_label,m.role,old_record,result);
 return result;
end;$$;

revoke all on function hris_private.cutoff_tax(bigint),hris_private.cutoff_allocated(bigint,integer),hris_private.cutoff_compute(jsonb,jsonb,date,integer,jsonb),hris_private.cutoff_basis(uuid,uuid,date,integer),hris_private.cutoff_check_item(public.payroll_cutoff_items,public.payroll_cutoff_runs),hris_private.cutoff_published_item(public.payroll_cutoff_items,integer),hris_private.monthly_cutoff_overlap_guard(),hris_private.manage_payroll_cutoff(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_payroll_cutoff(text,jsonb)to authenticated;
create function public.manage_payroll_cutoff(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_payroll_cutoff(p_operation,p_record);$$;
revoke all on function public.manage_payroll_cutoff(text,jsonb)from public,anon,authenticated;
grant execute on function public.manage_payroll_cutoff(text,jsonb)to authenticated;
commit;
