begin;
-- Kept separate from onboarding so installed recruiting clients remain compatible.
create table public.exit_cases(
 id uuid primary key,company_id uuid not null references public.companies(id) on delete restrict,
 employee_id uuid not null,employee_name text not null,employee_code text not null,
 last_working_on date not null check(last_working_on between date '1900-01-01' and date '2200-12-31'),
 category text not null check(category in ('resignation','end_of_contract','retirement','other')),
 private_notes text not null default '' check(char_length(private_notes)<=2000),
 status text not null default 'open' check(status in ('open','cleared','finalized','cancelled')),
 revision bigint not null default 0 check(revision>=0),birth_payload jsonb not null,
 created_by uuid not null references auth.users(id) on delete restrict,
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id,employee_id),foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict
);
create unique index exit_cases_open_person_idx on public.exit_cases(company_id,employee_id) where status in ('open','cleared');
create index exit_cases_employee_idx on public.exit_cases(company_id,employee_id);
create index exit_cases_creator_idx on public.exit_cases(created_by);
create table public.clearance_tasks(
 id uuid primary key,company_id uuid not null,case_id uuid not null,employee_id uuid not null,
 employee_name text not null,employee_code text not null,last_working_on date not null,
 case_status text not null default 'open' check(case_status in ('open','cleared','finalized','cancelled')),
 assigned_employee_id uuid,title text not null check(char_length(title) between 1 and 120 and title=btrim(title) and title !~ '[[:cntrl:]]'),
 instructions text not null default '' check(char_length(instructions)<=2000),due_on date check(due_on between date '1900-01-01' and date '2200-12-31'),
 required boolean not null default true,status text not null default 'open' check(status in ('open','done','waived')),
 revision bigint not null default 0 check(revision>=0),birth_payload jsonb not null,
 created_by uuid not null references auth.users(id) on delete restrict,
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),foreign key(company_id,case_id,employee_id) references public.exit_cases(company_id,id,employee_id) on delete restrict,
 foreign key(company_id,assigned_employee_id) references public.employees(company_id,id) on delete restrict
);
create index clearance_tasks_case_idx on public.clearance_tasks(company_id,case_id,employee_id);
create index clearance_tasks_assignee_idx on public.clearance_tasks(company_id,assigned_employee_id);
create index clearance_tasks_creator_idx on public.clearance_tasks(created_by);
create table public.exit_events(
 id uuid primary key default gen_random_uuid(),company_id uuid not null,case_id uuid not null,employee_id uuid not null,task_id uuid,
 kind text not null,reason text not null check(char_length(reason) between 1 and 500),
 actor_id uuid not null references auth.users(id) on delete restrict,actor_name text not null,actor_role text not null,
 before_record jsonb,after_record jsonb not null,recorded_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,case_id,employee_id) references public.exit_cases(company_id,id,employee_id) on delete restrict,
 foreign key(company_id,task_id) references public.clearance_tasks(company_id,id) on delete restrict
);
create index exit_events_case_idx on public.exit_events(company_id,case_id,employee_id,recorded_at desc);
create index exit_events_task_idx on public.exit_events(company_id,task_id);
create index exit_events_actor_idx on public.exit_events(actor_id);
create function hris_private.can_read_clearance_task(p_company uuid,p_task uuid) returns boolean
language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.company_members m join public.clearance_tasks t on t.company_id=m.company_id
 where m.company_id=p_company and t.id=p_task and m.user_id=(select auth.uid()) and m.active
 and (m.role in ('administrator','hr_generalist') or (m.role='manager' and m.employee_id=t.assigned_employee_id)));
$$;
revoke all on function hris_private.can_read_clearance_task(uuid,uuid) from public,anon,authenticated;
grant execute on function hris_private.can_read_clearance_task(uuid,uuid) to authenticated;
alter table public.exit_cases enable row level security;
alter table public.clearance_tasks enable row level security;
alter table public.exit_events enable row level security;
revoke all on public.exit_cases,public.clearance_tasks,public.exit_events from public,anon,authenticated;
grant select on public.exit_cases,public.clearance_tasks,public.exit_events to authenticated;
create policy exit_cases_hr_read on public.exit_cases for select to authenticated using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));
create policy clearance_tasks_scoped_read on public.clearance_tasks for select to authenticated using(hris_private.can_read_clearance_task(company_id,id));
create policy exit_events_scoped_read on public.exit_events for select to authenticated using(
 hris_private.company_role(company_id) in ('administrator','hr_generalist') or(task_id is not null and hris_private.can_read_clearance_task(company_id,task_id)));
