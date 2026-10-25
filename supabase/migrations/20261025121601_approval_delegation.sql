-- Temporary operational approval authority. No personnel, account or payroll role
-- is transferred. Original plans, request revisions and financial guards remain.
begin;
set local search_path=public,extensions,pg_catalog;

create table public.approval_delegation_state(
 company_id uuid primary key references public.companies(id) on delete restrict,
 revision bigint not null default 0 check(revision between 0 and 5000)
);
create table public.approval_delegations(
 id uuid primary key,company_id uuid not null references public.companies(id) on delete restrict,
 scope text not null check(scope in('leave','attendance','overtime')),
 principal_id uuid not null references auth.users(id) on delete restrict,
 delegate_id uuid not null references auth.users(id) on delete restrict,
 principal_employee_id uuid not null,delegate_employee_id uuid not null,
 principal_role text not null check(principal_role in('administrator','hr_generalist','manager')),
 delegate_role text not null check(delegate_role in('administrator','hr_generalist','manager')),
 principal_name text not null,delegate_name text not null,
 starts_on date not null,ends_on date not null,
 reason text not null,reference text not null default '',
 created_by uuid not null references auth.users(id) on delete restrict,created_by_name text not null,
 company_revision bigint not null,request_payload jsonb not null,
 created_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),check(principal_id<>delegate_id),check(principal_employee_id<>delegate_employee_id),
 check(ends_on>=starts_on and ends_on-starts_on<366),
 foreign key(company_id,principal_employee_id)references public.employees(company_id,id) on delete restrict,
 foreign key(company_id,delegate_employee_id)references public.employees(company_id,id) on delete restrict
);
create index approval_delegations_delegate_idx on public.approval_delegations(delegate_id,company_id,scope,starts_on,ends_on);
create index approval_delegations_principal_idx on public.approval_delegations(principal_id,company_id,scope,starts_on,ends_on);
create index approval_delegations_creator_idx on public.approval_delegations(created_by);
create index approval_delegations_original_employee_idx on public.approval_delegations(company_id,principal_employee_id);
create index approval_delegations_substitute_employee_idx on public.approval_delegations(company_id,delegate_employee_id);
create table public.approval_delegation_revocations(
 id uuid primary key,company_id uuid not null,
 delegation_id uuid not null unique,reason text not null,
 created_by uuid not null references auth.users(id) on delete restrict,created_by_name text not null,
 company_revision bigint not null,request_payload jsonb not null,
 created_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,delegation_id)references public.approval_delegations(company_id,id) on delete restrict
);
create index approval_delegation_revocations_company_idx on public.approval_delegation_revocations(company_id,created_at,id);
create index approval_delegation_revocations_creator_idx on public.approval_delegation_revocations(created_by);
create table public.approval_delegation_decisions(
 id uuid primary key default gen_random_uuid(),company_id uuid not null,
 delegation_id uuid not null,scope text not null check(scope in('leave','attendance','overtime')),
 request_id uuid not null,request_revision bigint not null,
 decision text not null check(decision in('approve','reject','revoke')),
 principal_id uuid not null references auth.users(id) on delete restrict,
 delegate_id uuid not null references auth.users(id) on delete restrict,
 principal_name text not null,delegate_name text not null,
 created_at timestamptz not null default clock_timestamp(),
 unique(scope,request_id,request_revision),
 foreign key(company_id,delegation_id)references public.approval_delegations(company_id,id) on delete restrict
);
create index approval_delegation_decisions_request_idx on public.approval_delegation_decisions(company_id,scope,request_id);
create index approval_delegation_decisions_delegate_idx on public.approval_delegation_decisions(delegate_id,company_id,created_at);
create index approval_delegation_decisions_principal_idx on public.approval_delegation_decisions(principal_id,company_id,created_at);
create index approval_delegation_decisions_grant_idx on public.approval_delegation_decisions(company_id,delegation_id);

