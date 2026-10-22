begin;
-- Private retry payloads never enter published review/employee projections.
create table hris_private.performance_birth(
 company_id uuid not null references public.companies(id),kind text not null,record_id uuid not null,
 actor_id uuid not null references auth.users(id),payload jsonb not null,created_at timestamptz not null default clock_timestamp(),
 primary key(company_id,kind,record_id)
);
create index performance_birth_actor_idx on hris_private.performance_birth(actor_id);
alter table hris_private.performance_birth enable row level security;
revoke all on hris_private.performance_birth from public,anon,authenticated;
create table public.performance_goals(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid not null,
 employee_name text not null,employee_code text not null,
 title text not null check(title=btrim(title) and char_length(title) between 1 and 120 and title !~ '[[:cntrl:]]'),
 description text not null default '' check(char_length(description)<=3000),measure text not null default '' check(char_length(measure)<=500),
 starts_on date not null check(starts_on between date '1900-01-01' and date '2200-12-31'),
 due_on date not null check(due_on between date '1900-01-01' and date '2200-12-31' and due_on>=starts_on),
 progress integer not null default 0 check(progress between 0 and 100),progress_note text not null default '' check(char_length(progress_note)<=1000),
 status text not null default 'active' check(status in ('active','completed','cancelled')),
 revision bigint not null default 0 check(revision>=0),created_by uuid not null references auth.users(id),
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),foreign key(company_id,employee_id) references public.employees(company_id,id)
);
create index performance_goals_employee_idx on public.performance_goals(company_id,employee_id,due_on);
create index performance_goals_creator_idx on public.performance_goals(created_by);
create table public.performance_cycles(
 id uuid primary key,company_id uuid not null references public.companies(id),
 name text not null check(name=btrim(name) and char_length(name) between 1 and 120 and name !~ '[[:cntrl:]]'),
 starts_on date not null check(starts_on between date '1900-01-01' and date '2200-12-31'),
 ends_on date not null check(ends_on between date '1900-01-01' and date '2200-12-31' and ends_on>=starts_on),
 due_on date not null check(due_on between date '1900-01-01' and date '2200-12-31' and due_on>=ends_on),
 status text not null default 'open' check(status in ('open','closed')),revision bigint not null default 0 check(revision>=0),
 created_by uuid not null references auth.users(id),created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id)
);
create index performance_cycles_company_idx on public.performance_cycles(company_id,status);
create index performance_cycles_creator_idx on public.performance_cycles(created_by);
create table public.performance_reviews(
 id uuid primary key,company_id uuid not null,cycle_id uuid not null,employee_id uuid not null,
 employee_name text not null,employee_code text not null,cycle_name text not null,starts_on date not null,ends_on date not null,due_on date not null,
 reviewer_employee_id uuid not null,reviewer_name text not null,
 rating smallint check(rating between 1 and 5),assessment text not null default '' check(char_length(assessment)<=4000),
 strengths text not null default '' check(char_length(strengths)<=2000),development text not null default '' check(char_length(development)<=2000),
 goals_snapshot jsonb not null default '[]' check(jsonb_typeof(goals_snapshot)='array' and jsonb_array_length(goals_snapshot)<=100),
 status text not null default 'draft' check(status in ('draft','submitted','published','cancelled')),
 revision bigint not null default 0 check(revision>=0),submitted_at timestamptz,published_at timestamptz,published_by uuid references auth.users(id),
 created_by uuid not null references auth.users(id),created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),unique(company_id,cycle_id,employee_id),check(reviewer_employee_id<>employee_id),
 foreign key(company_id,cycle_id) references public.performance_cycles(company_id,id),
 foreign key(company_id,employee_id) references public.employees(company_id,id),
 foreign key(company_id,reviewer_employee_id) references public.employees(company_id,id)
);
create index performance_reviews_employee_idx on public.performance_reviews(company_id,employee_id);
create index performance_reviews_reviewer_idx on public.performance_reviews(company_id,reviewer_employee_id,status);
create index performance_reviews_creator_idx on public.performance_reviews(created_by);
create index performance_reviews_publisher_idx on public.performance_reviews(published_by);
create table public.performance_hr_notes(
 id uuid primary key,company_id uuid not null,review_id uuid not null,
 note text not null default '' check(char_length(note)<=2000),revision bigint not null default 0 check(revision>=0),
 created_by uuid not null references auth.users(id),created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),unique(company_id,review_id),foreign key(company_id,review_id) references public.performance_reviews(company_id,id)
);
create index performance_hr_notes_creator_idx on public.performance_hr_notes(created_by);
create table public.performance_events(
 id uuid primary key default gen_random_uuid(),company_id uuid not null references public.companies(id),goal_id uuid,cycle_id uuid,review_id uuid,note_id uuid,
 kind text not null,reason text not null check(char_length(reason) between 1 and 500),actor_id uuid not null references auth.users(id),actor_name text not null,actor_role text not null,
 before_record jsonb,after_record jsonb not null,recorded_at timestamptz not null default clock_timestamp(),
 check(goal_id is not null or cycle_id is not null or review_id is not null),check(note_id is null or review_id is not null),
 foreign key(company_id,goal_id) references public.performance_goals(company_id,id),
 foreign key(company_id,cycle_id) references public.performance_cycles(company_id,id),
 foreign key(company_id,review_id) references public.performance_reviews(company_id,id),
 foreign key(company_id,note_id) references public.performance_hr_notes(company_id,id)
);
create index performance_events_goal_idx on public.performance_events(company_id,goal_id,recorded_at);
create index performance_events_cycle_idx on public.performance_events(company_id,cycle_id);
create index performance_events_review_idx on public.performance_events(company_id,review_id,recorded_at);
create index performance_events_note_idx on public.performance_events(company_id,note_id);
create index performance_events_actor_idx on public.performance_events(actor_id);
create trigger performance_events_immutable before update or delete on public.performance_events for each row execute function hris_private.employment_history_immutable();
create function hris_private.performance_scope(p_company uuid,p_employee uuid) returns boolean language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=p_employee
 where m.company_id=p_company and m.user_id=(select auth.uid()) and m.active and (m.role in ('administrator','hr_generalist') or m.employee_id=e.id or(m.role='manager' and e.reports_to=m.employee_id)));
