-- Shared employee service requests. No case, payroll or canonical personnel mutation.
begin;
create function hris_private.service_text(p_value text,p_min integer,p_max integer)returns boolean
language sql immutable security invoker set search_path=''as $$
 select coalesce(char_length(p_value)between p_min and p_max
 and p_value=btrim(p_value,chr(9)||chr(10)||chr(11)||chr(12)||chr(13)||chr(32)||chr(133)||chr(160)||chr(5760)||chr(8192)||chr(8193)||chr(8194)||chr(8195)||chr(8196)||chr(8197)||chr(8198)||chr(8199)||chr(8200)||chr(8201)||chr(8202)||chr(8232)||chr(8233)||chr(8239)||chr(8287)||chr(12288))
 and p_value!~('['||chr(1)||'-'||chr(8)||chr(11)||'-'||chr(31)||chr(127)||'-'||chr(159)||']'),false)
$$;

create table hris_private.service_requests (
 id uuid primary key,
 company_id uuid not null references public.companies(id) on delete restrict,
 employee_id uuid not null references public.employees(id) on delete restrict,
 requester_code text not null check(hris_private.service_text(requester_code,1,40)),
 requester_name text not null check(hris_private.service_text(requester_name,1,200)),
 created_by uuid not null references auth.users(id) on delete restrict,
 assignee_user_id uuid references auth.users(id) on delete restrict,
 assignee_name text,
 code text not null,
 title text not null check(hris_private.service_text(title,3,160)),
 category text not null check(category in('employment_document','information_update','benefits_question','general_support')),
 priority text not null check(priority in('normal','high')),
 status text not null check(status in('submitted','in_progress','waiting_on_employee','completed','cancelled')),
 due_on date check(due_on between date'2000-01-01' and date'2100-12-31'),
 revision bigint not null check(revision>0),
 created_at timestamptz not null, updated_at timestamptz not null,
 check(updated_at>=created_at),
 check((assignee_user_id is null)=(assignee_name is null)),
 check(assignee_name is null or hris_private.service_text(assignee_name,1,200)),
 check(code='SR-'||upper(replace(id::text,'-',''))),
 unique(company_id,code)
);
create index service_requests_company on hris_private.service_requests(company_id,updated_at desc,id);
create index service_requests_owner on hris_private.service_requests(company_id,employee_id,updated_at desc,id);
create index service_requests_assignee on hris_private.service_requests(company_id,assignee_user_id,updated_at desc,id);
create index service_requests_creator on hris_private.service_requests(created_by);
create index service_requests_employee on hris_private.service_requests(employee_id);
create index service_requests_assignee_fk on hris_private.service_requests(assignee_user_id);
create table hris_private.service_events (
 id uuid primary key,
 company_id uuid not null references public.companies(id) on delete restrict,
 request_id uuid not null references hris_private.service_requests(id) on delete restrict,
 actor_id uuid not null references auth.users(id) on delete restrict,
 actor_name text not null check(hris_private.service_text(actor_name,1,200)),
 revision bigint not null check(revision>0),
 kind text not null check(kind in('submit','reply','route','progress','cancel')),
 body text not null check(hris_private.service_text(body,3,12000)),
 previous_state jsonb, current_state jsonb not null, payload jsonb not null, receipt jsonb not null,
 created_at timestamptz not null, unique(request_id,revision)
);
create index service_events_scope on hris_private.service_events(company_id,request_id,revision);
create index service_events_actor on hris_private.service_events(actor_id);
alter table hris_private.service_requests enable row level security;
alter table hris_private.service_events enable row level security;
revoke all on hris_private.service_requests,hris_private.service_events from public,anon,authenticated,service_role;
create function hris_private.keep_service_history() returns trigger
language plpgsql security invoker set search_path='' as $$
begin raise exception 'Service history is retained and immutable.' using errcode='23514'; end; $$;
create trigger service_events_immutable before update or delete on hris_private.service_events for each row execute function hris_private.keep_service_history();
create trigger service_requests_retained before delete on hris_private.service_requests for each row execute function hris_private.keep_service_history();
revoke all on function hris_private.service_text(text,integer,integer),hris_private.keep_service_history() from public,anon,authenticated,service_role;