create trigger exit_events_immutable before update or delete on public.exit_events for each row execute function hris_private.employment_history_immutable();
create function hris_private.exit_readiness(p_case uuid) returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare m public.company_members;c public.exit_cases;today date;begin
 select * into m from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Only active HR and Administrators can review exit readiness.' using errcode='42501';end if;
 select * into c from public.exit_cases where id=p_case and company_id=m.company_id;
 if c.id is null then raise exception 'Exit case not found.' using errcode='42501';end if;
 select (now() at time zone timezone)::date into today from public.companies where id=m.company_id;
 return jsonb_build_object('case_id',c.id,'company_id',m.company_id,
 'last_day_reached',c.last_working_on<=today,
 'employee_active',exists(select 1 from public.employees where company_id=m.company_id and id=c.employee_id and status='active'),
 'open_required_tasks',(select count(*)from public.clearance_tasks where company_id=m.company_id and case_id=c.id and required and status='open'),
 'active_direct_reports',(select count(*)from public.employees where company_id=m.company_id and reports_to=c.employee_id and status='active'),
 'open_attendance',(select count(*)from public.attendance where company_id=m.company_id and employee_id=c.employee_id and checked_out_at is null),
 'pending_leave',(select count(*)from public.leave_requests where company_id=m.company_id and employee_id=c.employee_id and status='pending'),
 'pending_attendance_requests',(select count(*)from public.attendance_requests where company_id=m.company_id and employee_id=c.employee_id and status='pending'),
 'pending_overtime',(select count(*)from public.overtime_requests where company_id=m.company_id and employee_id=c.employee_id and status='pending'),
 'installation_owner',exists(select 1 from public.company_members cm join public.companies co on co.id=cm.company_id and co.owner_id=cm.user_id where cm.company_id=m.company_id and cm.employee_id=c.employee_id),
 'own_account',m.employee_id=c.employee_id);
end;$$;
revoke all on function hris_private.exit_readiness(uuid) from public,anon,authenticated;
grant execute on function hris_private.exit_readiness(uuid) to authenticated;
create function public.exit_readiness(p_case uuid) returns jsonb language sql security invoker set search_path='' as $$select hris_private.exit_readiness(p_case);$$;
revoke all on function public.exit_readiness(uuid) from public,anon,authenticated;
grant execute on function public.exit_readiness(uuid) to authenticated;
create function hris_private.manage_exit(p_operation text,p_record jsonb) returns jsonb
language plpgsql security definer set search_path='' as $$
declare m public.company_members;c public.exit_cases;workflow public.exit_cases;task public.clearance_tasks;e public.employees;account public.company_members;
 v_id uuid;expected bigint;why text;actor_name text;prior jsonb;result jsonb;ready jsonb;kind text;next_status text;template text;account_email text;
