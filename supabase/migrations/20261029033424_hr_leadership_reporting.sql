-- Aggregate-only Administrator leadership reporting. No writes to company facts.
begin;
create function hris_private.hr_leadership_report(p_start date,p_end date)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;company public.companies;today date;at_time timestamptz:=statement_timestamp();counts jsonb;result jsonb;
begin
 select cm.* into m from public.company_members cm join public.employees e
 on e.company_id=cm.company_id and e.id=cm.employee_id and e.status='active'
 where cm.user_id=(select auth.uid())and cm.active
 and hris_private.has_staff_capability(cm.company_id,'desktop')
 and hris_private.has_staff_capability(cm.company_id,'account_administration');
 if m.user_id is null or m.user_id='00000000-0000-0000-0000-000000000000'::uuid then
 raise exception 'Current Administrator leadership access is required.'using errcode='42501';end if;
 select c.* into strict company from public.companies c where c.id=m.company_id;
 today:=(at_time at time zone company.timezone)::date;
 if p_start is null or p_end is null or p_start<date'1900-01-01' or p_end>date'2200-12-31'
 or p_start>p_end or p_end>today or p_end-p_start>=366 then
 raise exception 'Choose up to 366 dates ending no later than company today.'using errcode='23514';end if;
 -- All reads in this STABLE function use the caller statement snapshot. Bounds
 -- reject a complete report rather than changing financial/workflow facts or truncating totals.
 select jsonb_build_object(
 'employees',(select count(*)from(select 1 from public.employees where company_id=m.company_id limit 50001)s),
 'attendance',(select count(*)from(select 1 from public.attendance where company_id=m.company_id and work_date between p_start and p_end limit 50001)s),
 'leave_requests',(select count(*)from(select 1 from public.leave_requests where company_id=m.company_id and status in('pending','approved')and starts_on<=p_end and ends_on>=p_start limit 50001)s),
 'leave_charge_dates',0,
 'applicants',(select count(*)from(select 1 from public.applicants where company_id=m.company_id limit 50001)s),
 'offers',(select count(*)from(select 1 from public.hiring_offers where company_id=m.company_id limit 50001)s),
 'hire_conversions',(select count(*)from(select 1 from public.hire_conversions where company_id=m.company_id limit 50001)s),
 'onboarding',(select count(*)from(select 1 from public.onboarding_cases where company_id=m.company_id limit 50001)s),
 'exits',(select count(*)from(select 1 from public.exit_cases where company_id=m.company_id limit 50001)s),
 'goals',(select count(*)from(select 1 from public.performance_goals where company_id=m.company_id limit 50001)s),
 'reviews',(select count(*)from(select 1 from public.performance_reviews where company_id=m.company_id limit 50001)s),
 'training',(select count(*)from(select 1 from public.employee_training where company_id=m.company_id limit 50001)s),
 'certificates',(select count(*)from(select 1 from public.employee_certifications where company_id=m.company_id limit 50001)s),
 'career',(select count(*)from(select 1 from public.career_plans where company_id=m.company_id limit 50001)s))into counts;
 if exists(select 1 from jsonb_each_text(counts)where value::bigint>50000)then
 raise exception 'Leadership source limit exceeded; no totals were truncated.'using errcode='54000';end if;
 if exists(select 1 from public.leave_requests r where r.company_id=m.company_id and r.status in('pending','approved')and r.starts_on<=p_end and r.ends_on>=p_start
 and case when jsonb_typeof(r.charged_dates)<>'array' then true else jsonb_array_length(r.charged_dates)>366 end)then
 raise exception 'A saved leave charge needs review; leadership totals were blocked.'using errcode='23514';end if;
 counts:=jsonb_set(counts,'{leave_charge_dates}',to_jsonb((select coalesce(sum(jsonb_array_length(r.charged_dates)),0)from public.leave_requests r
 where r.company_id=m.company_id and r.status in('pending','approved')and r.starts_on<=p_end and r.ends_on>=p_start)));
 if(counts->>'leave_charge_dates')::bigint>50000 then raise exception 'Leadership leave-charge limit exceeded; no totals were truncated.'using errcode='54000';end if;
 if exists(select 1 from public.leave_requests r where r.company_id=m.company_id and r.status in('pending','approved')and r.starts_on<=p_end and r.ends_on>=p_start
 and(exists(select 1 from jsonb_array_elements(r.charged_dates)d where jsonb_typeof(d)<>'object' or not(d?'date'and d?'units')
 or(d-array['date','units'])<>'{}'::jsonb or jsonb_typeof(d->'date')<>'string' or(d->>'date')!~'^\d{4}-\d{2}-\d{2}$'
 or jsonb_typeof(d->'units')<>'number' or(d->>'units')not in('1','2'))))then
 raise exception 'A saved leave charge needs review; leadership totals were blocked.'using errcode='23514';end if;
 if exists(select 1 from public.leave_requests r where r.company_id=m.company_id and r.status in('pending','approved')and r.starts_on<=p_end and r.ends_on>=p_start
 and((select coalesce(sum((d->>'units')::int),0)from jsonb_array_elements(r.charged_dates)d)<>r.charged_units
 or(select count(distinct d->>'date')from jsonb_array_elements(r.charged_dates)d)<>jsonb_array_length(r.charged_dates)
 or exists(select 1 from jsonb_array_elements(r.charged_dates)d where(d->>'date')::date not between r.starts_on and r.ends_on)))then
 raise exception 'Saved leave dates or totals need review; leadership totals were blocked.'using errcode='23514';end if;
 with e as materialized(select * from public.employees where company_id=m.company_id),
 a as materialized(select * from public.attendance where company_id=m.company_id and work_date between p_start and p_end),
 x as materialized(select * from public.exit_cases where company_id=m.company_id),
 lc as materialized(select r.status,(d->>'date')::date work_date,(d->>'units')::int units from public.leave_requests r cross join lateral jsonb_array_elements(r.charged_dates)d
 where r.company_id=m.company_id and r.status in('pending','approved')and r.starts_on<=p_end and r.ends_on>=p_start and(d->>'date')::date between p_start and p_end),
 dimensions as(
 select 'department'by,e.department name,btrim(e.department)=''unassigned,e.status from e
 union all select 'location',coalesce(o.name,''),e.work_location_id is null,e.status from e left join public.work_locations o on o.company_id=m.company_id and o.id=e.work_location_id
 union all select 'employment_type',coalesce(o.name,''),e.employment_type_id is null,e.status from e left join public.employment_types o on o.company_id=m.company_id and o.id=e.employment_type_id),
 groups as(select by,case when unassigned then''else btrim(name)end name,unassigned,count(*)filter(where status='active')active,count(*)filter(where status='archived')archived
 from dimensions group by by,case when unassigned then''else btrim(name)end,unassigned),
 intervals as(select greatest(p_start,d::date)starts_on,least(p_end,(d+interval'1 month'-interval'1 day')::date)ends_on
 from generate_series(date_trunc('month',p_start::timestamp),date_trunc('month',p_end::timestamp),interval'1 month')d)
 select jsonb_build_object('schema_version',1,'company_id',m.company_id,'user_id',m.user_id,'company_name',company.name,'generated_at',at_time,'today',today,'time_zone',company.timezone,
 'filter',jsonb_build_object('starts_on',p_start,'ends_on',p_end),'source_counts',counts,
 'workforce',(select jsonb_build_object('active',count(*)filter(where status='active'),'archived',count(*)filter(where status='archived'),
 'unknown_join',count(*)filter(where joined_on is null),'future_join',count(*)filter(where joined_on>today),'joined_in_period',count(*)filter(where joined_on between p_start and p_end),
 'finalized_exits_in_period',(select count(*)from x where status='finalized'and last_working_on between p_start and p_end),
 'active_without_department',count(*)filter(where status='active'and btrim(department)=''),'active_without_location',count(*)filter(where status='active'and work_location_id is null),
 'active_without_type',count(*)filter(where status='active'and employment_type_id is null),'active_without_manager',count(*)filter(where status='active'and reports_to is null))from e),
 'groups',(select coalesce(jsonb_agg(to_jsonb(g)order by by,unassigned,name),'[]')from groups g),
 'months',(select jsonb_agg(jsonb_build_object('starts_on',i.starts_on,'ends_on',i.ends_on,
 'joined',(select count(*)from e where joined_on between i.starts_on and i.ends_on),'finalized_exits',(select count(*)from x where status='finalized'and last_working_on between i.starts_on and i.ends_on),
 'attendance_completed',(select count(*)from a where work_date between i.starts_on and i.ends_on and checked_out_at is not null),
 'attendance_open',(select count(*)from a where work_date between i.starts_on and i.ends_on and checked_out_at is null),
 'approved_leave_units',(select coalesce(sum(units),0)from lc where work_date between i.starts_on and i.ends_on and status='approved'),
 'pending_leave_units',(select coalesce(sum(units),0)from lc where work_date between i.starts_on and i.ends_on and status='pending'))order by i.starts_on)from intervals i),
 'workload',jsonb_build_object(
 'applicants_pipeline',(select count(*)from public.applicants where company_id=m.company_id and not archived and stage in('applied','screening','interview','offer')),
 'offers_sent',(select count(*)from public.hiring_offers where company_id=m.company_id and status='sent'),
 'accepted_unconverted',(select count(*)from public.hiring_offers o where o.company_id=m.company_id and o.status='accepted'and not exists(select 1 from public.hire_conversions h where h.company_id=m.company_id and h.offer_id=o.id)),
 'onboarding_open',(select count(*)from public.onboarding_cases where company_id=m.company_id and status='open'),
 'exits_open',(select count(*)from x where status in('open','cleared')),
 'goals_active',(select count(*)from public.performance_goals where company_id=m.company_id and status='active'),
 'goals_overdue',(select count(*)from public.performance_goals where company_id=m.company_id and status='active'and due_on<today),
 'reviews_unpublished',(select count(*)from public.performance_reviews where company_id=m.company_id and status in('draft','submitted')),
 'reviews_overdue',(select count(*)from public.performance_reviews where company_id=m.company_id and status in('draft','submitted')and due_on<today),
 'training_open',(select count(*)from public.employee_training where company_id=m.company_id and status in('planned','in_progress')),
 'training_overdue',(select count(*)from public.employee_training where company_id=m.company_id and status in('planned','in_progress')and due_on<today),
 'training_completed_in_period',(select count(*)from public.employee_training where company_id=m.company_id and status='completed'and completed_on between p_start and p_end),
 'certificates_expired',(select count(*)from public.employee_certifications where company_id=m.company_id and not archived and status='valid'and issued_on<=today and expires_on<today),
 'certificates_expiring30',(select count(*)from public.employee_certifications where company_id=m.company_id and not archived and status='valid'and issued_on<=today and expires_on between today and today+30),
 'career_active',(select count(*)from public.career_plans where company_id=m.company_id and status='active')))into result;
 if jsonb_array_length(result->'groups')>600 then raise exception 'Leadership group limit exceeded; no totals were truncated.'using errcode='54000';end if;
 return result;
end;$$;
create function public.hr_leadership_report(p_start date,p_end date)returns jsonb
language sql stable security invoker set search_path=''as $$select hris_private.hr_leadership_report(p_start,p_end)$$;
revoke all on function hris_private.hr_leadership_report(date,date),public.hr_leadership_report(date,date)from public,anon,authenticated,service_role;
grant execute on function hris_private.hr_leadership_report(date,date),public.hr_leadership_report(date,date)to authenticated;
commit;
