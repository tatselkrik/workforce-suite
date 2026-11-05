-- Dated attendance policies and daily work reporting. No policy is activated by migration.
begin;

create table hris_private.work_arrangements (
 id uuid primary key, company_id uuid not null references public.companies(id),
 employee_id uuid not null, starts_on date not null, ends_on date,
 arrangement text not null check(arrangement in ('onsite','remote','hybrid')),
 office_days integer[] not null default '{1,2,3,4,5}',
 enabled boolean not null default false, reason text not null,
 sequence bigint generated always as identity, created_by uuid not null, created_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,employee_id) references public.employees(company_id,id),
 check(ends_on is null or ends_on>=starts_on),
 check(office_days <@ array[1,2,3,4,5,6,7] and cardinality(office_days)<=7),
 check(char_length(reason) between 1 and 500)
);
create index work_arrangements_employee_date on hris_private.work_arrangements(company_id,employee_id,starts_on desc,sequence desc);

create table hris_private.work_tasks (
 id uuid primary key, company_id uuid not null references public.companies(id), employee_id uuid not null,
 title text not null check(char_length(title) between 1 and 200),
 outcome text not null check(char_length(outcome) between 1 and 4000), due_on date not null,
 priority text not null check(priority in ('normal','high','urgent')),
 status text not null default 'assigned' check(status in ('assigned','in_progress','submitted','accepted','returned')),
 progress text not null default '' check(char_length(progress)<=4000),
 review_note text not null default '' check(char_length(review_note)<=2000),
 revision bigint not null default 0, created_by uuid not null, updated_by uuid not null,
 created_at timestamptz not null default clock_timestamp(), updated_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,employee_id) references public.employees(company_id,id)
);
create index work_tasks_employee_due on hris_private.work_tasks(company_id,employee_id,due_on,id);

create table hris_private.daily_work_reports (
 id uuid primary key, company_id uuid not null references public.companies(id), employee_id uuid not null,
 work_date date not null, status text not null default 'draft' check(status in ('draft','submitted','returned','approved')),
 revision bigint not null default 0, submission integer not null default 0,
 body jsonb not null, task_ids uuid[] not null default '{}', review_note text not null default '', updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,employee_id,work_date), foreign key(company_id,employee_id) references public.employees(company_id,id)
);
create index daily_work_reports_company_date on hris_private.daily_work_reports(company_id,work_date desc,id);
create table hris_private.work_report_submissions (
 report_id uuid not null references hris_private.daily_work_reports(id), version integer not null,
 company_id uuid not null, employee_id uuid not null, body jsonb not null, task_snapshot jsonb not null,
 submitted_by uuid not null, submitted_at timestamptz not null default clock_timestamp(), primary key(report_id,version)
);
create table hris_private.work_activity_events (
 id bigint generated always as identity primary key, company_id uuid not null, employee_id uuid not null,
 record_id uuid not null, kind text not null, actor_id uuid not null, body jsonb not null,
 created_at timestamptz not null default clock_timestamp()
);
create index work_activity_events_record on hris_private.work_activity_events(company_id,record_id,id);
create table hris_private.work_activity_receipts (
 id uuid primary key, company_id uuid not null, actor_id uuid not null, request jsonb not null, result jsonb not null
);

do $$ declare n text; begin
 foreach n in array array['work_arrangements','work_tasks','daily_work_reports','work_report_submissions','work_activity_events','work_activity_receipts'] loop
  execute format('alter table hris_private.%I enable row level security',n);
  execute format('revoke all on hris_private.%I from public,anon,authenticated,service_role',n);
 end loop;
end $$;

create function hris_private.work_activity_member() returns public.company_members
language plpgsql security definer set search_path='' as $$
declare m public.company_members;
begin
 select c.* into m from public.company_members c join public.employees e on e.company_id=c.company_id and e.id=c.employee_id
 where c.user_id=(select auth.uid()) and c.active and e.status='active';
 if not found or not hris_private.has_staff_capability(m.company_id,'own_records') then
  raise exception 'Current employee access is required.' using errcode='42501'; end if;
 return m;
