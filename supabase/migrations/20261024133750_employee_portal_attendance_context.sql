-- Read-only own context for the separate employee portal; existing guarded
-- correction/overtime writers and desktop contracts remain unchanged.
begin;
create function hris_private.portal_attendance_context(p_day date) returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare m public.company_members;person public.employees;a public.attendance;
 zone text;day date;v_basis jsonb;revisions jsonb:='[]'::jsonb;
 pending_correction boolean;blocked_overtime boolean;approved integer:=0;
begin
 select *into m from public.company_members where user_id=(select auth.uid()) and active;
 if not found then raise exception 'Active HRIS membership required.'using errcode='42501';end if;
 select *into person from public.employees where company_id=m.company_id and id=m.employee_id and status='active';
 if not found then raise exception 'Active own employee required.'using errcode='42501';end if;
 select timezone into strict zone from public.companies where id=m.company_id;
 day:=coalesce(p_day,(now()at time zone zone)::date);
 if not isfinite(day)or day<date'1900-01-01'or day>(now()at time zone zone)::date then
  raise exception 'Choose a past or current work date.'using errcode='23514';end if;
 select *into a from public.attendance where company_id=m.company_id and employee_id=m.employee_id and work_date=day;
 if found then
  v_basis:=hris_private.overtime_basis(a);
  select coalesce(jsonb_agg(jsonb_build_object('id',b.id,'revision',b.revision)order by b.id),'[]'::jsonb)
   into revisions from public.attendance_breaks b where b.attendance_id=a.id and not b.voided;
  select coalesce(max(r.approved_minutes)filter(where r.status='approved'and r.basis=v_basis),0),
   coalesce(bool_or(r.status in('pending','approved')),false)
   into approved,blocked_overtime from public.overtime_requests r where r.company_id=m.company_id and r.employee_id=m.employee_id and r.attendance_id=a.id;
 else blocked_overtime:=false;
 end if;
 select exists(select 1 from public.attendance_requests r where r.company_id=m.company_id and r.employee_id=m.employee_id and r.work_date=day and r.status='pending')into pending_correction;
 return jsonb_build_object('company_id',m.company_id,'employee_id',m.employee_id,'work_date',day,'timezone',zone,
  'attendance',case when a.id is null then null else jsonb_build_object('id',a.id,'revision',a.revision,'work_date',a.work_date,'checked_in_at',a.checked_in_at,'checked_out_at',a.checked_out_at)end,
  'candidate_minutes',coalesce((v_basis->>'minutes')::integer,0),'break_revisions',revisions,
  'correction_pending',pending_correction,'overtime_blocked',blocked_overtime,'current_approved_minutes',approved);
end;$$;
create function public.portal_attendance_context(p_day date)returns jsonb
language sql stable security invoker set search_path=''as $$select hris_private.portal_attendance_context(p_day);$$;
revoke all on function public.portal_attendance_context(date),hris_private.portal_attendance_context(date)from public,anon,authenticated;
grant execute on function public.portal_attendance_context(date),hris_private.portal_attendance_context(date)to authenticated;
commit;
