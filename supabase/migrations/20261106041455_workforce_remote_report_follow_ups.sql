-- A read-only work-date queue based on saved remote punches and current report status.
begin;

create function hris_private.work_report_follow_ups(p_work_date date) returns jsonb
language plpgsql stable security definer set search_path = '' as $$
declare
  member public.company_members := hris_private.work_activity_member();
  today date;
  follow_ups jsonb;
begin
  if not (hris_private.has_staff_capability(member.company_id, 'attendance_administration')
          or hris_private.has_staff_capability(member.company_id, 'team_review')) then
    raise exception 'Current attendance review access is required.' using errcode = '42501';
  end if;

  select (statement_timestamp() at time zone c.timezone)::date into today
    from public.companies c where c.id = member.company_id;
  if p_work_date is null or not isfinite(p_work_date) or p_work_date > today then
    raise exception 'Choose today or an earlier work date.' using errcode = '23514';
  end if;

  select coalesce(jsonb_agg(jsonb_build_object(
    'employee_id', a.employee_id, 'state', coalesce(r.status, 'missing')
  ) order by a.employee_id), '[]'::jsonb) into follow_ups
  from public.attendance a
  join hris_private.attendance_policy_snapshots p
    on p.attendance_id = a.id and p.company_id = a.company_id and p.employee_id = a.employee_id
  left join hris_private.daily_work_reports r
    on r.company_id = a.company_id and r.employee_id = a.employee_id and r.work_date = a.work_date
  where a.company_id = member.company_id and a.work_date = p_work_date
    and a.checked_out_at is not null
    and p.method = 'remote' and p.policy->>'arrangement' in ('remote', 'hybrid')
    and (r.id is null or r.status in ('draft', 'returned'))
    and hris_private.work_activity_scope(a.company_id, a.employee_id, true);

  return jsonb_build_object('company_id', member.company_id, 'user_id', member.user_id,
    'work_date', p_work_date, 'rows', follow_ups);
end $$;

create function public.work_report_follow_ups(p_work_date date) returns jsonb
language sql stable security invoker set search_path = '' as $$
  select hris_private.work_report_follow_ups(p_work_date);
$$;

revoke all on function hris_private.work_report_follow_ups(date), public.work_report_follow_ups(date)
  from public, anon, authenticated, service_role;
grant execute on function hris_private.work_report_follow_ups(date), public.work_report_follow_ups(date)
  to authenticated;

comment on function public.work_report_follow_ups(date) is
  'Current reviewer queue for a work date, based on completed punches with saved remote policy.';
commit;
