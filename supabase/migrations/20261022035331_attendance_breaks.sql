-- Recorded paid/unpaid breaks, with retained policy and correction history.
begin;
alter table public.shift_templates add column break_rules jsonb not null default '[]'::jsonb
 check(jsonb_typeof(break_rules)='array' and jsonb_array_length(break_rules)<=8);
create function hris_private.validate_break_rules(p_rules jsonb,p_shift_minutes integer) returns void
language plpgsql immutable security invoker set search_path='' as $$
declare rule jsonb; ids uuid[]:='{}'; names text[]:='{}'; rid uuid; label text; mins integer; total integer:=0;
begin
 if jsonb_typeof(p_rules) is distinct from 'array' or jsonb_array_length(p_rules)>8 then raise exception 'Use up to eight break rules.' using errcode='23514';end if;
 for rule in select value from jsonb_array_elements(p_rules) loop
  if jsonb_typeof(rule) is distinct from 'object' or jsonb_typeof(rule->'name') is distinct from 'string' or coalesce(rule->>'minutes','') !~ '^[0-9]{1,3}$'
   or jsonb_typeof(rule->'paid') is distinct from 'boolean' or jsonb_typeof(rule->'required') is distinct from 'boolean' then raise exception 'Enter a name, whole-minute duration and paid/required choices for each break.' using errcode='23514';end if;
  rid:=(rule->>'id')::uuid;label:=btrim(rule->>'name');mins:=(rule->>'minutes')::integer;
  if rid is null or rid='00000000-0000-0000-0000-000000000000'::uuid or rid=any(ids) or char_length(label) not between 1 and 80 or label ~ '[[:cntrl:]]' or lower(label)=any(names) or mins not between 1 and 720 then raise exception 'Use unique break identities/names and durations of 1–720 minutes.' using errcode='23514';end if;
  ids:=array_append(ids,rid);names:=array_append(names,lower(label));total:=total+mins;
 end loop;
 if jsonb_array_length(p_rules)>0 and total>=p_shift_minutes then raise exception 'Combined break durations must be shorter than the shift.' using errcode='23514';end if;
end;
$$;
revoke all on function hris_private.validate_break_rules(jsonb,integer) from public,anon,authenticated;
-- Older clients may omit break_rules. They must preserve existing rules on edits.
do $$
declare body text;
begin
 select pg_get_functiondef('hris_private.save_shift_template(jsonb)'::regprocedure) into body;
 if strpos(body,'expected bigint;')=0 or strpos(body,'if target is null then')=0 or strpos(body,'company_id,name,days,start_minute,end_minute,works_on_holidays,active)')=0 or strpos(body,$fragment$(p_shift->>'active')::boolean) returning * into saved$fragment$)=0 or strpos(body,$fragment$active=(p_shift->>'active')::boolean,revision=revision+1$fragment$)=0 then raise exception 'Unexpected shift handler; review before applying break support.';end if;
 body:=replace(body,'expected bigint;','expected bigint; rule_data jsonb;');
 body:=replace(body,'if target is null then',
  'rule_data:=case when p_shift ? ''break_rules'' then p_shift->''break_rules'' when target is null then ''[]''::jsonb else (select break_rules from public.shift_templates where company_id=member.company_id and id=target) end; perform hris_private.validate_break_rules(rule_data,(end_m+1440-start_m)%1440); if target is null then');
 body:=replace(body,'company_id,name,days,start_minute,end_minute,works_on_holidays,active)','company_id,name,days,start_minute,end_minute,works_on_holidays,active,break_rules)');
 body:=replace(body,$fragment$(p_shift->>'active')::boolean) returning * into saved$fragment$,$fragment$(p_shift->>'active')::boolean,rule_data) returning * into saved$fragment$);
 body:=replace(body,$fragment$active=(p_shift->>'active')::boolean,revision=revision+1$fragment$,$fragment$active=(p_shift->>'active')::boolean,break_rules=rule_data,revision=revision+1$fragment$);
 execute body;
 select pg_get_functiondef('hris_private.assign_shift(jsonb)'::regprocedure) into body;
 if strpos(body,$fragment$'works_on_holidays',template.works_on_holidays,'timezone',tz$fragment$)=0 then raise exception 'Unexpected assignment handler; review break snapshots.';end if;
 body:=replace(body,$fragment$'works_on_holidays',template.works_on_holidays,'timezone',tz$fragment$,$fragment$'works_on_holidays',template.works_on_holidays,'timezone',tz,'break_rules',template.break_rules$fragment$);execute body;
