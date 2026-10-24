-- Separate own-person inbox/read markers. Desktop staff-only read history and
-- its existing RLS/RPC contracts are unchanged.
begin;
create function hris_private.valid_portal_notification_keys(p_keys text[],p_limit integer)returns boolean
language sql immutable set search_path=''as $$
 select hris_private.valid_notification_keys(p_keys,p_limit)and not exists(
  select 1 from unnest(p_keys)k where split_part(k,':',1)not in('leave-status','leave-upcoming','correction-status','overtime-status','checkout'));
$$;
revoke all on function hris_private.valid_portal_notification_keys(text[],integer)from public,anon,authenticated;
create table public.portal_notification_read_state(
 user_id uuid not null references auth.users(id)on delete cascade,
 company_id uuid not null references public.companies(id)on delete cascade,
 read_keys text[]not null default'{}',updated_at timestamptz not null default clock_timestamp(),
 primary key(user_id,company_id),
 check(hris_private.valid_portal_notification_keys(read_keys,5000))
);
create index portal_notification_read_state_company_idx on public.portal_notification_read_state(company_id);
alter table public.portal_notification_read_state enable row level security;
revoke all on public.portal_notification_read_state from public,anon,authenticated;
grant select on public.portal_notification_read_state to authenticated;
grant all on public.portal_notification_read_state to service_role;
create policy portal_notification_state_own on public.portal_notification_read_state for select to authenticated using(
 user_id=(select auth.uid())and exists(select 1 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id
  where m.user_id=(select auth.uid())and m.company_id=portal_notification_read_state.company_id and m.active and e.status='active')
);

-- Parameterized helper is private and not executable by client roles. Only the
-- caller-bound protected functions below may select a person for it.
create function hris_private.portal_notification_rows(p_company uuid,p_employee uuid,p_today date,p_now timestamptz)
returns table(alert_key text,kind text,title text,detail text,on_date date,urgent boolean,target_id uuid,target_page text)
language sql stable security invoker set search_path=''as $$
 with settings as(select timezone from public.companies where id=p_company)
 select 'leave-status:'||r.id||':'||r.revision,'leave-status','Your leave was '||r.status,
  r.type_name||' · '||r.starts_on||' to '||r.ends_on,(e.created_at at time zone s.timezone)::date,false,r.id,'leave'
 from public.leave_requests r cross join settings s cross join lateral(
  select max(v.created_at)created_at from public.leave_events v where v.company_id=p_company and v.employee_id=p_employee and v.request_id=r.id)e
 where r.company_id=p_company and r.employee_id=p_employee and r.status in('approved','rejected','cancelled')and(e.created_at at time zone s.timezone)::date between p_today-29 and p_today
 union all
 select 'leave-upcoming:'||r.id||':'||r.revision||case when r.starts_on=p_today then':today'else':soon'end,'leave-upcoming',
  case when r.starts_on=p_today then'Your approved leave starts today'else'Your approved leave starts soon'end,
  r.type_name||' · '||r.starts_on||' to '||r.ends_on,r.starts_on,r.starts_on=p_today,r.id,'leave'
 from public.leave_requests r where r.company_id=p_company and r.employee_id=p_employee and r.status='approved'and r.starts_on between p_today and p_today+7
 union all
 select 'correction-status:'||r.id||':'||r.revision,'correction-status','Your attendance correction was '||r.status,
  'Work date '||r.work_date,(e.created_at at time zone s.timezone)::date,false,r.id,'attendance'
 from public.attendance_requests r cross join settings s cross join lateral(
  select max(v.created_at)created_at from public.attendance_request_events v where v.company_id=p_company and v.employee_id=p_employee and v.request_id=r.id)e
 where r.company_id=p_company and r.employee_id=p_employee and r.status<>'pending'and(e.created_at at time zone s.timezone)::date between p_today-29 and p_today
 union all
 select 'overtime-status:'||r.id||':'||r.revision,'overtime-status','Your overtime request was '||r.status,
  'Work date '||r.work_date||' · Check the current saved attendance basis',(e.created_at at time zone s.timezone)::date,false,r.id,'attendance'
 from public.overtime_requests r cross join settings s cross join lateral(
  select max(v.created_at)created_at from public.overtime_events v where v.company_id=p_company and v.employee_id=p_employee and v.request_id=r.id)e
 where r.company_id=p_company and r.employee_id=p_employee and r.status<>'pending'and(e.created_at at time zone s.timezone)::date between p_today-29 and p_today
 union all
 select 'checkout:'||a.id||':'||a.revision,'checkout','An earlier shift is still open','Work date '||a.work_date,
  a.work_date,true,a.id,'attendance'from public.attendance a where a.company_id=p_company and a.employee_id=p_employee
  and a.checked_out_at is null and a.work_date<p_today and p_now-a.checked_in_at>=interval'24 hours';
$$;
revoke all on function hris_private.portal_notification_rows(uuid,uuid,date,timestamptz)from public,anon,authenticated;