alter table public.approval_delegation_state enable row level security;
alter table public.approval_delegations enable row level security;
alter table public.approval_delegation_revocations enable row level security;
alter table public.approval_delegation_decisions enable row level security;
revoke all on public.approval_delegation_state,public.approval_delegations,public.approval_delegation_revocations,public.approval_delegation_decisions from public,anon,authenticated;
create function hris_private.approval_immutable() returns trigger
language plpgsql set search_path='' as $$begin raise exception 'Approval delegation history is immutable.' using errcode='23514';end;$$;
create trigger approval_delegations_immutable before update or delete on public.approval_delegations for each row execute function hris_private.approval_immutable();
create trigger approval_delegation_revocations_immutable before update or delete on public.approval_delegation_revocations for each row execute function hris_private.approval_immutable();
create trigger approval_delegation_decisions_immutable before update or delete on public.approval_delegation_decisions for each row execute function hris_private.approval_immutable();
revoke all on function hris_private.approval_immutable() from public,anon,authenticated;

-- This is original authority only. It never consults another delegation.
create function hris_private.approval_principal_allowed(p_company uuid,p_scope text,p_employee uuid,p_submitter uuid,p_steps jsonb,p_user uuid)returns boolean
language plpgsql stable security definer set search_path='' as $$
declare m public.company_members;s jsonb;manager uuid;
begin
 select cm.* into m from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id
 where cm.company_id=p_company and cm.user_id=p_user and cm.active and cm.role<>'employee' and e.status='active';
 if not found or m.employee_id=p_employee or (p_submitter is not null and m.user_id=p_submitter) then return false;end if;
 select e.reports_to into manager from public.employees e where e.company_id=p_company and e.id=p_employee;
 if not found then return false;end if;
 if p_scope='leave' then
  select v into s from jsonb_array_elements(p_steps)with ordinality a(v,n)where v->>'approved_by' is null order by n limit 1;
  if s is null then return false;end if;
  return case s->>'kind'
   when 'reviewer' then m.role in('administrator','hr_generalist')or(m.role='manager'and m.employee_id=manager)
   when 'manager' then m.employee_id=(s->>'employee_id')::uuid
   when 'person' then m.employee_id=(s->>'employee_id')::uuid
   when 'hr' then m.role in('administrator','hr_generalist')
   when 'administrator' then m.role='administrator' else false end;
 end if;
 return p_scope in('attendance','overtime')and(m.role in('administrator','hr_generalist')or(m.role='manager'and m.employee_id=manager));
end;$$;

create function hris_private.approval_grant_current(p_id uuid)returns boolean
language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.approval_delegations d
 join public.companies c on c.id=d.company_id
 join public.company_members p on p.company_id=d.company_id and p.user_id=d.principal_id and p.employee_id=d.principal_employee_id and p.role=d.principal_role and p.active
 join public.company_members m on m.company_id=d.company_id and m.user_id=d.delegate_id and m.employee_id=d.delegate_employee_id and m.role=d.delegate_role and m.active
 join public.employees pe on pe.company_id=d.company_id and pe.id=p.employee_id and pe.status='active'
 join public.employees me on me.company_id=d.company_id and me.id=m.employee_id and me.status='active'
 where d.id=p_id and (now()at time zone c.timezone)::date between d.starts_on and d.ends_on
 and not exists(select 1 from public.approval_delegation_revocations r where r.delegation_id=d.id));
$$;

create function hris_private.approval_match(p_company uuid,p_scope text,p_employee uuid,p_submitter uuid,p_steps jsonb)returns uuid
language sql stable security definer set search_path='' as $$
 select d.id from public.approval_delegations d
 where d.company_id=p_company and d.delegate_id=(select auth.uid())and d.scope=p_scope
 and exists(select 1 from public.company_members m where m.company_id=p_company and m.user_id=(select auth.uid())and m.active and m.role<>'employee'and m.employee_id<>p_employee and(p_submitter is null or m.user_id<>p_submitter))
 and hris_private.approval_grant_current(d.id)
 and hris_private.approval_principal_allowed(p_company,p_scope,p_employee,p_submitter,p_steps,d.principal_id)
 order by d.created_at,d.id limit 1;
$$;

