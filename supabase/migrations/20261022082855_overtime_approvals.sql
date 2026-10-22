-- Overtime authorization is retained separately from attendance facts and payroll.
begin;
set local search_path=public,extensions,pg_catalog;
create table public.overtime_requests (
 id uuid primary key default gen_random_uuid(),company_id uuid not null references public.companies(id) on delete restrict,
 attendance_id uuid not null references public.attendance(id) on delete restrict,employee_id uuid not null,
 employee_name text not null,employee_code text not null,submitted_by uuid not null references auth.users(id) on delete restrict,
 work_date date not null,requested_minutes integer not null check(requested_minutes>0),basis jsonb not null check(jsonb_typeof(basis)='object'),
 status text not null default 'pending' check(status in ('pending','approved','rejected','cancelled','revoked')),
 approved_minutes integer,reason text not null check(char_length(btrim(reason)) between 1 and 500 and reason !~ '[[:cntrl:]]'),
 revision bigint not null default 0 check(revision>=0),created_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict,
 check((status in ('approved','revoked') and approved_minutes is not null and approved_minutes between 1 and requested_minutes) or (status not in ('approved','revoked') and approved_minutes is null))
);
create unique index overtime_requests_active_idx on public.overtime_requests(attendance_id) where status in ('pending','approved');
create index overtime_requests_employee_idx on public.overtime_requests(company_id,employee_id,created_at desc,id);
create index overtime_requests_submitter_idx on public.overtime_requests(submitted_by);
create index overtime_requests_attendance_idx on public.overtime_requests(attendance_id);
create table public.overtime_events (
 id uuid primary key default gen_random_uuid(),company_id uuid not null,request_id uuid not null,employee_id uuid not null,
 actor_id uuid not null references auth.users(id) on delete restrict,actor_name text not null,
 actor_role text not null check(actor_role in ('administrator','hr_generalist','manager','employee')),
 status text not null check(status in ('pending','approved','rejected','cancelled','revoked')),
 reason text not null check(char_length(btrim(reason)) between 1 and 500),
 before_status text,before_approved_minutes integer,approved_minutes integer,created_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,request_id) references public.overtime_requests(company_id,id) on delete restrict,
 foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict
);
create index overtime_events_request_idx on public.overtime_events(company_id,request_id,created_at,id);
create index overtime_events_employee_idx on public.overtime_events(company_id,employee_id);
create index overtime_events_actor_idx on public.overtime_events(actor_id);
alter table public.overtime_requests enable row level security;
alter table public.overtime_events enable row level security;
revoke all on public.overtime_requests,public.overtime_events from public,anon,authenticated;
grant select on public.overtime_requests,public.overtime_events to authenticated;
create policy overtime_requests_read on public.overtime_requests for select to authenticated using(hris_private.can_read_employee(company_id,employee_id));
create policy overtime_events_read on public.overtime_events for select to authenticated using(hris_private.can_read_employee(company_id,employee_id));