create function hris_private.service_member(p_staff boolean) returns public.company_members
language plpgsql stable security definer set search_path='' as $$
declare m public.company_members;
begin
 select cm.* into m from public.company_members cm
 join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id and e.status='active'
 where cm.user_id=(select auth.uid()) and cm.active
 and hris_private.has_staff_capability(cm.company_id,'own_records')
 and (not p_staff or hris_private.has_staff_capability(cm.company_id,'desktop'));
 if m.user_id is null or m.user_id='00000000-0000-0000-0000-000000000000'::uuid then
 raise exception 'Current employee service access is required.' using errcode='42501'; end if;
 return m;
end; $$;

create function hris_private.service_context(p_own boolean) returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare m public.company_members; is_hr boolean; scope uuid[]; rows_json jsonb; events_json jsonb; handlers_json jsonb:='[]'::jsonb; result jsonb;
begin
 m:=hris_private.service_member(not p_own);
 is_hr:=not p_own and hris_private.has_staff_capability(m.company_id,'personnel');
 select coalesce(array_agg(r.id),'{}'::uuid[]) into scope from hris_private.service_requests r
 where r.company_id=m.company_id and (r.employee_id=m.employee_id or is_hr or (not p_own and r.assignee_user_id=m.user_id));
 if cardinality(scope)>1000 or (select count(*) from hris_private.service_events where request_id=any(scope))>10000 then
 raise exception 'Service history exceeds its supported budget. Nothing was truncated.' using errcode='54000'; end if;
 select coalesce(jsonb_agg(to_jsonb(r) order by r.updated_at desc,r.id),'[]'::jsonb) into rows_json
 from hris_private.service_requests r where r.id=any(scope);
 select coalesce(jsonb_agg(jsonb_build_object('id',e.id,'company_id',e.company_id,'request_id',e.request_id,
 'actor_id',e.actor_id,'actor_name',e.actor_name,'revision',e.revision,'kind',e.kind,'body',e.body,
 'previous_state',e.previous_state,'current_state',e.current_state,'created_at',e.created_at) order by e.request_id,e.revision),'[]'::jsonb)
 into events_json from hris_private.service_events e where e.request_id=any(scope);
 if is_hr then
 select coalesce(jsonb_agg(jsonb_build_object('user_id',cm.user_id,'employee_id',cm.employee_id,'full_name',e.full_name) order by e.full_name,cm.user_id),'[]'::jsonb)
 into handlers_json from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id and e.status='active'
 where cm.company_id=m.company_id and cm.active and case when cardinality(hris_private.staff_roles_for(cm.company_id,cm.user_id))>0 then 'desktop'=any(hris_private.staff_role_capabilities(hris_private.staff_roles_for(cm.company_id,cm.user_id))) else false end;
 end if;
 result:=jsonb_build_object('schema_version',1,'company_id',m.company_id,'user_id',m.user_id,'employee_id',m.employee_id,
 'audience',case when p_own then 'own' else 'staff' end,'can_route',is_hr,'requests',rows_json,'events',events_json,'handlers',handlers_json);
 if jsonb_array_length(handlers_json)>2000 or pg_column_size(result)>8388608 then
 raise exception 'Service context exceeds its byte budget. Nothing was truncated.' using errcode='54000'; end if;
 return result;
end; $$;

create function hris_private.save_service(p_change jsonb,p_own boolean) returns jsonb
language plpgsql security definer set search_path='' as $$
declare m public.company_members; r hris_private.service_requests; old_state jsonb; prior hris_private.service_events;
 save_id uuid; ticket_id uuid; expected bigint; change jsonb; kind text; body text; employee_id uuid; handler uuid;
 is_hr boolean; owns boolean; handles boolean; next_status text; at_time timestamptz; actor_name text; receipt jsonb; allowed text[];
 requester public.employees; handler_name text;