create or replace function hris_private.can_act_leave(p_company uuid,p_employee uuid,p_steps jsonb)returns boolean
language sql stable security definer set search_path='' as $$
 select auth.uid()is not null and(hris_private.approval_principal_allowed(p_company,'leave',p_employee,null,p_steps,auth.uid())
 or hris_private.approval_match(p_company,'leave',p_employee,null,p_steps)is not null);
$$;

create function hris_private.operational_can_review(p_scope text,p_id uuid)returns boolean
language plpgsql stable security definer set search_path='' as $$
declare c uuid;e uuid;u uuid;
begin
 if p_scope='attendance' then select r.company_id,r.employee_id,r.submitted_by into c,e,u from public.attendance_requests r where r.id=p_id;
 elsif p_scope='overtime' then select r.company_id,r.employee_id,r.submitted_by into c,e,u from public.overtime_requests r where r.id=p_id;
 else return false;end if;
 return c is not null and auth.uid()is not null and auth.uid()<>u
 and(hris_private.can_review_employee(c,e)or hris_private.approval_match(c,p_scope,e,u,'[]'::jsonb)is not null);
end;$$;

create function hris_private.delegated_request_read(p_scope text,p_id uuid)returns boolean
language plpgsql stable security definer set search_path='' as $$
declare c uuid;e uuid;u uuid;steps jsonb:='[]';state text;member public.company_members;
begin
 if p_scope='leave' then select r.company_id,r.employee_id,null::uuid,r.approval_steps,r.status into c,e,u,steps,state from public.leave_requests r where r.id=p_id;
 elsif p_scope='attendance' then select r.company_id,r.employee_id,r.submitted_by,r.status into c,e,u,state from public.attendance_requests r where r.id=p_id;
 elsif p_scope='overtime' then select r.company_id,r.employee_id,r.submitted_by,r.status into c,e,u,state from public.overtime_requests r where r.id=p_id;
 else return false;end if;
 select m.*into member from public.company_members m join public.employees p on p.company_id=m.company_id and p.id=m.employee_id
 where m.company_id=c and m.user_id=(select auth.uid())and m.active and m.role<>'employee'and p.status='active';
 if not found then return false;end if;
 return exists(select 1 from public.approval_delegation_decisions d where d.company_id=c and d.scope=p_scope and d.request_id=p_id and d.delegate_id=member.user_id)
 or((state='pending'or(p_scope='overtime'and state='approved'))and hris_private.approval_match(c,p_scope,e,u,steps)is not null);
end;$$;

-- Add only request-specific reviewer visibility. Personnel/attendance/pay tables
-- and their RLS are deliberately unchanged.
create policy delegated_leave_read on public.leave_requests for select to authenticated using(hris_private.readable_leave_contract(kind,status)and hris_private.delegated_request_read('leave',id));
create policy delegated_leave_event_read on public.leave_events for select to authenticated using(exists(select 1 from public.leave_requests r where r.id=request_id and hris_private.readable_leave_contract(r.kind,r.status)and hris_private.delegated_request_read('leave',r.id)));
create policy delegated_attendance_request_read on public.attendance_requests for select to authenticated using(hris_private.delegated_request_read('attendance',id));
create policy delegated_attendance_event_read on public.attendance_request_events for select to authenticated using(hris_private.delegated_request_read('attendance',request_id));
create policy delegated_overtime_request_read on public.overtime_requests for select to authenticated using(hris_private.delegated_request_read('overtime',id));
create policy delegated_overtime_event_read on public.overtime_events for select to authenticated using(hris_private.delegated_request_read('overtime',request_id));

