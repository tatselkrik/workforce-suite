-- Own proposals and one-step review; only approval changes attendance facts.
begin;
set local search_path=public,extensions,pg_catalog;
create table public.attendance_requests (
 id uuid primary key default gen_random_uuid(),
 company_id uuid not null references public.companies(id) on delete restrict,
 employee_id uuid not null, employee_name text not null,employee_code text not null,
 submitted_by uuid not null references auth.users(id) on delete restrict,
 work_date date not null,timezone text not null,base_record jsonb,planned_shift jsonb,
 proposed_in timestamptz not null,proposed_out timestamptz,
 reason text not null check(char_length(btrim(reason)) between 1 and 500 and reason !~ '[[:cntrl:]]'),
 status text not null default 'pending' check(status in ('pending','approved','rejected','cancelled')),
 revision bigint not null default 0 check(revision>=0),
 applied_attendance_id uuid references public.attendance(id) on delete restrict,
 correction_id uuid references public.attendance_corrections(id) on delete restrict,
 created_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict,
 unique(company_id,id),
 check(proposed_out is null or proposed_out>=proposed_in),
 check(base_record is null or jsonb_typeof(base_record)='object'),
 check(planned_shift is null or jsonb_typeof(planned_shift)='object'),
 check((status='approved')=(applied_attendance_id is not null and correction_id is not null))
);
create unique index attendance_requests_pending_idx on public.attendance_requests(company_id,employee_id,work_date) where status='pending';
create index attendance_requests_employee_idx on public.attendance_requests(company_id,employee_id,created_at desc,id);
create index attendance_requests_submitter_idx on public.attendance_requests(submitted_by);
create index attendance_requests_attendance_idx on public.attendance_requests(applied_attendance_id);
create index attendance_requests_correction_idx on public.attendance_requests(correction_id);
create table public.attendance_request_events (
 id uuid primary key default gen_random_uuid(),company_id uuid not null,request_id uuid not null,employee_id uuid not null,
 actor_id uuid not null references auth.users(id) on delete restrict,actor_name text not null,actor_role text not null check(actor_role in ('administrator','hr_generalist','manager','employee')),
 status text not null check(status in ('pending','approved','rejected','cancelled')),
 reason text not null check(char_length(btrim(reason)) between 1 and 500),
 before_record jsonb,after_record jsonb,created_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,request_id) references public.attendance_requests(company_id,id) on delete restrict,
 foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict
);
create index attendance_request_events_request_idx on public.attendance_request_events(company_id,request_id,created_at,id);
create index attendance_request_events_employee_idx on public.attendance_request_events(company_id,employee_id);
create index attendance_request_events_actor_idx on public.attendance_request_events(actor_id);
alter table public.attendance_requests enable row level security;
alter table public.attendance_request_events enable row level security;
revoke all on public.attendance_requests,public.attendance_request_events from public,anon,authenticated;
grant select on public.attendance_requests,public.attendance_request_events to authenticated;
create policy attendance_requests_read on public.attendance_requests for select to authenticated using(hris_private.can_read_employee(company_id,employee_id));
create policy attendance_request_events_read on public.attendance_request_events for select to authenticated using(hris_private.can_read_employee(company_id,employee_id));

-- Private transaction proof narrowly permits a Manager to use the existing correction
-- handler for the exact approved proposal. No client-settable flags or role changes.
create table hris_private.attendance_request_context(transaction_id bigint primary key,request_id uuid not null references public.attendance_requests(id),actor_id uuid not null);
revoke all on hris_private.attendance_request_context from public,anon,authenticated;
create function hris_private.can_apply_attendance_request(p_change jsonb) returns boolean
language sql volatile security definer set search_path='' as $$
 select exists(select 1 from hris_private.attendance_request_context c join public.attendance_requests r on r.id=c.request_id
 where c.transaction_id=pg_catalog.txid_current() and c.actor_id=(select auth.uid()) and r.status='pending'
 and r.submitted_by<>(select auth.uid()) and hris_private.can_review_employee(r.company_id,r.employee_id)
 and r.employee_id=(p_change->>'employee_id')::uuid and r.work_date=(p_change->>'work_date')::date
 and r.proposed_in=(p_change->>'checked_in_at')::timestamptz and r.proposed_out is not distinct from (p_change->>'checked_out_at')::timestamptz
 and (r.base_record->>'id')::uuid is not distinct from (p_change->>'attendance_id')::uuid
 and (r.base_record->>'revision')::bigint is not distinct from (p_change->>'expected_revision')::bigint);