$$;
create function hris_private.performance_goal_read(p_company uuid,p_goal uuid) returns boolean language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.performance_goals g where g.company_id=p_company and g.id=p_goal and hris_private.performance_scope(g.company_id,g.employee_id));
$$;
create function hris_private.performance_review_read(p_company uuid,p_review uuid) returns boolean language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.company_members m join public.performance_reviews r on r.company_id=m.company_id join public.employees e on e.company_id=r.company_id and e.id=r.employee_id
 where m.company_id=p_company and r.id=p_review and m.user_id=(select auth.uid()) and m.active and(m.role in ('administrator','hr_generalist') or(m.employee_id=r.employee_id and r.status='published') or(m.role='manager' and m.employee_id=r.reviewer_employee_id and e.reports_to=m.employee_id)));
$$;
create function hris_private.performance_event_read(p_company uuid,p_event uuid) returns boolean language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.company_members m join public.performance_events v on v.company_id=m.company_id
 where m.company_id=p_company and v.id=p_event and m.user_id=(select auth.uid()) and m.active and(m.role in ('administrator','hr_generalist') or(v.note_id is null and (
 (v.goal_id is not null and hris_private.performance_goal_read(p_company,v.goal_id)) or(v.review_id is not null and hris_private.performance_review_read(p_company,v.review_id) and(v.actor_id=m.user_id or(v.kind='published' and exists(select 1 from public.performance_reviews r where r.id=v.review_id and r.employee_id=m.employee_id))))))));
