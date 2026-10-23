-- Shared career coaching only: no employment, payroll or source learning/performance writes.
begin;
create table hris_private.career_birth(
 company_id uuid not null references public.companies(id),kind text not null check(kind in('plan','milestone')),
 record_id uuid not null,actor_id uuid not null references auth.users(id),payload jsonb not null,
 primary key(company_id,kind,record_id)
);
create index career_birth_actor_idx on hris_private.career_birth(actor_id);
alter table hris_private.career_birth enable row level security;
revoke all on hris_private.career_birth from public,anon,authenticated;
create table public.career_plans(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid not null,employee_name text not null,employee_code text not null,
 title text not null check(title=btrim(title)and char_length(title)between 1 and 120 and title!~'[[:cntrl:]]'),
 target_role text not null default ''check(char_length(target_role)<=120 and target_role!~'[[:cntrl:]]'),
 target_department text not null default ''check(char_length(target_department)<=120 and target_department!~'[[:cntrl:]]'),
 skills_focus text not null default ''check(char_length(skills_focus)<=2000),shared_notes text not null default ''check(char_length(shared_notes)<=2000),
 starts_on date not null check(starts_on between date '1900-01-01'and date '2200-12-31'),
 target_on date check(target_on between date '1900-01-01'and date '2200-12-31'and target_on>=starts_on),
 status text not null default 'active'check(status in('active','completed','cancelled')),
 completed_on date check(completed_on between date '1900-01-01'and date '2200-12-31'and completed_on>=starts_on),
 completion_summary text not null default ''check(char_length(completion_summary)<=2000),completed_by uuid references auth.users(id),
 milestones_snapshot jsonb not null default '[]'check(jsonb_typeof(milestones_snapshot)='array'and jsonb_array_length(milestones_snapshot)<=50),
 revision bigint not null default 0 check(revision>=0),created_by uuid not null references auth.users(id),created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),unique(company_id,employee_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id),
 check((status='completed'and completed_on is not null and completed_by is not null and btrim(completion_summary)<>'')or(status<>'completed'and completed_on is null and completed_by is null and completion_summary=''and milestones_snapshot='[]'::jsonb))
);
create index career_plans_employee_idx on public.career_plans(company_id,employee_id,status);
create index career_plans_creator_idx on public.career_plans(created_by);
create index career_plans_completer_idx on public.career_plans(completed_by);
create table public.career_milestones(
 id uuid primary key,company_id uuid not null,employee_id uuid not null,plan_id uuid not null,
 title text not null check(title=btrim(title)and char_length(title)between 1 and 120 and title!~'[[:cntrl:]]'),
 action_steps text not null default ''check(char_length(action_steps)<=2000),
 due_on date check(due_on between date '1900-01-01'and date '2200-12-31'),required boolean not null default true,
 goal_id uuid,training_id uuid,certificate_id uuid,check(num_nonnulls(goal_id,training_id,certificate_id)<=1),
 status text not null default 'open'check(status in('open','done','waived')),
 progress_note text not null default ''check(char_length(progress_note)<=1000),review_result text not null default ''check(char_length(review_result)<=2000),
 completed_on date check(completed_on between date '1900-01-01'and date '2200-12-31'),reviewed_by uuid references auth.users(id),
 revision bigint not null default 0 check(revision>=0),created_by uuid not null references auth.users(id),created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),unique(company_id,plan_id,id),foreign key(company_id,employee_id,plan_id)references public.career_plans(company_id,employee_id,id),
 foreign key(company_id,goal_id)references public.performance_goals(company_id,id),
 foreign key(company_id,training_id)references public.employee_training(company_id,id),
 foreign key(company_id,certificate_id)references public.employee_certifications(company_id,id),
 check((status='open'and completed_on is null and reviewed_by is null and review_result='')or(status='done'and completed_on is not null and reviewed_by is not null and btrim(review_result)<>'')or(status='waived'and completed_on is null and reviewed_by is not null and btrim(review_result)<>''))
);
create index career_milestones_plan_idx on public.career_milestones(company_id,employee_id,plan_id);
create index career_milestones_plan_status_idx on public.career_milestones(company_id,plan_id,status);
create index career_milestones_goal_idx on public.career_milestones(company_id,goal_id);
create index career_milestones_training_idx on public.career_milestones(company_id,training_id);
create index career_milestones_certificate_idx on public.career_milestones(company_id,certificate_id);
create index career_milestones_creator_idx on public.career_milestones(created_by);
create index career_milestones_reviewer_idx on public.career_milestones(reviewed_by);
create table public.career_events(
 id uuid primary key default gen_random_uuid(),company_id uuid not null references public.companies(id),plan_id uuid not null,milestone_id uuid,
 kind text not null,reason text not null check(char_length(reason)between 1 and 500),actor_id uuid not null references auth.users(id),actor_name text not null,actor_role text not null,
 before_record jsonb,after_record jsonb not null,recorded_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,plan_id)references public.career_plans(company_id,id),
 foreign key(company_id,plan_id,milestone_id)references public.career_milestones(company_id,plan_id,id)
);
create index career_events_plan_idx on public.career_events(company_id,plan_id,recorded_at);
create index career_events_milestone_idx on public.career_events(company_id,plan_id,milestone_id);
create index career_events_actor_idx on public.career_events(actor_id);
create trigger career_events_immutable before update or delete on public.career_events for each row execute function hris_private.employment_history_immutable();
create function hris_private.career_scope(p_company uuid,p_employee uuid)returns boolean language sql stable security definer set search_path=''as $$
 select exists(select 1 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=p_employee where m.company_id=p_company and m.user_id=(select auth.uid())and m.active and(m.role in('administrator','hr_generalist')or m.employee_id=e.id or(m.role='manager'and e.status='active'and e.reports_to=m.employee_id)));
