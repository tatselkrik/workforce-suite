-- Versioned company policy and immutable per-request charge/approval snapshots.
-- Writes move to revision-checked RPCs; legacy clients remain able to read history.
begin;

create function hris_private.default_leave_types() returns jsonb
language sql immutable security invoker set search_path='' as $$
select '[
 {"code":"annual","name":"Annual leave","active":true,"paid":true,"use_employee_allowance":true,"annual_units":30,"accrual":"upfront","carryover_units":0,"carryover_expiry_month":3,"approval_chain":[{"kind":"reviewer","employee_id":null}]},
 {"code":"sick","name":"Sick leave","active":true,"paid":true,"use_employee_allowance":false,"annual_units":30,"accrual":"unlimited","carryover_units":0,"carryover_expiry_month":3,"approval_chain":[{"kind":"reviewer","employee_id":null}]},
 {"code":"unpaid","name":"Unpaid leave","active":true,"paid":false,"use_employee_allowance":false,"annual_units":30,"accrual":"unlimited","carryover_units":0,"carryover_expiry_month":3,"approval_chain":[{"kind":"reviewer","employee_id":null}]}
]'::jsonb;
$$;
create table public.leave_policies (
 company_id uuid primary key references public.companies(id) on delete restrict,
 revision bigint not null default 0 check(revision>=0),
 workweek integer[] not null default array[1,2,3,4,5],
 holidays jsonb not null default '[]', types jsonb not null default hris_private.default_leave_types(),
 check(jsonb_typeof(holidays)='array' and jsonb_typeof(types)='array')
);
create table public.leave_policy_versions (
 company_id uuid not null references public.companies(id) on delete restrict,
 revision bigint not null, policy jsonb not null,
 actor_id uuid, actor_name text not null, created_at timestamptz not null default now(),
 primary key(company_id,revision)
);
alter table public.leave_requests
 add column type_code text not null default 'annual',
 add column type_name text not null default 'Annual leave',
 add column day_part text not null default 'full' check(day_part in ('full','am','pm')),
 add column charged_units integer not null default 0 check(charged_units>=0 and charged_units<=732),
 add column charged_dates jsonb not null default '[]' check(jsonb_typeof(charged_dates)='array'),
 add column policy_revision bigint not null default 0,
 add column revision bigint not null default 0 check(revision>=0),
 add column approval_steps jsonb not null default '[]' check(jsonb_typeof(approval_steps)='array'),
 add column requester_name text not null default '';
alter table public.leave_requests drop constraint leave_requests_kind_check;
alter table public.leave_requests add constraint leave_requests_kind_check check(kind in ('annual','sick','unpaid','custom'));
alter table public.leave_requests drop constraint leave_requests_status_check;
alter table public.leave_requests add constraint leave_requests_status_check check(status in ('pending','approved','rejected','cancelled'));
alter table public.leave_requests drop constraint leave_requests_employee_id_daterange_excl;
drop trigger leave_validate on public.leave_requests;

insert into public.leave_policies(company_id) select id from public.companies;
insert into public.leave_policy_versions(company_id,revision,policy,actor_name)
 select company_id,revision,to_jsonb(p),'System - initial policy' from public.leave_policies p;
-- Preserve original weekday deductions and final decisions, including archived people.
update public.leave_requests r set type_code=r.kind,type_name=case r.kind when 'annual' then 'Annual leave' when 'sick' then 'Sick leave' else 'Unpaid leave' end,
 charged_units=hris_private.workdays(r.starts_on,r.ends_on)*2,
 charged_dates=(select coalesce(jsonb_agg(jsonb_build_object('date',r.starts_on+g.d,'units',2) order by g.d),'[]')
  from generate_series(0,r.ends_on-r.starts_on) g(d) where extract(isodow from r.starts_on+g.d)<=5),
 requester_name=e.full_name,approval_steps=case when r.status='pending' then '[{"kind":"reviewer","employee_id":null,"approved_by":null,"approved_at":null}]'::jsonb else '[]'::jsonb end
 from public.employees e where e.company_id=r.company_id and e.id=r.employee_id;

