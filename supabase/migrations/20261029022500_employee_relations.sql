-- Confidential case workflow. No automatic canonical employee/payroll changes.
begin;
create function hris_private.relation_text(p_value text,p_min integer,p_max integer)returns boolean
language sql immutable security invoker set search_path=''as $$
 select coalesce(char_length(p_value)between p_min and p_max
 and p_value=btrim(p_value,chr(9)||chr(10)||chr(11)||chr(12)||chr(13)||chr(32)||chr(133)||chr(160)||chr(5760)||chr(8192)||chr(8193)||chr(8194)||chr(8195)||chr(8196)||chr(8197)||chr(8198)||chr(8199)||chr(8200)||chr(8201)||chr(8202)||chr(8232)||chr(8233)||chr(8239)||chr(8287)||chr(12288))
 and p_value!~('['||chr(1)||'-'||chr(8)||chr(11)||'-'||chr(31)||chr(127)||'-'||chr(159)||']'),false)
$$;
create table hris_private.relation_cases(
 id uuid primary key,company_id uuid not null references public.companies(id)on delete restrict,
 employee_id uuid not null references public.employees(id)on delete restrict,
 owner_id uuid not null references auth.users(id)on delete restrict,
 code text not null,title text not null check(char_length(title)between 3 and 160),
 category text not null check(category in('workplace_concern','grievance','conduct','other')),
 priority text not null check(priority in('normal','high')),
 status text not null check(status in('open','in_review','awaiting_follow_up','resolved','closed')),
 revision bigint not null check(revision>0),created_at timestamptz not null,updated_at timestamptz not null,
 unique(company_id,code)
);
create index relation_cases_scope on hris_private.relation_cases(company_id,owner_id,updated_at desc,id);
create index relation_cases_employee on hris_private.relation_cases(employee_id,company_id);
create index relation_cases_owner on hris_private.relation_cases(owner_id,company_id);
create table hris_private.relation_follow_ups(
 id uuid primary key,case_id uuid not null references hris_private.relation_cases(id)on delete restrict,
 company_id uuid not null references public.companies(id)on delete restrict,
 task text not null check(char_length(task)between 3 and 2000),due_on date not null,
 created_by uuid not null references auth.users(id)on delete restrict,created_at timestamptz not null,
 completed_by uuid references auth.users(id)on delete restrict,completed_at timestamptz,
 completion_note text,check((completed_at is null)=(completed_by is null)),
 check((completed_at is null)=(completion_note is null))
);
create index relation_follow_ups_case on hris_private.relation_follow_ups(case_id,company_id,id);
create index relation_follow_ups_company on hris_private.relation_follow_ups(company_id,due_on);
create index relation_follow_ups_creator on hris_private.relation_follow_ups(created_by);
create index relation_follow_ups_completer on hris_private.relation_follow_ups(completed_by);
create table hris_private.relation_events(
 id uuid primary key,company_id uuid not null references public.companies(id)on delete restrict,
 case_id uuid not null references hris_private.relation_cases(id)on delete restrict,
 actor_id uuid not null references auth.users(id)on delete restrict,actor_name text not null,
 revision bigint not null,kind text not null,body text not null,
 previous_state jsonb,current_state jsonb not null,payload jsonb not null,receipt jsonb not null,
 created_at timestamptz not null,unique(case_id,revision)
);
create index relation_events_case on hris_private.relation_events(company_id,case_id,revision);
create index relation_events_actor on hris_private.relation_events(actor_id);
alter table hris_private.relation_cases enable row level security;
alter table hris_private.relation_follow_ups enable row level security;
alter table hris_private.relation_events enable row level security;
revoke all on hris_private.relation_cases,hris_private.relation_follow_ups,hris_private.relation_events from public,anon,authenticated,service_role;
create function hris_private.keep_relation_event()returns trigger language plpgsql security invoker set search_path=''as $$
begin raise exception 'Confidential case history is immutable.'using errcode='23514';end;$$;
create trigger relation_events_immutable before update or delete on hris_private.relation_events for each row execute function hris_private.keep_relation_event();
create trigger relation_cases_retained before delete on hris_private.relation_cases for each row execute function hris_private.keep_relation_event();
create trigger relation_follow_ups_retained before delete on hris_private.relation_follow_ups for each row execute function hris_private.keep_relation_event();