$$;
revoke all on function hris_private.can_apply_attendance_request(jsonb) from public,anon,authenticated;
alter table public.attendance_corrections drop constraint attendance_corrections_actor_role_check;
alter table public.attendance_corrections add constraint attendance_corrections_actor_role_check check(actor_role in ('administrator','hr_generalist','manager'));
alter table public.attendance_corrections add column request_id uuid references public.attendance_requests(id) on delete restrict;
create unique index attendance_corrections_request_idx on public.attendance_corrections(request_id) where request_id is not null;
create function hris_private.link_attendance_request_correction() returns trigger
language plpgsql security definer set search_path='' as $$
begin
 select c.request_id into new.request_id from hris_private.attendance_request_context c
 where c.transaction_id=pg_catalog.txid_current() and c.actor_id=new.actor_id and c.actor_id=(select auth.uid());
 return new;
end;$$;
revoke all on function hris_private.link_attendance_request_correction() from public,anon,authenticated;
create trigger attendance_correction_request_link before insert on public.attendance_corrections for each row execute function hris_private.link_attendance_request_correction();
do $$declare body text;needle text:='and role in (''administrator'',''hr_generalist'') for share;';begin
 select pg_get_functiondef('hris_private.correct_attendance(jsonb)'::regprocedure) into body;
 if strpos(body,needle)=0 then raise exception 'Unexpected correction handler. Review its permissions before adding request approvals.';end if;
 body:=replace(body,needle,'and (role in (''administrator'',''hr_generalist'') or (role=''manager'' and hris_private.can_apply_attendance_request(p_change))) for share;');
 execute body;
end;$$;

-- Missing attendance approved by a Manager must also pass the existing capture
-- trigger. The exception requires both private proofs and the exact proposal.
create or replace function hris_private.record_attendance_calculation() returns trigger
language plpgsql security definer set search_path='' as $$begin
 if not exists(select 1 from public.company_members m where m.user_id=(select auth.uid()) and m.active and m.company_id=new.company_id
  and (m.role in ('administrator','hr_generalist') or m.employee_id=new.employee_id or
   (m.role='manager' and hris_private.is_attendance_correction(new.id) and exists(
    select 1 from hris_private.attendance_request_context c join public.attendance_requests r on r.id=c.request_id
    where c.transaction_id=pg_catalog.txid_current() and c.actor_id=m.user_id and r.status='pending' and r.submitted_by<>m.user_id
    and r.company_id=new.company_id and r.employee_id=new.employee_id and r.work_date=new.work_date
    and r.proposed_in=new.checked_in_at and r.proposed_out is not distinct from new.checked_out_at
    and hris_private.can_review_employee(r.company_id,r.employee_id))))) then
  raise exception 'Active membership and attendance access are required.' using errcode='42501';end if;
 new.calculation_snapshot:=hris_private.capture_attendance_calculation(new);return new;
end;$$;

create function hris_private.attendance_request(p_request jsonb) returns uuid
language plpgsql security definer set search_path='' as $$
declare m public.company_members;person public.employees;r public.attendance_requests;parent public.attendance;saved public.attendance;
 op text;note text;day date;tin timestamptz;tout timestamptz;zone text;plan jsonb;expected bigint;record_id uuid;event_id uuid;actor_label text;
 server_now timestamptz:=clock_timestamp(); next_status text; correction_note text;
