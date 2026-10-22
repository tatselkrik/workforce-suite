-- Saved attendance calculation policies. Existing attendance is not backfilled.
begin;
create table public.attendance_calculation_policies (
 company_id uuid primary key references public.companies(id) on delete restrict,
 revision bigint not null default 0 check(revision>=0),
 late_grace_minutes integer not null default 0 check(late_grace_minutes between 0 and 120),
 undertime_grace_minutes integer not null default 0 check(undertime_grace_minutes between 0 and 120),
 overtime_min_minutes integer not null default 0 check(overtime_min_minutes between 0 and 240),
 overtime_round_minutes integer not null default 1 check(overtime_round_minutes in (1,5,10,15,30,60)),
 include_pre_shift_overtime boolean not null default false
);
insert into public.attendance_calculation_policies(company_id) select id from public.companies;
create function hris_private.initialize_attendance_calculation_policy() returns trigger
language plpgsql security definer set search_path='' as $$begin
 insert into public.attendance_calculation_policies(company_id) values(new.id);return new;
end;$$;
revoke all on function hris_private.initialize_attendance_calculation_policy() from public,anon,authenticated;
create trigger company_attendance_calculation_policy after insert on public.companies for each row execute function hris_private.initialize_attendance_calculation_policy();

create table public.attendance_calculation_policy_events (
 id uuid primary key default gen_random_uuid(),company_id uuid not null references public.companies(id) on delete restrict,
 attendance_id uuid check(attendance_id is null),employee_id uuid check(employee_id is null),
 actor_id uuid not null references auth.users(id) on delete restrict,actor_name text not null,
 actor_role text not null check(actor_role='administrator'),
 reason text not null check(char_length(btrim(reason)) between 1 and 500 and reason !~ '[[:cntrl:]]'),
 before_snapshot jsonb not null,after_snapshot jsonb not null,created_at timestamptz not null default clock_timestamp()
);
create index attendance_calculation_policy_events_company_idx on public.attendance_calculation_policy_events(company_id,created_at,id);
create index attendance_calculation_policy_events_actor_idx on public.attendance_calculation_policy_events(actor_id);
create table public.attendance_calculation_events (
 id uuid primary key default gen_random_uuid(),company_id uuid not null references public.companies(id) on delete restrict,
 attendance_id uuid not null,employee_id uuid not null,
 foreign key(company_id,attendance_id,employee_id) references public.attendance(company_id,id,employee_id) on delete restrict,
 actor_id uuid not null references auth.users(id) on delete restrict,actor_name text not null,
 actor_role text not null check(actor_role in ('administrator','hr_generalist')),
 reason text not null check(char_length(btrim(reason)) between 1 and 500 and reason !~ '[[:cntrl:]]'),
 before_snapshot jsonb,after_snapshot jsonb not null,created_at timestamptz not null default clock_timestamp()
);
create index attendance_calculation_events_attendance_idx on public.attendance_calculation_events(company_id,attendance_id,employee_id,created_at,id);
create index attendance_calculation_events_actor_idx on public.attendance_calculation_events(actor_id);
alter table public.attendance_calculation_policies enable row level security;
alter table public.attendance_calculation_policy_events enable row level security;
alter table public.attendance_calculation_events enable row level security;
revoke all on public.attendance_calculation_policies,public.attendance_calculation_policy_events,public.attendance_calculation_events from public,anon,authenticated;
grant select on public.attendance_calculation_policies,public.attendance_calculation_policy_events,public.attendance_calculation_events to authenticated;
create policy attendance_calculation_policies_read on public.attendance_calculation_policies for select to authenticated using(hris_private.company_role(company_id) in ('administrator','hr_generalist','manager'));
create policy attendance_calculation_policy_events_read on public.attendance_calculation_policy_events for select to authenticated using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));
create policy attendance_calculation_events_read on public.attendance_calculation_events for select to authenticated using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));

alter table public.attendance add column calculation_snapshot jsonb
 check(calculation_snapshot is null or (jsonb_typeof(calculation_snapshot)='object' and calculation_snapshot->>'version'='1'));
create function hris_private.capture_attendance_calculation(p_record public.attendance) returns jsonb
language plpgsql stable security invoker set search_path='' as $$
declare policy public.attendance_calculation_policies; plan jsonb:=p_record.planned_shift; tz text; local_start timestamp; local_end timestamp;
 starts timestamptz; ends timestamptz; state text:='no_shift'; working boolean:=false;