-- The same saved-rule calculation as the Rust adapter. Called only behind a
-- protected RPC after locking the parent, which also serializes break writes.
create function hris_private.overtime_basis(a public.attendance) returns jsonb
language plpgsql stable security invoker set search_path='' as $$
declare s jsonb:=a.calculation_snapshot;p jsonb;starts timestamptz;ends timestamptz;raw numeric;deduct numeric;mins integer;step integer;breaks jsonb;
begin
 if a.checked_out_at is null or a.planned_shift is null or s is null or s->>'version'<>'1' or s->>'window_status'<>'ready' or not coalesce((s->>'scheduled_workday')::boolean,false) then return null;end if;
 starts:=(s->>'planned_start')::timestamptz;ends:=(s->>'planned_end')::timestamptz;p:=s->'policy';
 if starts is null or ends is null or ends<=starts or a.checked_out_at<a.checked_in_at or (p->>'company_id')::uuid is distinct from a.company_id then return null;end if;
 if exists(select 1 from public.attendance_breaks b where b.attendance_id=a.id and not b.voided and b.ended_at is null)
  or exists(select 1 from public.leave_requests l cross join lateral jsonb_array_elements(l.charged_dates) d where l.company_id=a.company_id and l.employee_id=a.employee_id and l.status='approved' and (d->>'date')::date=a.work_date and (d->>'units')::integer>0) then return null;end if;
 raw:=greatest(0,floor(extract(epoch from (a.checked_out_at-greatest(a.checked_in_at,ends)))));
 select coalesce(sum(greatest(0,floor(extract(epoch from (least(b.ended_at,a.checked_out_at)-greatest(b.started_at,a.checked_in_at,ends)))))),0) into deduct from public.attendance_breaks b where b.attendance_id=a.id and not b.voided and not b.paid and b.ended_at is not null;
 raw:=greatest(0,raw-deduct);
 if coalesce((p->>'include_pre_shift_overtime')::boolean,false) then
  select coalesce(sum(greatest(0,floor(extract(epoch from (least(b.ended_at,a.checked_out_at,starts)-greatest(b.started_at,a.checked_in_at)))))),0) into deduct from public.attendance_breaks b where b.attendance_id=a.id and not b.voided and not b.paid and b.ended_at is not null;
  raw:=raw+greatest(0,greatest(0,floor(extract(epoch from (least(a.checked_out_at,starts)-a.checked_in_at))))-deduct);
 end if;
 mins:=floor(raw/60)::integer;step:=(p->>'overtime_round_minutes')::integer;
 if step is null or step not in (1,5,10,15,30,60) or mins<(p->>'overtime_min_minutes')::integer then return null;end if;
 mins:=(mins/step)*step;if mins<=0 then return null;end if;
 select coalesce(jsonb_agg(to_jsonb(b) order by b.id),'[]'::jsonb) into breaks from public.attendance_breaks b where b.attendance_id=a.id and not b.voided;
 return jsonb_build_object('attendance',to_jsonb(a),'breaks',breaks,'minutes',mins);
end;$$;
revoke all on function hris_private.overtime_basis(public.attendance) from public,anon,authenticated;

