-- Free in-app lifecycle notices. Legacy inbox marker contracts stay intact.
begin;
create function hris_private.valid_lifecycle_keys(p_keys text[],p_limit integer)returns boolean
language sql immutable set search_path=''as $$
 select p_keys is not null and cardinality(p_keys)<=p_limit
 and cardinality(p_keys)=(select count(distinct k)from unnest(p_keys)k)
 and not exists(select 1 from unnest(p_keys)k where k is null or
 case when k !~ '^(payroll-(monthly|cutoff|thirteenth|tax|final|amendment)-issued|payroll-receipt|payroll-final-settled|payroll-amendment-paid|appraisal-review|appraisal-publish|appraisal-published|training-start|training-due|certificate-expiry):[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}:(0|[1-9][0-9]{0,18})(:soon|:today|:expired)?$'
 then true else split_part(k,':',2)='00000000-0000-0000-0000-000000000000' or split_part(k,':',3)::numeric>9223372036854775807
 or case when split_part(k,':',1)in('appraisal-review','training-start','training-due','certificate-expiry')then array_length(string_to_array(k,':'),1)<>4 else array_length(string_to_array(k,':'),1)<>3 end end);
$$;
revoke all on function hris_private.valid_lifecycle_keys(text[],integer)from public,anon,authenticated;

create table public.lifecycle_notification_read_state(
 user_id uuid not null references auth.users(id),company_id uuid not null references public.companies(id),
 surface text not null check(surface in('desktop','portal')),read_keys text[]not null default'{}',
 updated_at timestamptz not null default clock_timestamp(),primary key(user_id,company_id,surface),
 check(hris_private.valid_lifecycle_keys(read_keys,5000))
);
create index lifecycle_notification_state_company_idx on public.lifecycle_notification_read_state(company_id);
alter table public.lifecycle_notification_read_state enable row level security;
revoke all on public.lifecycle_notification_read_state from public,anon,authenticated;
grant select on public.lifecycle_notification_read_state to authenticated;
grant all on public.lifecycle_notification_read_state to service_role;
create policy lifecycle_notification_state_own on public.lifecycle_notification_read_state for select to authenticated using(
 user_id=(select auth.uid())and exists(select 1 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id
 where m.company_id=lifecycle_notification_read_state.company_id and m.user_id=(select auth.uid())and m.active and e.status='active'
 and(surface='portal'or m.role in('administrator','hr_generalist','manager')))
);

create view hris_private.lifecycle_pay_statements with(security_invoker=true)as
 select s.company_id,s.employee_id,s.id,'monthly'::text kind,s.month::text period,s.approved_at from public.payroll_slips s
 union all select s.company_id,s.employee_id,s.id,'cutoff',s.starts_on||' to '||s.ends_on,s.approved_at from public.payroll_cutoff_slips s
 union all select s.company_id,s.employee_id,s.id,'thirteenth',s.year::text,s.approved_at from public.payroll_thirteenth_slips s
 union all select s.company_id,s.employee_id,s.id,'tax',s.year::text,s.approved_at from public.payroll_tax_slips s
 union all select s.company_id,s.employee_id,s.id,'final',s.service_end::text,s.approved_at from public.payroll_final_slips s
 where not exists(select 1 from public.payroll_final_status t where t.company_id=s.company_id and t.id=s.id and t.kind='withdrawn')
 union all select s.company_id,s.employee_id,s.id,'amendment',s.starts_on||' to '||s.ends_on,s.approved_at from public.payroll_amendment_slips s
 where not exists(select 1 from public.payroll_amendment_status t where t.company_id=s.company_id and t.id=s.id and t.kind='withdrawn');
revoke all on hris_private.lifecycle_pay_statements from public,anon,authenticated;

