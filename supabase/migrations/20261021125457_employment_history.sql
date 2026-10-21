-- Server-recorded employment history. Existing records receive a starting snapshot;
-- no earlier events or effective employment dates are inferred.
begin;

create table public.employment_history (
    id uuid primary key default gen_random_uuid(),
    sequence bigint generated always as identity unique,
    company_id uuid not null references public.companies(id) on delete restrict,
    employee_id uuid not null,
    kind text not null check(kind in ('starting_snapshot','created','updated')),
    actor_id uuid references auth.users(id) on delete restrict,
    actor_name text not null,
    actor_role text check(actor_role in ('administrator','hr_generalist')),
    before_record jsonb,
    after_record jsonb not null check(jsonb_typeof(after_record)='object'),
    recorded_at timestamptz not null default clock_timestamp(),
    foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict,
    check((kind='updated' and before_record is not null and jsonb_typeof(before_record)='object') or
          (kind in ('starting_snapshot','created') and before_record is null)),
    check((actor_id is null and actor_role is null) or (actor_id is not null and actor_role is not null))
);
create index employment_history_person_idx on public.employment_history(company_id,employee_id,sequence desc);
create index employment_history_actor_idx on public.employment_history(actor_id);
create unique index employment_history_initial_idx on public.employment_history(employee_id)
    where kind in ('starting_snapshot','created');
alter table public.employment_history enable row level security;
revoke all on public.employment_history from public,anon,authenticated;
grant select on public.employment_history to authenticated;
create policy employment_history_read on public.employment_history for select to authenticated
    using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));

-- Explicit employment-only fields avoid copying private contacts or future columns.
-- Referenced labels are stored at save time so later renames cannot rewrite history.
create function hris_private.employment_state(p public.employees) returns jsonb
language sql stable security invoker set search_path='' as $$
    select jsonb_build_object('employee',jsonb_build_object(
        'id',p.id,'company_id',p.company_id,'employee_code',p.employee_code,'full_name',p.full_name,
        'department',p.department,'job_title',p.job_title,'joined_on',p.joined_on,
        'annual_leave_days',p.annual_leave_days,'status',p.status,'reports_to',p.reports_to,
        'employment_type_id',p.employment_type_id,'work_location_id',p.work_location_id),
        'reports_to_name',(select full_name from public.employees where company_id=p.company_id and id=p.reports_to),
        'employment_type_name',(select name from public.employment_types where company_id=p.company_id and id=p.employment_type_id),
        'work_location_name',(select name from public.work_locations where company_id=p.company_id and id=p.work_location_id));
$$;
revoke all on function hris_private.employment_state(public.employees) from public,anon,authenticated;

create function hris_private.record_employment_history() returns trigger
language plpgsql security definer set search_path='' as $$
declare prior jsonb; saved jsonb; actor uuid; actor_label text; actor_kind text;
begin
    -- Unchanged employee saves (including contacts/role-only saves) create no noise.
    saved:=hris_private.employment_state(new);
    if tg_op='UPDATE' then
        prior:=hris_private.employment_state(old);
        if prior->'employee'=saved->'employee' then return new; end if;
    end if;
    select m.user_id,e.full_name,m.role into actor,actor_label,actor_kind
        from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id
        where m.company_id=new.company_id and m.user_id=(select auth.uid()) and m.active
          and m.role in ('administrator','hr_generalist');
    -- Trusted provisioning/bootstrap or operator SQL without an HR actor is labeled System.
    insert into public.employment_history(company_id,employee_id,kind,actor_id,actor_name,actor_role,before_record,after_record)
        values(new.company_id,new.id,case when tg_op='INSERT' then 'created' else 'updated' end,
            actor,coalesce(actor_label,'System'),actor_kind,prior,saved);
    return new;
end;
$$;
revoke all on function hris_private.record_employment_history() from public,anon,authenticated;
create trigger employees_record_history after insert or update on public.employees
    for each row execute function hris_private.record_employment_history();

-- Database writes and audit entries commit/roll back together, including old clients.
insert into public.employment_history(company_id,employee_id,kind,actor_name,after_record)
    select company_id,id,'starting_snapshot','System',hris_private.employment_state(e) from public.employees e;

create function hris_private.employment_history_immutable() returns trigger
language plpgsql security invoker set search_path='' as $$
begin
    raise exception 'Employment history cannot be edited or deleted. Correct the employee record to add a new event.' using errcode='42501';
end;
$$;
revoke all on function hris_private.employment_history_immutable() from public,anon,authenticated;
create trigger employment_history_immutable before update or delete on public.employment_history
    for each row execute function hris_private.employment_history_immutable();
commit;