$$;
create function hris_private.career_manage(p_company uuid,p_employee uuid)returns boolean language sql stable security definer set search_path=''as $$
 select exists(select 1 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=p_employee where m.company_id=p_company and m.user_id=(select auth.uid())and m.active and m.employee_id is not null and m.employee_id<>e.id and(m.role in('administrator','hr_generalist')or(m.role='manager'and e.status='active'and e.reports_to=m.employee_id)));
$$;
create function hris_private.career_plan_read(p_company uuid,p_plan uuid)returns boolean language sql stable security definer set search_path=''as $$
 select exists(select 1 from public.career_plans p where p.company_id=p_company and p.id=p_plan and hris_private.career_scope(p_company,p.employee_id));
$$;
revoke all on function hris_private.career_scope(uuid,uuid),hris_private.career_manage(uuid,uuid),hris_private.career_plan_read(uuid,uuid)from public,anon,authenticated;
grant execute on function hris_private.career_scope(uuid,uuid),hris_private.career_manage(uuid,uuid),hris_private.career_plan_read(uuid,uuid)to authenticated;
alter table public.career_plans enable row level security;
alter table public.career_milestones enable row level security;
alter table public.career_events enable row level security;
revoke all on public.career_plans,public.career_milestones,public.career_events from public,anon,authenticated;
grant select on public.career_plans,public.career_milestones,public.career_events to authenticated;
create policy career_plans_read on public.career_plans for select to authenticated using(hris_private.career_scope(company_id,employee_id));
create policy career_milestones_read on public.career_milestones for select to authenticated using(hris_private.career_plan_read(company_id,plan_id));
create policy career_events_read on public.career_events for select to authenticated using(hris_private.career_plan_read(company_id,plan_id));
create function hris_private.career_retry(p_company uuid,p_kind text,p_id uuid,p_actor uuid,p_record jsonb)returns boolean language plpgsql security invoker set search_path=''as $$
declare b hris_private.career_birth;begin
 select * into b from hris_private.career_birth where company_id=p_company and kind=p_kind and record_id=p_id;
 if b.record_id is null then return false;end if;
 if b.actor_id=p_actor and b.payload=p_record then return true;end if;
 raise exception 'This record ID already exists. Refresh and reopen it.'using errcode='PT409';