-- Modify only the exact original reviewer predicates; keep all attendance,
-- overtime/source-revision and issued-payroll protections around them.
do $$declare body text;needle text;target text;begin
 foreach target in array array['hris_private.attendance_request(jsonb)','hris_private.overtime_request(jsonb)','hris_private.can_apply_attendance_request(jsonb)','hris_private.record_attendance_calculation()']loop
  body:=pg_get_functiondef(target::regprocedure);
  needle:=case when target like '%can_apply%'or target like '%record_attendance_calculation%'then 'hris_private.can_review_employee(r.company_id,r.employee_id)'else 'hris_private.can_review_employee(m.company_id,r.employee_id)'end;
  if strpos(body,needle)=0 or(length(body)-length(replace(body,needle,'')))/length(needle)<>1 then raise exception 'Unexpected original reviewer predicate in %. Review migration compatibility.',target;end if;
  body:=replace(body,needle,case when target like '%overtime%'then 'hris_private.operational_can_review(''overtime'',r.id)'else 'hris_private.operational_can_review(''attendance'',r.id)'end);
  execute body;
 end loop;
end;$$;

alter function hris_private.review_leave(uuid,bigint,boolean,text)rename to review_leave_without_delegation_receipt;
alter function hris_private.attendance_request(jsonb)rename to attendance_request_without_delegation_receipt;
alter function hris_private.overtime_request(jsonb)rename to overtime_request_without_delegation_receipt;
revoke all on function hris_private.review_leave_without_delegation_receipt(uuid,bigint,boolean,text),hris_private.attendance_request_without_delegation_receipt(jsonb),hris_private.overtime_request_without_delegation_receipt(jsonb)from public,anon,authenticated;

create function hris_private.record_delegated_decision(p_delegation uuid,p_scope text,p_request uuid,p_revision bigint,p_decision text)returns void
language sql security definer set search_path='' as $$
 insert into public.approval_delegation_decisions(company_id,delegation_id,scope,request_id,request_revision,decision,principal_id,delegate_id,principal_name,delegate_name)
 select d.company_id,d.id,p_scope,p_request,p_revision,p_decision,d.principal_id,d.delegate_id,d.principal_name,d.delegate_name
 from public.approval_delegations d where d.id=p_delegation and d.scope=p_scope and d.delegate_id=auth.uid();
$$;
create function hris_private.review_leave(p_id uuid,p_revision bigint,p_approve boolean,p_note text)returns uuid
language plpgsql security definer set search_path='' as $$
declare r public.leave_requests;d uuid;result uuid;
begin
 select *into r from public.leave_requests where id=p_id;
 if not found or auth.uid()is null or hris_private.company_role(r.company_id)is null then raise exception 'Leave request unavailable.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(r.company_id::text,7));
 select *into strict r from public.leave_requests where id=p_id for update;
 if not hris_private.approval_principal_allowed(r.company_id,'leave',r.employee_id,null,r.approval_steps,auth.uid())then
  d:=hris_private.approval_match(r.company_id,'leave',r.employee_id,null,r.approval_steps);
 end if;
 result:=hris_private.review_leave_without_delegation_receipt(p_id,p_revision,p_approve,p_note);
 if d is not null then perform hris_private.record_delegated_decision(d,'leave',p_id,r.revision+1,case when p_approve then 'approve'else'reject'end);end if;
 return result;
end;$$;

create function hris_private.approval_operational_request(p_scope text,p_request jsonb)returns uuid
language plpgsql security definer set search_path='' as $$
declare m public.company_members;c uuid;e uuid;u uuid;d uuid;rid uuid;rev bigint;op text;result uuid;
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active;
 if not found then raise exception 'Current account access is required.'using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));
 op:=p_request->>'operation';
 if op in('approve','reject','revoke')then
  rid:=(p_request->>'request_id')::uuid;
  if p_scope='attendance'then select r.company_id,r.employee_id,r.submitted_by,r.revision into c,e,u,rev from public.attendance_requests r where r.id=rid and r.company_id=m.company_id for update;
  elsif p_scope='overtime'then select r.company_id,r.employee_id,r.submitted_by,r.revision into c,e,u,rev from public.overtime_requests r where r.id=rid and r.company_id=m.company_id for update;
  else raise exception 'Invalid operational scope.'using errcode='23514';end if;
  if c is null then raise exception 'Request unavailable in your company.'using errcode='42501';end if;
  if not hris_private.can_review_employee(c,e)then d:=hris_private.approval_match(c,p_scope,e,u,'[]'::jsonb);end if;
 end if;
 if p_scope='attendance'then result:=hris_private.attendance_request_without_delegation_receipt(p_request);
 elsif p_scope='overtime'then result:=hris_private.overtime_request_without_delegation_receipt(p_request);
 else raise exception 'Invalid operational scope.'using errcode='23514';end if;
 if d is not null then perform hris_private.record_delegated_decision(d,p_scope,rid,rev+1,op);end if;
 return result;
