begin;
-- Pay/offer terms remain HR-private. Assigned Managers see only their checklist tasks.
create table public.hiring_offers(
 id uuid primary key,company_id uuid not null references public.companies(id) on delete restrict,
 applicant_id uuid not null,applicant_name text not null,
 job_title text not null check(char_length(job_title) between 1 and 100 and job_title=btrim(job_title) and job_title !~ '[[:cntrl:]]'),
 department text not null default '' check(char_length(department)<=80 and department !~ '[[:cntrl:]]'),
 starts_on date not null check(starts_on between date '1900-01-01' and date '2200-12-31'),
 amount_minor bigint not null check(amount_minor between 0 and 999999999999),
 currency text not null check(currency ~ '^[A-Z]{3}$'),pay_period text not null check(pay_period in ('monthly','annual','hourly')),
 terms text not null default '' check(char_length(terms)<=4000),expires_on date check(expires_on between date '1900-01-01' and date '2200-12-31'),
 status text not null default 'draft' check(status in ('draft','approved','sent','accepted','declined','withdrawn')),
 revision bigint not null default 0 check(revision>=0),birth_payload jsonb not null,
 approved_by uuid references auth.users(id) on delete restrict,approved_at timestamptz,
 created_by uuid not null references auth.users(id) on delete restrict,created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),foreign key(company_id,applicant_id) references public.applicants(company_id,id) on delete restrict,
 check((approved_by is null)=(approved_at is null)),check(status not in ('approved','sent','accepted') or approved_by is not null)
);
create unique index hiring_offers_active_applicant_idx on public.hiring_offers(company_id,applicant_id) where status in ('draft','approved','sent','accepted');
create index hiring_offers_approver_idx on public.hiring_offers(approved_by);
create index hiring_offers_creator_idx on public.hiring_offers(created_by);
create table public.hiring_offer_events(
 id uuid primary key default gen_random_uuid(),company_id uuid not null,offer_id uuid not null,
 kind text not null,reason text not null check(char_length(reason) between 1 and 500),
 actor_id uuid not null references auth.users(id) on delete restrict,actor_name text not null,actor_role text not null,
 before_record jsonb,after_record jsonb not null,recorded_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,offer_id) references public.hiring_offers(company_id,id) on delete restrict
);
create index hiring_offer_events_offer_idx on public.hiring_offer_events(company_id,offer_id,recorded_at desc);
create index hiring_offer_events_actor_idx on public.hiring_offer_events(actor_id);
create table public.hire_conversions(
 id uuid primary key,company_id uuid not null,applicant_id uuid not null,offer_id uuid not null,
 employee_id uuid not null,birth_payload jsonb not null,created_by uuid not null references auth.users(id) on delete restrict,
 created_at timestamptz not null default clock_timestamp(),unique(company_id,applicant_id),unique(company_id,offer_id),unique(company_id,employee_id),
 foreign key(company_id,applicant_id) references public.applicants(company_id,id) on delete restrict,
 foreign key(company_id,offer_id) references public.hiring_offers(company_id,id) on delete restrict,
 foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict
);
create index hire_conversions_creator_idx on public.hire_conversions(created_by);
create table public.onboarding_cases(
 id uuid primary key,company_id uuid not null,employee_id uuid not null,conversion_id uuid not null unique references public.hire_conversions(id) on delete restrict,
 employee_name text not null,employee_code text not null,starts_on date not null,
 status text not null default 'open' check(status in ('open','completed')),revision bigint not null default 0 check(revision>=0),
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict
);
create index onboarding_cases_employee_idx on public.onboarding_cases(company_id,employee_id);
create table public.onboarding_tasks(
 id uuid primary key,company_id uuid not null,case_id uuid not null,employee_id uuid not null,
 employee_name text not null,employee_code text not null,starts_on date not null,
 case_status text not null default 'open' check(case_status in ('open','completed')),
 assigned_employee_id uuid,title text not null check(char_length(title) between 1 and 120 and title=btrim(title) and title !~ '[[:cntrl:]]'),
 instructions text not null default '' check(char_length(instructions)<=2000),due_on date check(due_on between date '1900-01-01' and date '2200-12-31'),
 required boolean not null default true,status text not null default 'open' check(status in ('open','done','waived')),
 revision bigint not null default 0 check(revision>=0),birth_payload jsonb not null,
 created_by uuid not null references auth.users(id) on delete restrict,created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),foreign key(company_id,case_id) references public.onboarding_cases(company_id,id) on delete restrict,
 foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict,
 foreign key(company_id,assigned_employee_id) references public.employees(company_id,id) on delete restrict
);
create index onboarding_tasks_case_idx on public.onboarding_tasks(company_id,case_id);
create index onboarding_tasks_assignee_idx on public.onboarding_tasks(company_id,assigned_employee_id);
create index onboarding_tasks_employee_idx on public.onboarding_tasks(company_id,employee_id);
create index onboarding_tasks_creator_idx on public.onboarding_tasks(created_by);
create table public.onboarding_events(
 id uuid primary key default gen_random_uuid(),company_id uuid not null,case_id uuid not null,task_id uuid,
 kind text not null,reason text not null check(char_length(reason) between 1 and 500),
 actor_id uuid not null references auth.users(id) on delete restrict,actor_name text not null,actor_role text not null,
 before_record jsonb,after_record jsonb not null,recorded_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,case_id) references public.onboarding_cases(company_id,id) on delete restrict,
 foreign key(company_id,task_id) references public.onboarding_tasks(company_id,id) on delete restrict
);
create index onboarding_events_case_idx on public.onboarding_events(company_id,case_id,recorded_at desc);
create index onboarding_events_task_idx on public.onboarding_events(company_id,task_id);
create index onboarding_events_actor_idx on public.onboarding_events(actor_id);