create or replace function hris_private.validate_employee() returns trigger
language plpgsql security invoker set search_path='' as $$
declare used_days numeric; company_zone text;
begin
    perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(new.company_id::text,7));
    if new.reports_to is not null then
        if new.reports_to=new.id or not exists(select 1 from public.employees where company_id=new.company_id and id=new.reports_to and status='active') then
            raise exception 'Choose another active employee as the reporting manager.' using errcode='23514';
        end if;
        if exists(with recursive chain as (
            select id,reports_to,array[id] as path from public.employees where id=new.reports_to and company_id=new.company_id
            union all select e.id,e.reports_to,c.path||e.id from public.employees e join chain c on c.reports_to=e.id
                where e.company_id=new.company_id and not e.id=any(c.path)
        ) select 1 from chain where id=new.id) then
            raise exception 'Reporting relationships cannot contain a cycle.' using errcode='23514';
        end if;
    end if;
    if new.status='archived' and exists(select 1 from public.employees where company_id=new.company_id and reports_to=new.id and status='active') then
        raise exception 'Reassign direct reports before archiving this employee.' using errcode='23514';
    end if;
    new.employee_code:=upper(btrim(new.employee_code)); new.full_name:=btrim(new.full_name);
    new.department:=btrim(new.department); new.job_title:=btrim(new.job_title);
    if tg_op='UPDATE' then
        perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(old.id::text,0));
        if new.status='archived' and hris_private.employee_has_account(old.company_id,old.id) then
            raise exception 'An active account is linked to this employee. Ask an administrator to deactivate or unlink it first.' using errcode='23514';
        end if;
        select timezone into company_zone from public.companies where id=old.company_id;
        used_days:=hris_private.minimum_annual_allowance(old.id,extract(year from(now() at time zone company_zone))::integer);
        if new.annual_leave_days<used_days then
            raise exception 'Allowance cannot be less than approved annual leave this year.' using errcode='23514';
        end if;
    end if;
    return new;
end;
$$;

create table public.leave_events (
 id uuid primary key default gen_random_uuid(), company_id uuid not null references public.companies(id) on delete restrict,
 request_id uuid not null references public.leave_requests(id) on delete restrict,
 employee_id uuid not null, actor_id uuid,actor_name text not null,
 action text not null check(action in ('starting_snapshot','submitted','approved_step','rejected','cancelled')),
 note text not null default '' check(char_length(note)<=500), revision bigint not null,
 created_at timestamptz not null default now(),
 foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict
);
create index leave_events_request_idx on public.leave_events(company_id,request_id,created_at,id);
insert into public.leave_events(company_id,request_id,employee_id,actor_name,action,revision)
 select company_id,id,employee_id,'System - earlier decisions not reconstructed','starting_snapshot',revision from public.leave_requests;

create function hris_private.seed_leave_policy() returns trigger
language plpgsql security definer set search_path='' as $$
begin
 insert into public.leave_policies(company_id) values(new.id);
 insert into public.leave_policy_versions(company_id,revision,policy,actor_name)
  select company_id,revision,to_jsonb(p),'System - initial policy' from public.leave_policies p where company_id=new.id;
 return new;
end;$$;
create trigger companies_seed_leave_policy after insert on public.companies for each row execute function hris_private.seed_leave_policy();

-- One company lock also serializes account changes, reporting changes and policies.
create function hris_private.leave_actor_name() returns text
language sql stable security definer set search_path='' as $$
 select coalesce((select e.full_name from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id where m.user_id=(select auth.uid()) and m.active),'HRIS staff');
$$;
create function hris_private.can_act_leave(p_company uuid,p_employee uuid,p_steps jsonb) returns boolean
language plpgsql stable security definer set search_path='' as $$
declare member public.company_members; step jsonb;
begin
 select * into member from public.company_members where company_id=p_company and user_id=(select auth.uid()) and active and role<>'employee';
 if not found or member.employee_id is null or member.employee_id=p_employee then return false; end if;
 select value into step from jsonb_array_elements(p_steps) with ordinality s(value,n) where value->>'approved_by' is null order by n limit 1;
 if step is null then return false; end if;
 return case step->>'kind'
  when 'reviewer' then hris_private.can_review_employee(p_company,p_employee)
  when 'manager' then (step->>'employee_id')::uuid=member.employee_id
  when 'person' then (step->>'employee_id')::uuid=member.employee_id
  when 'hr' then member.role in ('administrator','hr_generalist')
  when 'administrator' then member.role='administrator' else false end;