end;$$;
revoke all on function hris_private.career_retry(uuid,text,uuid,uuid,jsonb)from public,anon,authenticated;
create function hris_private.career_reference(p_company uuid,p_employee uuid,p_goal uuid,p_training uuid,p_certificate uuid)returns jsonb language plpgsql security invoker set search_path=''as $$
declare result jsonb;begin
 if num_nonnulls(p_goal,p_training,p_certificate)>1 then raise exception 'Choose at most one related goal, training or certificate.'using errcode='23514';end if;
 if p_goal is not null then select jsonb_build_object('kind','goal','id',id,'title',title,'status',status,'progress',progress,'due_on',due_on)into result from public.performance_goals where company_id=p_company and employee_id=p_employee and id=p_goal;
 elsif p_training is not null then select jsonb_build_object('kind','training','id',id,'title',title,'status',status,'completed_on',completed_on,'learning_minutes',learning_minutes)into result from public.employee_training where company_id=p_company and employee_id=p_employee and id=p_training;
 elsif p_certificate is not null then select jsonb_build_object('kind','certificate','id',id,'title',title,'issuer',issuer,'status',status,'archived',archived,'issued_on',issued_on,'expires_on',expires_on)into result from public.employee_certifications where company_id=p_company and employee_id=p_employee and id=p_certificate;
 else return null;end if;
 if result is null then raise exception 'Choose a related record for the same employee and company.'using errcode='23514';end if;return result;
