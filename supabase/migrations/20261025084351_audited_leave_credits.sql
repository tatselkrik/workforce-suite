-- Dated exact-half-day credit deltas. Not opening balances or cash conversion.
begin;
create table public.leave_credit_state(
 company_id uuid primary key references public.companies(id),revision bigint not null default 0 check(revision between 0 and 5000)
);
create table public.leave_credit_entries(
 id uuid primary key check(id<>'00000000-0000-0000-0000-000000000000'),
 company_id uuid not null references public.companies(id),employee_id uuid not null,
 type_code text not null check(type_code~'^[a-z0-9_]{1,30}$'),effective_on date not null check(effective_on between date'1900-01-01'and date'2200-12-31'),
 units integer not null check(units between -732 and 732 and units<>0),
 reverses_id uuid unique references public.leave_credit_entries(id),
 reason text not null check(char_length(btrim(reason))between 5 and 500),reference text not null default''check(char_length(reference)<=160),
 created_by uuid not null references auth.users(id),created_by_name text not null,
 posted_at timestamptz not null default clock_timestamp(),company_revision bigint not null check(company_revision>0),
 request_digest text not null check(length(request_digest)=32),
 unique(company_id,company_revision),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create index leave_credit_employee_date_idx on public.leave_credit_entries(company_id,employee_id,type_code,effective_on);
create index leave_credit_actor_idx on public.leave_credit_entries(created_by);
create trigger leave_credit_immutable before update or delete on public.leave_credit_entries for each row execute function hris_private.leave_history_immutable();
alter table public.leave_credit_state enable row level security;
alter table public.leave_credit_entries enable row level security;
revoke all on public.leave_credit_state,public.leave_credit_entries from public,anon,authenticated;
grant select on public.leave_credit_entries to authenticated;
grant all on public.leave_credit_state,public.leave_credit_entries to service_role;
create policy leave_credit_hr_history on public.leave_credit_entries for select to authenticated using(
 exists(select 1 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id
 where m.company_id=leave_credit_entries.company_id and m.user_id=(select auth.uid())and m.active and m.role in('administrator','hr_generalist')and e.status='active')
);
create function hris_private.leave_credit_units(p_employee uuid,p_code text,p_year integer,p_asof date)returns integer
language sql stable security definer set search_path=''as $$
 select coalesce(sum(units),0)::integer from public.leave_credit_entries
 where company_id=(select company_id from public.employees where id=p_employee)
 and employee_id=p_employee and type_code=p_code and effective_on>=make_date(p_year,1,1)and effective_on<=least(p_asof,make_date(p_year,12,31));
$$;
revoke all on function hris_private.leave_credit_units(uuid,text,integer,date)from public,anon,authenticated;

-- The established balance algorithm is replaced below only to add dated base
-- credits, including prior-year unused-base carry caps. Charges/plans stay intact.

create or replace function hris_private.leave_balance(p_employee uuid,p_type jsonb,p_year integer,p_asof date,p_exclude uuid default null) returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare first_year integer; y integer; carry integer:=0; total integer; early integer; used integer; earned integer; boundary date; applicable integer;
begin
 if p_year not between 1900 and 2200 then raise exception 'Invalid balance year.' using errcode='23514'; end if;
 select least(p_year,greatest(p.tracking_starts_year,coalesce(extract(year from e.joined_on)::integer,p_year))) into strict first_year from public.employees e join public.leave_policies p on p.company_id=e.company_id where e.id=p_employee;
 for y in first_year..p_year loop
  boundary:=(make_date(y,(p_type->>'carryover_expiry_month')::integer,1)+interval '1 month - 1 day')::date;
  select coalesce(sum(r.charged_units),0),coalesce(sum(case when r.status='approved' then r.charged_units else 0 end),0) into total,used
   from public.leave_requests r where r.employee_id=p_employee and r.type_code=p_type->>'code' and extract(year from r.starts_on)=y and r.status in ('pending','approved') and (p_exclude is null or r.id<>p_exclude);
  select coalesce(sum((d->>'units')::integer),0) into early from public.leave_requests r cross join lateral jsonb_array_elements(r.charged_dates) d
   where r.employee_id=p_employee and r.type_code=p_type->>'code' and extract(year from r.starts_on)=y and r.status in ('pending','approved') and (p_exclude is null or r.id<>p_exclude) and (d->>'date')::date<=boundary;
  earned:=hris_private.leave_earned(p_employee,p_type,y,case when y=p_year then p_asof else make_date(y,12,31) end);
  earned:=earned+hris_private.leave_credit_units(p_employee,p_type->>'code',y,case when y=p_year then p_asof else make_date(y,12,31)end);
  if y=p_year then
   applicable:=case when p_asof<=boundary then carry else least(carry,early) end;
   return jsonb_build_object('earned_units',earned,'carryover_units',case when p_asof<=boundary then carry else 0 end,'used_units',used,'reserved_units',total-used,'available_units',earned+applicable-total);
  end if;
  carry:=least(greatest(earned-(total-least(early,carry)),0),(p_type->>'carryover_units')::integer);
 end loop;
 return null;
end;$$;

create function hris_private.leave_credit_context()returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;result jsonb;today date;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active;
 if not found or not exists(select 1 from public.employees where id=m.employee_id and company_id=m.company_id and status='active')then raise exception 'Active employee membership is required.'using errcode='42501';end if;
 select(now()at time zone timezone)::date into strict today from public.companies where id=m.company_id;
 if(select count(*)from public.leave_credit_entries where company_id=m.company_id)>5000 then raise exception 'Credit history exceeds the supported review limit.'using errcode='54000';end if;
 with scoped as materialized(select c.*from public.leave_credit_entries c join public.employees e on e.company_id=c.company_id and e.id=c.employee_id
 where c.company_id=m.company_id and(m.role in('administrator','hr_generalist')or c.employee_id=m.employee_id or(m.role='manager'and e.reports_to=m.employee_id)))
 select jsonb_build_object('version','leave-credits-v1','company_id',m.company_id,'user_id',m.user_id,'employee_id',m.employee_id,'role',m.role,'as_of',today,
 'revision',coalesce((select revision from public.leave_credit_state where company_id=m.company_id),0),
 'entries',coalesce((select jsonb_agg(jsonb_build_object('id',id,'employee_id',employee_id,'type_code',type_code,'effective_on',effective_on,'units',units,'reverses_id',reverses_id)order by company_revision)from scoped),'[]'),
 'audit',case when m.role in('administrator','hr_generalist')then coalesce((select jsonb_agg(jsonb_build_object('id',id,'reason',reason,'reference',reference,'created_by_name',created_by_name,'posted_at',posted_at,'company_revision',company_revision)order by company_revision)from scoped),'[]')else'[]'::jsonb end)into result;
 return result;
end;$$;
create function public.leave_credit_context()returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.leave_credit_context();$$;
revoke all on function hris_private.leave_credit_context(),public.leave_credit_context()from public,anon,authenticated;
grant execute on function hris_private.leave_credit_context(),public.leave_credit_context()to authenticated;

create function hris_private.post_leave_credit(p_record jsonb)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;e public.employees;p public.leave_policies;t jsonb;r public.leave_credit_entries;original public.leave_credit_entries;
 rid uuid;reverse_id uuid;person uuid;code text;on_date date;amount integer;rev bigint;today date;check_day date;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and role='administrator';
 if not found or not exists(select 1 from public.employees where id=m.employee_id and company_id=m.company_id and status='active')then raise exception 'An active Administrator is required.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));
 if not exists(select 1 from public.company_members where company_id=m.company_id and user_id=m.user_id and employee_id=m.employee_id and role='administrator'and active)then raise exception 'Your access changed.'using errcode='42501';end if;
 if jsonb_typeof(p_record)is distinct from'object'or octet_length(p_record::text)>6000
 or exists(select 1 from jsonb_object_keys(p_record)k where k not in('id','employee_id','type_code','effective_on','units','reverses_id','expected_revision','reason','reference'))
 or jsonb_typeof(p_record->'id')is distinct from'string'or jsonb_typeof(p_record->'employee_id')is distinct from'string'
 or jsonb_typeof(p_record->'type_code')is distinct from'string'or jsonb_typeof(p_record->'effective_on')is distinct from'string'
 or jsonb_typeof(p_record->'units')is distinct from'number'or(p_record->>'units')::numeric<>trunc((p_record->>'units')::numeric)
 or jsonb_typeof(p_record->'expected_revision')is distinct from'number'or(p_record->>'expected_revision')::numeric<>trunc((p_record->>'expected_revision')::numeric)
 or(p_record->>'expected_revision')::numeric not between 0 and 5000
 or jsonb_typeof(p_record->'reason')is distinct from'string'or char_length(btrim(p_record->>'reason'))not between 5 and 500
 or jsonb_typeof(p_record->'reference')is distinct from'string'or char_length(p_record->>'reference')>160
 or(p_record->>'reason')~'[[:cntrl:]]'or(p_record->>'reference')~'[[:cntrl:]]'
 or coalesce(p_record->>'effective_on','')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'
 or not(p_record?'reverses_id')or jsonb_typeof(p_record->'reverses_id')not in('null','string')then raise exception 'Enter a valid dated half-day credit with a reason and current revision.'using errcode='23514';end if;
 rid:=(p_record->>'id')::uuid;person:=(p_record->>'employee_id')::uuid;code:=p_record->>'type_code';on_date:=(p_record->>'effective_on')::date;amount:=(p_record->>'units')::integer;reverse_id:=(p_record->>'reverses_id')::uuid;
 select *into r from public.leave_credit_entries where id=rid;
 if found then
  if r.company_id is distinct from m.company_id or r.created_by is distinct from m.user_id or r.request_digest is distinct from md5(p_record::text)then raise exception 'That credit request identity was already used. Refresh.'using errcode='PT409';end if;
  return jsonb_build_object('id',r.id,'company_revision',r.company_revision);
 end if;
 select *into e from public.employees where id=person and company_id=m.company_id;
 if not found then raise exception 'Choose an employee in this company.'using errcode='42501';end if;
 select *into strict p from public.leave_policies where company_id=m.company_id;
 select value into t from jsonb_array_elements(p.types)where value->>'code'=code;
 select(now()at time zone timezone)::date into strict today from public.companies where id=m.company_id;
 if e.status<>'active'or rid='00000000-0000-0000-0000-000000000000'or t is null or t->>'accrual'='unlimited'
 or amount not between -732 and 732 or amount=0 or on_date<today or on_date>make_date(least(extract(year from today)::integer+1,2200),12,31)
 or extract(year from on_date)<p.tracking_starts_year or(e.joined_on is not null and on_date<e.joined_on)then raise exception 'Use an active employee, a finite leave type, and today or a date through next year. Credits are not backdated.'using errcode='23514';end if;
 insert into public.leave_credit_state(company_id)values(m.company_id)on conflict do nothing;
 select revision into strict rev from public.leave_credit_state where company_id=m.company_id for update;
 if rev is distinct from(p_record->>'expected_revision')::bigint then raise exception 'Credit history changed. Refresh before posting.'using errcode='PT409';end if;
 if rev>=5000 then raise exception 'Credit history exceeds the supported review limit.'using errcode='54000';end if;
 if reverse_id is not null then
  select *into original from public.leave_credit_entries where id=reverse_id and company_id=m.company_id;
  if not found or original.employee_id<>person or original.type_code<>code or original.reverses_id is not null or amount<>-original.units
  or extract(year from on_date)<>extract(year from original.effective_on)or on_date<original.effective_on then raise exception 'A reversal must exactly offset one original entry for the same employee, type and year, on or after its date.'using errcode='23514';end if;
  if exists(select 1 from public.leave_credit_entries where reverses_id=reverse_id)then raise exception 'This credit already has a reversal.'using errcode='PT409';end if;
 end if;
 if amount<0 and exists(select 1 from public.leave_requests where employee_id=person and type_code=code and status in('pending','approved')and extract(year from starts_on)>extract(year from on_date))then
 raise exception 'A credit reduction may affect later-year leave. Review those reservations before reducing it.'using errcode='23514';end if;
 insert into public.leave_credit_entries(id,company_id,employee_id,type_code,effective_on,units,reverses_id,reason,reference,created_by,created_by_name,company_revision,request_digest)
 values(rid,m.company_id,person,code,on_date,amount,reverse_id,btrim(p_record->>'reason'),p_record->>'reference',m.user_id,hris_private.leave_actor_name(),rev+1,md5(p_record::text));
 if amount<0 then
  for check_day in select d from(
   select on_date d union select make_date(extract(year from on_date)::integer,12,31)
   union select g::date from generate_series(date_trunc('month',on_date::timestamp),make_date(extract(year from on_date)::integer,12,1)::timestamp,interval'1month')g
   union select(g+interval'1 month - 1 day')::date from generate_series(date_trunc('month',on_date::timestamp),make_date(extract(year from on_date)::integer,12,1)::timestamp,interval'1month')g
   union select effective_on from public.leave_credit_entries where employee_id=person and type_code=code and extract(year from effective_on)=extract(year from on_date)
   union select starts_on from public.leave_requests where employee_id=person and type_code=code and status in('pending','approved')and extract(year from starts_on)=extract(year from on_date)
  )knots where d>=on_date order by d loop
   if(hris_private.leave_balance(person,t,extract(year from on_date)::integer,check_day)->>'available_units')::integer<0 then raise exception 'This reduction would exceed available leave or existing reservations.'using errcode='23514';end if;
  end loop;
 end if;
 update public.leave_credit_state set revision=rev+1 where company_id=m.company_id;
 return jsonb_build_object('id',rid,'company_revision',rev+1);
exception when invalid_text_representation or datetime_field_overflow or invalid_datetime_format or numeric_value_out_of_range then raise exception 'Enter valid dates, whole half-day units and a current revision.'using errcode='23514';
end;$$;
create function public.post_leave_credit(p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.post_leave_credit(p_record);$$;
revoke all on function hris_private.post_leave_credit(jsonb),public.post_leave_credit(jsonb)from public,anon,authenticated;
grant execute on function hris_private.post_leave_credit(jsonb),public.post_leave_credit(jsonb)to authenticated;
-- Older native clients may still read/update, but cannot submit a misleading
-- uncredited preview once an effective/scheduled delta exists. Portal wrappers
-- already obtain the protected server balance and keep their existing contract.
create function hris_private.require_credit_preview(p_employee uuid)returns void
language plpgsql security definer set search_path=''as $$
declare company uuid;
begin
 select company_id into company from public.employees where id=p_employee;
 if company is null or not hris_private.can_record_employee(company,p_employee)then raise exception 'Your role cannot request leave for this employee.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(company::text,7));
 if not hris_private.can_record_employee(company,p_employee)then raise exception 'Your access changed.'using errcode='42501';end if;
 if exists(select 1 from public.leave_credit_entries where company_id=company and employee_id=p_employee group by type_code,effective_on having sum(units)<>0)
 and coalesce(nullif(current_setting('request.headers',true),'')::jsonb->>'x-hris-leave-credits','')<>'1'then
 raise exception 'Update HRIS Desktop to review dated leave credits before submitting. The employee portal remains available.'using errcode='PT409';end if;
end;$$;
revoke all on function hris_private.require_credit_preview(uuid)from public,anon,authenticated;
grant execute on function hris_private.require_credit_preview(uuid)to authenticated;
create or replace function public.submit_leave(p_employee uuid,p_type_code text,p_starts date,p_ends date,p_part text,p_reason text)returns uuid
language plpgsql security invoker set search_path=''as $$begin perform hris_private.require_credit_preview(p_employee);return hris_private.submit_leave(p_employee,p_type_code,p_starts,p_ends,p_part,p_reason);end;$$;
commit;