end;$$;
create function hris_private.attendance_request(p_request jsonb)returns uuid language sql security invoker set search_path=''as $$select hris_private.approval_operational_request('attendance',p_request);$$;
create function hris_private.overtime_request(p_request jsonb)returns uuid language sql security invoker set search_path=''as $$select hris_private.approval_operational_request('overtime',p_request);$$;
create or replace function public.review_leave(p_id uuid,p_revision bigint,p_approve boolean,p_note text)returns uuid language sql security invoker set search_path=''as $$select hris_private.review_leave(p_id,p_revision,p_approve,p_note);$$;
create or replace function public.attendance_request(p_request jsonb)returns uuid language sql security invoker set search_path=''as $$select hris_private.attendance_request(p_request);$$;
create or replace function public.overtime_request(p_request jsonb)returns uuid language sql security invoker set search_path=''as $$select hris_private.overtime_request(p_request);$$;

create function hris_private.save_approval_delegation(p_change jsonb)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;p public.company_members;d public.company_members;old public.approval_delegations;old_revoke public.approval_delegation_revocations;
 company uuid;today date;rev bigint;rid uuid;gid uuid;op text;chosen_scope text;note text;ref text;starts date;ends date;pn text;dn text;actor text;keys text[];
begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and role='administrator';
 if not found then raise exception 'Only Administrators configure approval delegation.'using errcode='42501';end if;
 company:=m.company_id;perform pg_advisory_xact_lock(hashtextextended(company::text,7));
 if hris_private.company_role(company)is distinct from'administrator'then raise exception 'Your Administrator access changed.'using errcode='42501';end if;
 if jsonb_typeof(p_change)is distinct from'object'or octet_length(p_change::text)>6000 then raise exception 'Enter valid delegation details.'using errcode='23514';end if;
 op:=p_change->>'operation';rid:=(p_change->>'id')::uuid;note:=btrim(p_change->>'reason');
 if rid is null or rid='00000000-0000-0000-0000-000000000000'or jsonb_typeof(p_change->'reason')is distinct from'string'or coalesce(char_length(note),0)not between 1 and 500 or note~'[[:cntrl:]]'
 or jsonb_typeof(p_change->'expected_revision')is distinct from'number'or coalesce(p_change->>'expected_revision','')!~'^[0-9]{1,8}$'then raise exception 'Use a request identity, current revision and reason of 1–500 characters.'using errcode='23514';end if;
 select array_agg(k order by k)into keys from jsonb_object_keys(p_change)k;
 if(op='grant'and exists(select 1 from public.approval_delegation_revocations where id=rid))or(op='revoke'and exists(select 1 from public.approval_delegations where id=rid))then raise exception 'This request identity already belongs to another delegation operation.'using errcode='PT409';end if;
 if op='grant'then
  if keys<>array['delegate_id','ends_on','expected_revision','id','operation','principal_id','reason','reference','scope','starts_on']::text[]then raise exception 'Invalid delegation fields.'using errcode='23514';end if;
  select *into old from public.approval_delegations where id=rid;
  if found then
   if old.company_id<>company or old.created_by<>m.user_id or old.request_payload<>p_change then raise exception 'This request identity has different stored details.'using errcode='PT409';end if;
   return jsonb_build_object('id',rid,'company_revision',old.company_revision);
  end if;
 elsif op='revoke'then
  if keys<>array['delegation_id','expected_revision','id','operation','reason']::text[]then raise exception 'Invalid revocation fields.'using errcode='23514';end if;
  select *into old_revoke from public.approval_delegation_revocations where id=rid;
  if found then
   if old_revoke.company_id<>company or old_revoke.created_by<>m.user_id or old_revoke.request_payload<>p_change then raise exception 'This request identity has different stored details.'using errcode='PT409';end if;
   return jsonb_build_object('id',rid,'company_revision',old_revoke.company_revision);
  end if;
 else raise exception 'Choose grant or revoke.'using errcode='23514';end if;
 insert into public.approval_delegation_state(company_id)values(company)on conflict do nothing;
 select revision into rev from public.approval_delegation_state where company_id=company for update;
 if rev is distinct from(p_change->>'expected_revision')::bigint then raise exception 'Delegation settings changed. Refresh and review the draft.'using errcode='PT409';end if;
 if rev>=5000 then raise exception 'Delegation history limit reached; review retained history before adding records.'using errcode='23514';end if;
 select(now()at time zone c.timezone)::date into today from public.companies c where c.id=company;
 select full_name into actor from public.employees where company_id=company and id=m.employee_id;
 if op='grant'then
  chosen_scope:=p_change->>'scope';ref:=btrim(p_change->>'reference');
  if chosen_scope not in('leave','attendance','overtime')or chosen_scope is null or jsonb_typeof(p_change->'reference')is distinct from'string'or char_length(ref)>200 or ref~'[[:cntrl:]]'
   or coalesce(p_change->>'starts_on','')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'or coalesce(p_change->>'ends_on','')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'then raise exception 'Choose a scope, dates and reference up to 200 characters.'using errcode='23514';end if;
  starts:=(p_change->>'starts_on')::date;ends:=(p_change->>'ends_on')::date;
  if starts<today or ends<starts or ends-starts>=366 or ends>today+366 then raise exception 'Use today or future dates, at most366 days, ending within the next year.'using errcode='23514';end if;
  select cm.*into p from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id and e.status='active'where cm.company_id=company and cm.user_id=(p_change->>'principal_id')::uuid and cm.active and cm.role<>'employee';
  select cm.*into d from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id and e.status='active'where cm.company_id=company and cm.user_id=(p_change->>'delegate_id')::uuid and cm.active and cm.role<>'employee';
  if p.user_id is null or d.user_id is null or p.user_id=d.user_id or p.employee_id=d.employee_id
   or(p.role='administrator'and d.role<>'administrator')or(p.role='hr_generalist'and d.role not in('administrator','hr_generalist'))then raise exception 'Choose distinct active staff; HR needs an HR/Admin substitute and an Administrator needs another Administrator.'using errcode='23514';end if;
  if exists(select 1 from public.approval_delegations g where g.company_id=company and g.principal_id=p.user_id and g.scope=chosen_scope and g.starts_on<=ends and g.ends_on>=starts and not exists(select 1 from public.approval_delegation_revocations r where r.delegation_id=g.id))then raise exception 'This approver already has an overlapping delegation for that workflow. Revoke it first.'using errcode='PT409';end if;
  select full_name into pn from public.employees where company_id=company and id=p.employee_id;
  select full_name into dn from public.employees where company_id=company and id=d.employee_id;
  insert into public.approval_delegations(id,company_id,scope,principal_id,delegate_id,principal_employee_id,delegate_employee_id,principal_role,delegate_role,principal_name,delegate_name,starts_on,ends_on,reason,reference,created_by,created_by_name,company_revision,request_payload)
  values(rid,company,chosen_scope,p.user_id,d.user_id,p.employee_id,d.employee_id,p.role,d.role,pn,dn,starts,ends,note,ref,m.user_id,coalesce(actor,'Administrator'),rev+1,p_change);
 else
  gid:=(p_change->>'delegation_id')::uuid;
  select *into old from public.approval_delegations where company_id=company and id=gid;
  if not found then raise exception 'Delegation unavailable.'using errcode='42501';end if;
  if exists(select 1 from public.approval_delegation_revocations where delegation_id=gid)then raise exception 'This delegation was already revoked. Refresh its history.'using errcode='PT409';end if;
  insert into public.approval_delegation_revocations(id,company_id,delegation_id,reason,created_by,created_by_name,company_revision,request_payload)
  values(rid,company,gid,note,m.user_id,coalesce(actor,'Administrator'),rev+1,p_change);
 end if;
 update public.approval_delegation_state set revision=rev+1 where company_id=company;
 return jsonb_build_object('id',rid,'company_revision',rev+1);