end $$;
create function hris_private.work_activity_scope(cid uuid,eid uuid,reviewing boolean default false) returns boolean
language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.company_members m join public.employees own on own.company_id=m.company_id and own.id=m.employee_id
 join public.employees e on e.company_id=m.company_id and e.id=eid
 where m.company_id=cid and m.user_id=(select auth.uid()) and m.active and own.status='active'
 and hris_private.has_staff_capability(cid,'own_records')
 and (not reviewing or (m.employee_id is distinct from eid and e.status='active'))
 and ((not reviewing and m.employee_id=eid)
 or hris_private.has_staff_capability(cid,'attendance_administration')
 or (hris_private.has_staff_capability(cid,'team_review') and e.status='active' and e.reports_to=m.employee_id)))
$$;

create function hris_private.work_arrangement_at(cid uuid,eid uuid,day date) returns jsonb
language sql stable security definer set search_path='' as $$
 select to_jsonb(p)-'created_by'-'reason' from hris_private.work_arrangements p
 where company_id=cid and employee_id=eid and enabled and starts_on<=day and (ends_on is null or ends_on>=day)
 order by starts_on desc,sequence desc limit 1
$$;

create function hris_private.work_activity_context(p_own boolean default false) returns jsonb
language plpgsql security definer set search_path='' as $$
declare m public.company_members:=hris_private.work_activity_member();
begin
 return jsonb_build_object('version','workforce-attendance-v1','company_id',m.company_id,'user_id',m.user_id,'employee_id',m.employee_id,
 'own_only',p_own,'can_administer',not p_own and hris_private.has_staff_capability(m.company_id,'attendance_administration'),
 'policies',coalesce((select jsonb_agg(to_jsonb(p) order by p.starts_on desc,p.sequence desc) from hris_private.work_arrangements p
   where p.company_id=m.company_id and (not p_own or p.employee_id=m.employee_id) and hris_private.work_activity_scope(m.company_id,p.employee_id)), '[]'),
 'tasks',coalesce((select jsonb_agg(to_jsonb(t) order by t.due_on,t.id) from hris_private.work_tasks t
   where t.company_id=m.company_id and (not p_own or t.employee_id=m.employee_id) and hris_private.work_activity_scope(m.company_id,t.employee_id)), '[]'),
 'reports',coalesce((select jsonb_agg(to_jsonb(r) order by r.work_date desc,r.id) from hris_private.daily_work_reports r
   where r.company_id=m.company_id and (not p_own or r.employee_id=m.employee_id) and hris_private.work_activity_scope(m.company_id,r.employee_id)), '[]'),
 'submissions',coalesce((select jsonb_agg(to_jsonb(s) order by s.submitted_at desc) from hris_private.work_report_submissions s
   where s.company_id=m.company_id and (not p_own or s.employee_id=m.employee_id) and hris_private.work_activity_scope(m.company_id,s.employee_id)), '[]'),
 'events',coalesce((select jsonb_agg(to_jsonb(v) order by v.id desc) from (select e.* from hris_private.work_activity_events e
   where e.company_id=m.company_id and (not p_own or e.employee_id=m.employee_id) and hris_private.work_activity_scope(m.company_id,e.employee_id)
   order by e.id desc limit 1000) v),'[]'));
end $$;

create function hris_private.work_activity_change(p_request jsonb) returns jsonb
language plpgsql security definer set search_path='' as $$
declare m public.company_members:=hris_private.work_activity_member(); op text:=p_request->>'operation';
 rid uuid:=(p_request->>'request_id')::uuid; ident uuid:=(p_request->>'id')::uuid; eid uuid; rev bigint;
 t hris_private.work_tasks; r hris_private.daily_work_reports; receipt hris_private.work_activity_receipts;
 v_body jsonb; result jsonb; v_status text; v_note text; policy jsonb; today date; v_task_ids uuid[]; task_snapshot jsonb; item text;station_ready boolean;