begin
 select * into strict policy from public.attendance_calculation_policies where company_id=p_record.company_id;
 select timezone into tz from public.companies where id=p_record.company_id;
 if plan is not null then
  tz:=plan->>'timezone';working:=coalesce(hris_private.shift_working(p_record.company_id,plan,p_record.work_date),false);
  local_start:=p_record.work_date::timestamp+make_interval(mins=>(plan->>'start_minute')::integer);
  local_end:=p_record.work_date::timestamp+make_interval(mins=>(plan->>'end_minute')::integer)+case when (plan->>'end_minute')::integer<(plan->>'start_minute')::integer then interval '1 day' else interval '0' end;
  starts:=local_start at time zone tz;ends:=local_end at time zone tz;state:='ready';
  if starts at time zone tz<>local_start or ends at time zone tz<>local_end then state:='dst_gap';starts:=null;ends:=null;
  elsif ends<=starts then state:='invalid_window';starts:=null;ends:=null;end if;
 end if;
 return jsonb_build_object('version',1,'policy',to_jsonb(policy),'timezone',tz,'scheduled_workday',working,'planned_start',starts,'planned_end',ends,'window_status',state);
end;$$;
revoke all on function hris_private.capture_attendance_calculation(public.attendance) from public,anon,authenticated;
create function hris_private.record_attendance_calculation() returns trigger
language plpgsql security definer set search_path='' as $$begin
 if not exists(select 1 from public.company_members m where m.user_id=(select auth.uid()) and m.active and m.company_id=new.company_id and (m.role in ('administrator','hr_generalist') or m.employee_id=new.employee_id)) then raise exception 'Active membership and attendance access are required.' using errcode='42501';end if;
 -- Capture one MVCC-consistent view without reversing the correction lock order.
 new.calculation_snapshot:=hris_private.capture_attendance_calculation(new);return new;
end;$$;
revoke all on function hris_private.record_attendance_calculation() from public,anon,authenticated;
-- BEFORE triggers run by name; the existing attendance_record guard captures the shift first.
create trigger zz_attendance_calculation before insert on public.attendance for each row execute function hris_private.record_attendance_calculation();

create function hris_private.save_attendance_calculation_policy(p_policy jsonb) returns uuid
language plpgsql security definer set search_path='' as $$
declare member public.company_members; old public.attendance_calculation_policies; saved public.attendance_calculation_policies; note text; field text; actor_label text;
begin
 select * into member from public.company_members where user_id=(select auth.uid()) and active and role='administrator';
 if not found then raise exception 'Only active Administrators can change calculation rules.' using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(member.company_id::text,7));
 if hris_private.company_role(member.company_id) is distinct from 'administrator' then raise exception 'Your Administrator access changed.' using errcode='42501';end if;
 if jsonb_typeof(p_policy) is distinct from 'object' or octet_length(p_policy::text)>10000 then raise exception 'Enter valid calculation rules.' using errcode='23514';end if;
 note:=btrim(p_policy->>'reason');if coalesce(char_length(note),0) not between 1 and 500 or note ~ '[[:cntrl:]]' then raise exception 'Enter a reason of 1–500 characters.' using errcode='23514';end if;
 foreach field in array array['late_grace_minutes','undertime_grace_minutes','overtime_min_minutes','overtime_round_minutes'] loop
  if jsonb_typeof(p_policy->field) is distinct from 'number' or coalesce(p_policy->>field,'') !~ '^[0-9]{1,3}$' then raise exception 'Calculation allowances and thresholds use whole minutes.' using errcode='23514';end if;
 end loop;
 if jsonb_typeof(p_policy->'include_pre_shift_overtime') is distinct from 'boolean' then raise exception 'Choose whether pre-shift time is counted.' using errcode='23514';end if;
 select * into strict old from public.attendance_calculation_policies where company_id=member.company_id for update;
 if (p_policy->>'expected_revision')::bigint is distinct from old.revision then raise exception 'Calculation rules changed. Refresh and reopen the editor.' using errcode='PT409';end if;
 saved:=old;saved.revision:=old.revision+1;saved.late_grace_minutes:=(p_policy->>'late_grace_minutes')::integer;saved.undertime_grace_minutes:=(p_policy->>'undertime_grace_minutes')::integer;
 saved.overtime_min_minutes:=(p_policy->>'overtime_min_minutes')::integer;saved.overtime_round_minutes:=(p_policy->>'overtime_round_minutes')::integer;saved.include_pre_shift_overtime:=(p_policy->>'include_pre_shift_overtime')::boolean;
 if (to_jsonb(saved)-'revision')=(to_jsonb(old)-'revision') then raise exception 'Change a calculation rule before saving.' using errcode='23514';end if;
 update public.attendance_calculation_policies set revision=saved.revision,late_grace_minutes=saved.late_grace_minutes,undertime_grace_minutes=saved.undertime_grace_minutes,overtime_min_minutes=saved.overtime_min_minutes,overtime_round_minutes=saved.overtime_round_minutes,include_pre_shift_overtime=saved.include_pre_shift_overtime where company_id=member.company_id;
 select coalesce((select full_name from public.employees where company_id=member.company_id and id=member.employee_id),'Administrator') into actor_label;
 insert into public.attendance_calculation_policy_events(company_id,actor_id,actor_name,actor_role,reason,before_snapshot,after_snapshot) values(member.company_id,member.user_id,actor_label,'administrator',note,to_jsonb(old),to_jsonb(saved));
 return member.company_id;