exception when invalid_text_representation or invalid_datetime_format or datetime_field_overflow or numeric_value_out_of_range then raise exception 'Enter valid delegation identities, dates and revision.'using errcode='23514';
end;$$;

create function hris_private.approval_delegation_context()returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare m public.company_members;today date;grants jsonb;audits jsonb;revokes jsonb;receipts jsonb;permits jsonb;staff jsonb;state_revision bigint;
begin
 select cm.*into m from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id and e.status='active'
 where cm.user_id=(select auth.uid())and cm.active and cm.role<>'employee';
 if not found then raise exception 'Current desktop staff access is required.'using errcode='42501';end if;
 select(now()at time zone c.timezone)::date into today from public.companies c where c.id=m.company_id;
 select coalesce((select s.revision from public.approval_delegation_state s where s.company_id=m.company_id),0)into state_revision;
 select coalesce(jsonb_agg(jsonb_build_object('id',d.id,'scope',d.scope,'principal_id',d.principal_id,'delegate_id',d.delegate_id,'principal_employee_id',d.principal_employee_id,'delegate_employee_id',d.delegate_employee_id,'principal_role',d.principal_role,'delegate_role',d.delegate_role,'principal_name',d.principal_name,'delegate_name',d.delegate_name,'starts_on',d.starts_on,'ends_on',d.ends_on,'current',hris_private.approval_grant_current(d.id),'company_revision',d.company_revision)order by d.company_revision),'[]')into grants
 from public.approval_delegations d where d.company_id=m.company_id and(m.role in('administrator','hr_generalist')or m.user_id in(d.principal_id,d.delegate_id));
 select coalesce(jsonb_agg(jsonb_build_object('id',r.id,'delegation_id',r.delegation_id,'created_at',r.created_at,'company_revision',r.company_revision)order by r.company_revision),'[]')into revokes
 from public.approval_delegation_revocations r join public.approval_delegations d on d.id=r.delegation_id where d.company_id=m.company_id and(m.role in('administrator','hr_generalist')or m.user_id in(d.principal_id,d.delegate_id));
 select coalesce(jsonb_agg(jsonb_build_object('id',d.id,'delegation_id',d.delegation_id,'scope',d.scope,'request_id',d.request_id,'request_revision',d.request_revision,'decision',d.decision,'principal_id',d.principal_id,'delegate_id',d.delegate_id,'principal_name',d.principal_name,'delegate_name',d.delegate_name,'created_at',d.created_at)order by d.created_at,d.id),'[]')into receipts
 from public.approval_delegation_decisions d where d.company_id=m.company_id and(m.role in('administrator','hr_generalist')or m.user_id in(d.principal_id,d.delegate_id));
 if exists(select 1 from public.approval_delegations d where d.company_id=m.company_id and d.delegate_id=m.user_id and hris_private.approval_grant_current(d.id))then
 with candidates as(
  select'leave'::text scope,r.id,r.employee_id,null::uuid submitted_by,r.approval_steps steps,r.revision,true source_current from public.leave_requests r where r.company_id=m.company_id and r.status='pending'
  union all select'attendance',r.id,r.employee_id,r.submitted_by,'[]'::jsonb,r.revision,
   r.timezone=c.timezone and r.base_record is not distinct from(case when a.id is null then null else to_jsonb(a)end)
   and(r.base_record is not null or hris_private.shift_plan_at(r.company_id,r.employee_id,r.work_date)is not distinct from r.planned_shift)
   from public.attendance_requests r join public.companies c on c.id=r.company_id left join public.attendance a on a.company_id=r.company_id and a.employee_id=r.employee_id and a.work_date=r.work_date where r.company_id=m.company_id and r.status='pending'
  union all select'overtime',r.id,r.employee_id,r.submitted_by,'[]'::jsonb,r.revision,
   a.id is not null and hris_private.overtime_basis(a)is not distinct from r.basis
   from public.overtime_requests r left join public.attendance a on a.id=r.attendance_id and a.company_id=r.company_id where r.company_id=m.company_id and r.status in('pending','approved')
 ),matched as(select c.*,hris_private.approval_match(m.company_id,c.scope,c.employee_id,c.submitted_by,c.steps)delegation_id from candidates c
 where not hris_private.approval_principal_allowed(m.company_id,c.scope,c.employee_id,c.submitted_by,c.steps,m.user_id))
 select coalesce(jsonb_agg(jsonb_build_object('scope',scope,'request_id',id,'request_revision',revision,'delegation_id',delegation_id,'source_current',source_current)order by scope,id),'[]')into permits from matched where delegation_id is not null;
 else permits:='[]';end if;
 if jsonb_array_length(receipts)>10000 or jsonb_array_length(permits)>10000 then raise exception 'Delegation work/history exceeds the supported limit; no partial result was returned.'using errcode='23514';end if;
 audits:='[]';staff:='[]';
 if m.role in('administrator','hr_generalist')then
  select coalesce(jsonb_agg(v order by n),'[]')into audits from(
   select d.company_revision n,jsonb_build_object('id',d.id,'reason',d.reason,'reference',d.reference,'actor_name',d.created_by_name,'created_at',d.created_at) v from public.approval_delegations d where d.company_id=m.company_id
   union all select r.company_revision,jsonb_build_object('id',r.id,'reason',r.reason,'reference','','actor_name',r.created_by_name,'created_at',r.created_at)from public.approval_delegation_revocations r where r.company_id=m.company_id)h;
 end if;
 if m.role='administrator'then
  select coalesce(jsonb_agg(jsonb_build_object('user_id',cm.user_id,'employee_id',cm.employee_id,'name',e.full_name,'role',cm.role)order by e.full_name,cm.user_id),'[]')into staff
  from public.company_members cm join public.employees e on e.company_id=cm.company_id and e.id=cm.employee_id where cm.company_id=m.company_id and cm.active and cm.role<>'employee'and e.status='active';
 end if;
 return jsonb_build_object('version','approval-delegation-v1','company_id',m.company_id,'user_id',m.user_id,'employee_id',m.employee_id,'role',m.role,'as_of',today,'revision',state_revision,'grants',grants,'revocations',revokes,'receipts',receipts,'permits',permits,'audit',audits,'staff',staff);
