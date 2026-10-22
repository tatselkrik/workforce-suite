-- Company-owned shift templates; immutable dated assignments and audit snapshots.
begin;
create table public.shift_templates (
 id uuid primary key default gen_random_uuid(),
 company_id uuid not null references public.companies(id) on delete restrict,
 name text not null check(char_length(btrim(name)) between 1 and 80 and name !~ '[[:cntrl:]]'),
 days integer[] not null,
 start_minute integer not null check(start_minute between 0 and 1439),
 end_minute integer not null check(end_minute between 0 and 1439 and end_minute<>start_minute),
 works_on_holidays boolean not null default false,
 active boolean not null default true,
 revision bigint not null default 0 check(revision>=0),
 unique(company_id,id)
);
create unique index shift_templates_name_idx on public.shift_templates(company_id,lower(name));
create table public.shift_template_versions (
 id uuid primary key default gen_random_uuid(),
 company_id uuid not null references public.companies(id) on delete restrict,
 template_id uuid not null,
 revision bigint not null,
 snapshot jsonb not null,
 actor_id uuid not null references auth.users(id) on delete restrict,
 actor_name text not null,
 created_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,template_id) references public.shift_templates(company_id,id) on delete restrict,
 unique(template_id,revision)
);
create index shift_versions_company_idx on public.shift_template_versions(company_id,created_at,id);
create index shift_versions_actor_idx on public.shift_template_versions(actor_id);
create table public.shift_assignments (
 id uuid primary key default gen_random_uuid(),
 company_id uuid not null references public.companies(id) on delete restrict,
 employee_id uuid not null,
 effective_on date not null check(effective_on between date '1900-01-01' and date '2200-12-31'),
 sequence bigint not null check(sequence>0),
 plan jsonb check(plan is null or jsonb_typeof(plan)='object'),
 reason text not null check(char_length(btrim(reason)) between 1 and 500 and reason !~ '[[:cntrl:]]'),
 actor_id uuid not null references auth.users(id) on delete restrict,
 actor_name text not null,
 created_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict,
 unique(employee_id,sequence)
);
create index shift_assignments_lookup_idx on public.shift_assignments(company_id,employee_id,effective_on desc,sequence desc);
create index shift_assignments_actor_idx on public.shift_assignments(actor_id);
alter table public.shift_templates enable row level security;
alter table public.shift_template_versions enable row level security;
alter table public.shift_assignments enable row level security;
revoke all on public.shift_templates,public.shift_template_versions,public.shift_assignments from public,anon,authenticated;
grant select on public.shift_templates,public.shift_template_versions,public.shift_assignments to authenticated;
create policy shift_templates_read on public.shift_templates for select to authenticated
 using(hris_private.company_role(company_id) in ('administrator','hr_generalist','manager'));
create policy shift_versions_read on public.shift_template_versions for select to authenticated
 using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));
create policy shift_assignments_read on public.shift_assignments for select to authenticated
 using(hris_private.can_read_employee(company_id,employee_id));

create function hris_private.shift_plan_at(p_company uuid,p_employee uuid,p_date date) returns jsonb
language sql stable security invoker set search_path='' as $$
 select plan from public.shift_assignments where company_id=p_company and employee_id=p_employee and effective_on<=p_date
 order by effective_on desc,sequence desc limit 1;
$$;
create function hris_private.shift_working(p_company uuid,p_plan jsonb,p_date date) returns boolean
language sql stable security invoker set search_path='' as $$
 select p_plan is not null and exists(select 1 from jsonb_array_elements_text(p_plan->'days') d where d::integer=extract(isodow from p_date)::integer)
 and ((p_plan->>'works_on_holidays')::boolean or not exists(select 1 from public.leave_policies l cross join lateral jsonb_array_elements(l.holidays) h where l.company_id=p_company and (h->>'date')::date=p_date));
$$;
create function hris_private.shift_contains_clock(p_plan jsonb,p_date date,p_at timestamptz) returns boolean
language sql stable security invoker set search_path='' as $$
 select p_plan is not null
 and (p_at at time zone (p_plan->>'timezone')) >= p_date::timestamp+make_interval(mins=>(p_plan->>'start_minute')::integer)
 and (p_at at time zone (p_plan->>'timezone')) < p_date::timestamp+make_interval(mins=>(p_plan->>'end_minute')::integer)
  +case when (p_plan->>'end_minute')::integer<(p_plan->>'start_minute')::integer then interval '1 day' else interval '0' end;
$$;
create function hris_private.shift_contains(p_company uuid,p_plan jsonb,p_date date,p_at timestamptz) returns boolean
language sql stable security invoker set search_path='' as $$
 select hris_private.shift_working(p_company,p_plan,p_date) and hris_private.shift_contains_clock(p_plan,p_date,p_at);