end;$$;
create function hris_private.leave_earned(p_employee uuid,p_type jsonb,p_year integer,p_asof date) returns integer
language plpgsql stable security definer set search_path='' as $$
declare e public.employees; units integer; months integer;
begin
 select * into strict e from public.employees where id=p_employee;
 if e.joined_on is not null and extract(year from e.joined_on)>p_year then return 0; end if;
 units:=case when (p_type->>'use_employee_allowance')::boolean then e.annual_leave_days*2 else (p_type->>'annual_units')::integer end;
 if p_type->>'accrual'<>'monthly' then return units; end if;
 select count(*) into months from generate_series(1,12) m where (make_date(p_year,m,1)+interval '1 month - 1 day')::date<=p_asof and (e.joined_on is null or e.joined_on<=make_date(p_year,m,1));
 return units*months/12;
end;$$;
create function hris_private.leave_balance(p_employee uuid,p_type jsonb,p_year integer,p_asof date,p_exclude uuid default null) returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare first_year integer; y integer; carry integer:=0; total integer; early integer; used integer; earned integer; boundary date; applicable integer;
begin
 if p_year not between 1900 and 2200 then raise exception 'Invalid balance year.' using errcode='23514'; end if;
 select least(p_year,greatest(1900,coalesce(extract(year from joined_on)::integer,1900))) into strict first_year from public.employees where id=p_employee;
 for y in first_year..p_year loop
  boundary:=(make_date(y,(p_type->>'carryover_expiry_month')::integer,1)+interval '1 month - 1 day')::date;
  select coalesce(sum(r.charged_units),0),coalesce(sum(case when r.status='approved' then r.charged_units else 0 end),0) into total,used
   from public.leave_requests r where r.employee_id=p_employee and r.type_code=p_type->>'code' and extract(year from r.starts_on)=y and r.status in ('pending','approved') and (p_exclude is null or r.id<>p_exclude);
  select coalesce(sum((d->>'units')::integer),0) into early from public.leave_requests r cross join lateral jsonb_array_elements(r.charged_dates) d
   where r.employee_id=p_employee and r.type_code=p_type->>'code' and extract(year from r.starts_on)=y and r.status in ('pending','approved') and (p_exclude is null or r.id<>p_exclude) and (d->>'date')::date<=boundary;
  earned:=hris_private.leave_earned(p_employee,p_type,y,case when y=p_year then p_asof else make_date(y,12,31) end);
  if y=p_year then
   applicable:=case when p_asof<=boundary then carry else least(carry,early) end;
   return jsonb_build_object('earned_units',earned,'carryover_units',case when p_asof<=boundary then carry else 0 end,'used_units',used,'reserved_units',total-used,'available_units',earned+applicable-total);
  end if;
  carry:=least(greatest(earned-(total-least(early,carry)),0),(p_type->>'carryover_units')::integer);
 end loop;
 return null;
end;$$;

create function hris_private.minimum_annual_allowance(p_employee uuid,p_year integer) returns numeric
language plpgsql stable security definer set search_path='' as $$
declare company uuid; t jsonb; carry integer; total integer; early integer; boundary date;
begin
 select company_id into company from public.employees where id=p_employee;
 if coalesce(hris_private.company_role(company),'') not in ('administrator','hr_generalist') or auth.uid() is null then raise exception 'HR access is required.' using errcode='42501'; end if;
 select value into t from public.leave_policies p cross join lateral jsonb_array_elements(p.types) where p.company_id=company and value->>'code'='annual';
 carry:=(hris_private.leave_balance(p_employee,t,p_year,make_date(p_year,1,1))->>'carryover_units')::integer;
 boundary:=(make_date(p_year,(t->>'carryover_expiry_month')::integer,1)+interval '1 month - 1 day')::date;
 select coalesce(sum(charged_units),0) into total from public.leave_requests where employee_id=p_employee and type_code='annual' and status='approved' and extract(year from starts_on)=p_year;
 select coalesce(sum((d->>'units')::integer),0) into early from public.leave_requests r cross join lateral jsonb_array_elements(r.charged_dates) d where r.employee_id=p_employee and r.type_code='annual' and r.status='approved' and extract(year from r.starts_on)=p_year and (d->>'date')::date<=boundary;
 return greatest(total-least(carry,early),0)/2.0;