-- Not callable by client roles. The caller-bound wrappers determine identity,
-- surface, role and company-local dates, never Auth user metadata or parameters.
create function hris_private.lifecycle_notification_rows(p_company uuid,p_employee uuid,p_role text,p_today date,p_now timestamptz)
returns table(alert_key text,kind text,title text,detail text,on_date date,urgent boolean,target_id uuid,target_page text,employee_id uuid)
language sql stable security invoker set search_path=''as $$
 with settings as(select timezone from public.companies where id=p_company),
 visible_people as(select e.*from public.employees e where e.company_id=p_company and e.status='active'
  and(p_role in('administrator','hr_generalist')or e.id=p_employee or(p_role='manager'and e.reports_to=p_employee)))
 select 'payroll-'||s.kind||'-issued:'||s.id||':0','payroll-'||s.kind||'-issued',
 case s.kind when'tax'then'Your annual tax advice is available'when'final'then'Your final-pay statement is available'when'amendment'then'Your payroll amendment advice is available'when'thirteenth'then'Your 13th-month payslip is available'else'Your '||s.kind||' payslip is available'end,
 'Period '||s.period||' · Approved advice is separate from payment',
 (s.approved_at at time zone t.timezone)::date,false,s.id,'payroll_'||s.kind,s.employee_id
 from hris_private.lifecycle_pay_statements s cross join settings t where s.company_id=p_company and s.employee_id=p_employee
 and(s.approved_at at time zone t.timezone)::date between p_today-29 and p_today
 union all
 select 'payroll-receipt:'||r.id||':0','payroll-receipt',
 case when r.final_case_id is null then'Your payslip payment was recorded'else'Your payslip was settled through final pay'end,
 'Recorded received date '||r.paid_on||case when r.final_case_id is null then' · '||r.kind else' · May include source offsets; not a second full-net cash payment'end,
 greatest((r.recorded_at at time zone t.timezone)::date,r.paid_on),false,r.slip_id,'payroll_'||r.kind,r.employee_id
 from public.payroll_payment_receipts r cross join settings t where r.company_id=p_company and r.employee_id=p_employee and r.paid_on<=p_today
 and greatest((r.recorded_at at time zone t.timezone)::date,r.paid_on)between p_today-29 and p_today
 union all
 select 'payroll-final-settled:'||s.id||':0','payroll-final-settled','Your final-pay settlement was recorded',
 'Settlement date '||s.recorded_on||' · Combined cash and source offsets; see your statement',
 (c.updated_at at time zone t.timezone)::date,false,s.id,'payroll_final',s.employee_id
 from public.payroll_final_status s join public.payroll_final_cases c on c.company_id=s.company_id and c.id=s.id cross join settings t
 where s.company_id=p_company and s.employee_id=p_employee and s.kind='settled'and s.recorded_on<=p_today
 and(c.updated_at at time zone t.timezone)::date between p_today-29 and p_today
 union all
 select 'payroll-amendment-paid:'||s.id||':0','payroll-amendment-paid','Your payroll amendment payment was recorded',
 'Recorded received date '||s.recorded_on,(c.updated_at at time zone t.timezone)::date,false,s.id,'payroll_amendment',s.employee_id
 from public.payroll_amendment_status s join public.payroll_amendment_cases c on c.company_id=s.company_id and c.id=s.id cross join settings t
 where s.company_id=p_company and s.employee_id=p_employee and s.kind='paid'and s.recorded_on<=p_today
 and(c.updated_at at time zone t.timezone)::date between p_today-29 and p_today
 union all
 select 'appraisal-review:'||r.id||':'||r.revision||case when r.due_on<p_today then':expired'when r.due_on=p_today then':today'else':soon'end,
 'appraisal-review',case when r.due_on<p_today then'Your assigned appraisal is overdue'when r.due_on=p_today then'Your assigned appraisal is due today'else'Your assigned appraisal is due soon'end,
 e.full_name||' · '||r.cycle_name||' · Due '||r.due_on,r.due_on,r.due_on<=p_today,r.id,'appraisal',r.employee_id
 from public.performance_reviews r join public.performance_cycles c on c.company_id=r.company_id and c.id=r.cycle_id
 join visible_people e on e.id=r.employee_id
 where r.company_id=p_company and p_role in('administrator','hr_generalist','manager')and r.reviewer_employee_id=p_employee and r.employee_id<>p_employee
 and r.status='draft'and c.status='open'and r.due_on<=p_today+7
 and hris_private.performance_reviewer_valid(p_company,r.employee_id,p_employee)
 union all
 select 'appraisal-publish:'||r.id||':'||r.revision,'appraisal-publish','Submitted appraisal awaits publication',
 e.full_name||' · '||r.cycle_name||' · Due '||r.due_on,r.due_on,r.due_on<=p_today,r.id,'appraisal',r.employee_id
 from public.performance_reviews r join public.performance_cycles c on c.company_id=r.company_id and c.id=r.cycle_id
 join visible_people e on e.id=r.employee_id where r.company_id=p_company and p_role in('administrator','hr_generalist')and r.employee_id<>p_employee
 and r.status='submitted'and c.status='open'and hris_private.performance_reviewer_valid(p_company,r.employee_id,r.reviewer_employee_id)
 union all
 select 'appraisal-published:'||r.id||':'||r.revision,'appraisal-published','Your published appraisal is available',
 r.cycle_name||' · Review period '||r.starts_on||' to '||r.ends_on,(r.published_at at time zone t.timezone)::date,false,r.id,'appraisal',r.employee_id
 from public.performance_reviews r cross join settings t where r.company_id=p_company and r.employee_id=p_employee and r.status='published'
 and(r.published_at at time zone t.timezone)::date between p_today-29 and p_today
 union all
 select 'training-start:'||r.id||':'||r.revision||case when r.starts_on=p_today then':today'else':soon'end,'training-start',
 case when r.starts_on=p_today then'Training starts today'else'Training starts soon'end,
 e.full_name||' · '||r.title||' · Starts '||r.starts_on,r.starts_on,r.starts_on=p_today,r.id,'training',r.employee_id
 from public.employee_training r join visible_people e on e.id=r.employee_id where r.company_id=p_company and r.status in('planned','in_progress')and r.starts_on between p_today and p_today+7
 union all
 select 'training-due:'||r.id||':'||r.revision||case when r.due_on<p_today then':expired'when r.due_on=p_today then':today'else':soon'end,'training-due',
 case when r.due_on<p_today then'Training is overdue'when r.due_on=p_today then'Training is due today'else'Training is due soon'end,
 e.full_name||' · '||r.title||' · Due '||r.due_on,r.due_on,r.due_on<=p_today,r.id,'training',r.employee_id
 from public.employee_training r join visible_people e on e.id=r.employee_id where r.company_id=p_company and r.status in('planned','in_progress')and r.due_on<=p_today+7
 union all
 select 'certificate-expiry:'||r.id||':'||r.revision||case when r.expires_on<p_today then':expired'when r.expires_on=p_today then':today'else':soon'end,'certificate-expiry',
 case when r.expires_on<p_today then'Recorded certificate is past expiry'when r.expires_on=p_today then'Recorded certificate expires today'else'Recorded certificate expires within 30 days'end,
 e.full_name||' · '||r.title||' · Expiry '||r.expires_on,r.expires_on,r.expires_on<=p_today,r.id,'certificate',r.employee_id
 from public.employee_certifications r join visible_people e on e.id=r.employee_id where r.company_id=p_company and r.status='valid'and not r.archived and r.expires_on<=p_today+30;