begin
 m:=hris_private.service_member(not p_own);
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 m:=hris_private.service_member(not p_own);
 is_hr:=not p_own and hris_private.has_staff_capability(m.company_id,'personnel');
 if p_change is null or jsonb_typeof(p_change)<>'object' or pg_column_size(p_change)>24576
 or exists(select 1 from jsonb_object_keys(p_change) k where k not in('id','company_id','actor_id','request_id','expected_revision','change'))
 or (p_change->>'company_id')::uuid is distinct from m.company_id or (p_change->>'actor_id')::uuid is distinct from m.user_id
 or jsonb_typeof(p_change->'expected_revision') is distinct from 'number' or jsonb_typeof(p_change->'change') is distinct from 'object' then
 raise exception 'Use the complete current service request.' using errcode='23514'; end if;
 save_id:=(p_change->>'id')::uuid; ticket_id:=(p_change->>'request_id')::uuid;
 expected:=(p_change->>'expected_revision')::bigint; change:=p_change->'change';kind:=change->>'kind';body:=change->>'body';
 if save_id is null or ticket_id is null or save_id='00000000-0000-0000-0000-000000000000'::uuid
 or ticket_id='00000000-0000-0000-0000-000000000000'::uuid or expected is null or expected<0
 or jsonb_typeof(change->'body') is distinct from 'string' or not hris_private.service_text(body,3,12000) then
 raise exception 'A service identity, revision and shared message are required.' using errcode='23514'; end if;
 allowed:=case kind when 'submit' then array['kind','employee_id','title','category','priority','body']
 when 'reply' then array['kind','body'] when 'route' then array['kind','assignee_user_id','due_on','body']
 when 'progress' then array['kind','status','body'] when 'cancel' then array['kind','body'] else null end;
 if allowed is null or exists(select 1 from jsonb_object_keys(change) k where not k=any(allowed))
 or (p_own and kind not in('submit','reply','cancel')) then
 raise exception 'This service operation is not available in the current workspace.' using errcode='42501'; end if;
 select * into r from hris_private.service_requests x where x.id=ticket_id and x.company_id=m.company_id for update;
 owns:=coalesce(r.employee_id=m.employee_id,false);
 handles:=not p_own and (is_hr or coalesce(r.assignee_user_id=m.user_id,false));
 if r.id is not null and not(owns or handles) then raise exception 'This service request is outside current scope.' using errcode='42501';end if;
 if (kind='route' and not is_hr) or (kind='progress' and not handles)
 or (kind='cancel' and not(owns or is_hr))
 or (kind='submit' and not is_hr and (change->>'employee_id')::uuid is distinct from m.employee_id) then
 raise exception 'Current responsibility does not permit this operation.' using errcode='42501';end if;
 select * into prior from hris_private.service_events e where e.id=save_id;
 if prior.id is not null then
 if prior.actor_id<>m.user_id or prior.company_id<>m.company_id or prior.payload<>p_change or r.id is null then
 raise exception 'This service save identity already has another actor or payload.' using errcode='23514';end if;
 -- The scoped row and current operation admission were rechecked before returning an old receipt.
 return prior.receipt;
 end if;
 at_time:=clock_timestamp();old_state:=case when r.id is null then null else to_jsonb(r) end;
 if kind='submit' then
 employee_id:=(change->>'employee_id')::uuid;
 if expected<>0 or r.id is not null or employee_id is null or (not is_hr and employee_id<>m.employee_id) then
 raise exception 'Use your own employee or authorized HR assistance.' using errcode='42501'; end if;
 select * into requester from public.employees e where e.id=employee_id and e.company_id=m.company_id;
 if requester.id is null then raise exception 'Choose a company employee.' using errcode='23514';end if;
 if jsonb_typeof(change->'title') is distinct from 'string' or not hris_private.service_text(change->>'title',3,160)
 or jsonb_typeof(change->'category') is distinct from 'string' or change->>'category' not in('employment_document','information_update','benefits_question','general_support')
 or jsonb_typeof(change->'priority') is distinct from 'string' or change->>'priority' not in('normal','high') then
 raise exception 'Review the service title, category and priority.' using errcode='23514';end if;
 insert into hris_private.service_requests(id,company_id,employee_id,requester_code,requester_name,created_by,code,title,category,priority,status,revision,created_at,updated_at)
 values(ticket_id,m.company_id,employee_id,requester.employee_code,requester.full_name,m.user_id,'SR-'||upper(replace(ticket_id::text,'-','')),
 change->>'title',change->>'category',change->>'priority','submitted',1,at_time,at_time) returning * into r;
 else
 if r.id is null or r.revision<>expected then raise exception 'The service request changed. Refresh and review it.' using errcode='23514';end if;
 if kind='reply' then
 if r.status in('completed','cancelled') then raise exception 'Terminal service requests must first be reopened by HR.' using errcode='23514';end if;
 elsif kind='route' then
 if not is_hr or r.status in('completed','cancelled') then raise exception 'Only HR may route active work.' using errcode='42501';end if;
 handler:=(change->>'assignee_user_id')::uuid;
 if handler is not null then
 select e.full_name into handler_name from public.company_members cm join public.employees e on e.id=cm.employee_id and e.company_id=cm.company_id and e.status='active'
 where cm.company_id=m.company_id and cm.user_id=handler and cm.active
 and case when cardinality(hris_private.staff_roles_for(cm.company_id,cm.user_id))>0 then 'desktop'=any(hris_private.staff_role_capabilities(hris_private.staff_roles_for(cm.company_id,cm.user_id))) else false end;
 if handler_name is null then raise exception 'Choose an active staff handler.' using errcode='42501';end if;
 end if;
 if handler is not distinct from r.assignee_user_id and (change->>'due_on')::date is not distinct from r.due_on then
 raise exception 'Routing values did not change.' using errcode='23514';end if;
 r.assignee_user_id:=handler;r.assignee_name:=handler_name;r.due_on:=(change->>'due_on')::date;
 elsif kind='progress' then
 next_status:=change->>'status';
 if not handles or jsonb_typeof(change->'status') is distinct from 'string' or next_status is null
 or not(case r.status when 'submitted' then next_status in('in_progress','waiting_on_employee','completed')
 when 'in_progress' then next_status in('waiting_on_employee','completed') when 'waiting_on_employee' then next_status in('in_progress','completed')
 when 'completed' then is_hr and next_status='in_progress' when 'cancelled' then is_hr and next_status='in_progress' else false end) then
 raise exception 'Use a supported handler transition; only HR can reopen terminal work.' using errcode='42501';end if;
 r.status:=next_status;
 elsif kind='cancel' then
 if not(owns or is_hr) or r.status in('completed','cancelled') then raise exception 'Only the requester or HR can cancel active work.' using errcode='42501';end if;
 r.status:='cancelled';
 end if;
 r.revision:=r.revision+1;r.updated_at:=at_time;
 update hris_private.service_requests set assignee_user_id=r.assignee_user_id,assignee_name=r.assignee_name,due_on=r.due_on,status=r.status,revision=r.revision,updated_at=r.updated_at where id=r.id;
 end if;
 receipt:=jsonb_build_object('schema_version',1,'id',save_id,'company_id',m.company_id,'actor_id',m.user_id,'request_id',r.id,'revision',r.revision,'applied_at',at_time);
 select e.full_name into strict actor_name from public.employees e where e.company_id=m.company_id and e.id=m.employee_id;
 insert into hris_private.service_events(id,company_id,request_id,actor_id,actor_name,revision,kind,body,previous_state,current_state,payload,receipt,created_at)
 values(save_id,m.company_id,r.id,m.user_id,actor_name,r.revision,kind,body,old_state,to_jsonb(r),p_change,receipt,at_time);
 return receipt;