end;$$;

create function hris_private.can_read_leave_receipt(p_company uuid,p_steps jsonb) returns boolean
language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.company_members m where m.company_id=p_company and m.user_id=(select auth.uid()) and m.active and m.role<>'employee'
  and exists(select 1 from jsonb_array_elements(p_steps) s where s->>'approved_by'=m.user_id::text));
$$;

create function hris_private.save_leave_policy(p_policy jsonb,p_expected_revision bigint) returns bigint
language plpgsql security definer set search_path='' as $$
declare company uuid; old public.leave_policies; t jsonb; s jsonb; h jsonb; codes text[]:='{}'; dates date[]:='{}'; week integer[]; new_revision bigint; code text;
begin
 select company_id into company from public.company_members where user_id=(select auth.uid()) and active and role='administrator';
 if company is null then raise exception 'Only administrators configure leave policies.' using errcode='42501'; end if;
 perform pg_advisory_xact_lock(hashtextextended(company::text,7));
 if hris_private.company_role(company) is distinct from 'administrator' then raise exception 'Your administrator access changed.' using errcode='42501'; end if;
 select * into strict old from public.leave_policies where company_id=company for update;
 if p_expected_revision is distinct from old.revision then raise exception 'Leave policies changed. Refresh before saving.' using errcode='PT409'; end if;
 if jsonb_typeof(p_policy) is distinct from 'object' or octet_length(p_policy::text)>300000
  or jsonb_typeof(p_policy->'workweek') is distinct from 'array' or jsonb_typeof(p_policy->'holidays') is distinct from 'array' or jsonb_typeof(p_policy->'types') is distinct from 'array' then raise exception 'Enter valid leave policy settings.' using errcode='23514'; end if;
 select array_agg(v::integer order by v::integer) into week from jsonb_array_elements_text(p_policy->'workweek') v;
 if coalesce(cardinality(week),0) not between 1 and 7 or exists(select 1 from unnest(week) d where d not between 1 and 7) or cardinality(week)<>(select count(distinct d) from unnest(week) d) then raise exception 'Choose unique workdays from Monday to Sunday.' using errcode='23514'; end if;
 if jsonb_array_length(p_policy->'holidays')>1000 or jsonb_array_length(p_policy->'types') not between 1 and 30 then raise exception 'Too many holidays or leave types.' using errcode='23514'; end if;
 for h in select value from jsonb_array_elements(p_policy->'holidays') loop
  if jsonb_typeof(h->'date') is distinct from 'string' or coalesce(h->>'date','') !~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}$' or jsonb_typeof(h->'name') is distinct from 'string' or coalesce(char_length(btrim(h->>'name')),0) not between 1 and 100 or (h->>'date')::date not between date '1900-01-01' and date '2200-12-31' or (h->>'date')::date=any(dates) then raise exception 'Holidays need unique dates and names up to 100 characters.' using errcode='23514'; end if;
  dates:=array_append(dates,(h->>'date')::date);
 end loop;
 for t in select value from jsonb_array_elements(p_policy->'types') loop
  code:=t->>'code';
  if code is null or code !~ '^[a-z0-9_]{1,30}$' or code=any(codes) or jsonb_typeof(t->'name') is distinct from 'string' or coalesce(char_length(btrim(t->>'name')),0) not between 1 and 80
   or jsonb_typeof(t->'active') is distinct from 'boolean' or jsonb_typeof(t->'paid') is distinct from 'boolean' or jsonb_typeof(t->'use_employee_allowance') is distinct from 'boolean'
   or jsonb_typeof(t->'annual_units') is distinct from 'number' or (t->>'annual_units')::numeric<>trunc((t->>'annual_units')::numeric) or (t->>'annual_units')::integer not between 0 and 732
   or jsonb_typeof(t->'carryover_units') is distinct from 'number' or (t->>'carryover_units')::numeric<>trunc((t->>'carryover_units')::numeric) or (t->>'carryover_units')::integer not between 0 and 732
   or jsonb_typeof(t->'carryover_expiry_month') is distinct from 'number' or (t->>'carryover_expiry_month')::integer not between 1 and 12
   or coalesce(t->>'accrual','') not in ('upfront','monthly','unlimited') or ((t->>'use_employee_allowance')::boolean and code<>'annual')
   or jsonb_typeof(t->'approval_chain') is distinct from 'array' or jsonb_array_length(t->'approval_chain') not between 1 and 5 then raise exception 'Enter valid leave type allowances and approval steps.' using errcode='23514'; end if;
  codes:=array_append(codes,code);
  for s in select value from jsonb_array_elements(t->'approval_chain') loop
   if coalesce(s->>'kind','') not in ('reviewer','manager','hr','administrator','person') then raise exception 'Choose a valid approval step.' using errcode='23514'; end if;
   if s->>'kind'='person' and not exists(select 1 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id where m.company_id=company and m.employee_id=(s->>'employee_id')::uuid and m.active and m.role<>'employee' and e.status='active') then raise exception 'Choose an active desktop staff member as approver.' using errcode='23514'; end if;
  end loop;
 end loop;
 if not array['annual','sick','unpaid']::text[]<@codes or exists(select 1 from jsonb_array_elements(old.types) x where not(x->>'code'=any(codes))) then raise exception 'Retain existing type codes. Deactivate instead of deleting.' using errcode='23514'; end if;
 update public.leave_policies set workweek=week,holidays=p_policy->'holidays',types=p_policy->'types',revision=revision+1 where company_id=company returning revision into new_revision;
 insert into public.leave_policy_versions(company_id,revision,policy,actor_id,actor_name) select company_id,revision,to_jsonb(p),auth.uid(),hris_private.leave_actor_name() from public.leave_policies p where company_id=company;
 return new_revision;