begin
 select * into m from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist','manager');
 if m.user_id is null then raise exception 'Active desktop staff access is required.' using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 perform 1 from public.companies where id=m.company_id for update;
 select * into m from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist','manager') for update;
 if m.user_id is null then raise exception 'Your access changed.' using errcode='42501';end if;
 select full_name into actor_name from public.employees where company_id=m.company_id and id=m.employee_id;
 if jsonb_typeof(p_record) is distinct from 'object' then raise exception 'Enter valid exit details.' using errcode='23514';end if;
 v_id:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=btrim(p_record->>'reason');
 if v_id is null or why is null or char_length(why) not between 1 and 500 or why ~ '[[:cntrl:]]' then raise exception 'Choose a record and enter a reason of 1–500 characters.' using errcode='23514';end if;
 if p_operation='case_save' then
  if m.role not in ('administrator','hr_generalist') then raise exception 'Only HR manages exit details.' using errcode='42501';end if;
  if p_record-array['id','expected_revision','employee_id','last_working_on','category','private_notes','reason']<>'{}'::jsonb then raise exception 'Unsupported exit fields.' using errcode='23514';end if;
  select * into c from public.exit_cases where company_id=m.company_id and id=v_id for update;
  if c.id is not null and expected is null then
   if c.created_by=m.user_id and c.birth_payload=p_record then return to_jsonb(c)-'birth_payload';end if;
   raise exception 'This exit ID already exists.' using errcode='PT409';
  end if;
  select * into e from public.employees where company_id=m.company_id and id=(p_record->>'employee_id')::uuid and status='active';
  if e.id is null then raise exception 'Choose an active employee in this company.' using errcode='23514';end if;
  if (p_record->>'last_working_on')::date<e.joined_on then raise exception 'Last working day cannot precede the employee start date.' using errcode='23514';end if;
  if c.id is null then
   if expected is not null then raise exception 'Exit not found.' using errcode='PT409';end if;
   insert into public.exit_cases(id,company_id,employee_id,employee_name,employee_code,last_working_on,category,private_notes,birth_payload,created_by)
    values(v_id,m.company_id,e.id,e.full_name,e.employee_code,(p_record->>'last_working_on')::date,p_record->>'category',btrim(coalesce(p_record->>'private_notes','')),p_record,m.user_id) returning * into c;
   foreach template in array array['Complete work handover','Return company equipment and property','Review system access and credentials','Complete HR documents and records','Record payroll and final-pay review'] loop
    insert into public.clearance_tasks(id,company_id,case_id,employee_id,employee_name,employee_code,last_working_on,title,due_on,created_by,birth_payload)
     values(gen_random_uuid(),m.company_id,c.id,e.id,e.full_name,e.employee_code,c.last_working_on,template,c.last_working_on,m.user_id,'{}');
   end loop;kind:='started';
  else
   if c.revision is distinct from expected then raise exception 'This exit changed. Refresh and reopen it.' using errcode='PT409';end if;
   if c.status<>'open' or c.employee_id<>e.id then raise exception 'Reopen clearance before editing exit details.' using errcode='23514';end if;
   prior:=to_jsonb(c)-'birth_payload';
   update public.exit_cases set last_working_on=(p_record->>'last_working_on')::date,category=p_record->>'category',private_notes=btrim(coalesce(p_record->>'private_notes','')),revision=revision+1,updated_at=clock_timestamp() where id=c.id returning * into c;
   update public.clearance_tasks set last_working_on=c.last_working_on,revision=revision+1,updated_at=clock_timestamp() where company_id=m.company_id and case_id=c.id and last_working_on<>c.last_working_on;kind:='edited';
  end if;
  result:=to_jsonb(c)-'birth_payload';
  insert into public.exit_events(company_id,case_id,employee_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record) values(m.company_id,c.id,c.employee_id,kind,why,m.user_id,actor_name,m.role,prior,result);
  return result;
 elsif p_operation='case_status' then
  if m.role not in ('administrator','hr_generalist') then raise exception 'Only HR manages exit clearance.' using errcode='42501';end if;
  if p_record-array['id','expected_revision','status','reason']<>'{}'::jsonb then raise exception 'Unsupported exit decision fields.' using errcode='23514';end if;
  select * into c from public.exit_cases where company_id=m.company_id and id=v_id for update;
  if c.id is null then raise exception 'Exit not found.' using errcode='42501';end if;
  if c.revision is distinct from expected then raise exception 'This exit changed. Refresh and reopen it.' using errcode='PT409';end if;
  next_status:=p_record->>'status';
  if not ((c.status='open' and next_status in ('cleared','cancelled')) or(c.status='cleared' and next_status in ('open','cancelled','finalized'))) or next_status is null then raise exception 'This exit transition is unavailable.' using errcode='23514';end if;
  if next_status in ('cleared','finalized') and exists(select 1 from public.clearance_tasks where company_id=m.company_id and case_id=c.id and required and status='open') then raise exception 'Resolve every required clearance task first.' using errcode='23514';end if;
  if next_status='finalized' then
   if m.role<>'administrator' then raise exception 'An Administrator must finalize exit and disable sign-in.' using errcode='42501';end if;
   -- Serialize with attendance/account archive guards before checking readiness.
   perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(c.employee_id::text,0));
   ready:=hris_private.exit_readiness(c.id);
   if not (ready->>'last_day_reached')::boolean or not (ready->>'employee_active')::boolean or (ready->>'installation_owner')::boolean or (ready->>'own_account')::boolean or (ready->>'active_direct_reports')::int>0 or (ready->>'open_attendance')::int>0 or (ready->>'pending_leave')::int>0 or (ready->>'pending_attendance_requests')::int>0 or (ready->>'pending_overtime')::int>0 then raise exception 'Exit is not ready: check the last day, reporting, attendance, pending requests and protected account safeguards.' using errcode='23514';end if;
   for account in select * from public.company_members where company_id=m.company_id and employee_id=c.employee_id and active for update loop
    select email into account_email from auth.users where id=account.user_id;
    perform public.save_company_account(account.user_id,account_email,account.role,false,account.employee_id,null);
   end loop;
   update public.employees set status='archived' where company_id=m.company_id and id=c.employee_id;
  end if;
  prior:=to_jsonb(c)-'birth_payload';
  update public.exit_cases set status=next_status,revision=revision+1,updated_at=clock_timestamp() where id=c.id returning * into c;
  update public.clearance_tasks set case_status=next_status,revision=revision+1,updated_at=clock_timestamp() where company_id=m.company_id and case_id=c.id;
  result:=to_jsonb(c)-'birth_payload';
  insert into public.exit_events(company_id,case_id,employee_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record) values(m.company_id,c.id,c.employee_id,next_status,why,m.user_id,actor_name,m.role,prior,result);
  return result;
 elsif p_operation in ('task_save','task_status') then
  if p_operation='task_save' then
   if m.role not in ('administrator','hr_generalist') then raise exception 'Only HR assigns clearance tasks.' using errcode='42501';end if;
   if p_record-array['id','expected_revision','case_id','assigned_employee_id','title','instructions','due_on','required','reason']<>'{}'::jsonb then raise exception 'Unsupported task fields.' using errcode='23514';end if;
   select * into task from public.clearance_tasks where company_id=m.company_id and id=v_id for update;
   if task.id is not null and expected is null then
    if task.created_by=m.user_id and task.birth_payload=p_record then return to_jsonb(task)-'birth_payload';end if;
    raise exception 'This task ID already exists.' using errcode='PT409';
   end if;
   select * into workflow from public.exit_cases where company_id=m.company_id and id=(p_record->>'case_id')::uuid and status='open' for update;
   if workflow.id is null then raise exception 'Reopen clearance before changing its checklist.' using errcode='23514';end if;
   if nullif(p_record->>'assigned_employee_id','') is not null and not exists(select 1 from public.company_members cm join public.employees emp on emp.id=cm.employee_id and emp.company_id=cm.company_id where cm.company_id=m.company_id and cm.employee_id=(p_record->>'assigned_employee_id')::uuid and cm.active and cm.role in ('administrator','hr_generalist','manager') and emp.status='active') then raise exception 'Assign active staff with desktop access.' using errcode='23514';end if;
   if task.id is null then
    if expected is not null then raise exception 'Task not found.' using errcode='PT409';end if;
    insert into public.clearance_tasks(id,company_id,case_id,employee_id,employee_name,employee_code,last_working_on,assigned_employee_id,title,instructions,due_on,required,created_by,birth_payload)
    values(v_id,m.company_id,workflow.id,workflow.employee_id,workflow.employee_name,workflow.employee_code,workflow.last_working_on,(p_record->>'assigned_employee_id')::uuid,btrim(p_record->>'title'),btrim(coalesce(p_record->>'instructions','')),nullif(p_record->>'due_on','')::date,(p_record->>'required')::boolean,m.user_id,p_record) returning * into task;kind:='task_created';
   else
    if task.revision is distinct from expected then raise exception 'This task changed. Refresh and reopen it.' using errcode='PT409';end if;
    if task.case_id<>workflow.id or task.status<>'open' then raise exception 'Reopen a finished task before editing it.' using errcode='23514';end if;
    prior:=to_jsonb(task)-'birth_payload';
    update public.clearance_tasks set assigned_employee_id=(p_record->>'assigned_employee_id')::uuid,title=btrim(p_record->>'title'),instructions=btrim(coalesce(p_record->>'instructions','')),due_on=nullif(p_record->>'due_on','')::date,required=(p_record->>'required')::boolean,revision=revision+1,updated_at=clock_timestamp() where id=task.id returning * into task;kind:='task_edited';
   end if;
  else
   if p_record-array['id','expected_revision','status','reason']<>'{}'::jsonb then raise exception 'Unsupported task decision fields.' using errcode='23514';end if;
   select * into task from public.clearance_tasks where company_id=m.company_id and id=v_id for update;
   if task.id is null or (m.role='manager' and task.assigned_employee_id is distinct from m.employee_id) then raise exception 'This task is not assigned to you.' using errcode='42501';end if;
   if task.revision is distinct from expected then raise exception 'This task changed. Refresh and reopen it.' using errcode='PT409';end if;
   next_status:=p_record->>'status';
   if task.case_status<>'open' or next_status is null or next_status not in ('open','done','waived') or next_status=task.status or (m.role='manager' and (next_status='waived' or task.status='waived')) then raise exception 'This task transition is unavailable. HR handles waivers and closed clearance.' using errcode='23514';end if;
   prior:=to_jsonb(task)-'birth_payload';
   update public.clearance_tasks set status=next_status,revision=revision+1,updated_at=clock_timestamp() where id=task.id returning * into task;kind:='task_'||next_status;
  end if;
  result:=to_jsonb(task)-'birth_payload';
  insert into public.exit_events(company_id,case_id,employee_id,task_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record) values(m.company_id,task.case_id,task.employee_id,task.id,kind,why,m.user_id,actor_name,m.role,prior,result);
  return result;
 end if;
 raise exception 'Unknown exit operation.' using errcode='23514';
end;$$;
revoke all on function hris_private.manage_exit(text,jsonb) from public,anon,authenticated;
grant execute on function hris_private.manage_exit(text,jsonb) to authenticated;
create function public.manage_exit(p_operation text,p_record jsonb) returns jsonb language sql security invoker set search_path='' as $$select hris_private.manage_exit(p_operation,p_record);$$;
revoke all on function public.manage_exit(text,jsonb) from public,anon,authenticated;
grant execute on function public.manage_exit(text,jsonb) to authenticated;
commit;

