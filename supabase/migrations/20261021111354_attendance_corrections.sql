-- HR-only backdated attendance with an immutable correction history.
-- Existing check-in/check-out clients retain their server-timed column grants.
begin;
set local search_path = public, extensions, pg_catalog;

alter table public.attendance add column revision bigint not null default 0 check (revision >= 0);
-- This also arbitrates concurrent inserts/edits without advisory/row-lock inversion.
alter table public.attendance add constraint attendance_no_overlapping_shifts
    exclude using gist (employee_id with =, tstzrange(checked_in_at,checked_out_at,'[)') with &&);

create table public.attendance_corrections (
    id uuid primary key default gen_random_uuid(),
    company_id uuid not null references public.companies(id) on delete restrict,
    attendance_id uuid not null references public.attendance(id) on delete restrict,
    employee_id uuid not null,
    work_date date not null,
    actor_id uuid not null references auth.users(id) on delete restrict,
    actor_name text not null,
    actor_role text not null check (actor_role in ('administrator','hr_generalist')),
    reason text not null check (char_length(btrim(reason)) between 1 and 500),
    before_record jsonb,
    after_record jsonb not null check (jsonb_typeof(after_record)='object'),
    created_at timestamptz not null default clock_timestamp(),
    foreign key (company_id,employee_id) references public.employees(company_id,id) on delete restrict,
    check (before_record is null or jsonb_typeof(before_record)='object')
);
create index attendance_corrections_record_idx on public.attendance_corrections(attendance_id,created_at desc,id);
create index attendance_corrections_company_idx on public.attendance_corrections(company_id,created_at desc,id);
create index attendance_corrections_employee_idx on public.attendance_corrections(company_id,employee_id);
create index attendance_corrections_actor_idx on public.attendance_corrections(actor_id);
alter table public.attendance_corrections enable row level security;
revoke all on public.attendance_corrections from public,anon,authenticated;
grant select on public.attendance_corrections to authenticated;
create policy attendance_corrections_read on public.attendance_corrections for select to authenticated
    using (hris_private.company_role(company_id) in ('administrator','hr_generalist'));

-- Only the guarded definer helper can create this short-lived transaction context.
-- A forged GUC, client timestamp or RPC payload cannot authorize a trigger bypass.
create table hris_private.attendance_correction_context (
    transaction_id bigint primary key,
    attendance_id uuid not null,
    actor_id uuid not null
);
revoke all on hris_private.attendance_correction_context from public,anon,authenticated;
create function hris_private.is_attendance_correction(p_id uuid) returns boolean
language sql volatile security definer set search_path='' as $$
    select exists(select 1 from hris_private.attendance_correction_context
        where transaction_id=pg_catalog.txid_current() and attendance_id=p_id and actor_id=(select auth.uid()));
$$;
revoke all on function hris_private.is_attendance_correction(uuid) from public,anon;
grant execute on function hris_private.is_attendance_correction(uuid) to authenticated;

create or replace function hris_private.record_attendance() returns trigger
language plpgsql security invoker set search_path='' as $$
declare company_zone text; correction boolean:=hris_private.is_attendance_correction(new.id);
begin
    if tg_op='UPDATE' and (new.id<>old.id or new.company_id<>old.company_id
        or new.employee_id<>old.employee_id or new.work_date<>old.work_date) then
        raise exception 'Attendance identity and work date cannot be changed.' using errcode='23514';
    end if;
    if not correction then
        if tg_op='INSERT' then
            if not exists(select 1 from public.employees where company_id=new.company_id and id=new.employee_id and status='active') then
                raise exception 'Archived employees cannot receive new records.' using errcode='23514';
            end if;
            select timezone into company_zone from public.companies where id=new.company_id;
            new.checked_in_at:=clock_timestamp();
            new.work_date:=(new.checked_in_at at time zone company_zone)::date;
            new.checked_out_at:=null;
        else
            if old.checked_out_at is not null or new.checked_out_at is null then
                raise exception 'Only an open attendance record can be checked out.' using errcode='23514';
            end if;
            new.checked_in_at:=old.checked_in_at;
            new.checked_out_at:=clock_timestamp();
        end if;
    end if;
    new.revision:=case when tg_op='INSERT' then 0 else old.revision+1 end;
    return new;
end;
$$;

create function hris_private.correct_attendance(p_change jsonb) returns uuid
language plpgsql security definer set search_path='' as $$
declare
    member public.company_members; person public.employees; prior public.attendance; saved public.attendance;
    company_zone text; actor_name text; record_id uuid; target uuid; day date; time_in timestamptz; time_out timestamptz;
    note text; expected bigint; editing boolean; server_now timestamptz:=clock_timestamp();