create function hris_private.portal_notifications(p_offset integer default 0,p_limit integer default 25,p_unread_only boolean default false)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;zone text;today date;keys text[];result jsonb;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active;
 if not found or not exists(select 1 from public.employees where company_id=m.company_id and id=m.employee_id and status='active')then raise exception 'Active own employee membership required.'using errcode='42501';end if;
 if p_offset is null or p_limit is null or p_unread_only is null or p_offset not between 0 and 50000 or p_limit not between 1 and 100 then raise exception 'Choose a bounded inbox page.'using errcode='23514';end if;
 select timezone into strict zone from public.companies where id=m.company_id;today:=(now()at time zone zone)::date;
 select read_keys into keys from public.portal_notification_read_state where user_id=m.user_id and company_id=m.company_id;keys:=coalesce(keys,'{}');
 with all_items as materialized(select *,alert_key=any(keys)is_read from hris_private.portal_notification_rows(m.company_id,m.employee_id,today,now())),
 matching as(select *from all_items where not p_unread_only or not is_read),
 page as(select *from matching order by urgent desc,on_date desc,alert_key limit p_limit offset p_offset)
 select jsonb_build_object('company_id',m.company_id,'employee_id',m.employee_id,'user_id',m.user_id,'asof',today,'generated_at',now(),
  'offset',p_offset,'limit',p_limit,'unread_only',p_unread_only,'total_items',(select count(*)from matching),'unread_count',(select count(*)from all_items where not is_read),
  'items',coalesce((select jsonb_agg(jsonb_build_object('key',alert_key,'kind',kind,'title',title,'detail',detail,'date',on_date,'urgent',urgent,'read',is_read,'target_id',target_id,'target_page',target_page)order by urgent desc,on_date desc,alert_key)from page),'[]'::jsonb))into result;
 return result;
end;$$;
create function public.portal_notifications(p_offset integer default 0,p_limit integer default 25,p_unread_only boolean default false)returns jsonb
language sql stable security invoker set search_path=''as $$select hris_private.portal_notifications(p_offset,p_limit,p_unread_only);$$;
revoke all on function public.portal_notifications(integer,integer,boolean),hris_private.portal_notifications(integer,integer,boolean)from public,anon,authenticated;
grant execute on function public.portal_notifications(integer,integer,boolean),hris_private.portal_notifications(integer,integer,boolean)to authenticated;

create function hris_private.portal_notification_reads(p_keys text[],p_read boolean,p_reset boolean default false)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;saved public.portal_notification_read_state;next_keys text[];zone text;today date;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active for share;
 if not found or not exists(select 1 from public.employees where company_id=m.company_id and id=m.employee_id and status='active')then raise exception 'Active own employee membership required.'using errcode='42501';end if;
 if p_read is null or p_reset is null or not hris_private.valid_portal_notification_keys(p_keys,100)or(p_reset and(cardinality(p_keys)<>0 or p_read))or(not p_reset and cardinality(p_keys)=0)then raise exception 'Choose1–100 current own notifications or reset only read history.'using errcode='23514';end if;
 select timezone into strict zone from public.companies where id=m.company_id;today:=(now()at time zone zone)::date;
 if not p_reset and exists(select 1 from unnest(p_keys)k where not exists(select 1 from hris_private.portal_notification_rows(m.company_id,m.employee_id,today,now())r where r.alert_key=k))then raise exception 'The notification changed or is unavailable. Refresh your inbox.'using errcode='PT409';end if;
 insert into public.portal_notification_read_state(user_id,company_id)values(m.user_id,m.company_id)on conflict(user_id,company_id)do nothing;
 select *into saved from public.portal_notification_read_state where user_id=m.user_id and company_id=m.company_id for update;
 if p_reset then next_keys:='{}';elsif p_read then select array_agg(k order by k)into next_keys from(select distinct unnest(saved.read_keys||p_keys)k)s;
 else select coalesce(array_agg(k order by k),'{}'::text[])into next_keys from unnest(saved.read_keys)k where not(k=any(p_keys));end if;
 if cardinality(next_keys)>5000 then raise exception 'Read history is full. Reset only your inbox read markers first.'using errcode='23514';end if;
 update public.portal_notification_read_state set read_keys=next_keys,updated_at=clock_timestamp()where user_id=m.user_id and company_id=m.company_id;
 return jsonb_build_object('company_id',m.company_id,'employee_id',m.employee_id,'user_id',m.user_id,'saved',true);
end;$$;
create function public.portal_notification_reads(p_keys text[],p_read boolean,p_reset boolean default false)returns jsonb
language sql security invoker set search_path=''as $$select hris_private.portal_notification_reads(p_keys,p_read,p_reset);$$;
revoke all on function public.portal_notification_reads(text[],boolean,boolean),hris_private.portal_notification_reads(text[],boolean,boolean)from public,anon,authenticated;
grant execute on function public.portal_notification_reads(text[],boolean,boolean),hris_private.portal_notification_reads(text[],boolean,boolean)to authenticated;
commit;