create function hris_private.relation_member()returns public.company_members
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;
begin
 select cm.*into m from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id
 where cm.user_id=(select auth.uid())and cm.active and e.status='active'
 and hris_private.has_staff_capability(cm.company_id,'personnel');
 if m.user_id is null then raise exception 'Current HR or Administrator case access is required.'using errcode='42501';end if;
 return m;
end;$$;

create function hris_private.relation_context()returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;is_admin boolean;scope uuid[];case_rows jsonb;follow_rows jsonb;event_rows jsonb;handlers jsonb;result jsonb;
begin
 m:=hris_private.relation_member();is_admin:='administrator'=any(hris_private.staff_roles_for(m.company_id,m.user_id));
 select coalesce(array_agg(c.id),'{}'::uuid[])into scope from hris_private.relation_cases c where c.company_id=m.company_id and(is_admin or c.owner_id=m.user_id);
 if cardinality(scope)>1000 or(select count(*)from hris_private.relation_events where case_id=any(scope))>10000
 or(select count(*)from hris_private.relation_follow_ups where case_id=any(scope))>5000 then
 raise exception 'Case history exceeds this context budget. Nothing was truncated.'using errcode='54000';end if;
 select coalesce(jsonb_agg(to_jsonb(c)order by c.updated_at desc,c.id),'[]'::jsonb)into case_rows from hris_private.relation_cases c where c.id=any(scope);
 select coalesce(jsonb_agg(to_jsonb(f)order by f.due_on,f.id),'[]'::jsonb)into follow_rows from hris_private.relation_follow_ups f where f.case_id=any(scope);
 select coalesce(jsonb_agg(jsonb_build_object('id',e.id,'company_id',e.company_id,'case_id',e.case_id,'actor_id',e.actor_id,'actor_name',e.actor_name,'revision',e.revision,'kind',e.kind,'body',e.body,'previous_state',e.previous_state,'current_state',e.current_state,'created_at',e.created_at)order by e.created_at,e.revision,e.id),'[]'::jsonb)
 into event_rows from hris_private.relation_events e where e.case_id=any(scope);
 select coalesce(jsonb_agg(jsonb_build_object('user_id',cm.user_id,'employee_id',cm.employee_id,'full_name',e.full_name)order by e.full_name,cm.user_id),'[]'::jsonb)into handlers
 from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id and e.status='active'
 where cm.company_id=m.company_id and cm.active and hris_private.staff_roles_for(cm.company_id,cm.user_id)&&array['administrator','hr_generalist']::text[];
 result:=jsonb_build_object('schema_version',1,'company_id',m.company_id,'user_id',m.user_id,'can_assign',is_admin,'cases',case_rows,'follow_ups',follow_rows,'events',event_rows,'handlers',handlers);
 if jsonb_array_length(handlers)>2000 or pg_column_size(result)>8388608 then raise exception 'Confidential context exceeds its byte budget. Nothing was truncated.'using errcode='54000';end if;
 return result;
end;$$;

create function hris_private.save_relation(p_change jsonb)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;c hris_private.relation_cases;old_state jsonb;prior hris_private.relation_events;
 f hris_private.relation_follow_ups;request_id uuid;case_id uuid;expected bigint;change jsonb;kind text;body text;owner_id uuid;employee_id uuid;
 is_admin boolean;next_status text;at_time timestamptz;receipt jsonb;actor_name text;follow_id uuid;allowed text[];