exception when invalid_text_representation or datetime_field_overflow or invalid_datetime_format or numeric_value_out_of_range then raise exception 'Enter valid leave policy values.' using errcode='23514';
end;$$;

create function hris_private.submit_leave(p_employee uuid,p_type_code text,p_starts date,p_ends date,p_part text,p_reason text) returns uuid
language plpgsql security definer set search_path='' as $$
declare e public.employees; policy public.leave_policies; t jsonb; dates jsonb; units integer; plan jsonb:='[]'; rule jsonb; target uuid; new_request_id uuid; member public.company_members;
begin
 select * into e from public.employees where id=p_employee;
 if not found or not hris_private.can_record_employee(e.company_id,e.id) then raise exception 'Your role cannot request leave for this employee.' using errcode='42501'; end if;
 perform pg_advisory_xact_lock(hashtextextended(e.company_id::text,7));
 select * into strict e from public.employees where id=p_employee;
 if not hris_private.can_record_employee(e.company_id,e.id) then raise exception 'Your access changed. Refresh before requesting leave.' using errcode='42501'; end if;
 if e.status<>'active' then raise exception 'Archived employees cannot receive new leave.' using errcode='23514'; end if;
 if p_starts is null or p_ends is null or p_starts>p_ends or extract(year from p_starts)<>extract(year from p_ends) or p_starts<date '1900-01-01' or p_ends>date '2200-12-31' or (e.joined_on is not null and p_starts<e.joined_on) or p_part is null or p_part not in ('full','am','pm') or (p_part<>'full' and p_starts<>p_ends) or p_reason is null or char_length(p_reason)>500 then raise exception 'Use valid dates in one year and one date for a half-day. Keep the note to 500 characters.' using errcode='23514'; end if;
 select * into strict policy from public.leave_policies where company_id=e.company_id;
 select value into t from jsonb_array_elements(policy.types) where value->>'code'=p_type_code and (value->>'active')::boolean;
 if t is null then raise exception 'Choose an active leave type.' using errcode='23514'; end if;
 select coalesce(jsonb_agg(jsonb_build_object('date',p_starts+g.d,'units',case when p_part='full' then 2 else 1 end) order by g.d),'[]') into dates
  from generate_series(0,p_ends-p_starts) g(d) where extract(isodow from p_starts+g.d)::integer=any(policy.workweek) and not exists(select 1 from jsonb_array_elements(policy.holidays) h where (h->>'date')::date=p_starts+g.d);
 select coalesce(sum((d->>'units')::integer),0) into units from jsonb_array_elements(dates) d;
 if units=0 then raise exception 'Choose a scheduled workday outside the holiday calendar.' using errcode='23514'; end if;
 if exists(select 1 from public.leave_requests r cross join lateral jsonb_array_elements(r.charged_dates) old_date cross join jsonb_array_elements(dates) new_date where r.employee_id=e.id and r.status in ('pending','approved') and old_date->>'date'=new_date->>'date' and (r.day_part='full' or p_part='full' or r.day_part=p_part)) then raise exception 'These dates overlap an existing pending or approved request.' using errcode='23P01'; end if;
 if t->>'accrual'<>'unlimited' and (hris_private.leave_balance(e.id,t,extract(year from p_starts)::integer,p_starts)->>'available_units')::integer<units then raise exception 'This request exceeds the available leave balance, including pending reservations.' using errcode='23514'; end if;
 for rule in select value from jsonb_array_elements(t->'approval_chain') loop
  target:=case when rule->>'kind'='manager' then e.reports_to when rule->>'kind'='person' then (rule->>'employee_id')::uuid else null end;
  if not exists(select 1 from public.company_members m join public.employees approver on approver.company_id=m.company_id and approver.id=m.employee_id where m.company_id=e.company_id and m.active and m.role<>'employee' and approver.status='active' and m.employee_id<>e.id and case rule->>'kind'
    when 'reviewer' then m.role in ('administrator','hr_generalist') or (m.role='manager' and m.employee_id=e.reports_to)
    when 'manager' then m.employee_id=target when 'person' then m.employee_id=target when 'hr' then m.role in ('administrator','hr_generalist') when 'administrator' then m.role='administrator' else false end) then raise exception 'An approval step has no eligible staff member. Set Reports to or update this leave type approval chain.' using errcode='23514'; end if;
  plan:=plan||jsonb_build_array(jsonb_build_object('kind',rule->>'kind','employee_id',target,'approved_by',null,'approved_at',null));
 end loop;
 insert into public.leave_requests(company_id,employee_id,kind,starts_on,ends_on,reason,type_code,type_name,day_part,charged_units,charged_dates,policy_revision,approval_steps,requester_name)
 values(e.company_id,e.id,case when p_type_code in ('annual','sick','unpaid') then p_type_code else 'custom' end,p_starts,p_ends,btrim(p_reason),p_type_code,t->>'name',p_part,units,dates,policy.revision,plan,e.full_name) returning leave_requests.id into new_request_id;
 insert into public.leave_events(company_id,request_id,employee_id,actor_id,actor_name,action,revision) values(e.company_id,new_request_id,e.id,auth.uid(),hris_private.leave_actor_name(),'submitted',0);
 return new_request_id;