begin
    select * into member from public.company_members where user_id=(select auth.uid()) and active
        and role in ('administrator','hr_generalist') for share;
    if not found then raise exception 'Only active HR staff and administrators can correct attendance.' using errcode='42501'; end if;
    if jsonb_typeof(p_change) is distinct from 'object' then
        raise exception 'Enter valid attendance correction details.' using errcode='23514';
    end if;
    note:=btrim(p_change->>'reason');
    if note is null or char_length(note) not between 1 and 500 or note ~ '[[:cntrl:]]' then
        raise exception 'Enter a correction reason of 1-500 characters without control characters.' using errcode='23514';
    end if;
    target:=(p_change->>'employee_id')::uuid;
    record_id:=(p_change->>'attendance_id')::uuid;
    editing:=record_id is not null;
    expected:=(p_change->>'expected_revision')::bigint;
    day:=(p_change->>'work_date')::date;
    time_in:=(p_change->>'checked_in_at')::timestamptz;
    time_out:=(p_change->>'checked_out_at')::timestamptz;
    select * into person from public.employees where company_id=member.company_id and id=target for share;
    if not found then raise exception 'Choose an employee in this company.' using errcode='42501'; end if;
    select timezone into company_zone from public.companies where id=member.company_id;
    if day is null or time_in is null or not isfinite(day) or not isfinite(time_in)
      or (time_out is not null and not isfinite(time_out))
      or day<date '1900-01-01' or day>(server_now at time zone company_zone)::date
      or (time_in at time zone company_zone)::date<>day or time_in>server_now
      or time_out>server_now or time_out<time_in then
        raise exception 'Use valid past or current times; check-out must be on or after check-in.' using errcode='23514';
    end if;
    if person.joined_on is not null and day<person.joined_on then
        raise exception 'Attendance cannot be recorded before the employee hire date.' using errcode='23514';
    end if;
    if person.status='archived' and day>=(server_now at time zone company_zone)::date then
        raise exception 'Archived employees can only have historical attendance corrected.' using errcode='23514';
    end if;
    if editing then
        select * into prior from public.attendance where id=record_id and company_id=member.company_id
            and employee_id=target and work_date=day for update;
        if not found then raise exception 'Attendance changed or is unavailable. Refresh before correcting it.' using errcode='23514'; end if;
        if expected is null or expected<>prior.revision then
            raise exception 'Someone changed this attendance record. Refresh, reopen it and review their changes.' using errcode='40001';
        end if;
        if prior.checked_in_at=time_in and prior.checked_out_at is not distinct from time_out then
            raise exception 'Change the check-in or check-out time before saving a correction.' using errcode='23514';
        end if;
    else
        if expected is not null then raise exception 'Refresh before recording attendance.' using errcode='23514'; end if;
        record_id:=gen_random_uuid();
    end if;
    select coalesce(e.full_name,'HR staff') into actor_name from public.company_members m
        left join public.employees e on e.company_id=m.company_id and e.id=m.employee_id where m.user_id=member.user_id;
    insert into hris_private.attendance_correction_context values(pg_catalog.txid_current(),record_id,member.user_id);
    if editing then
        update public.attendance set checked_in_at=time_in,checked_out_at=time_out where id=record_id returning * into saved;
    else
        insert into public.attendance(id,company_id,employee_id,work_date,checked_in_at,checked_out_at)
            values(record_id,member.company_id,target,day,time_in,time_out) returning * into saved;
    end if;
    delete from hris_private.attendance_correction_context where transaction_id=pg_catalog.txid_current();
    insert into public.attendance_corrections(company_id,attendance_id,employee_id,work_date,actor_id,actor_name,actor_role,reason,before_record,after_record)
        values(member.company_id,record_id,target,day,member.user_id,actor_name,member.role,note,
            case when editing then to_jsonb(prior) else null end,to_jsonb(saved));
    return record_id;
exception
    when unique_violation then raise exception 'Attendance already exists for this date, or another shift is still open. Refresh and review it.' using errcode='23505';
    when exclusion_violation then raise exception 'These times overlap another attendance record. Review both shifts before saving.' using errcode='23P01';
end;
$$;
create function public.correct_attendance(p_change jsonb) returns uuid
language sql security invoker set search_path='' as $$ select hris_private.correct_attendance(p_change); $$;
revoke all on function hris_private.correct_attendance(jsonb),public.correct_attendance(jsonb) from public,anon;
grant execute on function hris_private.correct_attendance(jsonb),public.correct_attendance(jsonb) to authenticated;
-- Explicitly preserve the legacy grants: no direct client access to custom timestamps/revisions/history.
revoke insert,update,delete on public.attendance_corrections from authenticated;
commit;