end;$$;
create function public.save_attendance_calculation_policy(p_policy jsonb) returns uuid
language sql security invoker set search_path='' as $$select hris_private.save_attendance_calculation_policy(p_policy);$$;
revoke all on function public.save_attendance_calculation_policy(jsonb),hris_private.save_attendance_calculation_policy(jsonb) from public,anon,authenticated;
grant execute on function public.save_attendance_calculation_policy(jsonb),hris_private.save_attendance_calculation_policy(jsonb) to authenticated;

create function hris_private.apply_attendance_calculation_policy(p_change jsonb) returns uuid
language plpgsql security definer set search_path='' as $$
declare member public.company_members; parent public.attendance; saved jsonb; note text; actor_label text;
begin
 select * into member from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist');
 if not found then raise exception 'Only active HR staff and Administrators can apply calculation rules.' using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(member.company_id::text,7));
 if hris_private.company_role(member.company_id) is null or hris_private.company_role(member.company_id) not in ('administrator','hr_generalist') then raise exception 'Your HR access changed.' using errcode='42501';end if;
 if jsonb_typeof(p_change) is distinct from 'object' or octet_length(p_change::text)>10000 then raise exception 'Enter valid calculation update details.' using errcode='23514';end if;
 note:=btrim(p_change->>'reason');if coalesce(char_length(note),0) not between 1 and 500 or note ~ '[[:cntrl:]]' then raise exception 'Enter a reason of 1–500 characters.' using errcode='23514';end if;
 select * into parent from public.attendance where company_id=member.company_id and id=(p_change->>'attendance_id')::uuid for update;
 if not found then raise exception 'Attendance unavailable in your company.' using errcode='42501';end if;
 if parent.revision is distinct from (p_change->>'expected_revision')::bigint or (select revision from public.attendance_calculation_policies where company_id=member.company_id) is distinct from (p_change->>'expected_policy_revision')::bigint then raise exception 'Attendance or calculation rules changed. Refresh and reopen this record.' using errcode='PT409';end if;
 if parent.planned_shift is null then raise exception 'This attendance has no saved shift. Earlier hours cannot be reconstructed.' using errcode='23514';end if;
 saved:=hris_private.capture_attendance_calculation(parent);
 if saved is not distinct from parent.calculation_snapshot then raise exception 'This record already has the current rules and calendar decision.' using errcode='23514';end if;
 -- Reuse the protected transaction context, never a client-settable bypass flag.
 insert into hris_private.attendance_correction_context values(pg_catalog.txid_current(),parent.id,member.user_id);
 update public.attendance set calculation_snapshot=saved where id=parent.id;
 delete from hris_private.attendance_correction_context where transaction_id=pg_catalog.txid_current();
 select coalesce((select full_name from public.employees where company_id=member.company_id and id=member.employee_id),'HR staff') into actor_label;
 insert into public.attendance_calculation_events(company_id,attendance_id,employee_id,actor_id,actor_name,actor_role,reason,before_snapshot,after_snapshot) values(member.company_id,parent.id,parent.employee_id,member.user_id,actor_label,hris_private.company_role(member.company_id),note,parent.calculation_snapshot,saved);
 return parent.id;
end;$$;
create function public.apply_attendance_calculation_policy(p_change jsonb) returns uuid
language sql security invoker set search_path='' as $$select hris_private.apply_attendance_calculation_policy(p_change);$$;
revoke all on function public.apply_attendance_calculation_policy(jsonb),hris_private.apply_attendance_calculation_policy(jsonb) from public,anon,authenticated;
grant execute on function public.apply_attendance_calculation_policy(jsonb),hris_private.apply_attendance_calculation_policy(jsonb) to authenticated;
commit;
