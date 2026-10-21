-- Do not infer grants before tracking or an unknown hire date.
begin;
alter table public.leave_policies add column tracking_starts_year integer not null default extract(year from now())::integer check(tracking_starts_year between 1900 and 2200);
update public.leave_policies p set tracking_starts_year=least(extract(year from c.created_at at time zone c.timezone)::integer,coalesce((select min(extract(year from r.starts_on))::integer from public.leave_requests r where r.company_id=p.company_id),2200)) from public.companies c where c.id=p.company_id;
-- Migration metadata is explicit. Retain version-0 history; later saves include the
-- tracking year in the full policy snapshot rather than rewriting earlier events.
create or replace function hris_private.leave_earned(p_employee uuid,p_type jsonb,p_year integer,p_asof date) returns integer
language plpgsql stable security definer set search_path='' as $$
declare e public.employees; units integer; months integer;
begin
 select * into strict e from public.employees where id=p_employee;
 if e.joined_on is not null and extract(year from e.joined_on)>p_year then return 0; end if;
 units:=case when (p_type->>'use_employee_allowance')::boolean then e.annual_leave_days*2 else (p_type->>'annual_units')::integer end;
 if p_type->>'accrual'<>'monthly' then return units; end if;
 if e.joined_on is null then return 0; end if;
 select count(*) into months from generate_series(1,12) m where (make_date(p_year,m,1)+interval '1 month - 1 day')::date<=p_asof and (e.joined_on is null or e.joined_on<=make_date(p_year,m,1));
 return units*months/12;
end;$$;
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
  if y=p_year then
   applicable:=case when p_asof<=boundary then carry else least(carry,early) end;
   return jsonb_build_object('earned_units',earned,'carryover_units',case when p_asof<=boundary then carry else 0 end,'used_units',used,'reserved_units',total-used,'available_units',earned+applicable-total);
  end if;
  carry:=least(greatest(earned-(total-least(early,carry)),0),(p_type->>'carryover_units')::integer);
 end loop;
 return null;
end;$$;
create or replace function hris_private.submit_leave(p_employee uuid,p_type_code text,p_starts date,p_ends date,p_part text,p_reason text) returns uuid
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
 if t->>'accrual'='monthly' and e.joined_on is null then raise exception 'Set the employee Date joined before requesting accrued leave.' using errcode='23514'; end if;
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
create or replace function hris_private.seed_leave_policy() returns trigger
language plpgsql security definer set search_path='' as $$
begin
 insert into public.leave_policies(company_id,tracking_starts_year) values(new.id,extract(year from new.created_at at time zone new.timezone)::integer);
 insert into public.leave_policy_versions(company_id,revision,policy,actor_name)
  select company_id,revision,to_jsonb(p),'System - initial policy' from public.leave_policies p where company_id=new.id;
 return new;
end;$$;
commit;