do $$declare t text;begin
 foreach t in array array['hiring_offers','hiring_offer_events','hire_conversions','onboarding_cases','onboarding_tasks','onboarding_events'] loop
  execute format('alter table public.%I enable row level security',t);
  execute format('revoke all on public.%I from public,anon,authenticated',t);
  execute format('grant select on public.%I to authenticated',t);
  execute format('create policy %I on public.%I for select to authenticated using(hris_private.company_role(company_id) in (''administrator'',''hr_generalist''))',t||'_hr_read',t);
 end loop;
 foreach t in array array['hiring_offer_events','hire_conversions','onboarding_events'] loop
  execute format('create trigger %I before update or delete on public.%I for each row execute function hris_private.employment_history_immutable()',t||'_immutable',t);
 end loop;
end;$$;
create function hris_private.can_read_onboarding_task(p_company uuid,p_task uuid) returns boolean
language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.company_members m join public.onboarding_tasks t on t.company_id=m.company_id
 where m.company_id=p_company and t.id=p_task and m.user_id=(select auth.uid()) and m.active
 and (m.role in ('administrator','hr_generalist') or (m.role='manager' and m.employee_id=t.assigned_employee_id)));
$$;
revoke all on function hris_private.can_read_onboarding_task(uuid,uuid) from public,anon,authenticated;
grant execute on function hris_private.can_read_onboarding_task(uuid,uuid) to authenticated;
create policy onboarding_tasks_assigned_read on public.onboarding_tasks for select to authenticated using(hris_private.can_read_onboarding_task(company_id,id));
create policy onboarding_events_assigned_read on public.onboarding_events for select to authenticated using(task_id is not null and hris_private.can_read_onboarding_task(company_id,task_id));

create function hris_private.manage_hiring(p_operation text,p_record jsonb) returns jsonb
language plpgsql security definer set search_path='' as $$
declare m public.company_members;actor_name text;offer public.hiring_offers;prior jsonb;app public.applicants;
 task public.onboarding_tasks;workflow public.onboarding_cases;conversion public.hire_conversions;
 person uuid;v_id uuid;expected bigint;why text;next_status text;kind text;result jsonb;employee_data jsonb;template text;