$$;
create function hris_private.attendance_work_date_at(p_company uuid,p_employee uuid,p_at timestamptz) returns date
language plpgsql stable security invoker set search_path='' as $$
declare today date; day date; plan jsonb;
begin
 select (p_at at time zone timezone)::date into today from public.companies where id=p_company;
 for day in select today+v from generate_series(-1,1) v loop
  plan:=hris_private.shift_plan_at(p_company,p_employee,day);
  if coalesce(hris_private.shift_contains(p_company,plan,day,p_at),false) then return day; end if;
 end loop;
 return today;
end;
$$;

create function hris_private.save_shift_template(p_shift jsonb) returns uuid
language plpgsql security definer set search_path='' as $$
declare member public.company_members; old public.shift_templates; saved public.shift_templates; target uuid; week integer[]; note text; start_m integer; end_m integer; expected bigint;
begin
 select * into member from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist');
 if not found then raise exception 'Only active HR staff and Administrators can manage shifts.' using errcode='42501'; end if;
 perform pg_advisory_xact_lock(hashtextextended(member.company_id::text,7));
 if hris_private.company_role(member.company_id) not in ('administrator','hr_generalist') or hris_private.company_role(member.company_id) is null then raise exception 'Your HR access changed.' using errcode='42501'; end if;
 if jsonb_typeof(p_shift) is distinct from 'object' or octet_length(p_shift::text)>5000 or jsonb_typeof(p_shift->'days') is distinct from 'array'
 or jsonb_typeof(p_shift->'name') is distinct from 'string' or jsonb_typeof(p_shift->'works_on_holidays') is distinct from 'boolean'
 or jsonb_typeof(p_shift->'active') is distinct from 'boolean'
 or coalesce(p_shift->>'start_minute','') !~ '^[0-9]{1,4}$' or coalesce(p_shift->>'end_minute','') !~ '^[0-9]{1,4}$' then raise exception 'Enter valid shift settings.' using errcode='23514'; end if;
 if exists(select 1 from jsonb_array_elements_text(p_shift->'days') d where d !~ '^[1-7]$') then raise exception 'Choose workdays from Monday to Sunday.' using errcode='23514'; end if;
 select array_agg(d::integer order by d::integer) into week from jsonb_array_elements_text(p_shift->'days') d;
 if coalesce(cardinality(week),0) not between 1 and 7 or cardinality(week)<>(select count(distinct d) from unnest(week) d) then raise exception 'Choose at least one unique workday.' using errcode='23514'; end if;
 note:=btrim(p_shift->>'name'); start_m:=(p_shift->>'start_minute')::integer; end_m:=(p_shift->>'end_minute')::integer;
 if coalesce(char_length(note),0) not between 1 and 80 or note ~ '[[:cntrl:]]' or start_m not between 0 and 1439 or end_m not between 0 and 1439 or start_m=end_m then raise exception 'Use a valid name and different start/end times shorter than 24 hours.' using errcode='23514'; end if;
 target:=(p_shift->>'id')::uuid; expected:=(p_shift->>'expected_revision')::bigint;
 if target is null then
  if expected is not null then raise exception 'Refresh before adding a shift.' using errcode='PT409'; end if;
  insert into public.shift_templates(company_id,name,days,start_minute,end_minute,works_on_holidays,active)
   values(member.company_id,note,week,start_m,end_m,(p_shift->>'works_on_holidays')::boolean,(p_shift->>'active')::boolean) returning * into saved;
 else
  select * into old from public.shift_templates where company_id=member.company_id and id=target for update;
  if not found then raise exception 'Shift unavailable in this company.' using errcode='42501'; end if;
  if expected is distinct from old.revision then raise exception 'This shift changed. Refresh and reopen it before saving.' using errcode='PT409'; end if;
  update public.shift_templates set name=note,days=week,start_minute=start_m,end_minute=end_m,works_on_holidays=(p_shift->>'works_on_holidays')::boolean,active=(p_shift->>'active')::boolean,revision=revision+1 where id=target returning * into saved;
 end if;
 insert into public.shift_template_versions(company_id,template_id,revision,snapshot,actor_id,actor_name)
 select member.company_id,saved.id,saved.revision,to_jsonb(saved),member.user_id,coalesce(e.full_name,'HR staff') from public.employees e where e.id=member.employee_id;
 return saved.id;