end;$$;
revoke all on function hris_private.career_reference(uuid,uuid,uuid,uuid,uuid)from public,anon,authenticated;
create function hris_private.manage_career(p_operation text,p_record jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;person public.employees;p public.career_plans;task public.career_milestones;
 v_id uuid;expected bigint;why text;actor_name text;today date;old_record jsonb;result jsonb;kind text;next_status text;create_kind text;event_plan uuid;event_milestone uuid;snapshot jsonb;deadline date;
begin
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist','manager');
 if m.user_id is null then raise exception 'Active desktop staff access is required.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));perform 1 from public.companies where id=m.company_id for update;
 select * into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist','manager')for update;
 if m.user_id is null then raise exception 'Your access changed. Refresh.'using errcode='42501';end if;
 select full_name into actor_name from public.employees where company_id=m.company_id and id=m.employee_id;
 select(now()at time zone timezone)::date into today from public.companies where id=m.company_id;
 if jsonb_typeof(p_record)is distinct from 'object'or octet_length(p_record::text)>32768 then raise exception 'Enter valid bounded career details.'using errcode='23514';end if;
 v_id:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=btrim(p_record->>'reason');
 if v_id is null or why is null or char_length(why)not between 1 and 500 or why~'[[:cntrl:]]'then raise exception 'Choose a record and enter a reason of 1-500 characters.'using errcode='23514';end if;
 if p_operation='plan_save'then
  if p_record-array['id','expected_revision','employee_id','title','target_role','target_department','skills_focus','shared_notes','starts_on','target_on','reason']<>'{}'::jsonb then raise exception 'Unsupported career plan fields.'using errcode='23514';end if;
  select * into person from public.employees where company_id=m.company_id and id=(p_record->>'employee_id')::uuid;
  if person.id is null or hris_private.career_manage(m.company_id,person.id)is not true then raise exception 'Another HR user or the current reporting Manager manages this plan.'using errcode='42501';end if;
  select * into p from public.career_plans where company_id=m.company_id and id=v_id for update;
  if p.id is not null and expected is null then if hris_private.career_retry(m.company_id,'plan',v_id,m.user_id,p_record)then return to_jsonb(p);end if;raise exception 'Plan ID exists.'using errcode='PT409';end if;
  if person.status<>'active'then raise exception 'Plan changes require an active employee.'using errcode='23514';end if;
  if p.id is null then
   if expected is not null then raise exception 'Plan not found. Refresh.'using errcode='PT409';end if;
   if person.status<>'active'then raise exception 'New plans require an active employee.'using errcode='23514';end if;
   if (p_record->>'starts_on')::date<person.joined_on then raise exception 'Plan start cannot precede joining.'using errcode='23514';end if;
   insert into public.career_plans(id,company_id,employee_id,employee_name,employee_code,title,target_role,target_department,skills_focus,shared_notes,starts_on,target_on,created_by)
   values(v_id,m.company_id,person.id,person.full_name,person.employee_code,btrim(p_record->>'title'),btrim(coalesce(p_record->>'target_role','')),btrim(coalesce(p_record->>'target_department','')),btrim(coalesce(p_record->>'skills_focus','')),btrim(coalesce(p_record->>'shared_notes','')),(p_record->>'starts_on')::date,nullif(p_record->>'target_on','')::date,m.user_id)returning * into p;create_kind:='plan';kind:='plan_created';
  else
   if p.revision is distinct from expected then raise exception 'Plan changed. Refresh and reopen it.'using errcode='PT409';end if;
   if p.status<>'active'or p.employee_id<>person.id then raise exception 'Reopen the same employee plan before editing.'using errcode='23514';end if;
   if (p_record->>'starts_on')::date<person.joined_on or exists(select 1 from public.career_milestones t where t.company_id=m.company_id and t.plan_id=p.id and(t.due_on<(p_record->>'starts_on')::date or t.due_on>nullif(p_record->>'target_on','')::date or t.completed_on<(p_record->>'starts_on')::date))then raise exception 'Plan dates must include joining and all saved milestone dates.'using errcode='23514';end if;
   old_record:=to_jsonb(p);update public.career_plans set title=btrim(p_record->>'title'),target_role=btrim(coalesce(p_record->>'target_role','')),target_department=btrim(coalesce(p_record->>'target_department','')),skills_focus=btrim(coalesce(p_record->>'skills_focus','')),shared_notes=btrim(coalesce(p_record->>'shared_notes','')),starts_on=(p_record->>'starts_on')::date,target_on=nullif(p_record->>'target_on','')::date,revision=revision+1,updated_at=clock_timestamp()where id=p.id returning * into p;kind:='plan_edited';
  end if;
  result:=to_jsonb(p);event_plan:=p.id;
 elsif p_operation='plan_status'then
  if p_record-array['id','expected_revision','status','completed_on','completion_summary','reason']<>'{}'::jsonb then raise exception 'Unsupported plan decision fields.'using errcode='23514';end if;
  select * into p from public.career_plans where company_id=m.company_id and id=v_id for update;
  if p.id is null or hris_private.career_manage(m.company_id,p.employee_id)is not true then raise exception 'Another HR user or current reporting Manager reviews this plan.'using errcode='42501';end if;
  if p.revision is distinct from expected then raise exception 'Plan changed. Refresh.'using errcode='PT409';end if;
  next_status:=p_record->>'status';if next_status is null or not((p.status='active'and next_status in('completed','cancelled'))or(p.status in('completed','cancelled')and next_status='active'))then raise exception 'Choose an available plan transition.'using errcode='23514';end if;
  select * into person from public.employees where company_id=m.company_id and id=p.employee_id;
  if next_status<>'cancelled'and person.status<>'active'then raise exception 'Archived employees cannot complete or reopen plans.'using errcode='23514';end if;
  old_record:=to_jsonb(p);
  if next_status='completed'then
   deadline:=nullif(p_record->>'completed_on','')::date;
   if deadline is null or deadline>today or deadline<p.starts_on or btrim(coalesce(p_record->>'completion_summary',''))=''then raise exception 'Completion needs a factual date within the plan period and a review summary.'using errcode='23514';end if;
   if exists(select 1 from public.career_milestones t where t.company_id=m.company_id and t.plan_id=p.id and t.required and t.status='open')or not exists(select 1 from public.career_milestones t where t.company_id=m.company_id and t.plan_id=p.id and t.status='done')then raise exception 'Resolve required milestones and record at least one completed milestone.'using errcode='23514';end if;
   if exists(select 1 from public.career_milestones t where t.company_id=m.company_id and t.plan_id=p.id and t.completed_on>deadline)then raise exception 'Plan completion cannot precede a completed milestone.'using errcode='23514';end if;
   select coalesce(jsonb_agg(jsonb_build_object('id',t.id,'title',t.title,'action_steps',t.action_steps,'due_on',t.due_on,'required',t.required,'status',t.status,'progress_note',t.progress_note,'review_result',t.review_result,'completed_on',t.completed_on,'revision',t.revision,'reference',hris_private.career_reference(t.company_id,t.employee_id,t.goal_id,t.training_id,t.certificate_id))order by t.due_on nulls last,t.created_at,t.id),'[]')into snapshot from public.career_milestones t where t.company_id=m.company_id and t.plan_id=p.id;

  else
   if nullif(p_record->>'completed_on','')is not null or coalesce(p_record->>'completion_summary','')<>''then raise exception 'Completion fields belong only to completed plans.'using errcode='23514';end if;

  end if;
  update public.career_plans set status=next_status,completed_on=case when next_status='completed'then deadline end,completion_summary=case when next_status='completed'then btrim(p_record->>'completion_summary')else ''end,completed_by=case when next_status='completed'then m.user_id end,milestones_snapshot=case when next_status='completed'then snapshot else '[]'::jsonb end,revision=revision+1,updated_at=clock_timestamp()where id=p.id returning * into p;result:=to_jsonb(p);event_plan:=p.id;kind:='plan_'||next_status;
 elsif p_operation='milestone_save'then
  if p_record-array['id','expected_revision','plan_id','title','action_steps','due_on','required','goal_id','training_id','certificate_id','reason']<>'{}'::jsonb then raise exception 'Unsupported milestone fields.'using errcode='23514';end if;
  select * into p from public.career_plans where company_id=m.company_id and id=(p_record->>'plan_id')::uuid for update;
  if p.id is null or hris_private.career_manage(m.company_id,p.employee_id)is not true then raise exception 'Another HR user or current reporting Manager manages these milestones.'using errcode='42501';end if;
  select * into task from public.career_milestones where company_id=m.company_id and id=v_id for update;
  if task.id is not null and expected is null then if hris_private.career_retry(m.company_id,'milestone',v_id,m.user_id,p_record)then return to_jsonb(task);end if;raise exception 'Milestone ID exists.'using errcode='PT409';end if;
  if p.status<>'active'then raise exception 'Reopen the plan before editing milestones.'using errcode='23514';end if;
  select * into person from public.employees where company_id=m.company_id and id=p.employee_id;
  if person.status<>'active'then raise exception 'Milestone changes require an active employee.'using errcode='23514';end if;
  deadline:=nullif(p_record->>'due_on','')::date;
  if deadline<p.starts_on or deadline>p.target_on then raise exception 'Milestone due date must fall within the plan horizon.'using errcode='23514';end if;
  perform hris_private.career_reference(m.company_id,p.employee_id,(p_record->>'goal_id')::uuid,(p_record->>'training_id')::uuid,(p_record->>'certificate_id')::uuid);
  if task.id is null then
   if expected is not null then raise exception 'Milestone not found. Refresh.'using errcode='PT409';end if;
   select * into person from public.employees where company_id=m.company_id and id=p.employee_id;
   if person.status<>'active'then raise exception 'New milestones require an active employee.'using errcode='23514';end if;
   if(select count(*)from public.career_milestones where company_id=m.company_id and plan_id=p.id)>=50 then raise exception 'A plan can hold at most 50 milestones. Use a follow-up plan.'using errcode='23514';end if;
   insert into public.career_milestones(id,company_id,employee_id,plan_id,title,action_steps,due_on,required,goal_id,training_id,certificate_id,created_by)
   values(v_id,m.company_id,p.employee_id,p.id,btrim(p_record->>'title'),btrim(coalesce(p_record->>'action_steps','')),deadline,coalesce((p_record->>'required')::boolean,true),(p_record->>'goal_id')::uuid,(p_record->>'training_id')::uuid,(p_record->>'certificate_id')::uuid,m.user_id)returning * into task;create_kind:='milestone';kind:='milestone_created';
  else
   if task.revision is distinct from expected then raise exception 'Milestone changed. Refresh.'using errcode='PT409';end if;
   if task.plan_id<>p.id or task.status<>'open'then raise exception 'Reopen the same plan milestone before editing.'using errcode='23514';end if;
   old_record:=to_jsonb(task);update public.career_milestones set title=btrim(p_record->>'title'),action_steps=btrim(coalesce(p_record->>'action_steps','')),due_on=deadline,required=coalesce((p_record->>'required')::boolean,true),goal_id=(p_record->>'goal_id')::uuid,training_id=(p_record->>'training_id')::uuid,certificate_id=(p_record->>'certificate_id')::uuid,revision=revision+1,updated_at=clock_timestamp()where id=task.id returning * into task;kind:='milestone_edited';
  end if;
  result:=to_jsonb(task);event_plan:=p.id;event_milestone:=task.id;
 elsif p_operation in('milestone_progress','milestone_status')then
  if p_record-(case when p_operation='milestone_progress'then array['id','expected_revision','progress_note','reason']else array['id','expected_revision','status','completed_on','review_result','reason']end)<>'{}'::jsonb then raise exception 'Unsupported milestone progress/decision fields.'using errcode='23514';end if;
  select * into task from public.career_milestones where company_id=m.company_id and id=v_id for update;
  if task.id is null or hris_private.career_scope(m.company_id,task.employee_id)is not true then raise exception 'Milestone is outside your current access.'using errcode='42501';end if;
  select * into p from public.career_plans where company_id=m.company_id and id=task.plan_id for update;
  if task.revision is distinct from expected then raise exception 'Milestone changed. Refresh.'using errcode='PT409';end if;
  if p.status<>'active'then raise exception 'Reopen the plan before changing a milestone.'using errcode='23514';end if;
  select * into person from public.employees where company_id=m.company_id and id=task.employee_id;
  if person.status<>'active'then raise exception 'Archived employees cannot record milestone progress or decisions.'using errcode='23514';end if;
  old_record:=to_jsonb(task);
  if p_operation='milestone_progress'then
   if task.status<>'open'then raise exception 'Only open milestones accept progress.'using errcode='23514';end if;
   update public.career_milestones set progress_note=btrim(coalesce(p_record->>'progress_note','')),revision=revision+1,updated_at=clock_timestamp()where id=task.id returning * into task;kind:='milestone_progress';
  else
   if hris_private.career_manage(m.company_id,task.employee_id)is not true then raise exception 'Another HR user or current Manager reviews your milestone.'using errcode='42501';end if;
   next_status:=p_record->>'status';if next_status is null or not((task.status='open'and next_status in('done','waived'))or(task.status in('done','waived')and next_status='open'))then raise exception 'Choose an available milestone transition.'using errcode='23514';end if;
   if next_status='waived'and m.role not in('administrator','hr_generalist')then raise exception 'Only another HR user can waive a milestone.'using errcode='42501';end if;
   deadline:=nullif(p_record->>'completed_on','')::date;
   if next_status='done'and(deadline is null or deadline>today or deadline<p.starts_on or btrim(coalesce(p_record->>'review_result',''))='')then raise exception 'Completion needs a factual date on/before today and a review result.'using errcode='23514';end if;
   if next_status='waived'and btrim(coalesce(p_record->>'review_result',''))=''then raise exception 'Record the waiver result.'using errcode='23514';end if;
   if next_status<>'done'and deadline is not null or next_status='open'and coalesce(p_record->>'review_result','')<>''then raise exception 'Use completion fields only for a completed milestone.'using errcode='23514';end if;
   update public.career_milestones set status=next_status,completed_on=case when next_status='done'then deadline end,review_result=case when next_status='open'then ''else btrim(p_record->>'review_result')end,reviewed_by=case when next_status='open'then null else m.user_id end,revision=revision+1,updated_at=clock_timestamp()where id=task.id returning * into task;kind:='milestone_'||next_status;
  end if;
  result:=to_jsonb(task);event_plan:=p.id;event_milestone:=task.id;
 else raise exception 'Unknown career operation.'using errcode='23514';end if;
 if create_kind is not null then insert into hris_private.career_birth(company_id,kind,record_id,actor_id,payload)values(m.company_id,create_kind,v_id,m.user_id,p_record);end if;
 insert into public.career_events(company_id,plan_id,milestone_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)values(m.company_id,event_plan,event_milestone,kind,why,m.user_id,actor_name,m.role,old_record,result);
 return result;
end;$$;
revoke all on function hris_private.manage_career(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_career(text,jsonb)to authenticated;
create function public.manage_career(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_career(p_operation,p_record);$$;
revoke all on function public.manage_career(text,jsonb)from public,anon,authenticated;
grant execute on function public.manage_career(text,jsonb)to authenticated;
commit;