end; $$;

create function hris_private.own_service_context() returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare c jsonb; tickets jsonb; result jsonb;
begin
 c:=hris_private.service_context(true);
 select coalesce(jsonb_agg(jsonb_build_object('id',t->'id','code',t->'code','title',t->'title','category',t->'category',
 'priority',t->'priority','status',t->'status','handler_name',t->'assignee_name','due_on',t->'due_on','revision',t->'revision',
 'created_at',t->'created_at','updated_at',t->'updated_at','can_reply',t->>'status' not in('completed','cancelled'),
 'can_cancel',t->>'status' not in('completed','cancelled'),
 'messages',(select coalesce(jsonb_agg(jsonb_build_object('id',e->'id','revision',e->'revision','kind',e->'kind','actor_name',e->'actor_name','body',e->'body',
 'status',e->'current_state'->'status','created_at',e->'created_at') order by (e->>'revision')::bigint),'[]'::jsonb)
 from jsonb_array_elements(c->'events') e where e->>'request_id'=t->>'id')) order by t->>'updated_at' desc,t->>'id'),'[]'::jsonb)
 into tickets from jsonb_array_elements(c->'requests') t;
 result:=jsonb_build_object('schema_version',1,'company_id',c->'company_id','employee_id',c->'employee_id','user_id',c->'user_id','requests',tickets);
 return result;