exception when unique_violation then raise exception 'This shift name already exists, including inactive shifts.' using errcode='23505';
end;
$$;
create function hris_private.assign_shift(p_assignment jsonb) returns uuid
language plpgsql security definer set search_path='' as $$
declare member public.company_members; person public.employees; template public.shift_templates; target uuid; tid uuid; day date; note text; seq bigint; expected bigint; plan jsonb; saved uuid; boundary date; left_plan jsonb; right_plan jsonb; tz text;
begin
 select * into member from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist');
 if not found then raise exception 'Only active HR staff and Administrators can assign shifts.' using errcode='42501'; end if;
 perform pg_advisory_xact_lock(hashtextextended(member.company_id::text,7));
 if hris_private.company_role(member.company_id) not in ('administrator','hr_generalist') or hris_private.company_role(member.company_id) is null then raise exception 'Your HR access changed.' using errcode='42501'; end if;
 if jsonb_typeof(p_assignment) is distinct from 'object' or octet_length(p_assignment::text)>5000 then raise exception 'Enter valid assignment details.' using errcode='23514'; end if;
 target:=(p_assignment->>'employee_id')::uuid; tid:=(p_assignment->>'template_id')::uuid; day:=(p_assignment->>'effective_on')::date;
 note:=btrim(p_assignment->>'reason'); expected:=(p_assignment->>'expected_sequence')::bigint;
 if day is null or day not between date '1900-01-01' and date '2200-12-31' or coalesce(char_length(note),0) not between 1 and 500 or note ~ '[[:cntrl:]]' then raise exception 'Enter a valid effective date and a reason of 1–500 characters.' using errcode='23514'; end if;
 select * into person from public.employees where company_id=member.company_id and id=target;
 if not found then raise exception 'Choose an employee in this company.' using errcode='42501'; end if;
 if person.status<>'active' or day<person.joined_on then raise exception 'Use an active employee and an effective date on or after their hire date.' using errcode='23514'; end if;
 select coalesce(max(sequence),0) into seq from public.shift_assignments where company_id=member.company_id and employee_id=target;
 if expected is distinct from seq then raise exception 'This employee schedule changed. Refresh and reopen it.' using errcode='PT409'; end if;
 select timezone into tz from public.companies where id=member.company_id;
 if tz is distinct from p_assignment->>'expected_timezone' then raise exception 'Company time zone changed. Refresh and reopen the assignment.' using errcode='PT409'; end if;
 if tid is not null then
  select * into template from public.shift_templates where company_id=member.company_id and id=tid and active;
  if not found then raise exception 'Choose an active shift in this company.' using errcode='23514'; end if;
  if (p_assignment->>'expected_template_revision')::bigint is distinct from template.revision then raise exception 'The selected shift changed. Refresh and reopen the assignment.' using errcode='PT409'; end if;
  plan:=jsonb_build_object('template_id',template.id,'template_revision',template.revision,'name',template.name,'days',template.days,'start_minute',template.start_minute,'end_minute',template.end_minute,'works_on_holidays',template.works_on_holidays,'timezone',tz);
 end if;
 insert into public.shift_assignments(company_id,employee_id,effective_on,sequence,plan,reason,actor_id,actor_name)
 select member.company_id,target,day,seq+1,plan,note,member.user_id,coalesce(e.full_name,'HR staff') from public.employees e where e.id=member.employee_id returning id into saved;
 for boundary in select distinct effective_on from public.shift_assignments where company_id=member.company_id and employee_id=target loop
  left_plan:=hris_private.shift_plan_at(member.company_id,target,boundary-1); right_plan:=hris_private.shift_plan_at(member.company_id,target,boundary);
  if left_plan is not null and right_plan is not null and (
   (((boundary-1)::timestamp+make_interval(mins=>(left_plan->>'end_minute')::integer)+case when (left_plan->>'end_minute')::integer<(left_plan->>'start_minute')::integer then interval '1 day' else interval '0' end) at time zone (left_plan->>'timezone')) at time zone (left_plan->>'timezone')
    <> ((boundary-1)::timestamp+make_interval(mins=>(left_plan->>'end_minute')::integer)+case when (left_plan->>'end_minute')::integer<(left_plan->>'start_minute')::integer then interval '1 day' else interval '0' end)
   or ((boundary::timestamp+make_interval(mins=>(right_plan->>'start_minute')::integer)) at time zone (right_plan->>'timezone')) at time zone (right_plan->>'timezone')
    <> (boundary::timestamp+make_interval(mins=>(right_plan->>'start_minute')::integer))) then
    raise exception 'A schedule boundary falls in a daylight-saving gap. Choose another date or time.' using errcode='23514';
  end if;
  if exists(select 1 from jsonb_array_elements_text(left_plan->'days') d where d::integer=extract(isodow from boundary-1)::integer)
   and exists(select 1 from jsonb_array_elements_text(right_plan->'days') d where d::integer=extract(isodow from boundary)::integer)
   and ((boundary-1)::timestamp+make_interval(mins=>(left_plan->>'end_minute')::integer)+case when (left_plan->>'end_minute')::integer<(left_plan->>'start_minute')::integer then interval '1 day' else interval '0' end) at time zone (left_plan->>'timezone')
    > (boundary::timestamp+make_interval(mins=>(right_plan->>'start_minute')::integer)) at time zone (right_plan->>'timezone') then
    raise exception 'This change overlaps the previous overnight shift. Choose a later start or another effective date.' using errcode='23P01';
  end if;
 end loop;
 return saved;