$$;
revoke all on function hris_private.performance_scope(uuid,uuid),hris_private.performance_goal_read(uuid,uuid),hris_private.performance_review_read(uuid,uuid),hris_private.performance_event_read(uuid,uuid) from public,anon,authenticated;
grant execute on function hris_private.performance_scope(uuid,uuid),hris_private.performance_goal_read(uuid,uuid),hris_private.performance_review_read(uuid,uuid),hris_private.performance_event_read(uuid,uuid) to authenticated;
alter table public.performance_goals enable row level security;
alter table public.performance_cycles enable row level security;
alter table public.performance_reviews enable row level security;
alter table public.performance_hr_notes enable row level security;
alter table public.performance_events enable row level security;
revoke all on public.performance_goals,public.performance_cycles,public.performance_reviews,public.performance_hr_notes,public.performance_events from public,anon,authenticated;
grant select on public.performance_goals,public.performance_cycles,public.performance_reviews,public.performance_hr_notes,public.performance_events to authenticated;
create policy performance_goals_read on public.performance_goals for select to authenticated using(hris_private.performance_scope(company_id,employee_id));
create policy performance_cycles_read on public.performance_cycles for select to authenticated using(hris_private.company_role(company_id) in ('administrator','hr_generalist') or exists(select 1 from public.performance_reviews r where r.cycle_id=performance_cycles.id and r.company_id=performance_cycles.company_id and hris_private.performance_review_read(r.company_id,r.id)));
create policy performance_reviews_read on public.performance_reviews for select to authenticated using(hris_private.performance_review_read(company_id,id));
create policy performance_hr_notes_read on public.performance_hr_notes for select to authenticated using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));
create policy performance_events_read on public.performance_events for select to authenticated using(hris_private.performance_event_read(company_id,id));
create function hris_private.performance_retry(p_company uuid,p_kind text,p_id uuid,p_actor uuid,p_record jsonb) returns boolean language plpgsql security definer set search_path='' as $$
declare b hris_private.performance_birth;begin
 select * into b from hris_private.performance_birth where company_id=p_company and kind=p_kind and record_id=p_id;
 if b.record_id is null then return false;end if;
 if b.actor_id=p_actor and b.payload=p_record then return true;end if;
 raise exception 'This record ID already exists. Refresh and reopen it.' using errcode='PT409';
end;$$;
revoke all on function hris_private.performance_retry(uuid,text,uuid,uuid,jsonb) from public,anon,authenticated;
create function hris_private.performance_reviewer_valid(p_company uuid,p_employee uuid,p_reviewer uuid) returns boolean language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.company_members m join public.employees reviewer on reviewer.company_id=m.company_id and reviewer.id=m.employee_id join public.employees subject on subject.company_id=m.company_id and subject.id=p_employee
 where m.company_id=p_company and m.active and reviewer.status='active' and m.employee_id=p_reviewer and m.employee_id<>p_employee and(m.role in ('administrator','hr_generalist') or(m.role='manager' and subject.reports_to=m.employee_id)));
$$;
revoke all on function hris_private.performance_reviewer_valid(uuid,uuid,uuid) from public,anon,authenticated;
-- The guarded writer and exit compatibility hooks are appended below.

create function hris_private.manage_performance(p_operation text,p_record jsonb) returns jsonb
language plpgsql security definer set search_path='' as $$
declare m public.company_members;person public.employees;g public.performance_goals;cycle public.performance_cycles;review public.performance_reviews;hr_note public.performance_hr_notes;
 v_id uuid;expected bigint;why text;actor_name text;old_record jsonb;result jsonb;kind text;next_status text;
 event_goal uuid;event_cycle uuid;event_review uuid;event_note uuid;create_kind text;snapshot jsonb;