begin
 select * into m from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist','manager');
 if m.user_id is null then raise exception 'Active desktop staff access is required.' using errcode='42501';end if;
 perform 1 from public.companies where id=m.company_id for update;
 select * into m from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist','manager') for update;
 if m.user_id is null then raise exception 'Your access changed.' using errcode='42501';end if;
 select full_name into actor_name from public.employees where company_id=m.company_id and id=m.employee_id;
 if jsonb_typeof(p_record) is distinct from 'object' then raise exception 'Enter valid hiring details.' using errcode='23514';end if;
 v_id:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=btrim(p_record->>'reason');
 if v_id is null or why is null or char_length(why) not between 1 and 500 or why ~ '[[:cntrl:]]' then raise exception 'Choose a record and enter a reason of 1–500 characters.' using errcode='23514';end if;
 if p_operation in ('offer_save','offer_status','hire') then
  if m.role not in ('administrator','hr_generalist') then raise exception 'Only HR and Administrators manage offers and hiring.' using errcode='42501';end if;
  if p_operation='offer_save' then
   if p_record-array['id','expected_revision','applicant_id','job_title','department','starts_on','amount_minor','currency','pay_period','terms','expires_on','reason']<>'{}'::jsonb then raise exception 'Unsupported offer fields.' using errcode='23514';end if;
   select * into offer from public.hiring_offers where company_id=m.company_id and id=v_id for update;
   if offer.id is not null and expected is null then
    if offer.created_by=m.user_id and offer.birth_payload=p_record then return to_jsonb(offer)-'birth_payload';end if;
    raise exception 'This offer ID already exists. Refresh and reopen it.' using errcode='PT409';
   end if;
   select * into app from public.applicants where company_id=m.company_id and id=(p_record->>'applicant_id')::uuid and not archived and stage not in ('rejected','withdrawn');
   if app.id is null or exists(select 1 from public.hire_conversions where company_id=m.company_id and applicant_id=app.id) then raise exception 'Choose an active application that has not been hired.' using errcode='23514';end if;
   if offer.id is null then
    if expected is not null then raise exception 'Offer not found.' using errcode='PT409';end if;
    insert into public.hiring_offers(id,company_id,applicant_id,applicant_name,job_title,department,starts_on,amount_minor,currency,pay_period,terms,expires_on,created_by,birth_payload)
    values(v_id,m.company_id,app.id,app.full_name,btrim(p_record->>'job_title'),btrim(coalesce(p_record->>'department','')),(p_record->>'starts_on')::date,
     (p_record->>'amount_minor')::bigint,p_record->>'currency',p_record->>'pay_period',btrim(coalesce(p_record->>'terms','')),nullif(p_record->>'expires_on','')::date,m.user_id,p_record) returning * into offer;
    kind:='created';
   else
    if offer.revision is distinct from expected then raise exception 'This offer changed. Refresh and reopen it.' using errcode='PT409';end if;
    if offer.status<>'draft' or offer.applicant_id<>app.id then raise exception 'Only draft offer terms can be edited. Withdraw and prepare a new offer for revised approved terms.' using errcode='23514';end if;
    prior:=to_jsonb(offer)-'birth_payload';
    update public.hiring_offers set job_title=btrim(p_record->>'job_title'),department=btrim(coalesce(p_record->>'department','')),starts_on=(p_record->>'starts_on')::date,
     amount_minor=(p_record->>'amount_minor')::bigint,currency=p_record->>'currency',pay_period=p_record->>'pay_period',terms=btrim(coalesce(p_record->>'terms','')),expires_on=nullif(p_record->>'expires_on','')::date,
     revision=revision+1,updated_at=clock_timestamp() where id=offer.id returning * into offer;kind:='edited';
   end if;
  elsif p_operation='offer_status' then
   if p_record-array['id','expected_revision','status','reason']<>'{}'::jsonb then raise exception 'Unsupported offer decision fields.' using errcode='23514';end if;
   select * into offer from public.hiring_offers where company_id=m.company_id and id=v_id for update;
   if offer.id is null then raise exception 'Offer not found.' using errcode='42501';end if;
   if offer.revision is distinct from expected then raise exception 'This offer changed. Refresh and reopen it.' using errcode='PT409';end if;
   next_status:=p_record->>'status';
   if not ((offer.status='draft' and next_status='approved') or (offer.status='approved' and next_status='sent') or (offer.status='sent' and next_status in ('accepted','declined')) or (offer.status in ('draft','approved','sent','accepted') and next_status='withdrawn')) then raise exception 'This offer transition is unavailable.' using errcode='23514';end if;
   if next_status='approved' and m.role<>'administrator' then raise exception 'An Administrator must approve offer terms.' using errcode='42501';end if;
   if exists(select 1 from public.hire_conversions where company_id=m.company_id and offer_id=offer.id) then raise exception 'This accepted offer has already been hired.' using errcode='23514';end if;
   if next_status in ('approved','sent','accepted') and (offer.expires_on < (clock_timestamp() at time zone (select timezone from public.companies where id=m.company_id))::date or not exists(select 1 from public.applicants where id=offer.applicant_id and not archived and stage not in ('rejected','withdrawn'))) then raise exception 'Restore an active application and use an unexpired offer.' using errcode='23514';end if;
   prior:=to_jsonb(offer)-'birth_payload';
   update public.hiring_offers set status=next_status,revision=revision+1,updated_at=clock_timestamp(),approved_by=case when next_status='approved' then m.user_id else approved_by end,approved_at=case when next_status='approved' then clock_timestamp() else approved_at end where id=offer.id returning * into offer;kind:=next_status;
  else
   if p_record-array['id','offer_id','expected_revision','employee_code','reports_to','employment_type_id','work_location_id','reason']<>'{}'::jsonb then raise exception 'Unsupported hiring fields.' using errcode='23514';end if;
   select * into conversion from public.hire_conversions where company_id=m.company_id and id=v_id;
   if conversion.id is not null then
    if conversion.created_by=m.user_id and conversion.birth_payload=p_record then return to_jsonb(conversion)-'birth_payload';end if;
    raise exception 'This hiring ID already exists.' using errcode='PT409';
   end if;
   select * into offer from public.hiring_offers where company_id=m.company_id and id=(p_record->>'offer_id')::uuid for update;
   if offer.id is null then raise exception 'Offer not found.' using errcode='42501';end if;
   if offer.revision is distinct from expected then raise exception 'This offer changed. Refresh and reopen it.' using errcode='PT409';end if;
   if offer.status<>'accepted' or exists(select 1 from public.hire_conversions where company_id=m.company_id and applicant_id=offer.applicant_id) then raise exception 'Hiring requires an accepted, approved offer and an application not already hired.' using errcode='23514';end if;
   select * into app from public.applicants where company_id=m.company_id and id=offer.applicant_id and not archived and stage not in ('rejected','withdrawn');
   if app.id is null then raise exception 'Restore the active application before hiring.' using errcode='23514';end if;
   employee_data:=jsonb_build_object('company_id',m.company_id,'employee_code',upper(btrim(p_record->>'employee_code')),'full_name',app.full_name,'department',offer.department,'job_title',offer.job_title,'joined_on',offer.starts_on,'annual_leave_days',(select annual_leave_days from public.companies where id=m.company_id),'status','active','reports_to',p_record->'reports_to','employment_type_id',p_record->'employment_type_id','work_location_id',p_record->'work_location_id');
   person:=public.save_employee_record(employee_data,jsonb_build_object('phone',app.phone,'personal_email',app.email));
   insert into public.hire_conversions(id,company_id,applicant_id,offer_id,employee_id,birth_payload,created_by) values(v_id,m.company_id,app.id,offer.id,person,p_record,m.user_id) returning * into conversion;
   insert into public.onboarding_cases(id,company_id,employee_id,conversion_id,employee_name,employee_code,starts_on) values(v_id,m.company_id,person,v_id,app.full_name,employee_data->>'employee_code',offer.starts_on) returning * into workflow;
   foreach template in array array['Verify contact and emergency details','File employment contract and required documents','Complete company orientation','Prepare equipment and access','Complete required introductory training'] loop
    insert into public.onboarding_tasks(id,company_id,case_id,employee_id,employee_name,employee_code,starts_on,title,due_on,created_by,birth_payload)
     values(gen_random_uuid(),m.company_id,v_id,person,app.full_name,employee_data->>'employee_code',offer.starts_on,template,offer.starts_on,m.user_id,'{}');
   end loop;
   insert into public.onboarding_events(company_id,case_id,kind,reason,actor_id,actor_name,actor_role,after_record) values(m.company_id,v_id,'started',why,m.user_id,actor_name,m.role,to_jsonb(workflow));
   insert into public.hiring_offer_events(company_id,offer_id,kind,reason,actor_id,actor_name,actor_role,after_record) values(m.company_id,offer.id,'hired',why,m.user_id,actor_name,m.role,to_jsonb(conversion)-'birth_payload');
   return to_jsonb(conversion)-'birth_payload';
  end if;
  result:=to_jsonb(offer)-'birth_payload';
  insert into public.hiring_offer_events(company_id,offer_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record) values(m.company_id,offer.id,kind,why,m.user_id,actor_name,m.role,prior,result);
  return result;
 elsif p_operation in ('task_save','task_status') then
  if p_operation='task_save' then
   if m.role not in ('administrator','hr_generalist') then raise exception 'Only HR assigns onboarding tasks.' using errcode='42501';end if;
   if p_record-array['id','expected_revision','case_id','assigned_employee_id','title','instructions','due_on','required','reason']<>'{}'::jsonb then raise exception 'Unsupported task fields.' using errcode='23514';end if;
   select * into task from public.onboarding_tasks where company_id=m.company_id and id=v_id for update;
   if task.id is not null and expected is null then
    if task.created_by=m.user_id and task.birth_payload=p_record then return to_jsonb(task)-'birth_payload';end if;
    raise exception 'This task ID already exists.' using errcode='PT409';
   end if;
   select * into workflow from public.onboarding_cases where company_id=m.company_id and id=(p_record->>'case_id')::uuid and status='open' for update;
   if workflow.id is null then raise exception 'Reopen onboarding before changing its checklist.' using errcode='23514';end if;
   if nullif(p_record->>'assigned_employee_id','') is not null and not exists(select 1 from public.company_members cm join public.employees e on e.id=cm.employee_id and e.company_id=cm.company_id where cm.company_id=m.company_id and cm.employee_id=(p_record->>'assigned_employee_id')::uuid and cm.active and cm.role in ('administrator','hr_generalist','manager') and e.status='active') then raise exception 'Assign active staff with desktop access.' using errcode='23514';end if;
   if task.id is null then
    if expected is not null then raise exception 'Task not found.' using errcode='PT409';end if;
    insert into public.onboarding_tasks(id,company_id,case_id,employee_id,employee_name,employee_code,starts_on,assigned_employee_id,title,instructions,due_on,required,created_by,birth_payload)
    values(v_id,m.company_id,workflow.id,workflow.employee_id,workflow.employee_name,workflow.employee_code,workflow.starts_on,(p_record->>'assigned_employee_id')::uuid,btrim(p_record->>'title'),btrim(coalesce(p_record->>'instructions','')),nullif(p_record->>'due_on','')::date,(p_record->>'required')::boolean,m.user_id,p_record) returning * into task;kind:='task_created';
   else
    if task.revision is distinct from expected then raise exception 'This task changed. Refresh and reopen it.' using errcode='PT409';end if;
    if task.case_id<>workflow.id or task.status<>'open' then raise exception 'Reopen a finished task before editing it.' using errcode='23514';end if;
    prior:=to_jsonb(task)-'birth_payload';
    update public.onboarding_tasks set assigned_employee_id=(p_record->>'assigned_employee_id')::uuid,title=btrim(p_record->>'title'),instructions=btrim(coalesce(p_record->>'instructions','')),due_on=nullif(p_record->>'due_on','')::date,required=(p_record->>'required')::boolean,revision=revision+1,updated_at=clock_timestamp() where id=task.id returning * into task;kind:='task_edited';
   end if;
  else
   if p_record-array['id','expected_revision','status','reason']<>'{}'::jsonb then raise exception 'Unsupported task decision fields.' using errcode='23514';end if;
   select * into task from public.onboarding_tasks where company_id=m.company_id and id=v_id for update;
   if task.id is null or (m.role='manager' and task.assigned_employee_id is distinct from m.employee_id) then raise exception 'This task is not assigned to you.' using errcode='42501';end if;
   if task.revision is distinct from expected then raise exception 'This task changed. Refresh and reopen it.' using errcode='PT409';end if;
   next_status:=p_record->>'status';
   if task.case_status<>'open' or next_status is null or next_status not in ('open','done','waived') or next_status=task.status or (m.role='manager' and (next_status='waived' or task.status='waived')) then raise exception 'This task transition is unavailable. HR handles waivers and closed onboarding.' using errcode='23514';end if;
   prior:=to_jsonb(task)-'birth_payload';
   update public.onboarding_tasks set status=next_status,revision=revision+1,updated_at=clock_timestamp() where id=task.id returning * into task;kind:='task_'||next_status;
  end if;
  result:=to_jsonb(task)-'birth_payload';
  insert into public.onboarding_events(company_id,case_id,task_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record) values(m.company_id,task.case_id,task.id,kind,why,m.user_id,actor_name,m.role,prior,result);
  return result;
 elsif p_operation='case_status' then
  if m.role not in ('administrator','hr_generalist') then raise exception 'Only HR completes or reopens onboarding.' using errcode='42501';end if;
  if p_record-array['id','expected_revision','status','reason']<>'{}'::jsonb then raise exception 'Unsupported onboarding decision fields.' using errcode='23514';end if;
  select * into workflow from public.onboarding_cases where company_id=m.company_id and id=v_id for update;
  if workflow.id is null then raise exception 'Onboarding not found.' using errcode='42501';end if;
  if workflow.revision is distinct from expected then raise exception 'This onboarding changed. Refresh and reopen it.' using errcode='PT409';end if;
  next_status:=p_record->>'status';
  if next_status is null or next_status not in ('open','completed') or next_status=workflow.status then raise exception 'Choose a different onboarding status.' using errcode='23514';end if;
  if next_status='completed' and exists(select 1 from public.onboarding_tasks where company_id=m.company_id and case_id=workflow.id and required and status='open') then raise exception 'Finish or waive every required task before completing onboarding.' using errcode='23514';end if;
  prior:=to_jsonb(workflow);
  update public.onboarding_cases set status=next_status,revision=revision+1,updated_at=clock_timestamp() where id=workflow.id returning * into workflow;
  update public.onboarding_tasks set case_status=next_status,revision=revision+1,updated_at=clock_timestamp() where company_id=m.company_id and case_id=workflow.id;
  insert into public.onboarding_events(company_id,case_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record) values(m.company_id,workflow.id,next_status,why,m.user_id,actor_name,m.role,prior,to_jsonb(workflow));
  return to_jsonb(workflow);
 end if;
 raise exception 'Unknown hiring operation.' using errcode='23514';
end;$$;
revoke all on function hris_private.manage_hiring(text,jsonb) from public,anon,authenticated;
grant execute on function hris_private.manage_hiring(text,jsonb) to authenticated;
create function public.manage_hiring(p_operation text,p_record jsonb) returns jsonb
language sql security invoker set search_path='' as $$select hris_private.manage_hiring(p_operation,p_record);$$;
revoke all on function public.manage_hiring(text,jsonb) from public,anon,authenticated;
grant execute on function public.manage_hiring(text,jsonb) to authenticated;
create function hris_private.retain_hired_applicant_stage() returns trigger
language plpgsql security definer set search_path='' as $$begin
 if new.stage is distinct from old.stage and exists(select 1 from public.hire_conversions where company_id=old.company_id and applicant_id=old.id) then
  raise exception 'This applicant has already been hired. Employment changes belong in the employee record.' using errcode='23514';
 end if;return new;end;$$;
revoke all on function hris_private.retain_hired_applicant_stage() from public,anon,authenticated;
create trigger applicants_hired_stage before update of stage on public.applicants for each row execute function hris_private.retain_hired_applicant_stage();
commit;