end;$$;

create function public.approval_delegation_context()returns jsonb language sql security invoker set search_path=''as $$select hris_private.approval_delegation_context();$$;
create function public.save_approval_delegation(p_change jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.save_approval_delegation(p_change);$$;
revoke all on function hris_private.approval_principal_allowed(uuid,text,uuid,uuid,jsonb,uuid),hris_private.approval_grant_current(uuid),hris_private.approval_match(uuid,text,uuid,uuid,jsonb),hris_private.operational_can_review(text,uuid),hris_private.delegated_request_read(text,uuid),hris_private.record_delegated_decision(uuid,text,uuid,bigint,text),hris_private.approval_operational_request(text,jsonb),hris_private.review_leave(uuid,bigint,boolean,text),hris_private.attendance_request(jsonb),hris_private.overtime_request(jsonb),hris_private.save_approval_delegation(jsonb),hris_private.approval_delegation_context(),public.approval_delegation_context(),public.save_approval_delegation(jsonb)from public,anon,authenticated;
grant execute on function hris_private.delegated_request_read(text,uuid),hris_private.approval_operational_request(text,jsonb),hris_private.review_leave(uuid,bigint,boolean,text),hris_private.attendance_request(jsonb),hris_private.overtime_request(jsonb),hris_private.save_approval_delegation(jsonb),hris_private.approval_delegation_context(),public.approval_delegation_context(),public.save_approval_delegation(jsonb)to authenticated;
comment on table public.approval_delegations is'Immutable dated operational approval assignments. Protected RPC only; never a payroll/account/personnel permission grant.';
notify pgrst,'reload schema';
commit;