begin
 select * into m from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist','manager');
 if m.user_id is null then raise exception 'Active desktop staff access is required.' using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 perform 1 from public.companies where id=m.company_id for update;
 select * into m from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist','manager') for update;
 if m.user_id is null then raise exception 'Your access changed. Refresh.' using errcode='42501';end if;
 select full_name into actor_name from public.employees where company_id=m.company_id and id=m.employee_id;
 if jsonb_typeof(p_record) is distinct from 'object' or octet_length(p_record::text)>65536 then raise exception 'Enter valid, bounded performance details.' using errcode='23514';end if;
 v_id:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=btrim(p_record->>'reason');
 if v_id is null or why is null or char_length(why) not between 1 and 500 or why ~ '[[:cntrl:]]' then raise exception 'Choose a record and enter a reason of 1–500 characters.' using errcode='23514';end if;
 if p_operation='goal_save' then
  if p_record-array['id','expected_revision','employee_id','title','description','measure','starts_on','due_on','reason']<>'{}'::jsonb then raise exception 'Unsupported goal fields.' using errcode='23514';end if;
  select * into person from public.employees where company_id=m.company_id and id=(p_record->>'employee_id')::uuid;
  if person.id is null or (m.role in ('administrator','hr_generalist') or(m.role='manager' and person.reports_to=m.employee_id)) is not true then raise exception 'Only HR or the current reporting Manager assigns goals.' using errcode='42501';end if;
  select * into g from public.performance_goals where company_id=m.company_id and id=v_id for update;
  if g.id is not null and expected is null then if hris_private.performance_retry(m.company_id,'goal',v_id,m.user_id,p_record) then return to_jsonb(g);end if;raise exception 'This goal ID exists.' using errcode='PT409';end if;
  if person.status<>'active' or (p_record->>'starts_on')::date<person.joined_on then raise exception 'Choose an active employee and dates on/after joining.' using errcode='23514';end if;
  if g.id is null then
   if expected is not null then raise exception 'Goal not found. Refresh.' using errcode='PT409';end if;
   insert into public.performance_goals(id,company_id,employee_id,employee_name,employee_code,title,description,measure,starts_on,due_on,created_by)
   values(v_id,m.company_id,person.id,person.full_name,person.employee_code,btrim(p_record->>'title'),btrim(coalesce(p_record->>'description','')),btrim(coalesce(p_record->>'measure','')),(p_record->>'starts_on')::date,(p_record->>'due_on')::date,m.user_id) returning * into g;
   create_kind:='goal';kind:='goal_created';
  else
   if g.revision is distinct from expected then raise exception 'This goal changed. Refresh and reopen it.' using errcode='PT409';end if;
   if g.employee_id<>person.id or g.status<>'active' then raise exception 'Reopen the same employee goal before editing it.' using errcode='23514';end if;
   old_record:=to_jsonb(g);
   update public.performance_goals set title=btrim(p_record->>'title'),description=btrim(coalesce(p_record->>'description','')),measure=btrim(coalesce(p_record->>'measure','')),starts_on=(p_record->>'starts_on')::date,due_on=(p_record->>'due_on')::date,revision=revision+1,updated_at=clock_timestamp() where id=g.id returning * into g;kind:='goal_edited';
  end if;
  result:=to_jsonb(g);event_goal:=g.id;
 elsif p_operation in ('goal_progress','goal_status') then
  if p_record-(case when p_operation='goal_progress' then array['id','expected_revision','progress','progress_note','reason'] else array['id','expected_revision','status','reason']end)<>'{}'::jsonb then raise exception 'Unsupported goal decision fields.' using errcode='23514';end if;
  select * into g from public.performance_goals where company_id=m.company_id and id=v_id for update;
  select * into person from public.employees where company_id=m.company_id and id=g.employee_id;
  if g.id is null or (m.role in ('administrator','hr_generalist') or(m.role='manager' and person.reports_to=m.employee_id) or(p_operation='goal_progress' and m.employee_id=g.employee_id)) is not true then raise exception 'Your role cannot change this goal.' using errcode='42501';end if;
  if g.revision is distinct from expected then raise exception 'This goal changed. Refresh and reopen it.' using errcode='PT409';end if;
  old_record:=to_jsonb(g);
  if p_operation='goal_progress' then
   if g.status<>'active' or person.status<>'active' then raise exception 'Only active employee goals accept progress.' using errcode='23514';end if;
   update public.performance_goals set progress=(p_record->>'progress')::integer,progress_note=btrim(coalesce(p_record->>'progress_note','')),revision=revision+1,updated_at=clock_timestamp() where id=g.id returning * into g;kind:='goal_progress';
  else
   next_status:=p_record->>'status';
   if next_status is null or next_status not in ('active','completed','cancelled') or next_status=g.status or (next_status='completed' and g.progress<>100) or(person.status<>'active' and not(m.role in ('administrator','hr_generalist') and next_status='cancelled')) then raise exception 'Choose an available goal status. Completion requires 100%% progress.' using errcode='23514';end if;
   update public.performance_goals set status=next_status,revision=revision+1,updated_at=clock_timestamp() where id=g.id returning * into g;kind:='goal_'||next_status;
  end if;
  result:=to_jsonb(g);event_goal:=g.id;
 elsif p_operation='cycle_save' then
  if m.role not in ('administrator','hr_generalist') then raise exception 'Only HR configures appraisal cycles.' using errcode='42501';end if;
  if p_record-array['id','expected_revision','name','starts_on','ends_on','due_on','reason']<>'{}'::jsonb then raise exception 'Unsupported cycle fields.' using errcode='23514';end if;
  select * into cycle from public.performance_cycles where company_id=m.company_id and id=v_id for update;
  if cycle.id is not null and expected is null then if hris_private.performance_retry(m.company_id,'cycle',v_id,m.user_id,p_record) then return to_jsonb(cycle);end if;raise exception 'This cycle ID exists.' using errcode='PT409';end if;
  if cycle.id is null then
   if expected is not null then raise exception 'Cycle not found. Refresh.' using errcode='PT409';end if;
   insert into public.performance_cycles(id,company_id,name,starts_on,ends_on,due_on,created_by)values(v_id,m.company_id,btrim(p_record->>'name'),(p_record->>'starts_on')::date,(p_record->>'ends_on')::date,(p_record->>'due_on')::date,m.user_id) returning * into cycle;create_kind:='cycle';kind:='cycle_created';
  else
   if cycle.revision is distinct from expected then raise exception 'This cycle changed. Refresh.' using errcode='PT409';end if;
   if cycle.status<>'open' or exists(select 1 from public.performance_reviews where company_id=m.company_id and cycle_id=cycle.id) then raise exception 'Cycle labels/dates are frozen after a review is assigned.' using errcode='23514';end if;
   old_record:=to_jsonb(cycle);
   update public.performance_cycles set name=btrim(p_record->>'name'),starts_on=(p_record->>'starts_on')::date,ends_on=(p_record->>'ends_on')::date,due_on=(p_record->>'due_on')::date,revision=revision+1,updated_at=clock_timestamp() where id=cycle.id returning * into cycle;kind:='cycle_edited';
  end if;
  result:=to_jsonb(cycle);event_cycle:=cycle.id;
 elsif p_operation='cycle_status' then
  if m.role not in ('administrator','hr_generalist') then raise exception 'Only HR manages appraisal cycles.' using errcode='42501';end if;
  if p_record-array['id','expected_revision','status','reason']<>'{}'::jsonb then raise exception 'Unsupported cycle decision fields.' using errcode='23514';end if;
  select * into cycle from public.performance_cycles where company_id=m.company_id and id=v_id for update;
  if cycle.id is null then raise exception 'Cycle not found.' using errcode='42501';end if;
  if cycle.revision is distinct from expected then raise exception 'This cycle changed. Refresh.' using errcode='PT409';end if;
  next_status:=p_record->>'status';
  if next_status is null or next_status not in ('open','closed') or next_status=cycle.status then raise exception 'Choose an available cycle status.' using errcode='23514';end if;
  if next_status='closed' and exists(select 1 from public.performance_reviews where company_id=m.company_id and cycle_id=cycle.id and status in ('draft','submitted')) then raise exception 'Publish or cancel every unfinished appraisal before closing the cycle.' using errcode='23514';end if;
  old_record:=to_jsonb(cycle);update public.performance_cycles set status=next_status,revision=revision+1,updated_at=clock_timestamp() where id=cycle.id returning * into cycle;kind:='cycle_'||next_status;result:=to_jsonb(cycle);event_cycle:=cycle.id;
 elsif p_operation='review_assign' then
  if m.role not in ('administrator','hr_generalist') then raise exception 'Only HR assigns appraisals.' using errcode='42501';end if;
  if p_record-array['id','expected_revision','cycle_id','employee_id','reviewer_employee_id','reason']<>'{}'::jsonb then raise exception 'Unsupported assignment fields.' using errcode='23514';end if;
  select * into review from public.performance_reviews where company_id=m.company_id and id=v_id for update;
  if review.id is not null and expected is null then if hris_private.performance_retry(m.company_id,'review',v_id,m.user_id,p_record) then return to_jsonb(review);end if;raise exception 'This appraisal ID exists.' using errcode='PT409';end if;
  select * into person from public.employees where company_id=m.company_id and id=(p_record->>'employee_id')::uuid and status='active';
  select * into cycle from public.performance_cycles where company_id=m.company_id and id=(p_record->>'cycle_id')::uuid and status='open';
  if person.id is null or cycle.id is null or person.joined_on>cycle.ends_on then raise exception 'Choose an active employee employed in an open review period.' using errcode='23514';end if;
  if person.id=m.employee_id then raise exception 'Another HR or Administrator must assign your appraisal.' using errcode='42501';end if;
  if not hris_private.performance_reviewer_valid(m.company_id,person.id,(p_record->>'reviewer_employee_id')::uuid) then raise exception 'Assign another active HR/Administrator or this employee reporting Manager.' using errcode='23514';end if;
  if review.id is null then
   if expected is not null then raise exception 'Appraisal not found. Refresh.' using errcode='PT409';end if;
   if exists(select 1 from public.performance_reviews where company_id=m.company_id and cycle_id=cycle.id and employee_id=person.id) then raise exception 'This employee already has an appraisal in this cycle.' using errcode='PT409';end if;
   insert into public.performance_reviews(id,company_id,cycle_id,employee_id,employee_name,employee_code,cycle_name,starts_on,ends_on,due_on,reviewer_employee_id,reviewer_name,created_by)
   select v_id,m.company_id,cycle.id,person.id,person.full_name,person.employee_code,cycle.name,cycle.starts_on,cycle.ends_on,cycle.due_on,emp.id,emp.full_name,m.user_id from public.employees emp where emp.company_id=m.company_id and emp.id=(p_record->>'reviewer_employee_id')::uuid returning * into review;create_kind:='review';kind:='review_assigned';
  else
   if review.revision is distinct from expected then raise exception 'This appraisal changed. Refresh.' using errcode='PT409';end if;
   if review.employee_id<>person.id or review.cycle_id<>cycle.id or review.status='published' or review.reviewer_employee_id=(p_record->>'reviewer_employee_id')::uuid then raise exception 'Choose a new reviewer for the same unpublished employee appraisal.' using errcode='23514';end if;
   old_record:=to_jsonb(review);
   update public.performance_reviews r set reviewer_employee_id=emp.id,reviewer_name=emp.full_name,rating=null,assessment='',strengths='',development='',goals_snapshot='[]',submitted_at=null,status='draft',revision=r.revision+1,updated_at=clock_timestamp() from public.employees emp where r.id=review.id and emp.company_id=m.company_id and emp.id=(p_record->>'reviewer_employee_id')::uuid returning r.* into review;kind:='review_reassigned';
  end if;
  result:=to_jsonb(review);event_review:=review.id;event_cycle:=review.cycle_id;
 elsif p_operation in ('review_save','review_status') then
  if p_record-(case when p_operation='review_save' then array['id','expected_revision','rating','assessment','strengths','development','reason']else array['id','expected_revision','status','reason']end)<>'{}'::jsonb then raise exception 'Unsupported appraisal fields.' using errcode='23514';end if;
  select * into review from public.performance_reviews where company_id=m.company_id and id=v_id for update;
  if review.id is null or not hris_private.performance_review_read(m.company_id,review.id) then raise exception 'This appraisal is not assigned to you.' using errcode='42501';end if;
  if review.revision is distinct from expected then raise exception 'This appraisal changed. Refresh and reopen it.' using errcode='PT409';end if;
  select * into person from public.employees where company_id=m.company_id and id=review.employee_id;
  select * into cycle from public.performance_cycles where company_id=m.company_id and id=review.cycle_id;
  old_record:=to_jsonb(review);
  if p_operation='review_save' then
   if review.reviewer_employee_id is distinct from m.employee_id or not hris_private.performance_reviewer_valid(m.company_id,review.employee_id,m.employee_id) then raise exception 'Only the currently assigned reviewer can write the assessment.' using errcode='42501';end if;
   if review.status<>'draft' or cycle.status<>'open' or person.status<>'active' then raise exception 'Only active draft appraisals in open cycles can be edited.' using errcode='23514';end if;
   update public.performance_reviews set rating=nullif(p_record->>'rating','')::smallint,assessment=btrim(coalesce(p_record->>'assessment','')),strengths=btrim(coalesce(p_record->>'strengths','')),development=btrim(coalesce(p_record->>'development','')),revision=revision+1,updated_at=clock_timestamp() where id=review.id returning * into review;kind:='review_saved';
  else
   next_status:=p_record->>'status';
   if cycle.status<>'open' or next_status is null or not((review.status='draft' and next_status in ('submitted','cancelled'))or(review.status='submitted' and next_status in ('draft','published','cancelled'))or(review.status='cancelled' and next_status='draft')) then raise exception 'Choose an available appraisal transition in an open cycle.' using errcode='23514';end if;
   if next_status='submitted' then
    if review.reviewer_employee_id is distinct from m.employee_id or not hris_private.performance_reviewer_valid(m.company_id,review.employee_id,m.employee_id) then raise exception 'Only the current assigned reviewer submits this appraisal.' using errcode='42501';end if;
    if person.status<>'active' or review.rating is null or btrim(review.assessment)='' then raise exception 'Submission needs an active employee, rating and written assessment.' using errcode='23514';end if;
    select coalesce(jsonb_agg(jsonb_build_object('id',pg.id,'title',pg.title,'measure',pg.measure,'starts_on',pg.starts_on,'due_on',pg.due_on,'progress',pg.progress,'status',pg.status,'revision',pg.revision)order by pg.due_on,pg.id),'[]') into snapshot from public.performance_goals pg where pg.company_id=m.company_id and pg.employee_id=review.employee_id and pg.status<>'cancelled' and pg.starts_on<=review.ends_on and pg.due_on>=review.starts_on;
    if jsonb_array_length(snapshot)>100 then raise exception 'The review period contains more than 100 goals. Review its period or cancel obsolete goals.' using errcode='23514';end if;
    update public.performance_reviews set goals_snapshot=snapshot,submitted_at=clock_timestamp() where id=review.id;
   else
    if m.role not in ('administrator','hr_generalist') or review.employee_id=m.employee_id then raise exception 'Another HR or Administrator must manage this appraisal decision.' using errcode='42501';end if;
    if next_status in ('draft','published') and(person.status<>'active' or not hris_private.performance_reviewer_valid(m.company_id,review.employee_id,review.reviewer_employee_id)) then raise exception 'Choose an active employee and an eligible active reviewer before continuing.' using errcode='23514';end if;
    if next_status='published' then update public.performance_reviews set published_at=clock_timestamp(),published_by=m.user_id where id=review.id;end if;
   end if;
   update public.performance_reviews set status=next_status,revision=revision+1,updated_at=clock_timestamp() where id=review.id returning * into review;kind:=case next_status when 'published' then 'published' else 'review_'||next_status end;
  end if;
  result:=to_jsonb(review);event_review:=review.id;event_cycle:=review.cycle_id;
 elsif p_operation='note_save' then
  if m.role not in ('administrator','hr_generalist') then raise exception 'HR notes are private to HR and Administrators.' using errcode='42501';end if;
  if p_record-array['id','expected_revision','review_id','note','reason']<>'{}'::jsonb then raise exception 'Unsupported private note fields.' using errcode='23514';end if;
  select * into review from public.performance_reviews where company_id=m.company_id and id=(p_record->>'review_id')::uuid;
  if review.id is null or review.employee_id=m.employee_id then raise exception 'Another HR or Administrator manages these private appraisal notes.' using errcode='42501';end if;
  select * into hr_note from public.performance_hr_notes where company_id=m.company_id and id=v_id for update;
  if hr_note.id is not null and expected is null then if hris_private.performance_retry(m.company_id,'note',v_id,m.user_id,p_record) then return to_jsonb(hr_note);end if;raise exception 'This note ID exists.' using errcode='PT409';end if;
  if hr_note.id is null then
   if expected is not null then raise exception 'Note not found. Refresh.' using errcode='PT409';end if;
   if exists(select 1 from public.performance_hr_notes where company_id=m.company_id and review_id=review.id) then raise exception 'A private note already exists. Refresh and reopen it.' using errcode='PT409';end if;
   insert into public.performance_hr_notes(id,company_id,review_id,note,created_by)values(v_id,m.company_id,review.id,btrim(coalesce(p_record->>'note','')),m.user_id) returning * into hr_note;create_kind:='note';kind:='note_created';
  else
   if hr_note.revision is distinct from expected then raise exception 'This private note changed. Refresh.' using errcode='PT409';end if;
   if hr_note.review_id<>review.id then raise exception 'Reopen the same appraisal note.' using errcode='23514';end if;
   old_record:=to_jsonb(hr_note);update public.performance_hr_notes set note=btrim(coalesce(p_record->>'note','')),revision=revision+1,updated_at=clock_timestamp() where id=hr_note.id returning * into hr_note;kind:='note_edited';
  end if;
  result:=to_jsonb(hr_note);event_review:=review.id;event_cycle:=review.cycle_id;event_note:=hr_note.id;
 else raise exception 'Unknown performance operation.' using errcode='23514';
 end if;
 if create_kind is not null then insert into hris_private.performance_birth(company_id,kind,record_id,actor_id,payload)values(m.company_id,create_kind,v_id,m.user_id,p_record);end if;
 insert into public.performance_events(company_id,goal_id,cycle_id,review_id,note_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)values(m.company_id,event_goal,event_cycle,event_review,event_note,kind,why,m.user_id,actor_name,m.role,old_record,result);
 return result;