end;
$$;
alter table public.attendance add constraint attendance_break_parent_key unique(company_id,id,employee_id);
create table public.attendance_breaks (
 id uuid primary key default gen_random_uuid(),
 company_id uuid not null references public.companies(id) on delete restrict,
 attendance_id uuid not null,
 employee_id uuid not null,
 rule_id uuid,
 name text not null check(char_length(btrim(name)) between 1 and 80 and name !~ '[[:cntrl:]]'),
 paid boolean not null,
 required boolean not null,
 expected_minutes integer not null check(expected_minutes between 0 and 720),
 started_at timestamptz not null check(isfinite(started_at)),
 ended_at timestamptz check(ended_at is null or (isfinite(ended_at) and ended_at>=started_at)),
 voided boolean not null default false,
 revision bigint not null default 0 check(revision>=0),
 foreign key(company_id,attendance_id,employee_id) references public.attendance(company_id,id,employee_id) on delete restrict,
 exclude using gist(employee_id with =,tstzrange(started_at,ended_at,'[)') with &&) where(not voided)
);
create unique index attendance_breaks_open_idx on public.attendance_breaks(employee_id) where(not voided and ended_at is null);
create unique index attendance_breaks_rule_idx on public.attendance_breaks(attendance_id,rule_id) where(not voided and rule_id is not null);
create index attendance_breaks_company_idx on public.attendance_breaks(company_id,employee_id,attendance_id);
create table public.attendance_break_events (
 id uuid primary key default gen_random_uuid(),
 company_id uuid not null references public.companies(id) on delete restrict,
 attendance_id uuid not null,
 employee_id uuid not null,
 break_id uuid not null references public.attendance_breaks(id) on delete restrict,
 actor_id uuid not null references auth.users(id) on delete restrict,
 actor_name text not null,
 actor_role text not null check(actor_role in ('administrator','hr_generalist','manager','employee')),
 action text not null check(action in ('start','end','record','correct','void')),
 reason text not null check(char_length(btrim(reason)) between 1 and 500 and reason !~ '[[:cntrl:]]'),
 before_record jsonb,
 after_record jsonb not null,
 created_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,attendance_id,employee_id) references public.attendance(company_id,id,employee_id) on delete restrict
);
create index attendance_break_events_attendance_idx on public.attendance_break_events(company_id,attendance_id,created_at,id);
create index attendance_break_events_actor_idx on public.attendance_break_events(actor_id);
create index attendance_break_events_break_idx on public.attendance_break_events(break_id);
alter table public.attendance_breaks enable row level security;
alter table public.attendance_break_events enable row level security;
revoke all on public.attendance_breaks,public.attendance_break_events from public,anon,authenticated;
grant select on public.attendance_breaks,public.attendance_break_events to authenticated;
create policy attendance_breaks_read on public.attendance_breaks for select to authenticated using(hris_private.can_read_employee(company_id,employee_id));
create policy attendance_break_events_read on public.attendance_break_events for select to authenticated using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));
create function hris_private.manage_attendance_break(p_operation text,p_break jsonb) returns uuid
language plpgsql security definer set search_path='' as $$
declare member public.company_members; parent public.attendance; old public.attendance_breaks; saved public.attendance_breaks;
 target uuid; bid uuid; rid uuid; rule jsonb; note text; server_now timestamptz:=clock_timestamp(); actor_label text;