create function hris_private.overtime_request(p_request jsonb) returns uuid
language plpgsql security definer set search_path='' as $$
declare m public.company_members;a public.attendance;r public.overtime_requests;person public.employees;op text;note text;basis jsonb;break_revisions jsonb;amount integer;actor_label text;next_status text;approved integer;
begin
 select * into m from public.company_members where user_id=(select auth.uid()) and active;
 if not found then raise exception 'Active HRIS membership required.' using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));
 select * into m from public.company_members where user_id=(select auth.uid()) and active;
 if not found then raise exception 'Your access changed.' using errcode='42501';end if;
 if jsonb_typeof(p_request) is distinct from 'object' or octet_length(p_request::text)>20000 then raise exception 'Enter valid overtime details.' using errcode='23514';end if;
 op:=p_request->>'operation';note:=btrim(p_request->>'reason');if coalesce(char_length(note),0) not between 1 and 500 or note ~ '[[:cntrl:]]' then raise exception 'Enter a reason of 1–500 characters without control characters.' using errcode='23514';end if;
 select coalesce((select full_name from public.employees where company_id=m.company_id and id=m.employee_id),'Staff') into actor_label;
 if op='submit' then
  select * into person from public.employees where company_id=m.company_id and id=m.employee_id and status='active' for share;
  if not found then raise exception 'Only active employees can request their own overtime.' using errcode='42501';end if;
  select * into a from public.attendance where company_id=m.company_id and id=(p_request->>'attendance_id')::uuid and employee_id=person.id for update;
  if not found then raise exception 'Choose your own attendance record.' using errcode='42501';end if;
  basis:=hris_private.overtime_basis(a);if basis is null then raise exception 'No available overtime candidate. Review completed attendance, saved rules, breaks and leave.' using errcode='23514';end if;
  select coalesce(jsonb_agg(jsonb_build_object('id',b.id,'revision',b.revision) order by b.id),'[]'::jsonb) into break_revisions from public.attendance_breaks b where b.attendance_id=a.id and not b.voided;
  if a.revision is distinct from (p_request->>'expected_revision')::bigint or break_revisions is distinct from p_request->'expected_breaks' or (basis->>'minutes')::integer is distinct from (p_request->>'expected_candidate_minutes')::integer then raise exception 'Attendance, breaks or candidate changed. Refresh and reopen the request.' using errcode='PT409';end if;
  if jsonb_typeof(p_request->'minutes') is distinct from 'number' or coalesce(p_request->>'minutes','') !~ '^[0-9]{1,8}$' then raise exception 'Enter whole overtime minutes.' using errcode='23514';end if;
  amount:=(p_request->>'minutes')::integer;if amount<1 or amount>(basis->>'minutes')::integer then raise exception 'Request between one minute and the calculated candidate.' using errcode='23514';end if;
  insert into public.overtime_requests(company_id,attendance_id,employee_id,employee_name,employee_code,submitted_by,work_date,requested_minutes,basis,reason)
  values(m.company_id,a.id,person.id,person.full_name,person.employee_code,m.user_id,a.work_date,amount,basis,note) returning * into r;
  insert into public.overtime_events(company_id,request_id,employee_id,actor_id,actor_name,actor_role,status,reason) values(m.company_id,r.id,person.id,m.user_id,actor_label,m.role,'pending',note);return r.id;
 elsif op in ('approve','reject','cancel','revoke') then
  select * into r from public.overtime_requests where company_id=m.company_id and id=(p_request->>'request_id')::uuid for update;
  if not found then raise exception 'Overtime request unavailable in your company.' using errcode='42501';end if;
  if op='cancel' then
   if r.submitted_by<>m.user_id or m.employee_id is distinct from r.employee_id then raise exception 'Only the requester can cancel a pending request.' using errcode='42501';end if;
  elsif r.submitted_by=m.user_id or not hris_private.can_review_employee(m.company_id,r.employee_id) then raise exception 'Another authorized HR reviewer or current direct manager must decide this request.' using errcode='42501';end if;
  if r.revision is distinct from (p_request->>'expected_revision')::bigint or (op='revoke' and r.status<>'approved') or (op<>'revoke' and r.status<>'pending') then raise exception 'This overtime request changed or was already resolved. Refresh and reopen it.' using errcode='PT409';end if;
  approved:=r.approved_minutes;
  if op='approve' then
   select * into a from public.attendance where company_id=m.company_id and id=r.attendance_id for update;
   basis:=hris_private.overtime_basis(a);if basis is distinct from r.basis then raise exception 'Attendance, breaks or leave changed. Reject or cancel and submit a fresh overtime request.' using errcode='PT409';end if;
   if jsonb_typeof(p_request->'minutes') is distinct from 'number' or coalesce(p_request->>'minutes','') !~ '^[0-9]{1,8}$' then raise exception 'Enter whole approved minutes.' using errcode='23514';end if;
   approved:=(p_request->>'minutes')::integer;if approved<1 or approved>r.requested_minutes then raise exception 'Approve between one minute and the requested amount.' using errcode='23514';end if;next_status:='approved';
  else next_status:=case op when 'reject' then 'rejected' when 'cancel' then 'cancelled' else 'revoked' end;end if;
  update public.overtime_requests set status=next_status,approved_minutes=approved,revision=r.revision+1 where id=r.id;
  insert into public.overtime_events(company_id,request_id,employee_id,actor_id,actor_name,actor_role,status,reason,before_status,before_approved_minutes,approved_minutes)
  values(m.company_id,r.id,r.employee_id,m.user_id,actor_label,m.role,next_status,note,r.status,r.approved_minutes,approved);return r.id;
 else raise exception 'Choose submit, approve, reject, cancel or revoke.' using errcode='23514';end if;
exception when unique_violation then raise exception 'A pending or approved request exists. Cancel the pending request or have its approval revoked before resubmitting.' using errcode='PT409';end;$$;
create function public.overtime_request(p_request jsonb) returns uuid language sql security invoker set search_path='' as $$select hris_private.overtime_request(p_request);$$;
revoke all on function public.overtime_request(jsonb),hris_private.overtime_request(jsonb) from public,anon,authenticated;
grant execute on function public.overtime_request(jsonb),hris_private.overtime_request(jsonb) to authenticated;
commit;