begin
 select * into m from public.company_members where user_id=(select auth.uid()) and active;
 if not found then raise exception 'Active HRIS membership required.' using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));
 select * into m from public.company_members where user_id=(select auth.uid()) and active;
 if not found then raise exception 'Your access changed.' using errcode='42501';end if;
 if jsonb_typeof(p_request) is distinct from 'object' or octet_length(p_request::text)>10000 then raise exception 'Enter valid request details.' using errcode='23514';end if;
 op:=p_request->>'operation';note:=btrim(p_request->>'reason');
 if coalesce(char_length(note),0) not between 1 and 500 or note ~ '[[:cntrl:]]' then raise exception 'Enter a reason of 1–500 characters without control characters.' using errcode='23514';end if;
 select coalesce((select full_name from public.employees where company_id=m.company_id and id=m.employee_id),'Staff') into actor_label;
 select timezone into zone from public.companies where id=m.company_id;
 if op='submit' then
  select * into person from public.employees where company_id=m.company_id and id=m.employee_id and status='active' for share;
  if not found then raise exception 'Only active employees can submit their own attendance requests.' using errcode='42501';end if;
  if p_request->>'expected_timezone' is distinct from zone then raise exception 'Company time zone changed. Refresh and reopen the request.' using errcode='PT409';end if;
  day:=(p_request->>'work_date')::date;tin:=(p_request->>'checked_in_at')::timestamptz;tout:=(p_request->>'checked_out_at')::timestamptz;
  select * into parent from public.attendance where company_id=m.company_id and employee_id=person.id and work_date=day for update;
  if found then
   if (p_request->>'attendance_id')::uuid is distinct from parent.id or (p_request->>'expected_revision')::bigint is distinct from parent.revision then raise exception 'Attendance changed. Refresh and reopen the request.' using errcode='PT409';end if;
   if parent.checked_in_at=tin and parent.checked_out_at is not distinct from tout then raise exception 'Change the check-in or check-out before submitting.' using errcode='23514';end if;
   plan:=parent.planned_shift;
  else
   if p_request->>'attendance_id' is not null or p_request->>'expected_revision' is not null then raise exception 'Attendance changed. Refresh and reopen the request.' using errcode='PT409';end if;
   plan:=hris_private.shift_plan_at(m.company_id,person.id,day);
  end if;
  if day is null or tin is null or not isfinite(day) or not isfinite(tin) or (tout is not null and not isfinite(tout))
   or day<date '1900-01-01' or day>(server_now at time zone zone)::date or tin>server_now or tout>server_now or tout<tin
   or ((tin at time zone zone)::date<>day and not coalesce(hris_private.shift_contains_clock(plan,day,tin),false)) then raise exception 'Use valid past or current times; check-out must follow check-in.' using errcode='23514';end if;
  if person.joined_on>day then raise exception 'Attendance cannot precede the hire date.' using errcode='23514';end if;
  if exists(select 1 from public.attendance a where a.employee_id=person.id and a.id is distinct from parent.id
    and (a.work_date=day or (a.checked_out_at is null and tout is null) or (tstzrange(a.checked_in_at,a.checked_out_at,'[)') && tstzrange(tin,tout,'[)')))) then raise exception 'Proposed times overlap another attendance record or open shift.' using errcode='23514';end if;
  if exists(select 1 from public.attendance_breaks b where b.attendance_id=parent.id and not b.voided and (b.started_at<tin or (tout is not null and (b.ended_at is null or b.ended_at>tout)))) then raise exception 'HR must resolve break times outside the proposed attendance first.' using errcode='23514';end if;
  insert into public.attendance_requests(company_id,employee_id,employee_name,employee_code,submitted_by,work_date,timezone,base_record,planned_shift,proposed_in,proposed_out,reason)
  values(m.company_id,person.id,person.full_name,person.employee_code,m.user_id,day,zone,case when parent.id is null then null else to_jsonb(parent) end,plan,tin,tout,note) returning * into r;
  insert into public.attendance_request_events(company_id,request_id,employee_id,actor_id,actor_name,actor_role,status,reason)
  values(m.company_id,r.id,person.id,m.user_id,actor_label,m.role,'pending',note);
  return r.id;
 elsif op in ('approve','reject','cancel') then
  select * into r from public.attendance_requests where company_id=m.company_id and id=(p_request->>'request_id')::uuid for update;
  if not found then raise exception 'Request unavailable in your company.' using errcode='42501';end if;
  if op='cancel' then
   if r.submitted_by<>m.user_id or m.employee_id is distinct from r.employee_id then raise exception 'Only the requester can cancel this request.' using errcode='42501';end if;
  elsif r.submitted_by=m.user_id or not hris_private.can_review_employee(m.company_id,r.employee_id) then raise exception 'Only another authorized HR reviewer or current direct manager can review this request.' using errcode='42501';end if;
  if r.status<>'pending' or r.revision is distinct from (p_request->>'expected_revision')::bigint then raise exception 'This request changed or was already resolved. Refresh and reopen it.' using errcode='PT409';end if;
  if op='approve' then
   select * into person from public.employees where company_id=m.company_id and id=r.employee_id for share;
   select * into parent from public.attendance where company_id=m.company_id and employee_id=r.employee_id and work_date=r.work_date for update;
   if (case when parent.id is null then null else to_jsonb(parent) end) is distinct from r.base_record or zone<>r.timezone
    or (r.base_record is null and hris_private.shift_plan_at(m.company_id,r.employee_id,r.work_date) is distinct from r.planned_shift) then raise exception 'Attendance, time zone or the missing record schedule changed. Reject or cancel and submit a fresh request.' using errcode='PT409';end if;
   insert into hris_private.attendance_request_context values(pg_catalog.txid_current(),r.id,m.user_id);
   correction_note:='Approved attendance request '||r.id::text;
   record_id:=hris_private.correct_attendance(jsonb_build_object('attendance_id',r.base_record->>'id','expected_revision',r.base_record->'revision','employee_id',r.employee_id,'work_date',r.work_date,'checked_in_at',r.proposed_in,'checked_out_at',r.proposed_out,'reason',correction_note));
   delete from hris_private.attendance_request_context where transaction_id=pg_catalog.txid_current();
   select * into saved from public.attendance where id=record_id;
   select id into strict event_id from public.attendance_corrections where request_id=r.id;
   next_status:='approved';
  else next_status:=case when op='reject' then 'rejected' else 'cancelled' end;
  end if;
  update public.attendance_requests set status=next_status,revision=r.revision+1,applied_attendance_id=record_id,correction_id=event_id where id=r.id;
  insert into public.attendance_request_events(company_id,request_id,employee_id,actor_id,actor_name,actor_role,status,reason,before_record,after_record)
  values(m.company_id,r.id,r.employee_id,m.user_id,actor_label,m.role,next_status,note,case when op='approve' then r.base_record else null end,case when op='approve' then to_jsonb(saved) else null end);
  return r.id;
 else raise exception 'Choose submit, approve, reject or cancel.' using errcode='23514';end if;
exception when unique_violation then raise exception 'A request or attendance record already exists. Refresh and review it.' using errcode='PT409';
 when serialization_failure then raise exception 'Attendance changed. Refresh and reopen the request.' using errcode='PT409';
end;$$;
create function public.attendance_request(p_request jsonb) returns uuid language sql security invoker set search_path='' as $$select hris_private.attendance_request(p_request);$$;
revoke all on function public.attendance_request(jsonb),hris_private.attendance_request(jsonb) from public,anon,authenticated;
grant execute on function public.attendance_request(jsonb),hris_private.attendance_request(jsonb) to authenticated;
commit;