begin
 m:=hris_private.relation_member();
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 m:=hris_private.relation_member();is_admin:='administrator'=any(hris_private.staff_roles_for(m.company_id,m.user_id));
 if p_change is null or jsonb_typeof(p_change)<>'object'or pg_column_size(p_change)>24576
 or exists(select 1 from jsonb_object_keys(p_change)k where k not in('id','company_id','actor_id','case_id','expected_revision','change'))
 or(p_change->>'company_id')::uuid is distinct from m.company_id or(p_change->>'actor_id')::uuid is distinct from m.user_id
 or jsonb_typeof(p_change->'expected_revision')is distinct from'number' or jsonb_typeof(p_change->'change')is distinct from'object' then raise exception 'Use the complete current case request.'using errcode='23514';end if;
 request_id:=(p_change->>'id')::uuid;case_id:=(p_change->>'case_id')::uuid;expected:=(p_change->>'expected_revision')::bigint;change:=p_change->'change';kind:=change->>'kind';body:=change->>'body';
 if request_id is null or case_id is null or request_id='00000000-0000-0000-0000-000000000000'::uuid or case_id='00000000-0000-0000-0000-000000000000'::uuid or expected is null or expected<0
 or not hris_private.relation_text(body,3,12000)then raise exception 'A case identity, revision and concise nonblank note are required.'using errcode='23514';end if;
 allowed:=case kind when'create'then array['kind','body','employee_id','owner_id','title','category','priority'] when'note'then array['kind','body'] when'status'then array['kind','body','status'] when'assign'then array['kind','body','owner_id'] when'follow_up'then array['kind','body','follow_up_id','due_on'] when'complete'then array['kind','body','follow_up_id'] else null end;
 if allowed is null or exists(select 1 from jsonb_each(change)x where jsonb_typeof(x.value)<>'string') or exists(select 1 from jsonb_object_keys(change)k where not k=any(allowed))then raise exception 'Unsupported case operation or fields.'using errcode='23514';end if;
 select *into c from hris_private.relation_cases x where x.id=case_id for update;
 if c.id is not null and(c.company_id<>m.company_id or(not is_admin and c.owner_id<>m.user_id))then raise exception 'This case is not assigned to your current HR access.'using errcode='42501';end if;
 select *into prior from hris_private.relation_events where id=request_id;
 if prior.id is not null then
  if prior.company_id<>m.company_id or prior.actor_id<>m.user_id or prior.payload<>p_change then raise exception 'This request belongs to another actor or payload.'using errcode='42501';end if;
  return prior.receipt;
 end if;
 old_state:=case when c.id is null then null else to_jsonb(c)end;at_time:=clock_timestamp();
 if kind='create'then
  if c.id is not null or expected<>0 then raise exception 'The case identity already exists. Review the current record.'using errcode='23514';end if;
  owner_id:=(change->>'owner_id')::uuid;employee_id:=(change->>'employee_id')::uuid;
  if owner_id is null or employee_id is null or(not is_admin and owner_id<>m.user_id)
  or not exists(select 1 from public.employees e where e.id=employee_id and e.company_id=m.company_id)
  or not(hris_private.staff_roles_for(m.company_id,owner_id)&&array['administrator','hr_generalist']::text[])then raise exception 'Choose a company employee and an active authorized HR handler.'using errcode='42501';end if;
  if not hris_private.relation_text(change->>'title',3,160)
  or change->>'category'is null or change->>'category'not in('workplace_concern','grievance','conduct','other')
  or change->>'priority'is null or change->>'priority'not in('normal','high')then raise exception 'Review the case title, category and priority.'using errcode='23514';end if;
  insert into hris_private.relation_cases(id,company_id,employee_id,owner_id,code,title,category,priority,status,revision,created_at,updated_at)
  values(case_id,m.company_id,employee_id,owner_id,'ER-'||upper(replace(case_id::text,'-','')),change->>'title',change->>'category',change->>'priority','open',1,at_time,at_time)returning *into c;
 else
  if c.id is null or c.revision<>expected then raise exception 'The case changed. Refresh and review it.'using errcode='23514';end if;
  if c.status='closed'and not(kind='status'and change->>'status'='in_review'and is_admin)then raise exception 'Only an Administrator may reopen a closed case.'using errcode='42501';end if;
  if kind='status'then
   next_status:=change->>'status';
   if next_status is null or not(case c.status when'open'then next_status in('in_review','awaiting_follow_up','resolved')when'in_review'then next_status in('awaiting_follow_up','resolved')when'awaiting_follow_up'then next_status in('in_review','resolved')when'resolved'then next_status in('in_review','closed')when'closed'then next_status='in_review'and is_admin else false end)then raise exception 'Choose a supported case transition.'using errcode='23514';end if;
   if next_status in('resolved','closed')and exists(select 1 from hris_private.relation_follow_ups x where x.case_id=c.id and x.completed_at is null)then raise exception 'Complete the pending follow-ups before resolving or closing the case.'using errcode='23514';end if;
   c.status:=next_status;
  elsif kind='assign'then
   owner_id:=(change->>'owner_id')::uuid;
   if not is_admin or owner_id is null or owner_id=c.owner_id or not(hris_private.staff_roles_for(m.company_id,owner_id)&&array['administrator','hr_generalist']::text[])then raise exception 'Only an Administrator can choose another active HR handler.'using errcode='42501';end if;
   c.owner_id:=owner_id;
  elsif kind='follow_up'then
   follow_id:=(change->>'follow_up_id')::uuid;
   if c.status='resolved'or follow_id is null or follow_id='00000000-0000-0000-0000-000000000000'::uuid or char_length(body)>2000 or change->>'due_on'is null or(change->>'due_on')::date not between date'2000-01-01'and date'2100-12-31' then raise exception 'Review the follow-up identity, task and due date.'using errcode='23514';end if;
   insert into hris_private.relation_follow_ups(id,case_id,company_id,task,due_on,created_by,created_at)values(follow_id,c.id,m.company_id,body,(change->>'due_on')::date,m.user_id,at_time);
  elsif kind='complete'then
   follow_id:=(change->>'follow_up_id')::uuid;select *into f from hris_private.relation_follow_ups x where x.id=follow_id and x.case_id=c.id and x.company_id=m.company_id for update;
   if f.id is null or f.completed_at is not null then raise exception 'Choose a pending follow-up on this case.'using errcode='23514';end if;
   update hris_private.relation_follow_ups set completed_by=m.user_id,completed_at=at_time,completion_note=body where id=f.id;
  elsif kind<>'note'then raise exception 'Unsupported case change.'using errcode='23514';end if;
  c.revision:=c.revision+1;c.updated_at:=at_time;
  update hris_private.relation_cases set owner_id=c.owner_id,status=c.status,revision=c.revision,updated_at=c.updated_at where id=c.id;
 end if;
 receipt:=jsonb_build_object('schema_version',1,'id',request_id,'company_id',m.company_id,'actor_id',m.user_id,'case_id',c.id,'revision',c.revision,'applied_at',at_time);
 select e.full_name into strict actor_name from public.employees e where e.id=m.employee_id and e.company_id=m.company_id;
 insert into hris_private.relation_events(id,company_id,case_id,actor_id,actor_name,revision,kind,body,previous_state,current_state,payload,receipt,created_at)
 values(request_id,m.company_id,c.id,m.user_id,actor_name,c.revision,kind,body,old_state,to_jsonb(c),p_change,receipt,at_time);
 return receipt;
end;$$;
create function public.employee_relation_context()returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.relation_context()$$;
create function public.save_employee_relation(p_change jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.save_relation(p_change)$$;
revoke all on function hris_private.relation_text(text,integer,integer),hris_private.keep_relation_event(),hris_private.relation_member(),hris_private.relation_context(),hris_private.save_relation(jsonb),public.employee_relation_context(),public.save_employee_relation(jsonb)from public,anon,authenticated,service_role;
grant execute on function hris_private.relation_context(),hris_private.save_relation(jsonb),public.employee_relation_context(),public.save_employee_relation(jsonb)to authenticated;
notify pgrst,'reload schema';
commit;