begin
 select * into member from public.company_members where user_id=(select auth.uid()) and active;
 if not found then raise exception 'Active HRIS membership is required.' using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(member.company_id::text,7));
 if hris_private.company_role(member.company_id) is null then raise exception 'Your HRIS access changed.' using errcode='42501';end if;
 if p_operation is null or p_operation not in ('start','end','record','correct','void') or jsonb_typeof(p_break) is distinct from 'object' or octet_length(p_break::text)>10000 then raise exception 'Choose a valid break operation.' using errcode='23514';end if;
 target:=(p_break->>'attendance_id')::uuid;
 select * into parent from public.attendance where company_id=member.company_id and id=target for update;
 if not found then raise exception 'Attendance unavailable in your company.' using errcode='42501';end if;
 if p_operation in ('start','end') then
  if member.employee_id is distinct from parent.employee_id then raise exception 'Routine break controls are for your own attendance.' using errcode='42501';end if;
  if parent.checked_out_at is not null then raise exception 'Only an open attendance record can use routine break controls.' using errcode='23514';end if;
 else
  if hris_private.company_role(member.company_id) not in ('administrator','hr_generalist') then raise exception 'Only HR and Administrators can record, correct or void breaks with a reason.' using errcode='42501';end if;
 end if;
 if (p_break->>'expected_parent_revision')::bigint is distinct from parent.revision then raise exception 'Attendance changed. Refresh and reopen its breaks.' using errcode='PT409';end if;
 if p_operation in ('end','correct','void') then
  bid:=(p_break->>'id')::uuid;
  select * into old from public.attendance_breaks where company_id=member.company_id and attendance_id=parent.id and id=bid and not voided for update;
  if not found then raise exception 'Break changed or is unavailable. Refresh before editing.' using errcode='23514';end if;
  if (p_break->>'expected_revision')::bigint is distinct from old.revision then raise exception 'This break changed. Refresh and reopen it.' using errcode='PT409';end if;
  saved:=old;
 else
  if (p_break->>'id') is not null or (p_break->>'expected_revision') is not null then raise exception 'Refresh before recording a new break.' using errcode='23514';end if;
  rid:=(p_break->>'rule_id')::uuid;
  if rid is not null then select value into rule from jsonb_array_elements(coalesce(parent.planned_shift->'break_rules','[]'::jsonb)) where (value->>'id')::uuid=rid;end if;
  if rid is not null and rule is null then raise exception 'Choose a break from this attendance record saved rules.' using errcode='23514';end if;
  if p_operation='start' and (rule is null or not exists(select 1 from public.employees where company_id=member.company_id and id=parent.employee_id and status='active')) then raise exception 'An active employee and a saved break rule are required.' using errcode='23514';end if;
  saved.id:=gen_random_uuid();saved.company_id:=member.company_id;saved.attendance_id:=parent.id;saved.employee_id:=parent.employee_id;saved.rule_id:=rid;saved.voided:=false;
  if rule is not null then saved.name:=btrim(rule->>'name');saved.paid:=(rule->>'paid')::boolean;saved.required:=(rule->>'required')::boolean;saved.expected_minutes:=(rule->>'minutes')::integer;
  else
   if jsonb_typeof(p_break->'name') is distinct from 'string' or jsonb_typeof(p_break->'paid') is distinct from 'boolean' then raise exception 'Enter a break name and paid/unpaid classification.' using errcode='23514';end if;
   saved.name:=btrim(p_break->>'name');saved.paid:=(p_break->>'paid')::boolean;saved.required:=false;saved.expected_minutes:=0;
  end if;
 end if;
 server_now:=clock_timestamp();
 if p_operation='start' then saved.started_at:=server_now;saved.ended_at:=null;note:='Employee started break';
 elsif p_operation='end' then
  if old.ended_at is not null then raise exception 'This break has already ended.' using errcode='23514';end if;
  saved.ended_at:=server_now;note:='Employee ended break';
 else
  note:=btrim(p_break->>'reason');
  if coalesce(char_length(note),0) not between 1 and 500 or note ~ '[[:cntrl:]]' then raise exception 'Enter a reason of 1–500 characters.' using errcode='23514';end if;
  if p_operation='void' then saved.voided:=true;
  else saved.started_at:=(p_break->>'started_at')::timestamptz;saved.ended_at:=(p_break->>'ended_at')::timestamptz;end if;
 end if;
 if not saved.voided and (saved.started_at is null or not isfinite(saved.started_at) or saved.started_at<parent.checked_in_at or saved.started_at>server_now or (saved.ended_at is not null and (not isfinite(saved.ended_at) or saved.ended_at>server_now or saved.ended_at<saved.started_at)) or (parent.checked_out_at is not null and (saved.ended_at is null or saved.ended_at>parent.checked_out_at))) then raise exception 'Use past/current break times inside the recorded attendance period. End open breaks before checking out.' using errcode='23514';end if;
 saved.revision:=case when old.id is null then 0 else old.revision+1 end;
 if old.id is null then
  insert into public.attendance_breaks select saved.*;
 else update public.attendance_breaks set started_at=saved.started_at,ended_at=saved.ended_at,voided=saved.voided,revision=saved.revision where id=saved.id;end if;
 select coalesce((select full_name from public.employees where company_id=member.company_id and id=member.employee_id),'Staff') into actor_label;
 insert into public.attendance_break_events(company_id,attendance_id,employee_id,break_id,actor_id,actor_name,actor_role,action,reason,before_record,after_record)
 values(member.company_id,parent.id,parent.employee_id,saved.id,member.user_id,actor_label,hris_private.company_role(member.company_id),p_operation,note,case when old.id is null then null else to_jsonb(old) end,to_jsonb(saved));
 return saved.id;
exception
 when unique_violation then raise exception 'This rule is already recorded or another break is open. Refresh and review its history.' using errcode='23505';
 when exclusion_violation then raise exception 'These break times overlap another break.' using errcode='23P01';
end;
$$;
create function public.manage_attendance_break(p_operation text,p_break jsonb) returns uuid
language sql security invoker set search_path='' as $$select hris_private.manage_attendance_break(p_operation,p_break);$$;
revoke all on function public.manage_attendance_break(text,jsonb),hris_private.manage_attendance_break(text,jsonb) from public,anon,authenticated;
grant execute on function public.manage_attendance_break(text,jsonb),hris_private.manage_attendance_break(text,jsonb) to authenticated;
create function hris_private.guard_attendance_breaks() returns trigger
language plpgsql security invoker set search_path='' as $$
begin
 if exists(select 1 from public.attendance_breaks b where b.company_id=new.company_id and b.attendance_id=new.id and not b.voided
  and (b.started_at<new.checked_in_at or (new.checked_out_at is not null and (b.ended_at is null or b.ended_at>new.checked_out_at)))) then
   raise exception 'End the open break or correct break times before changing attendance or checking out.' using errcode='23514';
 end if;
 return new;
end;
$$;
revoke all on function hris_private.guard_attendance_breaks() from public,anon,authenticated;
create trigger attendance_break_bounds after update on public.attendance for each row execute function hris_private.guard_attendance_breaks();
commit;