$$;
revoke all on function hris_private.lifecycle_notification_rows(uuid,uuid,text,date,timestamptz)from public,anon,authenticated;

create function hris_private.lifecycle_notifications(p_surface text default'desktop')returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;zone text;today date;keys text[];result jsonb;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active;
 if not found or not exists(select 1 from public.employees where company_id=m.company_id and id=m.employee_id and status='active')
 or p_surface is null or p_surface not in('desktop','portal')or(p_surface='desktop'and m.role not in('administrator','hr_generalist','manager'))then
 raise exception 'Active permitted employee membership is required.'using errcode='42501';end if;
 select timezone into strict zone from public.companies where id=m.company_id;today:=(now()at time zone zone)::date;
 select read_keys into keys from public.lifecycle_notification_read_state where user_id=m.user_id and company_id=m.company_id and surface=p_surface;
 with all_items as materialized(select *from hris_private.lifecycle_notification_rows(m.company_id,m.employee_id,case when p_surface='portal'then'employee'else m.role end,today,now())),
 page as(select *from all_items order by urgent desc,on_date desc,alert_key limit 5000)
 select jsonb_build_object('version','lifecycle-notifications-v1','company_id',m.company_id,'user_id',m.user_id,'employee_id',m.employee_id,'role',m.role,'surface',p_surface,
 'as_of',today,'generated_at',now(),'read_keys',coalesce(to_jsonb(keys),'[]'::jsonb),'total_items',(select count(*)from all_items),'truncated',(select count(*)>5000 from all_items),
 'items',coalesce((select jsonb_agg(jsonb_build_object('key',alert_key,'kind',kind,'title',title,'detail',detail,'date',on_date,'urgent',urgent,'target_id',target_id,'target_page',target_page,'employee_id',employee_id)order by urgent desc,on_date desc,alert_key)from page),'[]'::jsonb))into result;
 return result;