exception when invalid_text_representation or datetime_field_overflow or invalid_datetime_format or numeric_value_out_of_range then raise exception 'Enter valid performance dates, identifiers and whole-number ratings/progress.' using errcode='23514';
end;$$;
revoke all on function hris_private.manage_performance(text,jsonb) from public,anon,authenticated;
grant execute on function hris_private.manage_performance(text,jsonb) to authenticated;
create function public.manage_performance(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path='' as $$select hris_private.manage_performance(p_operation,p_record);$$;
revoke all on function public.manage_performance(text,jsonb) from public,anon,authenticated;
grant execute on function public.manage_performance(text,jsonb) to authenticated;
create function hris_private.published_appraisal_immutable()returns trigger language plpgsql security invoker set search_path='' as $$begin if old.status='published' then raise exception 'Published appraisals are immutable. Create a follow-up cycle to correct them.' using errcode='23514';end if;if tg_op='DELETE' then return old;end if;return new;end;$$;
revoke all on function hris_private.published_appraisal_immutable() from public,anon,authenticated;
create trigger performance_review_published_immutable before update or delete on public.performance_reviews for each row execute function hris_private.published_appraisal_immutable();
create function hris_private.exit_performance_guard()returns trigger language plpgsql security definer set search_path='' as $$begin
 if new.status='finalized' and old.status is distinct from new.status and exists(select 1 from public.performance_reviews r where r.company_id=new.company_id and r.status in ('draft','submitted') and(r.employee_id=new.employee_id or r.reviewer_employee_id=new.employee_id)) then raise exception 'Resolve, cancel or reassign unfinished appraisals before finalizing this employee exit.' using errcode='23514';end if;return new;end;$$;
revoke all on function hris_private.exit_performance_guard() from public,anon,authenticated;
create trigger exit_performance_readiness before update of status on public.exit_cases for each row execute function hris_private.exit_performance_guard();
create or replace function hris_private.exit_readiness(p_case uuid) returns jsonb
language plpgsql security definer set search_path='' as $$
declare m public.company_members;c public.exit_cases;today date;begin
 select * into m from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Only active HR and Administrators can review exit readiness.' using errcode='42501';end if;
 select * into c from public.exit_cases where id=p_case and company_id=m.company_id;
 if c.id is null then raise exception 'Exit case not found.' using errcode='42501';end if;
 select (now() at time zone timezone)::date into today from public.companies where id=m.company_id;
 return jsonb_build_object('case_id',c.id,'company_id',m.company_id,
 'case_revision',c.revision,'last_day_reached',c.last_working_on<=today,
 'employee_active',exists(select 1 from public.employees where company_id=m.company_id and id=c.employee_id and status='active'),
 'open_required_tasks',(select count(*)from public.clearance_tasks where company_id=m.company_id and case_id=c.id and required and status='open'),
 'active_direct_reports',(select count(*)from public.employees where company_id=m.company_id and reports_to=c.employee_id and status='active'),
 'open_attendance',(select count(*)from public.attendance where company_id=m.company_id and employee_id=c.employee_id and checked_out_at is null),
 'pending_leave',(select count(*)from public.leave_requests where company_id=m.company_id and employee_id=c.employee_id and status='pending'),
 'pending_attendance_requests',(select count(*)from public.attendance_requests where company_id=m.company_id and employee_id=c.employee_id and status='pending'),
 'pending_overtime',(select count(*)from public.overtime_requests where company_id=m.company_id and employee_id=c.employee_id and status='pending'),
 'pending_assigned_leave',(select count(*)from public.leave_requests r where r.company_id=m.company_id and r.status='pending' and exists(select 1 from jsonb_array_elements(r.approval_steps) s where s->>'approved_by' is null and s->>'kind' in ('manager','person') and s->>'employee_id'=c.employee_id::text)),
 'named_leave_policies',(select count(*)from public.leave_policies p cross join lateral jsonb_array_elements(p.types) t where p.company_id=m.company_id and (t->>'active')::boolean and exists(select 1 from jsonb_array_elements(t->'approval_chain') s where s->>'kind'='person' and s->>'employee_id'=c.employee_id::text)),
 'open_onboarding_tasks',(select count(*)from public.onboarding_tasks where company_id=m.company_id and assigned_employee_id=c.employee_id and case_status='open' and status='open'),
 'open_clearance_tasks',(select count(*)from public.clearance_tasks where company_id=m.company_id and assigned_employee_id=c.employee_id and case_status='open' and status='open'),
 'scheduled_interviews',(select count(*)from public.recruiting_interviews where company_id=m.company_id and interviewer_employee_id=c.employee_id and status='scheduled'),
 'open_performance_reviews',(select count(*)from public.performance_reviews r where r.company_id=m.company_id and r.status in ('draft','submitted') and(r.employee_id=c.employee_id or r.reviewer_employee_id=c.employee_id)),
 'installation_owner',exists(select 1 from public.company_members cm join public.companies co on co.id=cm.company_id and co.owner_id=cm.user_id where cm.company_id=m.company_id and cm.employee_id=c.employee_id),
 'own_account',m.employee_id=c.employee_id);
end;$$;

commit;