end;$$;

create function hris_private.review_leave(p_id uuid,p_revision bigint,p_approve boolean,p_note text) returns uuid
language plpgsql security definer set search_path='' as $$
declare r public.leave_requests; plan jsonb; step_index integer; t jsonb;
begin
 select * into r from public.leave_requests where id=p_id;
 if not found or not hris_private.can_act_leave(r.company_id,r.employee_id,r.approval_steps) then raise exception 'This request is awaiting another reviewer, or is your own request.' using errcode='42501'; end if;
 perform pg_advisory_xact_lock(hashtextextended(r.company_id::text,7));
 select * into strict r from public.leave_requests where id=p_id for update;
 if r.revision is distinct from p_revision then raise exception 'This request changed. Refresh before reviewing.' using errcode='PT409'; end if;
 if r.status<>'pending' or not hris_private.can_act_leave(r.company_id,r.employee_id,r.approval_steps) then raise exception 'This request is no longer awaiting your review.' using errcode='23514'; end if;
 if p_approve is null or p_note is null or char_length(p_note)>500 then raise exception 'Keep the review note to 500 characters.' using errcode='23514'; end if;
 if p_approve then
  if not exists(select 1 from public.employees where id=r.employee_id and status='active') then raise exception 'Archived employees cannot have new leave approved.' using errcode='23514'; end if;
  select value into t from public.leave_policies p cross join lateral jsonb_array_elements(p.types) where p.company_id=r.company_id and value->>'code'=r.type_code;
  if t->>'accrual'<>'unlimited' and (hris_private.leave_balance(r.employee_id,t,extract(year from r.starts_on)::integer,r.starts_on,r.id)->>'available_units')::integer<r.charged_units then raise exception 'This request exceeds the available leave balance.' using errcode='23514'; end if;
  select (n-1)::integer into step_index from jsonb_array_elements(r.approval_steps) with ordinality a(v,n) where v->>'approved_by' is null order by n limit 1;
  plan:=jsonb_set(r.approval_steps,array[step_index::text,'approved_by'],to_jsonb(auth.uid()));
  plan:=jsonb_set(plan,array[step_index::text,'approved_at'],to_jsonb(now()));
  update public.leave_requests set approval_steps=plan,status=case when exists(select 1 from jsonb_array_elements(plan) s where s->>'approved_by' is null) then 'pending' else 'approved' end,revision=revision+1 where id=r.id;
 else update public.leave_requests set status='rejected',revision=revision+1 where id=r.id; end if;
 insert into public.leave_events(company_id,request_id,employee_id,actor_id,actor_name,action,note,revision) values(r.company_id,r.id,r.employee_id,auth.uid(),hris_private.leave_actor_name(),case when p_approve then 'approved_step' else 'rejected' end,btrim(p_note),r.revision+1);
 return r.id;