end; $$;

create function hris_private.save_own_service(p_request jsonb) returns jsonb
language plpgsql security definer set search_path='' as $$
declare m public.company_members; change jsonb; allowed text[]; r jsonb; result jsonb;
begin
 m:=hris_private.service_member(false);
 if p_request is null or jsonb_typeof(p_request)<>'object' or pg_column_size(p_request)>24576
 or exists(select 1 from jsonb_object_keys(p_request) k where k not in('id','request_id','expected_revision','change'))
 or jsonb_typeof(p_request->'change') is distinct from 'object' then
 raise exception 'Use an own service request without staff identity fields.' using errcode='23514';end if;
 change:=p_request->'change';
 allowed:=case change->>'kind' when 'submit' then array['kind','title','category','priority','body'] when 'reply' then array['kind','body'] when 'cancel' then array['kind','body'] else null end;
 if allowed is null or exists(select 1 from jsonb_object_keys(change) k where not k=any(allowed)) then
 raise exception 'This service operation is not available in the portal.' using errcode='42501';end if;
 if change->>'kind'='submit' then change:=change||jsonb_build_object('employee_id',m.employee_id);end if;
 r:=p_request||jsonb_build_object('company_id',m.company_id,'actor_id',m.user_id,'change',change);
 result:=hris_private.save_service(r,true);
 return jsonb_build_object('schema_version',1,'company_id',m.company_id,'employee_id',m.employee_id,'user_id',m.user_id,
 'id',result->'id','request_id',result->'request_id','revision',result->'revision','applied_at',result->'applied_at');
end; $$;

create function public.employee_service_context() returns jsonb language sql stable security invoker set search_path='' as $$select hris_private.service_context(false)$$;
create function public.save_employee_service(p_change jsonb) returns jsonb language sql security invoker set search_path='' as $$select hris_private.save_service(p_change,false)$$;
create function public.portal_service_context() returns jsonb language sql stable security invoker set search_path='' as $$select hris_private.own_service_context()$$;
create function public.portal_save_service(p_request jsonb) returns jsonb language sql security invoker set search_path='' as $$select hris_private.save_own_service(p_request)$$;
revoke all on function hris_private.service_member(boolean),hris_private.service_context(boolean),hris_private.save_service(jsonb,boolean),hris_private.own_service_context(),hris_private.save_own_service(jsonb),public.employee_service_context(),public.save_employee_service(jsonb),public.portal_service_context(),public.portal_save_service(jsonb) from public,anon,authenticated,service_role;
-- Public wrappers are invokers. Their private targets recheck current identity and scope even if called directly.
grant execute on function hris_private.service_context(boolean),hris_private.save_service(jsonb,boolean),hris_private.own_service_context(),hris_private.save_own_service(jsonb),public.employee_service_context(),public.save_employee_service(jsonb),public.portal_service_context(),public.portal_save_service(jsonb) to authenticated;
notify pgrst,'reload schema';
commit;