end;$$;
create function public.lifecycle_notifications(p_surface text default'desktop')returns jsonb
language sql stable security invoker set search_path=''as $$select hris_private.lifecycle_notifications(p_surface);$$;
revoke all on function hris_private.lifecycle_notifications(text),public.lifecycle_notifications(text)from public,anon,authenticated;
grant execute on function hris_private.lifecycle_notifications(text),public.lifecycle_notifications(text)to authenticated;

create function hris_private.lifecycle_notification_reads(p_surface text,p_keys text[],p_read boolean,p_reset boolean default false)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;saved public.lifecycle_notification_read_state;zone text;today date;next_keys text[];
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active for share;
 if not found or not exists(select 1 from public.employees where company_id=m.company_id and id=m.employee_id and status='active')
 or p_surface is null or p_surface not in('desktop','portal')or(p_surface='desktop'and m.role not in('administrator','hr_generalist','manager'))then
 raise exception 'Active permitted employee membership is required.'using errcode='42501';end if;
 if p_read is null or p_reset is null or not hris_private.valid_lifecycle_keys(p_keys,100)or(p_reset and(cardinality(p_keys)<>0 or p_read))or(not p_reset and cardinality(p_keys)=0)then
 raise exception 'Choose1–100 current notices or reset only read history.'using errcode='23514';end if;
 select timezone into strict zone from public.companies where id=m.company_id;today:=(now()at time zone zone)::date;
 if not p_reset and exists(select 1 from unnest(p_keys)k where not exists(select 1 from hris_private.lifecycle_notification_rows(m.company_id,m.employee_id,case when p_surface='portal'then'employee'else m.role end,today,now())r where r.alert_key=k))then
 raise exception 'A notice changed or is unavailable. Refresh your inbox.'using errcode='PT409';end if;
 insert into public.lifecycle_notification_read_state(user_id,company_id,surface)values(m.user_id,m.company_id,p_surface)on conflict do nothing;
 select *into strict saved from public.lifecycle_notification_read_state where user_id=m.user_id and company_id=m.company_id and surface=p_surface for update;
 if p_reset then next_keys:='{}';elsif p_read then select array_agg(distinct k order by k)into next_keys from unnest(saved.read_keys||p_keys)k;
 else select coalesce(array_agg(k order by k),'{}')into next_keys from unnest(saved.read_keys)k where not(k=any(p_keys));end if;
 if cardinality(next_keys)>5000 then raise exception 'Lifecycle read history is full. Reset it in Notifications.'using errcode='23514';end if;
 update public.lifecycle_notification_read_state set read_keys=next_keys,updated_at=clock_timestamp()where user_id=m.user_id and company_id=m.company_id and surface=p_surface returning *into saved;
 return to_jsonb(saved);