begin
 if jsonb_typeof(p_request)<>'object' or octet_length(p_request::text)>60000 or rid is null or ident is null then
  raise exception 'Use a bounded work request with stable identifiers.' using errcode='23514'; end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,520));
 select * into receipt from hris_private.work_activity_receipts where id=rid;
 if found then
  if receipt.company_id<>m.company_id or receipt.actor_id<>m.user_id or receipt.request<>p_request then
   raise exception 'This request identifier belongs to another operation.' using errcode='42501'; end if;
  if op='policy_save'and not hris_private.has_staff_capability(m.company_id,'attendance_administration')then raise exception 'Attendance permissions changed.'using errcode='42501';end if;
  if op in('task_save','task_progress','task_review')then select source.employee_id into eid from hris_private.work_tasks source where source.id=ident;
  elsif op in('report_save','report_submit','report_review')then select source.employee_id into eid from hris_private.daily_work_reports source where source.id=ident;end if;
  if op in('task_save','task_review','report_review')and not hris_private.work_activity_scope(m.company_id,eid,true)then raise exception 'Your review scope changed.'using errcode='42501';end if;
  if op in('task_progress','report_save','report_submit')and eid is distinct from m.employee_id then raise exception 'Your employee access changed.'using errcode='42501';end if;
  return receipt.result;
 end if;
 select (clock_timestamp() at time zone c.timezone)::date into today from public.companies c where c.id=m.company_id;

 if op='policy_save' then
  eid:=(p_request->>'employee_id')::uuid;
  if not hris_private.has_staff_capability(m.company_id,'attendance_administration') then
   raise exception 'HR attendance administration is required.' using errcode='42501'; end if;
  if not exists(select 1 from public.employees where company_id=m.company_id and id=eid and status='active')
   or (p_request->>'starts_on')::date<today then
   raise exception 'Choose an active employee and a current or future effective date.' using errcode='23514'; end if;
  if coalesce((p_request->>'enabled')::boolean,false) and p_request->>'arrangement' in ('onsite','hybrid')then
   if to_regclass('hris_private.attendance_stations')is null then raise exception 'Keep office policies in draft until the kiosk is verified.'using errcode='23514';end if;
   execute 'select exists(select 1 from hris_private.attendance_stations where company_id=$1 and active and verified_at is not null)'into station_ready using m.company_id;
   if not station_ready then raise exception 'Record and verify a station punch before activating office policies.'using errcode='23514';end if;
  end if;
  insert into hris_private.work_arrangements(id,company_id,employee_id,starts_on,ends_on,arrangement,office_days,enabled,reason,created_by)
  values(ident,m.company_id,eid,(p_request->>'starts_on')::date,nullif(p_request->>'ends_on','')::date,p_request->>'arrangement',
   array(select jsonb_array_elements_text(coalesce(p_request->'office_days','[1,2,3,4,5]'))::integer),coalesce((p_request->>'enabled')::boolean,false),btrim(p_request->>'reason'),m.user_id);
  result:=jsonb_build_object('id',ident,'revision',0);
 elsif op='task_save' then
  eid:=(p_request->>'employee_id')::uuid;
  if not hris_private.work_activity_scope(m.company_id,eid,true) then raise exception 'Assign only within your current authorized team.' using errcode='42501'; end if;
  select * into t from hris_private.work_tasks where id=ident for update;
  if found then
   if t.company_id<>m.company_id or t.employee_id<>eid then raise exception 'Task scope changed.' using errcode='42501'; end if;
   if t.revision is distinct from (p_request->>'expected_revision')::bigint or t.status='accepted' then raise exception 'Task changed or is already accepted.' using errcode='PT409'; end if;
   update hris_private.work_tasks set title=btrim(p_request->>'title'),outcome=btrim(p_request->>'outcome'),due_on=(p_request->>'due_on')::date,
    priority=p_request->>'priority',revision=revision+1,updated_by=m.user_id,updated_at=clock_timestamp() where id=ident returning revision into rev;
  else
   if (p_request->>'expected_revision')::bigint is distinct from -1 then raise exception 'Task baseline changed.' using errcode='PT409'; end if;
   insert into hris_private.work_tasks(id,company_id,employee_id,title,outcome,due_on,priority,created_by,updated_by)
   values(ident,m.company_id,eid,btrim(p_request->>'title'),btrim(p_request->>'outcome'),(p_request->>'due_on')::date,p_request->>'priority',m.user_id,m.user_id);
   rev:=0;
  end if;
  result:=jsonb_build_object('id',ident,'revision',rev);
 elsif op in ('task_progress','task_review') then
  select * into t from hris_private.work_tasks where id=ident for update;
  if not found or t.company_id<>m.company_id then raise exception 'Task unavailable.' using errcode='42501'; end if; eid:=t.employee_id;
  if t.revision is distinct from (p_request->>'expected_revision')::bigint or t.status='accepted' then raise exception 'Task changed or is already accepted.' using errcode='PT409'; end if;
  v_status:=p_request->>'status'; v_note:=btrim(coalesce(p_request->>'note',''));
  if op='task_progress' then
   if m.employee_id<>eid or v_status not in ('in_progress','submitted') then raise exception 'Update only your own task progress.' using errcode='42501'; end if;
   update hris_private.work_tasks set status=v_status,progress=v_note,revision=revision+1,updated_by=m.user_id,updated_at=clock_timestamp() where id=ident returning revision into rev;
  else
   if not hris_private.work_activity_scope(m.company_id,eid,true) or v_status not in ('accepted','returned') then raise exception 'Independent current-team review is required.' using errcode='42501'; end if;
   if t.status<>'submitted' or char_length(v_note) not between 1 and 2000 then raise exception 'Review a submitted task and give a reason.' using errcode='23514'; end if;
   update hris_private.work_tasks set status=v_status,review_note=v_note,revision=revision+1,updated_by=m.user_id,updated_at=clock_timestamp() where id=ident returning revision into rev;
  end if;
  result:=jsonb_build_object('id',ident,'revision',rev);
 elsif op in ('report_save','report_submit','report_review') then
  select * into r from hris_private.daily_work_reports where id=ident for update;
  if not found then
   if op='report_review' or (p_request->>'expected_revision')::bigint is distinct from -1 then raise exception 'Report baseline changed.' using errcode='PT409'; end if;
   eid:=m.employee_id;
   if (p_request->>'work_date')::date>today or (p_request->>'work_date')::date<today-93 then raise exception 'Choose a work date within the recent 93 days.' using errcode='23514'; end if;
   if exists(select 1 from public.employees where id=eid and joined_on>(p_request->>'work_date')::date) then raise exception 'A report cannot precede employment.' using errcode='23514'; end if;
   insert into hris_private.daily_work_reports(id,company_id,employee_id,work_date,body)
   values(ident,m.company_id,eid,(p_request->>'work_date')::date,'{}') returning * into r;
  else
   if r.company_id<>m.company_id then raise exception 'Report unavailable.' using errcode='42501'; end if;
   if r.revision is distinct from (p_request->>'expected_revision')::bigint then raise exception 'Report changed. Refresh before editing.' using errcode='PT409'; end if;
   eid:=r.employee_id;
  end if;
  if op='report_review' then
   v_status:=p_request->>'status';v_note:=btrim(coalesce(p_request->>'note',''));
   if not hris_private.work_activity_scope(m.company_id,eid,true) then raise exception 'Independent current-team review is required.' using errcode='42501'; end if;
   if r.status<>'submitted' or v_status not in ('approved','returned') or char_length(v_note) not between 1 and 2000 then raise exception 'Review a submitted report and give a reason.' using errcode='23514'; end if;
   update hris_private.daily_work_reports set status=v_status,review_note=v_note,revision=revision+1,updated_at=clock_timestamp() where id=ident returning revision into rev;
  else
   if eid<>m.employee_id then raise exception 'Edit only your own report.' using errcode='42501'; end if;
   if r.status not in ('draft','returned') then raise exception 'A submitted or approved report cannot be edited.' using errcode='PT409'; end if;
   v_body:=p_request->'body';
   if jsonb_typeof(v_body) is distinct from 'object' or octet_length(v_body::text)>20000 then raise exception 'Enter a bounded daily report.' using errcode='23514'; end if;
   foreach item in array array['summary','completed','unfinished','blockers','next_steps'] loop
    if jsonb_typeof(v_body->item) is distinct from 'string' or char_length(v_body->>item)>4000 then raise exception 'Use the five daily-report text fields.' using errcode='23514'; end if;
   end loop;
   if (select count(*) from jsonb_object_keys(v_body))<>5 or (op='report_submit' and char_length(btrim(v_body->>'summary'))=0) then raise exception 'Complete the work summary before submission.' using errcode='23514'; end if;
   if jsonb_typeof(coalesce(p_request->'task_ids','[]'))<>'array' or jsonb_array_length(coalesce(p_request->'task_ids','[]'))>100 then raise exception 'Include at most 100 own tasks.' using errcode='23514'; end if;
   v_task_ids:=array(select jsonb_array_elements_text(coalesce(p_request->'task_ids','[]'))::uuid);
   if cardinality(v_task_ids)<>(select count(distinct v) from unnest(v_task_ids)v) or exists(select 1 from unnest(v_task_ids)v where not exists(select 1 from hris_private.work_tasks task_ref where task_ref.id=v and task_ref.company_id=m.company_id and task_ref.employee_id=eid)) then raise exception 'A task reference is outside your own work.' using errcode='42501'; end if;
   select coalesce(jsonb_agg(to_jsonb(task_source) order by task_source.id),'[]') into task_snapshot from hris_private.work_tasks task_source where task_source.id=any(v_task_ids);
   update hris_private.daily_work_reports set body=v_body,task_ids=v_task_ids,status=case when op='report_submit' then 'submitted' else 'draft' end,
    submission=submission+case when op='report_submit' then 1 else 0 end,review_note='',revision=revision+1,updated_at=clock_timestamp() where id=ident returning * into r;
   if op='report_submit' then insert into hris_private.work_report_submissions(report_id,version,company_id,employee_id,body,task_snapshot,submitted_by)
    values(ident,r.submission,m.company_id,eid,v_body,task_snapshot,m.user_id); end if;
   rev:=r.revision;
  end if;
  result:=jsonb_build_object('id',ident,'revision',rev);
 else raise exception 'Unknown work operation.' using errcode='23514';
 end if;
 insert into hris_private.work_activity_events(company_id,employee_id,record_id,kind,actor_id,body)
 values(m.company_id,eid,ident,op,m.user_id,jsonb_build_object('request',p_request-'request_id','result',result));
 insert into hris_private.work_activity_receipts values(rid,m.company_id,m.user_id,p_request,result);
 return result;
end $$;

create function public.work_activity_context(p_own boolean default false) returns jsonb language sql security invoker set search_path='' as $$select hris_private.work_activity_context(p_own);$$;
create function public.work_activity_change(p_request jsonb) returns jsonb language sql security invoker set search_path='' as $$select hris_private.work_activity_change(p_request);$$;
revoke all on function hris_private.work_activity_member(),hris_private.work_activity_scope(uuid,uuid,boolean),hris_private.work_arrangement_at(uuid,uuid,date),hris_private.work_activity_context(boolean),hris_private.work_activity_change(jsonb),public.work_activity_context(boolean),public.work_activity_change(jsonb) from public,anon,authenticated,service_role;
grant execute on function hris_private.work_activity_member(),hris_private.work_activity_scope(uuid,uuid,boolean),hris_private.work_activity_context(boolean),hris_private.work_activity_change(jsonb),public.work_activity_context(boolean),public.work_activity_change(jsonb) to authenticated;
comment on function public.work_activity_change(jsonb) is 'Dated work policies, own reports and independent current-team reviews; no attendance or financial side effects.';
commit;