end;
$$;
create function public.save_shift_template(p_shift jsonb) returns uuid
 language sql security invoker set search_path='' as $$ select hris_private.save_shift_template(p_shift); $$;
create function public.assign_shift(p_assignment jsonb) returns uuid
 language sql security invoker set search_path='' as $$ select hris_private.assign_shift(p_assignment); $$;
revoke all on function public.save_shift_template(jsonb),public.assign_shift(jsonb),hris_private.save_shift_template(jsonb),hris_private.assign_shift(jsonb),hris_private.shift_plan_at(uuid,uuid,date),hris_private.shift_working(uuid,jsonb,date),hris_private.shift_contains(uuid,jsonb,date,timestamptz),hris_private.shift_contains_clock(jsonb,date,timestamptz),hris_private.attendance_work_date_at(uuid,uuid,timestamptz) from public,anon,authenticated;
grant execute on function public.save_shift_template(jsonb),public.assign_shift(jsonb),hris_private.save_shift_template(jsonb),hris_private.assign_shift(jsonb),hris_private.shift_plan_at(uuid,uuid,date),hris_private.shift_working(uuid,jsonb,date),hris_private.shift_contains(uuid,jsonb,date,timestamptz),hris_private.shift_contains_clock(jsonb,date,timestamptz),hris_private.attendance_work_date_at(uuid,uuid,timestamptz) to authenticated;

-- No retrofit of historical punches. New records retain the plan used at creation.
alter table public.attendance add column planned_shift jsonb;
create or replace function hris_private.record_attendance() returns trigger
language plpgsql security invoker set search_path='' as $$
declare correction boolean:=hris_private.is_attendance_correction(new.id);
begin
 if tg_op='UPDATE' and (new.id<>old.id or new.company_id<>old.company_id or new.employee_id<>old.employee_id or new.work_date<>old.work_date) then raise exception 'Attendance identity and work date cannot be changed.' using errcode='23514'; end if;
 if not correction then
  if tg_op='INSERT' then
   if not exists(select 1 from public.employees where company_id=new.company_id and id=new.employee_id and status='active') then raise exception 'Archived employees cannot receive new records.' using errcode='23514'; end if;
   new.checked_in_at:=clock_timestamp();
   new.work_date:=hris_private.attendance_work_date_at(new.company_id,new.employee_id,new.checked_in_at);
   if exists(select 1 from public.employees where id=new.employee_id and joined_on>new.work_date) then raise exception 'Attendance cannot precede the hire date.' using errcode='23514'; end if;
   new.checked_out_at:=null;
  else
   if old.checked_out_at is not null or new.checked_out_at is null then raise exception 'Only an open attendance record can be checked out.' using errcode='23514'; end if;
   new.checked_in_at:=old.checked_in_at; new.checked_out_at:=clock_timestamp();
  end if;
 end if;
 new.planned_shift:=case when tg_op='INSERT' then hris_private.shift_plan_at(new.company_id,new.employee_id,new.work_date) else old.planned_shift end;
 new.revision:=case when tg_op='INSERT' then 0 else old.revision+1 end;
 return new;
end;
$$;
-- Extend the existing guarded correction handler without altering its identity/revision/audit protections.
do $$
declare body text;
begin
 select pg_get_functiondef('hris_private.correct_attendance(jsonb)'::regprocedure) into body;
 if strpos(body,'(time_in at time zone company_zone)::date<>day')=0 then raise exception 'Unexpected correction handler; review before applying shift support.'; end if;
 body:=replace(body,'(time_in at time zone company_zone)::date<>day',
  '((time_in at time zone company_zone)::date<>day and not coalesce(hris_private.shift_contains_clock(case when editing then (select planned_shift from public.attendance where id=record_id and company_id=member.company_id and employee_id=target and work_date=day) else hris_private.shift_plan_at(member.company_id,target,day) end,day,time_in),false))');
 execute body;
end;
$$;
commit;