end;$$;
create function public.lifecycle_notification_reads(p_surface text,p_keys text[],p_read boolean,p_reset boolean default false)returns jsonb
language sql security invoker set search_path=''as $$select hris_private.lifecycle_notification_reads(p_surface,p_keys,p_read,p_reset);$$;
revoke all on function hris_private.lifecycle_notification_reads(text,text[],boolean,boolean),public.lifecycle_notification_reads(text,text[],boolean,boolean)from public,anon,authenticated;
grant execute on function hris_private.lifecycle_notification_reads(text,text[],boolean,boolean),public.lifecycle_notification_reads(text,text[],boolean,boolean)to authenticated;

-- New desktop can mark a mixed page atomically. The legacy row never receives
-- a new key, so released clients keep their original parser/read-history contract.
create function hris_private.staff_notification_reads(p_company_id uuid,p_keys text[],p_read boolean,p_reset boolean default false)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;old_keys text[];new_keys text[];result jsonb;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active for share;
 if not found or m.company_id is distinct from p_company_id or m.role not in('administrator','hr_generalist','manager')then raise exception 'Current staff membership is required.'using errcode='42501';end if;
 if p_keys is null or p_read is null or p_reset is null or cardinality(p_keys)>100 or cardinality(p_keys)<>(select count(distinct k)from unnest(p_keys)k)
 or(p_reset and(cardinality(p_keys)<>0 or p_read))or(not p_reset and cardinality(p_keys)=0)then raise exception 'Choose1–100 unique keys or reset only history.'using errcode='23514';end if;
 select coalesce(array_agg(k)filter(where hris_private.valid_notification_keys(array[k],1)),'{}'),coalesce(array_agg(k)filter(where not hris_private.valid_notification_keys(array[k],1)),'{}')into old_keys,new_keys from unnest(p_keys)k;
 if not hris_private.valid_lifecycle_keys(new_keys,100)then raise exception 'Invalid lifecycle key.'using errcode='23514';end if;
 if p_reset or cardinality(old_keys)>0 then result:=hris_private.set_notification_reads(p_company_id,old_keys,p_read,p_reset);else
 select to_jsonb(r)into result from public.notification_read_state r where r.user_id=m.user_id and r.company_id=m.company_id;
 result:=coalesce(result,jsonb_build_object('user_id',m.user_id,'company_id',m.company_id,'read_keys','[]'::jsonb));end if;
 if p_reset or cardinality(new_keys)>0 then perform hris_private.lifecycle_notification_reads('desktop',new_keys,p_read,p_reset);end if;
 return result;
end;$$;
create function public.staff_notification_reads(p_company_id uuid,p_keys text[],p_read boolean,p_reset boolean default false)returns jsonb
language sql security invoker set search_path=''as $$select hris_private.staff_notification_reads(p_company_id,p_keys,p_read,p_reset);$$;
revoke all on function hris_private.staff_notification_reads(uuid,text[],boolean,boolean),public.staff_notification_reads(uuid,text[],boolean,boolean)from public,anon,authenticated;
grant execute on function hris_private.staff_notification_reads(uuid,text[],boolean,boolean),public.staff_notification_reads(uuid,text[],boolean,boolean)to authenticated;