end;$$;
create function hris_private.cancel_leave(p_id uuid,p_revision bigint,p_reason text) returns uuid
language plpgsql security definer set search_path='' as $$
declare r public.leave_requests; member public.company_members; today date;
begin
 select * into r from public.leave_requests where id=p_id;
 select * into member from public.company_members where company_id=r.company_id and user_id=(select auth.uid()) and active;
 if not found or not(member.role in ('administrator','hr_generalist') or member.employee_id=r.employee_id) then raise exception 'Your role cannot cancel this leave request.' using errcode='42501'; end if;
 perform pg_advisory_xact_lock(hashtextextended(r.company_id::text,7));
 select * into strict r from public.leave_requests where id=p_id for update;
 if r.revision is distinct from p_revision then raise exception 'This request changed. Refresh before cancelling.' using errcode='PT409'; end if;
 select * into member from public.company_members where company_id=r.company_id and user_id=(select auth.uid()) and active;
 if not found or not(member.role in ('administrator','hr_generalist') or member.employee_id=r.employee_id) then raise exception 'Your cancellation access changed.' using errcode='42501'; end if;
 select (now() at time zone timezone)::date into today from public.companies where id=r.company_id;
 if r.status not in ('pending','approved') or (member.role not in ('administrator','hr_generalist') and r.status='approved' and r.starts_on<today) then raise exception 'Only HR can cancel approved leave after it begins. Only active requests can be cancelled.' using errcode='23514'; end if;
 if coalesce(char_length(btrim(p_reason)),0) not between 1 and 500 then raise exception 'Enter a cancellation reason up to 500 characters.' using errcode='23514'; end if;
 update public.leave_requests set status='cancelled',revision=revision+1 where id=r.id;
 insert into public.leave_events(company_id,request_id,employee_id,actor_id,actor_name,action,note,revision) values(r.company_id,r.id,r.employee_id,auth.uid(),hris_private.leave_actor_name(),'cancelled',btrim(p_reason),r.revision+1);
 return r.id;
end;$$;

-- All exposed functions remain invokers. Privileged entry points recheck protected
-- membership; internal accounting helpers cannot be called by client roles.
create function public.save_leave_policy(p_policy jsonb,p_expected_revision bigint) returns bigint language sql security invoker set search_path='' as $$select hris_private.save_leave_policy(p_policy,p_expected_revision);$$;
create function public.submit_leave(p_employee uuid,p_type_code text,p_starts date,p_ends date,p_part text,p_reason text) returns uuid language sql security invoker set search_path='' as $$select hris_private.submit_leave(p_employee,p_type_code,p_starts,p_ends,p_part,p_reason);$$;
create function public.review_leave(p_id uuid,p_revision bigint,p_approve boolean,p_note text) returns uuid language sql security invoker set search_path='' as $$select hris_private.review_leave(p_id,p_revision,p_approve,p_note);$$;
create function public.cancel_leave(p_id uuid,p_revision bigint,p_reason text) returns uuid language sql security invoker set search_path='' as $$select hris_private.cancel_leave(p_id,p_revision,p_reason);$$;

