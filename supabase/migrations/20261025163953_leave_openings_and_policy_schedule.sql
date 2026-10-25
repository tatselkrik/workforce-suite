-- Prospective policy snapshots and reviewed balance takeover. No payroll or
-- already-filed charge/approval snapshot is rewritten. Companies opt in when
-- the first opening or dated policy is posted; older unchanged companies retain
-- their existing policy contract.
begin;
create table public.leave_dated_state (
 company_id uuid primary key references public.companies(id),
 revision bigint not null default 0 check(revision between 0 and 5000),
 baseline jsonb not null,
 enabled_on date not null
);
create table public.leave_policy_schedules (
 id uuid primary key check(id<>'00000000-0000-0000-0000-000000000000'),
 company_id uuid not null references public.companies(id),
 effective_on date not null check(effective_on between date'1900-01-01'and date'2200-12-31'),
 policy jsonb not null, policy_revision bigint not null,
 reason text not null check(char_length(btrim(reason))between 5 and 500),
 reference text not null check(char_length(reference)<=160),
 created_by uuid not null references auth.users(id),created_by_name text not null,
 posted_at timestamptz not null default clock_timestamp(),
 company_revision bigint not null,request_digest text not null,
 unique(company_id,policy_revision),unique(company_id,company_revision)
);
create table public.leave_policy_withdrawals (
 id uuid primary key check(id<>'00000000-0000-0000-0000-000000000000'),
 company_id uuid not null references public.companies(id),
 schedule_id uuid not null unique references public.leave_policy_schedules(id),
 reason text not null check(char_length(btrim(reason))between 5 and 500),
 reference text not null check(char_length(reference)<=160),
 created_by uuid not null references auth.users(id),created_by_name text not null,
 posted_at timestamptz not null default clock_timestamp(),
 company_revision bigint not null,request_digest text not null,
 unique(company_id,company_revision)
);
create table public.leave_entitlement_anchors (
 company_id uuid not null references public.companies(id),employee_id uuid not null,
 type_code text not null,year integer not null check(year between 1900 and 2200),
 basis jsonb not null,type_snapshot jsonb not null,incoming_carry_units integer not null check(incoming_carry_units between 0 and 732),
 primary key(company_id,employee_id,type_code,year),
 foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create table public.leave_opening_entries (
 id uuid primary key check(id<>'00000000-0000-0000-0000-000000000000'),
 company_id uuid not null references public.companies(id),employee_id uuid not null,
 type_code text not null,year integer not null check(year between 1900 and 2200),
 opening jsonb not null,
 reason text not null check(char_length(btrim(reason))between 5 and 500),
 reference text not null check(char_length(reference)<=160),
 source_requests_reviewed boolean not null check(source_requests_reviewed),
 created_by uuid not null references auth.users(id),created_by_name text not null,
 posted_at timestamptz not null default clock_timestamp(),
 company_revision bigint not null,request_digest text not null,
 unique(company_id,employee_id,type_code,year),unique(company_id,company_revision),
 foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create table public.leave_opening_batches (
 id uuid primary key check(id<>'00000000-0000-0000-0000-000000000000'),
 company_id uuid not null references public.companies(id),created_by uuid not null references auth.users(id),
 request_digest text not null,opening_ids jsonb not null,company_revision bigint not null,
 posted_at timestamptz not null default clock_timestamp()
);
do $$declare tab text;begin
 foreach tab in array array['leave_dated_state','leave_policy_schedules','leave_policy_withdrawals','leave_entitlement_anchors','leave_opening_entries','leave_opening_batches']loop
  execute format('alter table public.%I enable row level security',tab);
  execute format('revoke all on public.%I from public,anon,authenticated',tab);
  execute format('grant all on public.%I to service_role',tab);
  if tab<>'leave_dated_state'then execute format('create trigger dated_history_immutable before update or delete on public.%I for each row execute function hris_private.leave_history_immutable()',tab);end if;
 end loop;
end;$$;

create function hris_private.dated_member()returns public.company_members
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active;
 if not found or not exists(select 1 from public.employees where company_id=m.company_id and id=m.employee_id and status='active')then raise exception 'Active employee access is required.'using errcode='42501';end if;
 return m;
end;$$;
create function hris_private.leave_policy_on(p_company uuid,p_on date)returns jsonb
language sql stable security definer set search_path=''as $$
 select coalesce((select s.policy from public.leave_policy_schedules s where s.company_id=p_company and s.effective_on<=p_on
 and not exists(select 1 from public.leave_policy_withdrawals w where w.schedule_id=s.id)order by s.effective_on desc limit 1),
 (select baseline from public.leave_dated_state where company_id=p_company),(select to_jsonb(p)from public.leave_policies p where p.company_id=p_company));
$$;
create function hris_private.leave_type_on(p_company uuid,p_code text,p_on date)returns jsonb
language sql stable security definer set search_path=''as $$
 select value from jsonb_array_elements(hris_private.leave_policy_on(p_company,p_on)->'types')where value->>'code'=p_code;
$$;
create function hris_private.make_leave_basis(p_employee uuid,p_type jsonb,p_on date)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare e public.employees;rate integer;months integer;earned integer;
begin
 select *into strict e from public.employees where id=p_employee;
 if p_type is null or p_type->>'accrual'='unlimited'or(e.joined_on is not null and e.joined_on>p_on)then raise exception 'Use an employed person and a finite leave entitlement.'using errcode='23514';end if;
 rate:=case when(p_type->>'use_employee_allowance')::boolean then e.annual_leave_days*2 else(p_type->>'annual_units')::integer end;
 if rate not between 0 and 732 then raise exception 'Unsupported annual allowance.'using errcode='23514';end if;
 earned:=rate;
 if p_type->>'accrual'='monthly'then
  if e.joined_on is null then raise exception 'Set a known Date joined before capturing monthly accrual.'using errcode='23514';end if;
  select count(*)into months from generate_series(1,12)m where make_date(extract(year from p_on)::integer,m,1)>=e.joined_on
   and(make_date(extract(year from p_on)::integer,m,1)+interval'1 month - 1 day')::date<=p_on;
  earned:=rate*months/12;
 end if;
 return jsonb_build_object('known_from',p_on,'rate_units',rate,'accrual',p_type->>'accrual','joined_on',e.joined_on,'earned_rate_at_capture',earned);
end;$$;
create function hris_private.leave_basis_earned(p_basis jsonb,p_on date)returns integer
language plpgsql immutable security invoker set search_path=''as $$
declare known date:=(p_basis->>'known_from')::date;months integer;
begin
 if p_on<known or extract(year from p_on)<>extract(year from known)then raise exception 'This entitlement is known only from its captured date; do not reconstruct earlier grants.'using errcode='23514';end if;
 if p_basis->>'accrual'='upfront'then return(p_basis->>'earned_rate_at_capture')::integer;end if;
 select count(*)into months from generate_series(1,12)m where make_date(extract(year from p_on)::integer,m,1)>=(p_basis->>'joined_on')::date
  and(make_date(extract(year from p_on)::integer,m,1)+interval'1 month - 1 day')::date<=p_on;
 return(p_basis->>'rate_units')::integer*months/12;
end;$$;
create function hris_private.leave_year_bookings(p_employee uuid,p_code text,p_year integer,p_expiry date,p_exclude uuid default null)returns jsonb
language sql stable security definer set search_path=''as $$
 select coalesce(jsonb_agg(jsonb_build_object('request_id',r.id,'units',r.charged_units,'early_units',
 (select coalesce(sum((d->>'units')::integer),0)from jsonb_array_elements(r.charged_dates)d where(d->>'date')::date<=p_expiry),
 'approved',r.status='approved')order by r.id),'[]')from public.leave_requests r
 where r.employee_id=p_employee and r.type_code=p_code and extract(year from r.starts_on)=p_year and r.status in('pending','approved')and(p_exclude is null or r.id<>p_exclude);
$$;

-- Keep the unchanged legacy calculation available only to the protected engine.
alter function hris_private.leave_balance(uuid,jsonb,integer,date,uuid)rename to leave_balance_before_dated;
revoke all on function hris_private.leave_balance_before_dated(uuid,jsonb,integer,date,uuid)from public,anon,authenticated;
create function hris_private.leave_balance_detail(p_employee uuid,p_code text,p_year integer,p_asof date,p_exclude uuid default null)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare e public.employees;first_year integer;y integer;carry integer:=0;unused integer;earned integer;t jsonb;boundary date;on_date date;
 rows jsonb;totals jsonb;result jsonb;op jsonb;projected jsonb;a public.leave_entitlement_anchors;policy jsonb;
begin
 if p_year not between 1900 and 2200 or p_asof is null or extract(year from p_asof)<>p_year then raise exception 'Use a supported date in the balance year.'using errcode='23514';end if;
 select *into strict e from public.employees where id=p_employee;
 policy:=hris_private.leave_policy_on(e.company_id,p_asof);
 first_year:=least(p_year,greatest((policy->>'tracking_starts_year')::integer,coalesce(extract(year from e.joined_on)::integer,p_year)));
 select least(first_year,coalesce(min(year),first_year))into first_year from public.leave_entitlement_anchors where employee_id=e.id and type_code=p_code;
 for y in first_year..p_year loop
  on_date:=case when y=p_year then p_asof else make_date(y,12,31)end;
  t:=hris_private.leave_type_on(e.company_id,p_code,make_date(y,1,1));
  select *into a from public.leave_entitlement_anchors where company_id=e.company_id and employee_id=e.id and type_code=p_code and year=y;
  if found then t:=a.type_snapshot;end if;
  if t is null or t->>'accrual'='unlimited'then
   if y=p_year then raise exception 'This date has no finite entitlement for that type.'using errcode='23514';end if;
   carry:=0;continue;
  end if;
  boundary:=(make_date(y,(t->>'carryover_expiry_month')::integer,1)+interval'1 month - 1 day')::date;
  carry:=case when a.employee_id is not null then a.incoming_carry_units else least(carry,(t->>'carryover_units')::integer)end;
  rows:=hris_private.leave_year_bookings(e.id,p_code,y,boundary,p_exclude);
  totals:=hris_private.opening_booking_totals(rows);
  if a.employee_id is not null then earned:=hris_private.leave_basis_earned(a.basis,on_date);
  else earned:=hris_private.leave_earned(e.id,t,y,on_date);end if;
  earned:=earned+hris_private.leave_credit_units(e.id,p_code,y,on_date);
  select opening into op from public.leave_opening_entries where company_id=e.company_id and employee_id=e.id and type_code=p_code and year=y and(opening->>'effective_on')::date<=on_date;
  if found then
   projected:=hris_private.opening_project(op,on_date,earned,rows);result:=projected->'balance';unused:=(projected->>'unused_base_units')::integer;
  else
   unused:=earned-(totals->>'total_units')::integer+least(carry,(totals->>'early_units')::integer);
   result:=jsonb_build_object('earned_units',earned,'carryover_units',case when on_date<=boundary then carry else 0 end,
    'used_units',(totals->>'approved_units')::integer,'reserved_units',(totals->>'total_units')::integer-(totals->>'approved_units')::integer,
    'available_units',unused+case when on_date<=boundary then carry-least(carry,(totals->>'early_units')::integer)else 0 end);
  end if;
  if y=p_year then return jsonb_build_object('balance',result,'unused_base_units',unused,'gross_carry_units',case when op is not null then(op->>'previously_consumed_carry_units')::integer+(op->>'remaining_carry_units')::integer else carry end);end if;
  carry:=greatest(unused,0);
 end loop;
 raise exception 'No supported balance basis.'using errcode='23514';
end;$$;
create function hris_private.leave_balance(p_employee uuid,p_type jsonb,p_year integer,p_asof date,p_exclude uuid default null)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare company uuid;
begin
 select company_id into strict company from public.employees where id=p_employee;
 if not exists(select 1 from public.leave_dated_state where company_id=company)then return hris_private.leave_balance_before_dated(p_employee,p_type,p_year,p_asof,p_exclude);end if;
 return hris_private.leave_balance_detail(p_employee,p_type->>'code',p_year,p_asof,p_exclude)->'balance';
end;$$;

create function hris_private.freeze_leave_year(p_employee uuid,p_on date)returns void
language plpgsql security definer set search_path=''as $$
declare e public.employees;t jsonb;basis jsonb;incoming integer;
begin
 select *into strict e from public.employees where id=p_employee;
 if e.joined_on is not null and e.joined_on>p_on then return;end if;
 for t in select value from jsonb_array_elements(hris_private.leave_policy_on(e.company_id,make_date(extract(year from p_on)::integer,1,1))->'types')loop
  if t->>'accrual'='unlimited'or(t->>'accrual'='monthly'and e.joined_on is null)then continue;end if;
  if not exists(select 1 from public.leave_entitlement_anchors where company_id=e.company_id and employee_id=e.id and type_code=t->>'code'and year=extract(year from p_on))then
   basis:=hris_private.make_leave_basis(e.id,t,p_on);
   incoming:=(hris_private.leave_balance_detail(e.id,t->>'code',extract(year from p_on)::integer,p_on)->>'gross_carry_units')::integer;
   insert into public.leave_entitlement_anchors(company_id,employee_id,type_code,year,basis,type_snapshot,incoming_carry_units)values(e.company_id,e.id,t->>'code',extract(year from p_on)::integer,basis,t,incoming);
  end if;
 end loop;
end;$$;
create function hris_private.freeze_leave_before_employment_change()returns trigger
language plpgsql security definer set search_path=''as $$
declare today date;
begin
 if(old.annual_leave_days is distinct from new.annual_leave_days or old.joined_on is distinct from new.joined_on)and exists(select 1 from public.leave_dated_state where company_id=old.company_id)then
  perform pg_advisory_xact_lock(hashtextextended(old.company_id::text,7));
  select(now()at time zone timezone)::date into strict today from public.companies where id=old.company_id;
  perform hris_private.freeze_leave_year(old.id,today);
 end if;
 return new;
end;$$;
create trigger employees_aa_freeze_leave before update on public.employees for each row execute function hris_private.freeze_leave_before_employment_change();

-- Once a current-year basis exists, the employment allowance becomes a future
-- default; current-year changes use the separate audited credit ledger.
alter function hris_private.minimum_annual_allowance(uuid,integer)rename to minimum_annual_allowance_before_dated;
revoke all on function hris_private.minimum_annual_allowance_before_dated(uuid,integer)from public,anon,authenticated;
create function hris_private.minimum_annual_allowance(p_employee uuid,p_year integer)returns numeric
language plpgsql stable security definer set search_path=''as $$
declare company uuid;
begin
 select company_id into company from public.employees where id=p_employee;
 if coalesce(hris_private.company_role(company),'')not in('administrator','hr_generalist')or auth.uid()is null then raise exception 'HR access is required.'using errcode='42501';end if;
 if exists(select 1 from public.leave_entitlement_anchors where company_id=company and employee_id=p_employee and type_code='annual'and year=p_year)then return 0;end if;
 return hris_private.minimum_annual_allowance_before_dated(p_employee,p_year);
end;$$;
grant execute on function hris_private.minimum_annual_allowance(uuid,integer)to authenticated;

-- Existing undated editing remains usable until this company's first takeover
-- or scheduled version. Thereafter the protected dated workflow is mandatory.
alter function hris_private.save_leave_policy(jsonb,bigint)rename to save_leave_policy_before_dated;
revoke all on function hris_private.save_leave_policy_before_dated(jsonb,bigint)from public,anon,authenticated;
create function hris_private.save_leave_policy(p_policy jsonb,p_expected_revision bigint)returns bigint
language plpgsql security definer set search_path=''as $$
declare m public.company_members;
begin
 m:=hris_private.dated_member();
 if m.role<>'administrator'then raise exception 'Only administrators configure leave policies.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));
 if exists(select 1 from public.leave_dated_state where company_id=m.company_id)then raise exception 'Use the updated effective-date policy editor. Existing versions are retained.'using errcode='PT409';end if;
 return hris_private.save_leave_policy_before_dated(p_policy,p_expected_revision);
end;$$;
grant execute on function hris_private.save_leave_policy(jsonb,bigint)to authenticated;

create function hris_private.leave_dated_fingerprint(p_company uuid,p_employee uuid,p_code text,p_on date)returns text
language sql stable security definer set search_path=''as $$
 select md5(jsonb_build_object('today',p_on,'company',(select jsonb_build_object('timezone',timezone)from public.companies where id=p_company),
 'state',(select to_jsonb(s)from public.leave_dated_state s where company_id=p_company),
 'legacy',(select to_jsonb(p)from public.leave_policies p where company_id=p_company),
 'policies',(select coalesce(jsonb_agg(to_jsonb(s)order by company_revision),'[]')from public.leave_policy_schedules s where company_id=p_company),
 'withdrawals',(select coalesce(jsonb_agg(to_jsonb(w)order by company_revision),'[]')from public.leave_policy_withdrawals w where company_id=p_company),
 'employee',(select to_jsonb(e)from public.employees e where company_id=p_company and id=p_employee),
 'requests',(select coalesce(jsonb_agg(to_jsonb(r)order by id),'[]')from public.leave_requests r where company_id=p_company and employee_id=p_employee and type_code=p_code),
 'credits',(select coalesce(jsonb_agg(to_jsonb(c)order by company_revision),'[]')from public.leave_credit_entries c where company_id=p_company and employee_id=p_employee and type_code=p_code),
 'anchors',(select coalesce(jsonb_agg(to_jsonb(a)order by year),'[]')from public.leave_entitlement_anchors a where company_id=p_company and employee_id=p_employee and type_code=p_code),
 'openings',(select coalesce(jsonb_agg(to_jsonb(o)order by year),'[]')from public.leave_opening_entries o where company_id=p_company and employee_id=p_employee and type_code=p_code))::text);
$$;
create function hris_private.require_unused_dated_identity(p_id uuid)returns void
language plpgsql stable security definer set search_path=''as $$begin
 if exists(select 1 from public.leave_opening_entries where id=p_id)or exists(select 1 from public.leave_policy_schedules where id=p_id)or exists(select 1 from public.leave_policy_withdrawals where id=p_id)or exists(select 1 from public.leave_opening_batches where id=p_id)then raise exception 'That leave setup request identity was already used.'using errcode='PT409';end if;
end;$$;
create function hris_private.preview_leave_opening(p_record jsonb)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;e public.employees;today date;on_date date;person uuid;code text;rid uuid;t jsonb;basis jsonb;
 expiry date;rows jsonb;totals jsonb;detail jsonb;op jsonb;projected jsonb;earned integer;gross integer;remaining integer;target integer;rev bigint;
begin
 m:=hris_private.dated_member();
 if m.role not in('administrator','hr_generalist')then raise exception 'HR access is required for an opening preview.'using errcode='42501';end if;
 if jsonb_typeof(p_record)is distinct from'object'or octet_length(p_record::text)>6000 then raise exception 'Enter a valid opening record.'using errcode='23514';end if;
 if(select array_agg(k order by k)from jsonb_object_keys(p_record)k)is distinct from array['effective_on','employee_id','expected_revision','id','reason','reference','remaining_carry_units','source_requests_reviewed','target_available_units','type_code']::text[]
 or jsonb_typeof(p_record->'id')is distinct from'string'or jsonb_typeof(p_record->'employee_id')is distinct from'string'
 or jsonb_typeof(p_record->'type_code')is distinct from'string'or coalesce(p_record->>'type_code','')!~'^[a-z0-9_]{1,30}$'
 or jsonb_typeof(p_record->'effective_on')is distinct from'string'or coalesce(p_record->>'effective_on','')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'
 or jsonb_typeof(p_record->'expected_revision')is distinct from'number'or coalesce(p_record->>'expected_revision','')!~'^[0-9]{1,4}$'
 or jsonb_typeof(p_record->'target_available_units')is distinct from'number'or coalesce(p_record->>'target_available_units','')!~'^[0-9]{1,3}$'
 or jsonb_typeof(p_record->'remaining_carry_units')is distinct from'number'or coalesce(p_record->>'remaining_carry_units','')!~'^[0-9]{1,3}$'
 or p_record->'source_requests_reviewed'is distinct from'true'::jsonb
 or jsonb_typeof(p_record->'reason')is distinct from'string'or char_length(btrim(p_record->>'reason'))not between 5 and 500
 or jsonb_typeof(p_record->'reference')is distinct from'string'or char_length(p_record->>'reference')>160
 or(p_record->>'reason')~'[[:cntrl:]]'or(p_record->>'reference')~'[[:cntrl:]]'then raise exception 'Use whole half-day units, a reason and the reviewed source-request declaration.'using errcode='23514';end if;
 person:=(p_record->>'employee_id')::uuid;rid:=(p_record->>'id')::uuid;code:=p_record->>'type_code';on_date:=(p_record->>'effective_on')::date;
 target:=(p_record->>'target_available_units')::integer;remaining:=(p_record->>'remaining_carry_units')::integer;
 select(now()at time zone timezone)::date into strict today from public.companies where id=m.company_id;
 select coalesce((select revision from public.leave_dated_state where company_id=m.company_id),0)into rev;
 if rev is distinct from(p_record->>'expected_revision')::bigint then raise exception 'Leave setup changed. Refresh the opening preview.'using errcode='PT409';end if;
 select *into e from public.employees where company_id=m.company_id and id=person;
 if not found then raise exception 'Choose an employee in this company.'using errcode='42501';end if;
 if rid='00000000-0000-0000-0000-000000000000'or e.status<>'active'or on_date<>today or target not between 0 and 732 or remaining not between 0 and target then raise exception 'An opening takes over an active employee balance today, in whole or half days through 366 days.'using errcode='23514';end if;
 if exists(select 1 from public.leave_opening_entries where company_id=m.company_id and employee_id=person and type_code=code and year=extract(year from today))then raise exception 'This year already has an immutable opening. Use an audited credit adjustment for a later correction.'using errcode='PT409';end if;
 if exists(select 1 from public.leave_requests where employee_id=person and type_code=code and status in('pending','approved')and extract(year from starts_on)>extract(year from today))then raise exception 'Review later-year reservations before taking over this opening balance.'using errcode='23514';end if;
 t:=hris_private.leave_type_on(m.company_id,code,make_date(extract(year from today)::integer,1,1));
 if t is null or t->>'accrual'='unlimited'then raise exception 'Choose a finite leave type available this year.'using errcode='23514';end if;
 select a.basis,a.type_snapshot into basis,t from public.leave_entitlement_anchors a where company_id=m.company_id and employee_id=person and type_code=code and year=extract(year from today);
 if not found then t:=hris_private.leave_type_on(m.company_id,code,make_date(extract(year from today)::integer,1,1));basis:=hris_private.make_leave_basis(person,t,today);end if;
 expiry:=(make_date(extract(year from today)::integer,(t->>'carryover_expiry_month')::integer,1)+interval'1 month - 1 day')::date;
 earned:=hris_private.leave_basis_earned(basis,today)+hris_private.leave_credit_units(person,code,extract(year from today)::integer,today);
 if earned<0 then raise exception 'Resolve the negative base-credit basis before importing an opening.'using errcode='23514';end if;
 rows:=hris_private.leave_year_bookings(person,code,extract(year from today)::integer,expiry);
 totals:=hris_private.opening_booking_totals(rows);
 detail:=hris_private.leave_balance_detail(person,code,extract(year from today)::integer,today);
 gross:=(detail->>'gross_carry_units')::integer;
 if least(gross,(totals->>'early_units')::integer)+remaining>(t->>'carryover_units')::integer
 or((least(gross,(totals->>'early_units')::integer)+remaining)>0 and(basis->>'joined_on'is null or extract(year from(basis->>'joined_on')::date)>=extract(year from today)))then raise exception 'Carry takeover needs prior-year service and must stay within the cap, including carry already consumed.'using errcode='23514';end if;
 op:=jsonb_build_object('id',rid,'employee_id',person,'type_code',code,'effective_on',today,'carry_expires_on',expiry,'base_earned_at_opening',earned,
 'target_available_units',target,'remaining_carry_units',remaining,'previously_consumed_carry_units',least(gross,(totals->>'early_units')::integer),'baseline',rows);
 projected:=hris_private.opening_project(op,today,earned,rows);
 -- Once imported carry expires, the already-booked base must still be funded.
 if(projected->>'unused_base_units')::integer<0 then raise exception 'The entered base component does not cover existing reservations after carry expiry.'using errcode='23514';end if;
 return jsonb_build_object('version','leave-opening-preview-v1','company_id',m.company_id,'user_id',m.user_id,'as_of',today,
 'record',p_record,'fingerprint',hris_private.leave_dated_fingerprint(m.company_id,person,code,today),'opening',op,'basis',basis,'type_snapshot',t,
 'before',detail->'balance','after',projected->'balance');
exception when invalid_text_representation or invalid_datetime_format or datetime_field_overflow or numeric_value_out_of_range then raise exception 'Enter valid dates, identities and whole half-day quantities.'using errcode='23514';
end;$$;
create function public.preview_leave_opening(p_record jsonb)returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.preview_leave_opening(p_record);$$;

create function hris_private.post_leave_opening(p_record jsonb,p_fingerprint text)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;preview jsonb;saved public.leave_opening_entries;rid uuid;rev bigint;today date;
begin
 m:=hris_private.dated_member();
 if m.role<>'administrator'then raise exception 'Only an Administrator posts opening balances.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));
 m:=hris_private.dated_member();if m.role<>'administrator'then raise exception 'Your Administrator access changed.'using errcode='42501';end if;
 if jsonb_typeof(p_record)is distinct from'object'or p_fingerprint is null or p_fingerprint!~'^[a-f0-9]{32}$'then raise exception 'Review a protected opening preview first.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;
 select *into saved from public.leave_opening_entries where id=rid;
 if found then
  if saved.company_id is distinct from m.company_id or saved.created_by is distinct from m.user_id or saved.request_digest is distinct from md5(jsonb_build_array(p_record,p_fingerprint)::text)then raise exception 'That opening request identity was used for a different action. Refresh.'using errcode='PT409';end if;
  return jsonb_build_object('id',saved.id,'company_revision',saved.company_revision);
 end if;
 perform hris_private.require_unused_dated_identity(rid);
 preview:=hris_private.preview_leave_opening(p_record);
 if preview->>'fingerprint'is distinct from p_fingerprint then raise exception 'The employee, policy, credits or reservations changed. Review a new preview.'using errcode='PT409';end if;
 today:=(preview->>'as_of')::date;rev:=(p_record->>'expected_revision')::bigint;
 if rev>=5000 then raise exception 'Leave setup history exceeds the supported review limit.'using errcode='54000';end if;
 insert into public.leave_dated_state(company_id,revision,baseline,enabled_on)select m.company_id,0,to_jsonb(p),today from public.leave_policies p where company_id=m.company_id on conflict do nothing;
 perform hris_private.freeze_leave_year((p_record->>'employee_id')::uuid,today);
 insert into public.leave_opening_entries(id,company_id,employee_id,type_code,year,opening,reason,reference,source_requests_reviewed,created_by,created_by_name,company_revision,request_digest)
 values(rid,m.company_id,(p_record->>'employee_id')::uuid,p_record->>'type_code',extract(year from today)::integer,preview->'opening',btrim(p_record->>'reason'),p_record->>'reference',true,m.user_id,hris_private.leave_actor_name(),rev+1,md5(jsonb_build_array(p_record,p_fingerprint)::text));
 update public.leave_dated_state set revision=rev+1 where company_id=m.company_id;
 perform hris_private.leave_dated_context();
 return jsonb_build_object('id',rid,'company_revision',rev+1);
exception when invalid_text_representation then raise exception 'Use a valid opening identity.'using errcode='23514';
end;$$;
create function public.post_leave_opening(p_record jsonb,p_fingerprint text)returns jsonb language sql security invoker set search_path=''as $$select hris_private.post_leave_opening(p_record,p_fingerprint);$$;

create function hris_private.preview_leave_openings(p_records jsonb)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;item jsonb;n integer:=0;rows jsonb:='[]';preview jsonb;err text;today date;result jsonb;
begin
 m:=hris_private.dated_member();if m.role not in('administrator','hr_generalist')then raise exception 'HR access is required for opening imports.'using errcode='42501';end if;
 if jsonb_typeof(p_records)is distinct from'array'or octet_length(p_records::text)>1000000 then raise exception 'Use a bounded array of opening rows.'using errcode='23514';end if;
 if jsonb_array_length(p_records)not between 1 and 100 then raise exception 'Import 1–100 opening rows at a time.'using errcode='23514';end if;
 select(now()at time zone timezone)::date into strict today from public.companies where id=m.company_id;
 for item in select value from jsonb_array_elements(p_records)loop
  n:=n+1;preview:=null;err:=null;
  begin
   if exists(select 1 from jsonb_array_elements(p_records)x where x->'id'=item->'id'group by x->'id'having count(*)>1)
   or exists(select 1 from jsonb_array_elements(p_records)x where x->'employee_id'=item->'employee_id'and x->'type_code'=item->'type_code'group by x->'employee_id',x->'type_code'having count(*)>1)then raise exception 'Duplicate request identity or employee/type opening in this import.'using errcode='23514';end if;
   preview:=hris_private.preview_leave_opening(item);
  exception when SQLSTATE'23514'or SQLSTATE'42501'or SQLSTATE'PT409'or SQLSTATE'54000'then err:=left(sqlerrm,250);
  end;
  rows:=rows||jsonb_build_array(jsonb_build_object('row_number',n,'record',item,'preview',preview,'error',err));
 end loop;
 result:=jsonb_build_object('version','leave-opening-batch-preview-v1','company_id',m.company_id,'user_id',m.user_id,'as_of',today,'rows',rows);
 if octet_length(result::text)>4000000 then raise exception 'Opening preview exceeds the review limit; split the file.'using errcode='54000';end if;
 return result;
end;$$;
create function public.preview_leave_openings(p_records jsonb)returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.preview_leave_openings(p_records);$$;

create function hris_private.post_leave_openings(p_batch_id uuid,p_records jsonb,p_fingerprints jsonb,p_expected_revision bigint)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;receipt public.leave_opening_batches;result jsonb;row jsonb;record jsonb;preview jsonb;rev bigint;today date;ids jsonb:='[]';n integer:=0;digest text;
begin
 m:=hris_private.dated_member();if m.role<>'administrator'then raise exception 'Only an Administrator posts an opening import.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));m:=hris_private.dated_member();if m.role<>'administrator'then raise exception 'Your Administrator access changed.'using errcode='42501';end if;
 digest:=md5(jsonb_build_array(p_records,p_fingerprints,p_expected_revision)::text);
 select *into receipt from public.leave_opening_batches where id=p_batch_id;
 if found then
  if receipt.company_id is distinct from m.company_id or receipt.created_by is distinct from m.user_id or receipt.request_digest is distinct from digest then raise exception 'That import request identity was already used for another action.'using errcode='PT409';end if;
  return jsonb_build_object('id',receipt.id,'company_revision',receipt.company_revision,'opening_ids',receipt.opening_ids);
 end if;
 if p_batch_id is null or p_batch_id='00000000-0000-0000-0000-000000000000'or p_expected_revision is null then raise exception 'Use a valid import request and current revision.'using errcode='23514';end if;
 perform hris_private.require_unused_dated_identity(p_batch_id);
 if jsonb_typeof(p_fingerprints)is distinct from'array'or octet_length(p_fingerprints::text)>10000 then raise exception 'Review all imported opening rows first.'using errcode='23514';end if;
 select coalesce((select revision from public.leave_dated_state where company_id=m.company_id),0)into rev;
 if rev is distinct from p_expected_revision then raise exception 'Leave setup changed. Preview the entire import again.'using errcode='PT409';end if;
 result:=hris_private.preview_leave_openings(p_records);
 if jsonb_array_length(p_fingerprints)<>jsonb_array_length(result->'rows')or exists(select 1 from jsonb_array_elements(result->'rows')r where r->'error'<>'null'::jsonb)then raise exception 'Resolve every row error before posting; no rows were applied.'using errcode='23514';end if;
 if rev+jsonb_array_length(p_records)>5000 then raise exception 'Leave setup exceeds the supported review limit.'using errcode='54000';end if;
 for row in select value from jsonb_array_elements(result->'rows')loop
  record:=row->'record';preview:=row->'preview';
  if p_fingerprints->n is distinct from preview->'fingerprint'or(record->>'expected_revision')::bigint<>rev then raise exception 'An imported employee, policy, credit or reservation changed. Preview all rows again.'using errcode='PT409';end if;
  if(record->>'id')::uuid=p_batch_id then raise exception 'Import and row identities must differ.'using errcode='23514';end if;
  perform hris_private.require_unused_dated_identity((record->>'id')::uuid);n:=n+1;
 end loop;
 today:=(result->>'as_of')::date;
 insert into public.leave_dated_state(company_id,revision,baseline,enabled_on)select m.company_id,0,to_jsonb(p),today from public.leave_policies p where company_id=m.company_id on conflict do nothing;
 n:=0;
 for row in select value from jsonb_array_elements(result->'rows')loop
  n:=n+1;record:=row->'record';preview:=row->'preview';
  perform hris_private.freeze_leave_year((record->>'employee_id')::uuid,today);
  insert into public.leave_opening_entries(id,company_id,employee_id,type_code,year,opening,reason,reference,source_requests_reviewed,created_by,created_by_name,company_revision,request_digest)
  values((record->>'id')::uuid,m.company_id,(record->>'employee_id')::uuid,record->>'type_code',extract(year from today)::integer,preview->'opening',btrim(record->>'reason'),record->>'reference',true,m.user_id,hris_private.leave_actor_name(),rev+n,md5(jsonb_build_array(record,preview->>'fingerprint')::text));
  ids:=ids||jsonb_build_array(record->'id');
 end loop;
 insert into public.leave_opening_batches(id,company_id,created_by,request_digest,opening_ids,company_revision)values(p_batch_id,m.company_id,m.user_id,digest,ids,rev+n);
 update public.leave_dated_state set revision=rev+n where company_id=m.company_id;
 perform hris_private.leave_dated_context();
 return jsonb_build_object('id',p_batch_id,'company_revision',rev+n,'opening_ids',ids);
end;$$;
create function public.post_leave_openings(p_batch_id uuid,p_records jsonb,p_fingerprints jsonb,p_expected_revision bigint)returns jsonb language sql security invoker set search_path=''as $$select hris_private.post_leave_openings(p_batch_id,p_records,p_fingerprints,p_expected_revision);$$;

create function hris_private.validate_dated_policy(p_company uuid,p_policy jsonb,p_previous jsonb)returns void
language plpgsql stable security definer set search_path=''as $$
declare t jsonb;s jsonb;h jsonb;v jsonb;codes text[]:='{}';dates date[]:='{}';week integer[]:='{}';code text;field text;
begin
 if jsonb_typeof(p_policy)is distinct from'object'or octet_length(p_policy::text)>300000 then raise exception 'Enter a valid full policy snapshot.'using errcode='23514';end if;
 if(select array_agg(k order by k)from jsonb_object_keys(p_policy)k)is distinct from array['company_id','holidays','revision','tracking_starts_year','types','workweek']::text[]
 or p_policy->'company_id'is distinct from to_jsonb(p_company)or p_policy->'tracking_starts_year'is distinct from p_previous->'tracking_starts_year'
 or jsonb_typeof(p_policy->'revision')is distinct from'number'or coalesce(p_policy->>'revision','')!~'^[0-9]{1,10}$'
 or jsonb_typeof(p_policy->'workweek')is distinct from'array'or jsonb_typeof(p_policy->'holidays')is distinct from'array'or jsonb_typeof(p_policy->'types')is distinct from'array'then raise exception 'Retain company/tracking identity and use a complete policy snapshot.'using errcode='23514';end if;
 for v in select value from jsonb_array_elements(p_policy->'workweek')loop
  if jsonb_typeof(v)is distinct from'number'or v::text!~'^[1-7]$'or(v::text)::integer=any(week)then raise exception 'Choose unique Monday–Sunday workdays.'using errcode='23514';end if;
  week:=array_append(week,(v::text)::integer);
 end loop;
 if cardinality(week)not between 1 and 7 or jsonb_array_length(p_policy->'holidays')>1000 or jsonb_array_length(p_policy->'types')not between 1 and 30 then raise exception 'Use a workweek, at most 1000 holidays and 1–30 types.'using errcode='23514';end if;
 for h in select value from jsonb_array_elements(p_policy->'holidays')loop
  if jsonb_typeof(h)is distinct from'object'then raise exception 'Invalid holiday.'using errcode='23514';end if;
  if(select array_agg(k order by k)from jsonb_object_keys(h)k)is distinct from array['date','name']::text[]or jsonb_typeof(h->'date')is distinct from'string'or coalesce(h->>'date','')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'
  or jsonb_typeof(h->'name')is distinct from'string'or char_length(btrim(h->>'name'))not between 1 and 100 or(h->>'name')~'[[:cntrl:]]'then raise exception 'Use named, dated holidays.'using errcode='23514';end if;
  if(h->>'date')::date not between date'1900-01-01'and date'2200-12-31'or(h->>'date')::date=any(dates)then raise exception 'Holiday dates must be unique and supported.'using errcode='23514';end if;
  dates:=array_append(dates,(h->>'date')::date);
 end loop;
 for t in select value from jsonb_array_elements(p_policy->'types')loop
  if jsonb_typeof(t)is distinct from'object'then raise exception 'Invalid leave type.'using errcode='23514';end if;
  if(select array_agg(k order by k)from jsonb_object_keys(t)k)is distinct from array['accrual','active','annual_units','approval_chain','carryover_expiry_month','carryover_units','code','name','paid','use_employee_allowance']::text[]then raise exception 'Use complete leave type settings.'using errcode='23514';end if;
  code:=t->>'code';
  if jsonb_typeof(t->'code')is distinct from'string'or code!~'^[a-z0-9_]{1,30}$'or code=any(codes)
  or jsonb_typeof(t->'name')is distinct from'string'or char_length(btrim(t->>'name'))not between 1 and 80 or(t->>'name')~'[[:cntrl:]]'
  or jsonb_typeof(t->'active')is distinct from'boolean'or jsonb_typeof(t->'paid')is distinct from'boolean'or jsonb_typeof(t->'use_employee_allowance')is distinct from'boolean'
  or jsonb_typeof(t->'accrual')is distinct from'string'or t->>'accrual'not in('upfront','monthly','unlimited')
  or((t->>'use_employee_allowance')::boolean and code<>'annual')or jsonb_typeof(t->'approval_chain')is distinct from'array'then raise exception 'Use valid leave type labels, flags and accrual.'using errcode='23514';end if;
  foreach field in array array['annual_units','carryover_units','carryover_expiry_month']loop
   if jsonb_typeof(t->field)is distinct from'number'or coalesce(t->>field,'')!~'^[0-9]{1,3}$'then raise exception 'Use whole half-day allowances and a whole expiry month.'using errcode='23514';end if;
  end loop;
  if(t->>'annual_units')::integer not between 0 and 732 or(t->>'carryover_units')::integer not between 0 and 732 or(t->>'carryover_expiry_month')::integer not between 1 and 12 or jsonb_array_length(t->'approval_chain')not between 1 and 5 then raise exception 'Allowances must be 0–366 days with 1–5 approval steps.'using errcode='23514';end if;
  codes:=array_append(codes,code);
  for s in select value from jsonb_array_elements(t->'approval_chain')loop
   if jsonb_typeof(s)is distinct from'object'then raise exception 'Invalid approval step.'using errcode='23514';end if;
   if(select array_agg(k order by k)from jsonb_object_keys(s)k)is distinct from array['employee_id','kind']::text[]or jsonb_typeof(s->'kind')is distinct from'string'or s->>'kind'not in('reviewer','manager','hr','administrator','person')
   or jsonb_typeof(s->'employee_id')not in('null','string')then raise exception 'Use an explicit approval rule and target.'using errcode='23514';end if;
   if s->>'kind'='person'then
    if not exists(select 1 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id where m.company_id=p_company and m.employee_id=(s->>'employee_id')::uuid and m.active and m.role<>'employee'and e.status='active')then raise exception 'Choose active desktop staff for a specific approver.'using errcode='23514';end if;
   elsif s->'employee_id'is distinct from'null'::jsonb then raise exception 'Only a specific-person rule supplies an employee target.'using errcode='23514';end if;
  end loop;
 end loop;
 if not array['annual','sick','unpaid']::text[]<@codes or exists(select 1 from jsonb_array_elements(p_previous->'types')x where not(x->>'code'=any(codes)))then raise exception 'Retain all earlier type codes; deactivate instead of removing.'using errcode='23514';end if;
exception when invalid_text_representation or datetime_field_overflow or invalid_datetime_format or numeric_value_out_of_range then raise exception 'Enter valid policy dates, targets and half-day quantities.'using errcode='23514';
end;$$;

create function hris_private.save_dated_leave_policy(p_record jsonb)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;today date;on_date date;rid uuid;rev bigint;prior jsonb;policy jsonb;t jsonb;old_t jsonb;
 last_date date;new_policy_rev bigint;saved public.leave_policy_schedules;person record;check_row record;balance jsonb;
begin
 m:=hris_private.dated_member();if m.role<>'administrator'then raise exception 'Only an Administrator schedules policies.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));m:=hris_private.dated_member();
 if m.role<>'administrator'then raise exception 'Your Administrator access changed.'using errcode='42501';end if;
 if jsonb_typeof(p_record)is distinct from'object'or octet_length(p_record::text)>305000 then raise exception 'Use a complete dated policy record.'using errcode='23514';end if;
 if(select array_agg(k order by k)from jsonb_object_keys(p_record)k)is distinct from array['effective_on','expected_revision','id','policy','reason','reference']::text[]
 or jsonb_typeof(p_record->'id')is distinct from'string'or jsonb_typeof(p_record->'effective_on')is distinct from'string'or coalesce(p_record->>'effective_on','')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'
 or jsonb_typeof(p_record->'expected_revision')is distinct from'number'or coalesce(p_record->>'expected_revision','')!~'^[0-9]{1,4}$'
 or jsonb_typeof(p_record->'reason')is distinct from'string'or char_length(btrim(p_record->>'reason'))not between 5 and 500
 or jsonb_typeof(p_record->'reference')is distinct from'string'or char_length(p_record->>'reference')>160 or(p_record->>'reason')~'[[:cntrl:]]'or(p_record->>'reference')~'[[:cntrl:]]'then raise exception 'Use a dated policy, revision and private reason/reference.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;on_date:=(p_record->>'effective_on')::date;
 select *into saved from public.leave_policy_schedules where id=rid;
 if found then
  if saved.company_id is distinct from m.company_id or saved.created_by is distinct from m.user_id or saved.request_digest is distinct from md5(p_record::text)then raise exception 'That policy request identity was already used. Refresh.'using errcode='PT409';end if;
  return jsonb_build_object('id',rid,'company_revision',saved.company_revision);
 end if;
 perform hris_private.require_unused_dated_identity(rid);
 select(now()at time zone timezone)::date into strict today from public.companies where id=m.company_id;
 select coalesce((select revision from public.leave_dated_state where company_id=m.company_id),0)into rev;
 if(p_record->>'expected_revision')::bigint is distinct from rev then raise exception 'Leave setup changed. Refresh before scheduling.'using errcode='PT409';end if;
 if rid='00000000-0000-0000-0000-000000000000'or on_date<today or on_date>make_date(least(extract(year from today)::integer+5,2200),12,31)or rev>=5000 then raise exception 'Choose today or a policy date within five years.'using errcode='23514';end if;
 select s.policy,s.effective_on into prior,last_date from public.leave_policy_schedules s where s.company_id=m.company_id and not exists(select 1 from public.leave_policy_withdrawals w where w.schedule_id=s.id)order by s.effective_on desc limit 1;
 if not found then prior:=hris_private.leave_policy_on(m.company_id,today);end if;
 if last_date is not null and on_date<=last_date then raise exception 'Append after the latest active version or withdraw its future tail first.'using errcode='23514';end if;
 perform hris_private.validate_dated_policy(m.company_id,p_record->'policy',prior);
 for t in select value from jsonb_array_elements(p_record->'policy'->'types')loop
  select value into old_t from jsonb_array_elements(prior->'types')where value->>'code'=t->>'code';
  if(old_t is null and t->>'accrual'<>'unlimited')or(old_t is not null and(t->'annual_units'is distinct from old_t->'annual_units'or t->'use_employee_allowance'is distinct from old_t->'use_employee_allowance'or t->'accrual'is distinct from old_t->'accrual'or t->'carryover_units'is distinct from old_t->'carryover_units'or t->'carryover_expiry_month'is distinct from old_t->'carryover_expiry_month'))then
   if extract(year from on_date)<=extract(year from today)or extract(month from on_date)<>1 or extract(day from on_date)<>1 then raise exception 'Grant, accrual and carry changes start on a future January1. Use audited credits for current-year corrections.'using errcode='23514';end if;
  end if;
 end loop;
 select greatest((prior->>'revision')::bigint,coalesce(max(policy_revision),0))+1 into new_policy_rev from public.leave_policy_schedules where company_id=m.company_id;
 policy:=jsonb_set(p_record->'policy','{revision}',to_jsonb(new_policy_rev));
 if(select count(*)from public.leave_policy_schedules where company_id=m.company_id)>=1000 then raise exception 'Too many retained policy versions.'using errcode='54000';end if;
 insert into public.leave_dated_state(company_id,revision,baseline,enabled_on)select m.company_id,0,to_jsonb(p),today from public.leave_policies p where company_id=m.company_id on conflict do nothing;
 -- Freeze what is known today before a future snapshot changes any term.
 for person in select id from public.employees where company_id=m.company_id and status='active'loop perform hris_private.freeze_leave_year(person.id,today);end loop;
 insert into public.leave_policy_schedules(id,company_id,effective_on,policy,policy_revision,reason,reference,created_by,created_by_name,company_revision,request_digest)
 values(rid,m.company_id,on_date,policy,new_policy_rev,btrim(p_record->>'reason'),p_record->>'reference',m.user_id,hris_private.leave_actor_name(),rev+1,md5(p_record::text));
 -- Existing saved charges/plans stay intact. Reject changed future annual terms
 -- that cannot fund those reservations at their original request starts.
 for check_row in select distinct r.employee_id,r.type_code,r.starts_on from public.leave_requests r where r.company_id=m.company_id and r.status in('pending','approved')and r.starts_on>=greatest(today,on_date)loop
  t:=hris_private.leave_type_on(m.company_id,check_row.type_code,make_date(extract(year from check_row.starts_on)::integer,1,1));
  if t->>'accrual'<>'unlimited'then
   balance:=hris_private.leave_balance_detail(check_row.employee_id,check_row.type_code,extract(year from check_row.starts_on)::integer,check_row.starts_on)->'balance';
   if(balance->>'available_units')::integer<0 then raise exception 'This version would underfund existing saved reservations. Review them first.'using errcode='23514';end if;
  end if;
 end loop;
 update public.leave_dated_state set revision=rev+1 where company_id=m.company_id;
 perform hris_private.leave_dated_context();
 return jsonb_build_object('id',rid,'company_revision',rev+1);
exception when invalid_text_representation or invalid_datetime_format or datetime_field_overflow or numeric_value_out_of_range then raise exception 'Enter valid policy dates, targets and revisions.'using errcode='23514';
end;$$;
create function public.save_dated_leave_policy(p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.save_dated_leave_policy(p_record);$$;

create function hris_private.withdraw_dated_leave_policy(p_record jsonb)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;today date;rid uuid;sid uuid;rev bigint;s public.leave_policy_schedules;receipt public.leave_policy_withdrawals;
begin
 m:=hris_private.dated_member();if m.role<>'administrator'then raise exception 'Only an Administrator withdraws future policies.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));m:=hris_private.dated_member();if m.role<>'administrator'then raise exception 'Your Administrator access changed.'using errcode='42501';end if;
 if jsonb_typeof(p_record)is distinct from'object'or octet_length(p_record::text)>6000 then raise exception 'Use a valid withdrawal record.'using errcode='23514';end if;
 if(select array_agg(k order by k)from jsonb_object_keys(p_record)k)is distinct from array['expected_revision','id','reason','reference','schedule_id']::text[]
 or jsonb_typeof(p_record->'id')is distinct from'string'or jsonb_typeof(p_record->'schedule_id')is distinct from'string'
 or jsonb_typeof(p_record->'expected_revision')is distinct from'number'or coalesce(p_record->>'expected_revision','')!~'^[0-9]{1,4}$'
 or jsonb_typeof(p_record->'reason')is distinct from'string'or char_length(btrim(p_record->>'reason'))not between 5 and 500 or(p_record->>'reason')~'[[:cntrl:]]'
 or jsonb_typeof(p_record->'reference')is distinct from'string'or char_length(p_record->>'reference')>160 or(p_record->>'reference')~'[[:cntrl:]]'then raise exception 'Supply a current revision and private reason/reference.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;sid:=(p_record->>'schedule_id')::uuid;
 select *into receipt from public.leave_policy_withdrawals where id=rid;
 if found then
  if receipt.company_id is distinct from m.company_id or receipt.created_by is distinct from m.user_id or receipt.request_digest is distinct from md5(p_record::text)then raise exception 'That withdrawal request identity was already used.'using errcode='PT409';end if;
  return jsonb_build_object('id',rid,'company_revision',receipt.company_revision);
 end if;
 perform hris_private.require_unused_dated_identity(rid);
 select(now()at time zone timezone)::date into strict today from public.companies where id=m.company_id;
 select revision into rev from public.leave_dated_state where company_id=m.company_id;
 if rev is null or rev is distinct from(p_record->>'expected_revision')::bigint then raise exception 'Leave setup changed. Refresh.'using errcode='PT409';end if;
 select *into s from public.leave_policy_schedules where company_id=m.company_id and id=sid;
 if not found or rid='00000000-0000-0000-0000-000000000000'or s.effective_on<=today or exists(select 1 from public.leave_policy_withdrawals where schedule_id=sid)
 or exists(select 1 from public.leave_policy_schedules n where n.company_id=m.company_id and n.effective_on>s.effective_on and not exists(select 1 from public.leave_policy_withdrawals w where w.schedule_id=n.id))then raise exception 'Only the unactivated future tail can be withdrawn.'using errcode='23514';end if;
 if exists(select 1 from public.leave_requests where company_id=m.company_id and policy_revision=s.policy_revision and status in('pending','approved'))then raise exception 'Active filed requests reference this policy. Retain its saved version.'using errcode='23514';end if;
 if rev>=5000 then raise exception 'Leave setup history exceeds the supported limit.'using errcode='54000';end if;
 insert into public.leave_policy_withdrawals(id,company_id,schedule_id,reason,reference,created_by,created_by_name,company_revision,request_digest)
 values(rid,m.company_id,sid,btrim(p_record->>'reason'),p_record->>'reference',m.user_id,hris_private.leave_actor_name(),rev+1,md5(p_record::text));
 if exists(with dated as(select employee_id,type_code,effective_on,sum(units)units from public.leave_credit_entries where company_id=m.company_id group by employee_id,type_code,effective_on),running as(
  select type_code,effective_on,sum(units)over(partition by employee_id,type_code,extract(year from effective_on)order by effective_on)units from dated)
  select 1 from running where effective_on>=s.effective_on and units<>0 and hris_private.leave_type_on(m.company_id,type_code,effective_on)is null)then raise exception 'Scheduled credits need this leave type. Reverse their dated effect before withdrawing its version.'using errcode='23514';end if;
 -- A request starting before the withdrawn version may still have saved charges
 -- in its date range. Check all affected future reservations, not just references.
 if exists(select 1 from public.leave_requests r where r.company_id=m.company_id and r.status in('pending','approved')and r.starts_on>=s.effective_on
 and(hris_private.leave_type_on(m.company_id,r.type_code,make_date(extract(year from r.starts_on)::integer,1,1))->>'accrual')<>'unlimited'
 and(hris_private.leave_balance_detail(r.employee_id,r.type_code,extract(year from r.starts_on)::integer,r.starts_on)->'balance'->>'available_units')::integer<0)then raise exception 'Withdrawal would underfund existing reservations.'using errcode='23514';end if;
 update public.leave_dated_state set revision=rev+1 where company_id=m.company_id;
 perform hris_private.leave_dated_context();
 return jsonb_build_object('id',rid,'company_revision',rev+1);
exception when invalid_text_representation then raise exception 'Use valid policy identities.'using errcode='23514';
end;$$;
create function public.withdraw_dated_leave_policy(p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.withdraw_dated_leave_policy(p_record);$$;

create function hris_private.leave_dated_context()returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;today date;baseline jsonb;versions jsonb;anchors jsonb;openings jsonb;audit jsonb;withdrawals jsonb;result jsonb;
begin
 m:=hris_private.dated_member();if m.role='employee'then raise exception 'Ordinary Employee accounts use their own portal balance.'using errcode='42501';end if;
 select(now()at time zone timezone)::date into strict today from public.companies where id=m.company_id;
 baseline:=coalesce((select s.baseline from public.leave_dated_state s where company_id=m.company_id),(select to_jsonb(p)from public.leave_policies p where company_id=m.company_id));
 select coalesce(jsonb_agg(jsonb_build_object('id',s.id,'effective_on',s.effective_on,'policy',s.policy,'withdrawn',exists(select 1 from public.leave_policy_withdrawals w where w.schedule_id=s.id))order by s.policy_revision),'[]')into versions from public.leave_policy_schedules s where company_id=m.company_id;
 with scoped as materialized(select a.*from public.leave_entitlement_anchors a join public.employees e on e.company_id=a.company_id and e.id=a.employee_id
 where a.company_id=m.company_id and(m.role in('administrator','hr_generalist')or a.employee_id=m.employee_id or(m.role='manager'and e.status='active'and e.reports_to=m.employee_id)))
 select coalesce(jsonb_agg(jsonb_build_object('employee_id',employee_id,'type_code',type_code,'year',year,'basis',basis,'type_snapshot',type_snapshot,'incoming_carry_units',incoming_carry_units)order by employee_id,type_code,year),'[]')into anchors from scoped;
 with scoped as materialized(select o.*from public.leave_opening_entries o join public.employees e on e.company_id=o.company_id and e.id=o.employee_id
 where o.company_id=m.company_id and(m.role in('administrator','hr_generalist')or o.employee_id=m.employee_id or(m.role='manager'and e.status='active'and e.reports_to=m.employee_id)))
 select coalesce(jsonb_agg(opening order by company_revision),'[]')into openings from scoped;
 audit:='[]';withdrawals:='[]';
 if m.role in('administrator','hr_generalist')then
  select coalesce(jsonb_agg(v order by rev),'[]')into audit from(
   select company_revision rev,jsonb_build_object('id',id,'kind','opening','reason',reason,'reference',reference,'created_by_name',created_by_name,'posted_at',posted_at,'company_revision',company_revision)v from public.leave_opening_entries where company_id=m.company_id
   union all select company_revision,jsonb_build_object('id',id,'kind','policy','reason',reason,'reference',reference,'created_by_name',created_by_name,'posted_at',posted_at,'company_revision',company_revision)from public.leave_policy_schedules where company_id=m.company_id
   union all select company_revision,jsonb_build_object('id',id,'kind','withdrawal','reason',reason,'reference',reference,'created_by_name',created_by_name,'posted_at',posted_at,'company_revision',company_revision)from public.leave_policy_withdrawals where company_id=m.company_id)x;
  select coalesce(jsonb_agg(jsonb_build_object('id',id,'schedule_id',schedule_id)order by company_revision),'[]')into withdrawals from public.leave_policy_withdrawals where company_id=m.company_id;
 end if;
 result:=jsonb_build_object('version','leave-dated-v1','company_id',m.company_id,'user_id',m.user_id,'employee_id',m.employee_id,'role',m.role,'as_of',today,
 'revision',coalesce((select revision from public.leave_dated_state where company_id=m.company_id),0),'enabled',exists(select 1 from public.leave_dated_state where company_id=m.company_id),
 'timeline',jsonb_build_object('baseline',baseline,'versions',versions),'anchors',anchors,'openings',openings,'withdrawals',withdrawals,'audit',audit);
 if jsonb_array_length(anchors)>5000 or jsonb_array_length(openings)>5000 or jsonb_array_length(audit)>5000 or octet_length(result::text)>4000000 then raise exception 'Dated leave exceeds the supported review size; no rows were silently omitted.'using errcode='54000';end if;
 return result;
end;$$;
create function public.leave_dated_context()returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.leave_dated_context();$$;

-- Calendar consumers must resolve the affected date, not today's cached policy.
create function hris_private.leave_holidays_between(p_company uuid,p_first date,p_last date)returns jsonb
language sql stable security definer set search_path=''as $$
 select coalesce(jsonb_agg(h order by h->>'date'),'[]')from generate_series(0,p_last-p_first)g(d)
 cross join lateral jsonb_array_elements(hris_private.leave_policy_on(p_company,p_first+g.d)->'holidays')h where(h->>'date')::date=p_first+g.d;
$$;
create function hris_private.leave_named_approver_count(p_company uuid,p_employee uuid)returns bigint
language sql stable security definer set search_path=''as $$
 with company_day as(select(now()at time zone timezone)::date today from public.companies where id=p_company),snapshots as(
 select hris_private.leave_policy_on(p_company,today)p from company_day
 union all select s.policy from public.leave_policy_schedules s cross join company_day d where s.company_id=p_company and s.effective_on>d.today and not exists(select 1 from public.leave_policy_withdrawals w where w.schedule_id=s.id))
 select count(*)from snapshots cross join lateral jsonb_array_elements(p->'types')t where(t->>'active')::boolean and exists(select 1 from jsonb_array_elements(t->'approval_chain')s where s->>'kind'='person'and s->>'employee_id'=p_employee::text);
$$;
create or replace function hris_private.shift_working(p_company uuid,p_plan jsonb,p_date date)returns boolean
language plpgsql stable security definer set search_path=''as $$begin
 if auth.uid()is null then
  if coalesce(current_setting('role',true),'none')not in('none','postgres','service_role')then raise exception 'Active company access is required.'using errcode='42501';end if;
 elsif not exists(select 1 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id where m.company_id=p_company and m.user_id=auth.uid()and m.active and e.status='active')then raise exception 'Active company access is required.'using errcode='42501';end if;
 return p_plan is not null and exists(select 1 from jsonb_array_elements_text(p_plan->'days')d where d::integer=extract(isodow from p_date)::integer)
 and((p_plan->>'works_on_holidays')::boolean or not exists(select 1 from jsonb_array_elements(hris_private.leave_policy_on(p_company,p_date)->'holidays')h where(h->>'date')::date=p_date));
end;$$;
do $$declare body text;needle text;begin
 body:=pg_get_functiondef('hris_private.payroll_basis(uuid,uuid,date)'::regprocedure);
 needle:='coalesce((select jsonb_agg(h order by h->>''date'')from public.leave_policies l cross join lateral jsonb_array_elements(l.holidays)h where l.company_id=cid and(h->>''date'')::date>=mon and(h->>''date'')::date<(mon+interval ''1month'')::date),''[]''::jsonb)';
 if strpos(body,needle)=0 then raise exception 'Unexpected payroll calendar projection; review migration compatibility.';end if;
 execute replace(body,needle,'hris_private.leave_holidays_between(cid,mon,(mon+interval ''1month - 1day'')::date)');
 body:=pg_get_functiondef('hris_private.exit_readiness(uuid)'::regprocedure);
 needle:='(select count(*)from public.leave_policies p cross join lateral jsonb_array_elements(p.types) t where p.company_id=m.company_id and (t->>''active'')::boolean and exists(select 1 from jsonb_array_elements(t->''approval_chain'') s where s->>''kind''=''person'' and s->>''employee_id''=c.employee_id::text))';
 if strpos(body,needle)=0 then raise exception 'Unexpected exit policy dependency; review migration compatibility.';end if;
 execute replace(body,needle,'hris_private.leave_named_approver_count(m.company_id,c.employee_id)');
 body:=pg_get_functiondef('hris_private.post_leave_credit(jsonb)'::regprocedure);
 needle:='select value into t from jsonb_array_elements(p.types)where value->>''code''=code;';
 if strpos(body,needle)=0 then raise exception 'Unexpected credit type lookup; review migration compatibility.';end if;
 body:=replace(body,needle,'t:=hris_private.leave_type_on(m.company_id,code,on_date);');
 needle:='if jsonb_typeof(p_record)is distinct from''object''';
 if strpos(body,needle)=0 then raise exception 'Unexpected credit validation guard; review migration compatibility.';end if;
 execute replace(body,needle,'if exists(select 1 from public.leave_dated_state where company_id=m.company_id)and coalesce(nullif(current_setting(''request.headers'',true),'''')::jsonb->>''x-hris-leave-dated'','''')<>''1''then raise exception ''Update HRIS Desktop to review openings and effective-date entitlements before adjusting credits.''using errcode=''PT409'';end if; '||needle);
end;$$;

create or replace function hris_private.submit_leave(p_employee uuid,p_type_code text,p_starts date,p_ends date,p_part text,p_reason text)returns uuid
language plpgsql security definer set search_path=''as $$
declare e public.employees;policy jsonb;t jsonb;dates jsonb;units integer;plan jsonb:='[]';rule jsonb;target uuid;rid uuid;dated boolean;known date;
begin
 select *into e from public.employees where id=p_employee;
 if not found or not hris_private.can_record_employee(e.company_id,e.id)then raise exception 'Your role cannot request leave for this employee.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(e.company_id::text,7));
 select *into strict e from public.employees where id=p_employee;
 if not hris_private.can_record_employee(e.company_id,e.id)then raise exception 'Your access changed. Refresh.'using errcode='42501';end if;
 if e.status<>'active'then raise exception 'Archived employees cannot receive new leave.'using errcode='23514';end if;
 if p_starts is null or p_ends is null or p_starts>p_ends or extract(year from p_starts)<>extract(year from p_ends)or p_starts<date'1900-01-01'or p_ends>date'2200-12-31'
 or(e.joined_on is not null and p_starts<e.joined_on)or p_part is null or p_part not in('full','am','pm')or(p_part<>'full'and p_starts<>p_ends)or p_reason is null or char_length(p_reason)>500 then raise exception 'Use valid same-year dates and one date for a half-day, with a note up to 500 characters.'using errcode='23514';end if;
 policy:=hris_private.leave_policy_on(e.company_id,p_starts);
 select value into t from jsonb_array_elements(policy->'types')where value->>'code'=p_type_code and(value->>'active')::boolean;
 if t is null then raise exception 'Choose a leave type active at the request start.'using errcode='23514';end if;
 if t->>'accrual'='monthly'and e.joined_on is null then raise exception 'Set Date joined before requesting accrued leave.'using errcode='23514';end if;
 if exists(select 1 from generate_series(0,p_ends-p_starts)g(d)where coalesce((hris_private.leave_type_on(e.company_id,p_type_code,p_starts+g.d)->>'active')::boolean,false)=false)then raise exception 'Split this request around dates where the leave type is inactive.'using errcode='23514';end if;
 select coalesce(jsonb_agg(jsonb_build_object('date',work_day,'units',case when p_part='full'then 2 else 1 end)order by work_day),'[]')into dates from(
  select p_starts+g.d work_day,hris_private.leave_policy_on(e.company_id,p_starts+g.d)p from generate_series(0,p_ends-p_starts)g(d))calendar
  where exists(select 1 from jsonb_array_elements_text(p->'workweek')w where w::integer=extract(isodow from work_day)::integer)
  and not exists(select 1 from jsonb_array_elements(p->'holidays')h where(h->>'date')::date=work_day);
 select coalesce(sum((d->>'units')::integer),0)into units from jsonb_array_elements(dates)d;
 if units=0 then raise exception 'Choose a scheduled workday outside the holiday calendar.'using errcode='23514';end if;
 if exists(select 1 from public.leave_requests r cross join lateral jsonb_array_elements(r.charged_dates)old_date cross join jsonb_array_elements(dates)new_date where r.employee_id=e.id and r.status in('pending','approved')and old_date->>'date'=new_date->>'date'and(r.day_part='full'or p_part='full'or r.day_part=p_part))then raise exception 'These dates overlap an existing pending or approved request.'using errcode='23P01';end if;
 if t->>'accrual'<>'unlimited'and(hris_private.leave_balance(e.id,t,extract(year from p_starts)::integer,p_starts)->>'available_units')::integer<units then raise exception 'This request exceeds the available leave balance, including pending reservations.'using errcode='23514';end if;
 for rule in select value from jsonb_array_elements(t->'approval_chain')loop
  target:=case when rule->>'kind'='manager'then e.reports_to when rule->>'kind'='person'then(rule->>'employee_id')::uuid else null end;
  if not exists(select 1 from public.company_members m join public.employees a on a.company_id=m.company_id and a.id=m.employee_id where m.company_id=e.company_id and m.active and m.role<>'employee'and a.status='active'and m.employee_id<>e.id and case rule->>'kind'
   when'reviewer'then m.role in('administrator','hr_generalist')or(m.role='manager'and m.employee_id=e.reports_to)when'manager'then m.employee_id=target when'person'then m.employee_id=target when'hr'then m.role in('administrator','hr_generalist')when'administrator'then m.role='administrator'else false end)then raise exception 'An approval step has no eligible staff. Set Reports to or update the chain.'using errcode='23514';end if;
  plan:=plan||jsonb_build_array(jsonb_build_object('kind',rule->>'kind','employee_id',target,'approved_by',null,'approved_at',null));
 end loop;
 insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on,reason,type_code,type_name,day_part,charged_units,charged_dates,policy_revision,approval_steps,requester_name)
 values(e.company_id,e.id,case when p_type_code in('annual','sick','unpaid')then p_type_code else'custom'end,p_starts,p_ends,btrim(p_reason),p_type_code,t->>'name',p_part,units,dates,(policy->>'revision')::bigint,plan,e.full_name)returning id into rid;
 insert into public.leave_events(company_id,request_id,employee_id,actor_id,actor_name,action,revision)values(e.company_id,rid,e.id,auth.uid(),hris_private.leave_actor_name(),'submitted',0);
 return rid;
end;$$;

-- Keep the proven delegation wrapper; change only its underlying funding check.
-- Pending bookings are already reserved. Approval must not require paying their
-- expired, originally reserved carry a second time by excluding that booking.
do $$declare body text;needle text;replacement text;begin
 body:=pg_get_functiondef('hris_private.review_leave_without_delegation_receipt(uuid,bigint,boolean,text)'::regprocedure);
 needle:='select value into t from public.leave_policies p cross join lateral jsonb_array_elements(p.types) where p.company_id=r.company_id and value->>''code''=r.type_code;';
 if strpos(body,needle)=0 then raise exception 'Unexpected original leave type lookup; review migration compatibility.';end if;
 body:=replace(body,needle,'t:=hris_private.leave_type_on(r.company_id,r.type_code,r.starts_on);');
 needle:='(hris_private.leave_balance(r.employee_id,t,extract(year from r.starts_on)::integer,r.starts_on,r.id)->>''available_units'')::integer<r.charged_units';
 replacement:='(case when exists(select 1 from public.leave_dated_state where company_id=r.company_id) then (hris_private.leave_balance(r.employee_id,t,extract(year from r.starts_on)::integer,greatest(r.starts_on,least(make_date(extract(year from r.starts_on)::integer,12,31),(select(now()at time zone timezone)::date from public.companies where id=r.company_id))))->>''available_units'')::integer<0 else (hris_private.leave_balance(r.employee_id,t,extract(year from r.starts_on)::integer,r.starts_on,r.id)->>''available_units'')::integer<r.charged_units end)';
 if strpos(body,needle)=0 then raise exception 'Unexpected original leave funding check; review migration compatibility.';end if;
 execute replace(body,needle,replacement);
end;$$;

create or replace function hris_private.require_credit_preview(p_employee uuid)returns void
language plpgsql security definer set search_path=''as $$
declare company uuid;headers jsonb:=coalesce(nullif(current_setting('request.headers',true),'')::jsonb,'{}');
begin
 select company_id into company from public.employees where id=p_employee;
 if company is null or not hris_private.can_record_employee(company,p_employee)then raise exception 'Your role cannot request leave for this employee.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(company::text,7));
 if not hris_private.can_record_employee(company,p_employee)then raise exception 'Your access changed.'using errcode='42501';end if;
 if exists(select 1 from public.leave_dated_state where company_id=company)then raise exception 'Update HRIS Desktop to use the checked dated-policy and opening-balance preview. The employee portal remains available.'using errcode='PT409';end if;
 if exists(select 1 from public.leave_credit_entries where company_id=company and employee_id=p_employee group by type_code,effective_on having sum(units)<>0)and coalesce(headers->>'x-hris-leave-credits','')<>'1'then raise exception 'Update HRIS Desktop to review dated leave credits. The employee portal remains available.'using errcode='PT409';end if;
end;$$;

create function hris_private.submit_leave_checked(p_employee uuid,p_type_code text,p_starts date,p_ends date,p_part text,p_reason text,p_expected jsonb)returns uuid
language plpgsql security definer set search_path=''as $$
declare e public.employees;policy jsonb;t jsonb;dates jsonb;balance jsonb;expected jsonb;
begin
 select *into e from public.employees where id=p_employee;
 if not found or not hris_private.can_record_employee(e.company_id,e.id)then raise exception 'Your role cannot request leave for this employee.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(e.company_id::text,7));
 if not hris_private.can_record_employee(e.company_id,e.id)then raise exception 'Your access changed.'using errcode='42501';end if;
 if p_starts is null or p_ends is null or p_starts>p_ends or extract(year from p_starts)<>extract(year from p_ends)or p_starts<date'1900-01-01'or p_ends>date'2200-12-31'or p_part is null or p_part not in('full','am','pm')then raise exception 'Use supported same-year request dates and a valid duration.'using errcode='23514';end if;
 if jsonb_typeof(p_expected)is distinct from'object'or octet_length(p_expected::text)>50000 then raise exception 'Review the checked request preview first.'using errcode='23514';end if;
 policy:=hris_private.leave_policy_on(e.company_id,p_starts);t:=hris_private.leave_type_on(e.company_id,p_type_code,p_starts);
 select coalesce(jsonb_agg(jsonb_build_object('date',work_day,'units',case when p_part='full'then 2 else 1 end)order by work_day),'[]')into dates from(
  select p_starts+g.d work_day,hris_private.leave_policy_on(e.company_id,p_starts+g.d)p from generate_series(0,p_ends-p_starts)g(d))calendar
 where exists(select 1 from jsonb_array_elements_text(p->'workweek')w where w::integer=extract(isodow from work_day)::integer)
 and not exists(select 1 from jsonb_array_elements(p->'holidays')h where(h->>'date')::date=work_day);
 balance:=case when t->>'accrual'='unlimited'then null else hris_private.leave_balance(e.id,t,extract(year from p_starts)::integer,p_starts)end;
 expected:=jsonb_build_object('setup_revision',coalesce((select revision from public.leave_dated_state where company_id=e.company_id),0),'credit_revision',coalesce((select revision from public.leave_credit_state where company_id=e.company_id),0),'policy_revision',(policy->>'revision')::bigint,'charged_dates',dates,'balance',balance);
 if p_expected is distinct from expected then raise exception 'The policy, entitlement, credits or reservations changed. Refresh and review the request preview.'using errcode='PT409';end if;
 return hris_private.submit_leave(p_employee,p_type_code,p_starts,p_ends,p_part,p_reason);
end;$$;
create function public.submit_leave_checked(p_employee uuid,p_type_code text,p_starts date,p_ends date,p_part text,p_reason text,p_expected jsonb)returns uuid language sql security invoker set search_path=''as $$select hris_private.submit_leave_checked(p_employee,p_type_code,p_starts,p_ends,p_part,p_reason,p_expected);$$;

create or replace function hris_private.portal_leave_context(p_asof date)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;p jsonb;t jsonb;asof date;options jsonb:='[]';
begin
 m:=hris_private.dated_member();
 select coalesce(p_asof,(now()at time zone timezone)::date)into asof from public.companies where id=m.company_id;
 if asof is null or not isfinite(asof)or asof<date'1900-01-01'or asof>date'2200-12-31'then raise exception 'Use a valid balance date.'using errcode='23514';end if;
 p:=hris_private.leave_policy_on(m.company_id,asof);
 for t in select value from jsonb_array_elements(p->'types')where(value->>'active')::boolean loop
  options:=options||jsonb_build_array(jsonb_build_object('code',t->>'code','name',t->>'name','paid',(t->>'paid')::boolean,'unlimited',t->>'accrual'='unlimited','balance',case when t->>'accrual'='unlimited'then null else hris_private.leave_balance(m.employee_id,t,extract(year from asof)::integer,asof)end));
 end loop;
 return jsonb_build_object('company_id',m.company_id,'employee_id',m.employee_id,'policy_revision',(p->>'revision')::bigint,'asof',asof,'workweek',p->'workweek','holidays',p->'holidays','types',options);
end;$$;
create or replace function hris_private.portal_submit_leave(p_type_code text,p_starts date,p_ends date,p_part text,p_reason text,p_policy_revision bigint)returns uuid
language plpgsql security definer set search_path=''as $$
declare m public.company_members;p jsonb;
begin
 m:=hris_private.dated_member();perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));m:=hris_private.dated_member();
 p:=hris_private.leave_policy_on(m.company_id,p_starts);
 if p_policy_revision is distinct from(p->>'revision')::bigint then raise exception 'The dated policy changed. Refresh the balance for your start date.'using errcode='PT409';end if;
 -- The existing portal renders one calendar. A cross-version request needs a
 -- split rather than silently displaying a calendar different from its charges.
 if exists(select 1 from public.leave_policy_schedules s where s.company_id=m.company_id and s.effective_on>p_starts and s.effective_on<=p_ends and not exists(select 1 from public.leave_policy_withdrawals w where w.schedule_id=s.id))then raise exception 'Split this request at the policy effective date to review each calendar.'using errcode='23514';end if;
 return hris_private.submit_leave(m.employee_id,p_type_code,p_starts,p_ends,p_part,p_reason);
end;$$;

-- Private helpers have no authenticated execution path except explicit protected
-- RPC entry points. No direct table access, user metadata authority or HR audit
-- projection is granted to ordinary Employees/Managers.
revoke all on function hris_private.dated_member(),hris_private.leave_policy_on(uuid,date),hris_private.leave_type_on(uuid,text,date),hris_private.make_leave_basis(uuid,jsonb,date),hris_private.leave_basis_earned(jsonb,date),hris_private.leave_year_bookings(uuid,text,integer,date,uuid),hris_private.leave_balance_detail(uuid,text,integer,date,uuid),hris_private.leave_balance(uuid,jsonb,integer,date,uuid),hris_private.freeze_leave_year(uuid,date),hris_private.freeze_leave_before_employment_change(),hris_private.leave_dated_fingerprint(uuid,uuid,text,date),hris_private.validate_dated_policy(uuid,jsonb,jsonb)from public,anon,authenticated;
revoke all on function hris_private.preview_leave_opening(jsonb),public.preview_leave_opening(jsonb),hris_private.post_leave_opening(jsonb,text),public.post_leave_opening(jsonb,text),hris_private.save_dated_leave_policy(jsonb),public.save_dated_leave_policy(jsonb),hris_private.withdraw_dated_leave_policy(jsonb),public.withdraw_dated_leave_policy(jsonb),hris_private.leave_dated_context(),public.leave_dated_context()from public,anon,authenticated;
revoke all on function hris_private.require_unused_dated_identity(uuid),hris_private.minimum_annual_allowance(uuid,integer),hris_private.save_leave_policy(jsonb,bigint)from public,anon,authenticated;
revoke all on function hris_private.leave_holidays_between(uuid,date,date),hris_private.leave_named_approver_count(uuid,uuid)from public,anon,authenticated;
grant execute on function hris_private.minimum_annual_allowance(uuid,integer),hris_private.save_leave_policy(jsonb,bigint)to authenticated;
revoke all on function hris_private.submit_leave_checked(uuid,text,date,date,text,text,jsonb),public.submit_leave_checked(uuid,text,date,date,text,text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.submit_leave_checked(uuid,text,date,date,text,text,jsonb),public.submit_leave_checked(uuid,text,date,date,text,text,jsonb)to authenticated;
revoke all on function hris_private.preview_leave_openings(jsonb),public.preview_leave_openings(jsonb),hris_private.post_leave_openings(uuid,jsonb,jsonb,bigint),public.post_leave_openings(uuid,jsonb,jsonb,bigint)from public,anon,authenticated;
grant execute on function hris_private.preview_leave_openings(jsonb),public.preview_leave_openings(jsonb),hris_private.post_leave_openings(uuid,jsonb,jsonb,bigint),public.post_leave_openings(uuid,jsonb,jsonb,bigint)to authenticated;
grant execute on function hris_private.preview_leave_opening(jsonb),public.preview_leave_opening(jsonb),hris_private.post_leave_opening(jsonb,text),public.post_leave_opening(jsonb,text),hris_private.save_dated_leave_policy(jsonb),public.save_dated_leave_policy(jsonb),hris_private.withdraw_dated_leave_policy(jsonb),public.withdraw_dated_leave_policy(jsonb),hris_private.leave_dated_context(),public.leave_dated_context()to authenticated;
notify pgrst,'reload schema';
commit;