-- A new portal contract merges old personal updates and own lifecycle notices.
-- The running0.1.5 portal keeps its original functions and marker schema.
create function hris_private.portal_notifications_v2(p_offset integer default 0,p_limit integer default 25,p_unread_only boolean default false)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;zone text;today date;old_keys text[];new_keys text[];result jsonb;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active;
 if not found or not exists(select 1 from public.employees where company_id=m.company_id and id=m.employee_id and status='active')then raise exception 'Active own employee membership is required.'using errcode='42501';end if;
 if p_offset is null or p_limit is null or p_unread_only is null or p_offset not between 0 and 50000 or p_limit not between 1 and 100 then raise exception 'Choose a bounded inbox page.'using errcode='23514';end if;
 select timezone into strict zone from public.companies where id=m.company_id;today:=(now()at time zone zone)::date;
 select read_keys into old_keys from public.portal_notification_read_state where user_id=m.user_id and company_id=m.company_id;
 select read_keys into new_keys from public.lifecycle_notification_read_state where user_id=m.user_id and company_id=m.company_id and surface='portal';
 with all_items as materialized(
  select r.*,r.alert_key=any(coalesce(old_keys,'{}'))is_read from hris_private.portal_notification_rows(m.company_id,m.employee_id,today,now())r
  union all
  select r.alert_key,r.kind,r.title,r.detail,r.on_date,r.urgent,r.target_id,r.target_page,r.alert_key=any(coalesce(new_keys,'{}'))
  from hris_private.lifecycle_notification_rows(m.company_id,m.employee_id,'employee',today,now())r),
 matching as(select *from all_items where not p_unread_only or not is_read),
 page as(select *from matching order by urgent desc,on_date desc,alert_key limit p_limit offset p_offset)
 select jsonb_build_object('version','portal-notifications-v2','company_id',m.company_id,'employee_id',m.employee_id,'user_id',m.user_id,'asof',today,'generated_at',now(),
 'offset',p_offset,'limit',p_limit,'unread_only',p_unread_only,'total_items',(select count(*)from matching),'unread_count',(select count(*)from all_items where not is_read),
 'items',coalesce((select jsonb_agg(jsonb_build_object('key',alert_key,'kind',kind,'title',title,'detail',detail,'date',on_date,'urgent',urgent,'read',is_read,'target_id',target_id,'target_page',target_page)order by urgent desc,on_date desc,alert_key)from page),'[]'::jsonb))into result;
 return result;
end;$$;
create function public.portal_notifications_v2(p_offset integer default 0,p_limit integer default 25,p_unread_only boolean default false)returns jsonb
language sql stable security invoker set search_path=''as $$select hris_private.portal_notifications_v2(p_offset,p_limit,p_unread_only);$$;
revoke all on function hris_private.portal_notifications_v2(integer,integer,boolean),public.portal_notifications_v2(integer,integer,boolean)from public,anon,authenticated;
grant execute on function hris_private.portal_notifications_v2(integer,integer,boolean),public.portal_notifications_v2(integer,integer,boolean)to authenticated;

create function hris_private.portal_notification_reads_v2(p_keys text[],p_read boolean,p_reset boolean default false)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;old_keys text[];new_keys text[];
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active for share;
 if not found or not exists(select 1 from public.employees where company_id=m.company_id and id=m.employee_id and status='active')then raise exception 'Active own employee membership is required.'using errcode='42501';end if;
 if p_keys is null or p_read is null or p_reset is null or cardinality(p_keys)>100 or cardinality(p_keys)<>(select count(distinct k)from unnest(p_keys)k)
 or(p_reset and(cardinality(p_keys)<>0 or p_read))or(not p_reset and cardinality(p_keys)=0)then raise exception 'Choose1–100 unique keys or reset only history.'using errcode='23514';end if;
 select coalesce(array_agg(k)filter(where hris_private.valid_portal_notification_keys(array[k],1)),'{}'),coalesce(array_agg(k)filter(where not hris_private.valid_portal_notification_keys(array[k],1)),'{}')into old_keys,new_keys from unnest(p_keys)k;
 if not hris_private.valid_lifecycle_keys(new_keys,100)then raise exception 'Invalid lifecycle key.'using errcode='23514';end if;
 if p_reset or cardinality(old_keys)>0 then perform hris_private.portal_notification_reads(old_keys,p_read,p_reset);end if;
 if p_reset or cardinality(new_keys)>0 then perform hris_private.lifecycle_notification_reads('portal',new_keys,p_read,p_reset);end if;
 return jsonb_build_object('saved',true,'company_id',m.company_id,'employee_id',m.employee_id,'user_id',m.user_id);
end;$$;
create function public.portal_notification_reads_v2(p_keys text[],p_read boolean,p_reset boolean default false)returns jsonb
language sql security invoker set search_path=''as $$select hris_private.portal_notification_reads_v2(p_keys,p_read,p_reset);$$;
revoke all on function hris_private.portal_notification_reads_v2(text[],boolean,boolean),public.portal_notification_reads_v2(text[],boolean,boolean)from public,anon,authenticated;
grant execute on function hris_private.portal_notification_reads_v2(text[],boolean,boolean),public.portal_notification_reads_v2(text[],boolean,boolean)to authenticated;
commit;