create function hris_private.leave_history_immutable() returns trigger language plpgsql security invoker set search_path='' as $$begin raise exception 'Leave audit history is immutable.' using errcode='23514';end;$$;
create trigger leave_events_immutable before update or delete on public.leave_events for each row execute function hris_private.leave_history_immutable();
create trigger leave_versions_immutable before update or delete on public.leave_policy_versions for each row execute function hris_private.leave_history_immutable();
create function hris_private.leave_request_immutable() returns trigger language plpgsql security invoker set search_path='' as $$
begin
 if tg_op='DELETE' then raise exception 'Retain leave requests. Cancel instead of deleting.' using errcode='23514'; end if;
 if (to_jsonb(new)-'status'-'approval_steps'-'revision') is distinct from (to_jsonb(old)-'status'-'approval_steps'-'revision') then raise exception 'Filed leave dates and charges are immutable. Cancel and submit a replacement.' using errcode='23514'; end if;
 return new;
end;$$;
create trigger leave_request_immutable before update or delete on public.leave_requests for each row execute function hris_private.leave_request_immutable();

alter table public.leave_policies enable row level security;
alter table public.leave_policy_versions enable row level security;
alter table public.leave_events enable row level security;
revoke all on public.leave_policies,public.leave_policy_versions,public.leave_events from public,anon,authenticated;
grant select on public.leave_policies,public.leave_policy_versions,public.leave_events to authenticated;
create policy leave_policy_read on public.leave_policies for select to authenticated using(hris_private.company_role(company_id) is not null);
create policy leave_versions_read on public.leave_policy_versions for select to authenticated using(hris_private.company_role(company_id)='administrator');
create policy leave_events_read on public.leave_events for select to authenticated using(exists(select 1 from public.leave_requests r where r.id=request_id and r.company_id=leave_events.company_id));
alter policy leave_read on public.leave_requests using(hris_private.can_read_employee(company_id,employee_id) or hris_private.can_read_leave_receipt(company_id,approval_steps) or (status='pending' and hris_private.can_act_leave(company_id,employee_id,approval_steps)));
-- The old column-level grants are revoked as well as table-level privileges.
revoke insert(company_id,employee_id,kind,starts_on,ends_on,reason),update(status) on public.leave_requests from authenticated;
revoke insert,update,delete on public.leave_requests from public,anon,authenticated;

revoke all on function hris_private.default_leave_types(),hris_private.seed_leave_policy(),hris_private.leave_actor_name(),hris_private.leave_earned(uuid,jsonb,integer,date),hris_private.leave_balance(uuid,jsonb,integer,date,uuid),hris_private.leave_history_immutable(),hris_private.leave_request_immutable(),hris_private.can_act_leave(uuid,uuid,jsonb) from public,anon,authenticated;
revoke all on function hris_private.minimum_annual_allowance(uuid,integer),hris_private.can_read_leave_receipt(uuid,jsonb) from public,anon,authenticated;
grant execute on function hris_private.minimum_annual_allowance(uuid,integer),hris_private.can_read_leave_receipt(uuid,jsonb) to authenticated;
grant execute on function hris_private.can_act_leave(uuid,uuid,jsonb) to authenticated;
revoke all on function hris_private.save_leave_policy(jsonb,bigint),hris_private.submit_leave(uuid,text,date,date,text,text),hris_private.review_leave(uuid,bigint,boolean,text),hris_private.cancel_leave(uuid,bigint,text),public.save_leave_policy(jsonb,bigint),public.submit_leave(uuid,text,date,date,text,text),public.review_leave(uuid,bigint,boolean,text),public.cancel_leave(uuid,bigint,text) from public,anon,authenticated;
grant execute on function hris_private.save_leave_policy(jsonb,bigint),hris_private.submit_leave(uuid,text,date,date,text,text),hris_private.review_leave(uuid,bigint,boolean,text),hris_private.cancel_leave(uuid,bigint,text),public.save_leave_policy(jsonb,bigint),public.submit_leave(uuid,text,date,date,text,text),public.review_leave(uuid,bigint,boolean,text),public.cancel_leave(uuid,bigint,text) to authenticated;
commit;
