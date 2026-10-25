-- HR-assisted leave imports use the existing leave engine and approval contract.
-- Preview is a bounded transactional rehearsal: its inner requests/events are
-- ALWAYS rolled back before returning. No shadow balance engine or impersonation.
alter table public.leave_requests add constraint leave_import_request_subject_key unique(company_id,id,employee_id);
create table public.leave_import_batches (
 id uuid primary key,company_id uuid not null references public.companies(id)on delete restrict,
 actor_id uuid not null references auth.users(id)on delete restrict,actor_name text not null,
 actor_role text not null check(actor_role in('administrator','hr_generalist')),timezone text not null,
 row_count integer not null check(row_count between 1 and 100),reason text not null check(char_length(btrim(reason))between 1 and 500),
 reference text not null check(char_length(reference)<=200),source_fingerprint text not null check(source_fingerprint~'^[0-9a-f]{64}$'),
 payload_sha256 text not null check(payload_sha256~'^[0-9a-f]{64}$'),request_payload jsonb not null,
 created_at timestamptz not null default clock_timestamp(),unique(company_id,id)
);
create index leave_import_batches_company_idx on public.leave_import_batches(company_id,created_at desc,id);
create index leave_import_batches_actor_idx on public.leave_import_batches(actor_id);
create table public.leave_import_items (
 id uuid primary key,company_id uuid not null,batch_id uuid not null,employee_id uuid not null,request_id uuid not null unique,
 source_row integer not null check(source_row between 2 and 501),employee_code text not null,employee_name text not null,
 reference text not null check(char_length(reference)<=200),
 foreign key(company_id,batch_id)references public.leave_import_batches(company_id,id)on delete restrict,
 foreign key(company_id,employee_id)references public.employees(company_id,id)on delete restrict,
 foreign key(company_id,request_id,employee_id)references public.leave_requests(company_id,id,employee_id)on delete restrict,
 unique(batch_id,source_row),unique(company_id,id)
);
create index leave_import_items_employee_idx on public.leave_import_items(company_id,employee_id);
alter table public.leave_import_batches enable row level security;
alter table public.leave_import_items enable row level security;
revoke all on public.leave_import_batches,public.leave_import_items from public,anon,authenticated;
create function hris_private.leave_import_immutable()returns trigger language plpgsql security invoker set search_path=''as $$begin
 raise exception 'Leave import history is immutable.'using errcode='23514';end;$$;
create trigger leave_import_batch_immutable before update or delete on public.leave_import_batches for each row execute function hris_private.leave_import_immutable();
create trigger leave_import_item_immutable before update or delete on public.leave_import_items for each row execute function hris_private.leave_import_immutable();
create function hris_private.leave_import_member()returns public.company_members language plpgsql security definer set search_path=''as $$
declare m public.company_members;begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and role in('administrator','hr_generalist');
 if not found or not exists(select 1 from public.employees where company_id=m.company_id and id=m.employee_id and status='active')then raise exception 'Active HR or Administrator access is required for leave imports.'using errcode='42501';end if;return m;
end;$$;
create function hris_private.leave_import_input(p_batch jsonb)returns uuid language plpgsql immutable security invoker set search_path=''as $$
declare bid uuid;r jsonb;rid uuid;line integer;ids uuid[]:='{}';lines integer[]:='{}';begin
 if jsonb_typeof(p_batch)is distinct from'object'then raise exception 'Use a leave import object.'using errcode='23514';end if;
 if jsonb_typeof(p_batch->'rows')is distinct from'array'then raise exception 'Include the leave import rows.'using errcode='23514';end if;
 if octet_length(p_batch::text)>262144 or(select count(*)from jsonb_object_keys(p_batch))<>5 or not(p_batch?&array['id','expected_timezone','reason','reference','rows'])
 or jsonb_typeof(p_batch->'id')is distinct from'string'or jsonb_typeof(p_batch->'expected_timezone')is distinct from'string'
 or jsonb_typeof(p_batch->'reason')is distinct from'string'or jsonb_typeof(p_batch->'reference')is distinct from'string'
 or char_length(btrim(p_batch->>'reason'))not between 1 and 500 or(p_batch->>'reason')~'[[:cntrl:]]'
 or char_length(p_batch->>'reference')>200 or(p_batch->>'reference')~'[[:cntrl:]]'
 or char_length(p_batch->>'expected_timezone')not between 1 and 100 or jsonb_array_length(p_batch->'rows')not between 1 and 100 then raise exception 'Use 1–100 leave rows and a private import note up to 500 characters.'using errcode='23514';end if;
 bid:=(p_batch->>'id')::uuid;if bid is null or bid='00000000-0000-0000-0000-000000000000'::uuid then raise exception 'An import identity is required.'using errcode='23514';end if;
 for r in select value from jsonb_array_elements(p_batch->'rows')loop
  if jsonb_typeof(r)is distinct from'object'then raise exception 'Use leave import row objects.'using errcode='23514';end if;
  if(select count(*)from jsonb_object_keys(r))<>9 or not(r?&array['id','source_row','employee_code','type_code','starts_on','ends_on','day_part','reason','reference'])
   or jsonb_typeof(r->'id')is distinct from'string'or jsonb_typeof(r->'source_row')is distinct from'number'or coalesce(r->>'source_row','')!~'^[0-9]{1,3}$'
   or jsonb_typeof(r->'employee_code')is distinct from'string'or char_length(r->>'employee_code')not between 1 and 50 or(r->>'employee_code')~'[[:cntrl:]]'
   or jsonb_typeof(r->'type_code')is distinct from'string'or char_length(r->>'type_code')not between 1 and 40 or(r->>'type_code')~'[[:cntrl:]]'
   or jsonb_typeof(r->'starts_on')is distinct from'string'or char_length(r->>'starts_on')>20
   or jsonb_typeof(r->'ends_on')is distinct from'string'or char_length(r->>'ends_on')>20
   or jsonb_typeof(r->'day_part')is distinct from'string'or char_length(r->>'day_part')>10
   or jsonb_typeof(r->'reason')is distinct from'string'or char_length(r->>'reason')>500 or(r->>'reason')~'[[:cntrl:]]'
   or jsonb_typeof(r->'reference')is distinct from'string'or char_length(r->>'reference')>200 or(r->>'reference')~'[[:cntrl:]]'then raise exception 'Use the bounded leave template fields and source rows.'using errcode='23514';end if;
  rid:=(r->>'id')::uuid;line:=(r->>'source_row')::integer;
  if rid is null or rid='00000000-0000-0000-0000-000000000000'::uuid or rid=any(ids)or line not between 2 and 501 or line=any(lines)then raise exception 'Use distinct import row identities and source row numbers.'using errcode='23514';end if;
  ids:=array_append(ids,rid);lines:=array_append(lines,line);
 end loop;return bid;
exception when invalid_text_representation or numeric_value_out_of_range then raise exception 'Use valid import/row identities and source row numbers.'using errcode='23514';end;$$;

create function hris_private.leave_import_basis(p_batch jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;company uuid;bid uuid;zone text;today date;source jsonb;rows jsonb:='[]';input jsonb;item jsonb;
 e public.employees;r public.leave_requests;tid uuid;t jsonb;start_day date;end_day date;before_balance jsonb;after_balance jsonb;problem text;
 errors jsonb;ready boolean:=true;span integer:=0;ids uuid[];fingerprint text;begin
 m:=hris_private.leave_import_member();company:=m.company_id;bid:=hris_private.leave_import_input(p_batch);
 perform pg_advisory_xact_lock(hashtextextended(company::text,7));m:=hris_private.leave_import_member();
 if m.company_id<>company then raise exception 'Import company access changed.'using errcode='42501';end if;
 select timezone into strict zone from public.companies where id=company;today:=(clock_timestamp()at time zone zone)::date;
 if p_batch->>'expected_timezone'is distinct from zone then raise exception 'Company time zone changed. Reopen the leave file.'using errcode='PT409';end if;
 if(select count(*)from public.leave_import_batches where company_id=company)>=500 or(select count(*)from public.leave_import_items where company_id=company)+jsonb_array_length(p_batch->'rows')>5000 then raise exception 'Leave import history limit reached; no history is silently truncated.'using errcode='23514';end if;
 if(select count(*)from public.employees where company_id=company)>5000 or(select count(*)from public.company_members where company_id=company)>5000 then raise exception 'Leave import source cohort exceeds the supported preview limit.'using errcode='54000';end if;
 select coalesce(array_agg(distinct emp.id),'{}'::uuid[])into ids from public.employees emp join jsonb_array_elements(p_batch->'rows')v on upper(emp.employee_code)=upper(btrim(v->>'employee_code'))where emp.company_id=company;
 if(select count(*)from public.leave_requests where employee_id=any(ids))>20000 then raise exception 'Leave import request sources exceed the supported preview limit.'using errcode='54000';end if;
 -- Capture sources BEFORE rehearsal. Generated UUIDs/times never enter the
 -- deterministic fingerprint; full existing facts and rule identities do.
 source:=jsonb_build_object('member',to_jsonb(m),'timezone',zone,'as_of',today,
  'employees',(select coalesce(jsonb_agg(to_jsonb(emp)order by emp.id),'[]')from public.employees emp where emp.company_id=company),
  'members',(select coalesce(jsonb_agg(to_jsonb(a)order by a.user_id),'[]')from public.company_members a where a.company_id=company),
  'policy',(select to_jsonb(p)from public.leave_policies p where p.company_id=company),
  'dated_state',(select to_jsonb(s)from public.leave_dated_state s where s.company_id=company),
  'credit_state',(select to_jsonb(s)from public.leave_credit_state s where s.company_id=company),
  'schedules',(select coalesce(jsonb_agg(to_jsonb(s)order by s.id),'[]')from public.leave_policy_schedules s where s.company_id=company),
  'withdrawals',(select coalesce(jsonb_agg(to_jsonb(s)order by s.id),'[]')from public.leave_policy_withdrawals s where s.company_id=company),
  'anchors',(select coalesce(jsonb_agg(to_jsonb(s)order by s.employee_id,s.type_code,s.year),'[]')from public.leave_entitlement_anchors s where s.employee_id=any(ids)),
  'openings',(select coalesce(jsonb_agg(to_jsonb(s)order by s.id),'[]')from public.leave_opening_entries s where s.employee_id=any(ids)),
  'credits',(select coalesce(jsonb_agg(to_jsonb(s)order by s.id),'[]')from public.leave_credit_entries s where s.employee_id=any(ids)),
  'requests',(select coalesce(jsonb_agg(to_jsonb(s)order by s.id),'[]')from public.leave_requests s where s.employee_id=any(ids)),
  'engine',md5(pg_get_functiondef('hris_private.submit_leave(uuid,text,date,date,text,text)'::regprocedure)||pg_get_functiondef('hris_private.leave_balance(uuid,jsonb,integer,date,uuid)'::regprocedure)||pg_get_functiondef('hris_private.leave_balance_detail(uuid,text,integer,date,uuid)'::regprocedure)));
 begin
  for input in select value from jsonb_array_elements(p_batch->'rows')loop
   e:=null;r:=null;t:=null;start_day:=null;end_day:=null;before_balance:=null;after_balance:=null;errors:='[]';
   begin
    select *into e from public.employees where company_id=company and upper(employee_code)=upper(btrim(input->>'employee_code'));
    if not found or e.status<>'active'then raise exception 'Employee code must match one active employee in this company.'using errcode='23514';end if;
    if(input->>'starts_on')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'or(input->>'ends_on')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'then raise exception 'Use ISO dates such as 2026-10-26.'using errcode='23514';end if;
    start_day:=(input->>'starts_on')::date;end_day:=(input->>'ends_on')::date;
    if start_day<date'1900-01-01'or end_day>date'2200-12-31'or start_day>end_day or extract(year from start_day)<>extract(year from end_day)or input->>'day_part'not in('full','am','pm')or(input->>'day_part'<>'full'and start_day<>end_day)then raise exception 'Use supported same-year dates, with one date for a half-day.'using errcode='23514';end if;
    span:=span+(end_day-start_day+1);if span>3660 then raise exception 'The file exceeds 3,660 combined calendar dates. Split it into smaller batches.'using errcode='54000';end if;
    t:=hris_private.leave_type_on(company,input->>'type_code',start_day);
    if t is null or not coalesce((t->>'active')::boolean,false)then raise exception 'Choose a leave type active at the request start.'using errcode='23514';end if;
    if t->>'accrual'<>'unlimited'then before_balance:=hris_private.leave_balance(e.id,t,extract(year from start_day)::integer,start_day);end if;
    tid:=hris_private.submit_leave(e.id,input->>'type_code',start_day,end_day,input->>'day_part',input->>'reason');
    select *into strict r from public.leave_requests where id=tid and company_id=company and employee_id=e.id;
    if t->>'accrual'<>'unlimited'then after_balance:=hris_private.leave_balance(e.id,t,extract(year from start_day)::integer,start_day);end if;
   exception when check_violation or exclusion_violation then get stacked diagnostics problem=message_text;errors:=jsonb_build_array(problem);ready:=false;
    when invalid_text_representation or invalid_datetime_format or datetime_field_overflow then errors:=jsonb_build_array('Use valid calendar dates and leave template values.');ready:=false;
   end;
   item:=jsonb_build_object('id',(input->>'id')::uuid,'source_row',(input->>'source_row')::integer,'employee_code',coalesce(e.employee_code,btrim(input->>'employee_code')),
    'employee_id',e.id,'employee_name',e.full_name,'type_code',input->>'type_code','starts_on',start_day,'ends_on',end_day,'day_part',input->>'day_part',
    'type_name',r.type_name,'charged_units',r.charged_units,'charged_dates',r.charged_dates,'policy_revision',r.policy_revision,'approval_steps',r.approval_steps,
    'balance_before',before_balance,'balance_after',after_balance,'errors',errors);
   rows:=rows||jsonb_build_array(item);
  end loop;
  raise exception 'Roll back the complete leave import rehearsal.'using errcode='P0L52';
 exception when sqlstate'P0L52'then null;
 end;
 fingerprint:=encode(sha256(convert_to(jsonb_build_object('batch',p_batch,'source',source,'rows',rows)::text,'UTF8')),'hex');
 return jsonb_build_object('version','leave-import-v1','company_id',company,'user_id',m.user_id,'role',m.role,'as_of',today,'timezone',zone,'batch_id',bid,'fingerprint',fingerprint,'ready',ready,'rows',rows);
end;$$;
create function hris_private.preview_leave_import(p_batch jsonb)returns jsonb language sql security definer set search_path=''as $$select hris_private.leave_import_basis(p_batch);$$;
create function hris_private.leave_import_receipt(bid uuid)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;b public.leave_import_batches;items jsonb;begin
 m:=hris_private.leave_import_member();select *into b from public.leave_import_batches where id=bid and company_id=m.company_id;
 if not found or b.actor_id<>m.user_id then raise exception 'This leave import receipt belongs to its original actor.'using errcode='42501';end if;
 select coalesce(jsonb_agg(jsonb_build_object('id',i.id,'source_row',i.source_row,'employee_id',i.employee_id,'request_id',i.request_id)order by i.source_row),'[]')into items from public.leave_import_items i where i.batch_id=bid;
 return jsonb_build_object('version','leave-import-receipt-v1','company_id',b.company_id,'user_id',m.user_id,'role',m.role,'batch_id',b.id,'payload_sha256',b.payload_sha256,'row_count',b.row_count,'items',items);
end;$$;
create function hris_private.post_leave_import(p_batch jsonb,p_fingerprint text)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;company uuid;bid uuid;b public.leave_import_batches;preview jsonb;item jsonb;input jsonb;rid uuid;actual public.leave_requests;actor text;begin
 m:=hris_private.leave_import_member();company:=m.company_id;bid:=hris_private.leave_import_input(p_batch);
 perform pg_advisory_xact_lock(hashtextextended(company::text,7));m:=hris_private.leave_import_member();if m.company_id<>company then raise exception 'Company access changed.'using errcode='42501';end if;
 select *into b from public.leave_import_batches where id=bid;
 if found then
  if b.company_id<>company or b.actor_id<>m.user_id or b.request_payload is distinct from p_batch then raise exception 'Leave import identity was already used by another actor or payload.'using errcode='PT409';end if;
  return hris_private.leave_import_receipt(bid);
 end if;
 preview:=hris_private.leave_import_basis(p_batch);
 if p_fingerprint is null or p_fingerprint!~'^[0-9a-f]{64}$'or p_fingerprint is distinct from preview->>'fingerprint'then raise exception 'Leave sources, reservations, approvals or policy changed. Preview every row again.'using errcode='PT409';end if;
 if not(preview->>'ready')::boolean then raise exception 'Every leave row must pass. No requests were imported.'using errcode='23514';end if;
 select full_name into strict actor from public.employees where company_id=company and id=m.employee_id;
 insert into public.leave_import_batches(id,company_id,actor_id,actor_name,actor_role,timezone,row_count,reason,reference,source_fingerprint,payload_sha256,request_payload)
 values(bid,company,m.user_id,actor,m.role,p_batch->>'expected_timezone',jsonb_array_length(p_batch->'rows'),btrim(p_batch->>'reason'),btrim(p_batch->>'reference'),p_fingerprint,encode(sha256(convert_to(p_batch::text,'UTF8')),'hex'),p_batch);
 for item in select value from jsonb_array_elements(preview->'rows')loop
  select value into strict input from jsonb_array_elements(p_batch->'rows')where value->>'id'=item->>'id';
  rid:=hris_private.submit_leave((item->>'employee_id')::uuid,item->>'type_code',(item->>'starts_on')::date,(item->>'ends_on')::date,item->>'day_part',input->>'reason');
  select *into strict actual from public.leave_requests where id=rid and company_id=company;
  if actual.charged_units is distinct from(item->>'charged_units')::integer or actual.charged_dates is distinct from item->'charged_dates' or actual.approval_steps is distinct from item->'approval_steps'or actual.policy_revision is distinct from(item->>'policy_revision')::bigint or actual.type_name is distinct from item->>'type_name'then raise exception 'Leave calculation changed after rehearsal. No requests were imported.'using errcode='PT409';end if;
  insert into public.leave_import_items(id,company_id,batch_id,employee_id,request_id,source_row,employee_code,employee_name,reference)
  values((item->>'id')::uuid,company,bid,(item->>'employee_id')::uuid,rid,(item->>'source_row')::integer,item->>'employee_code',item->>'employee_name',btrim(input->>'reference'));
 end loop;return hris_private.leave_import_receipt(bid);
exception when unique_violation or serialization_failure then raise exception 'A leave import identity or source changed. Retain the exact submission and reload history.'using errcode='PT409';end;$$;
create function hris_private.leave_import_context()returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;zone text;today date;batches jsonb;items jsonb;begin
 m:=hris_private.leave_import_member();select timezone into strict zone from public.companies where id=m.company_id;today:=(clock_timestamp()at time zone zone)::date;
 if(select count(*)from public.leave_import_batches where company_id=m.company_id)>500 or(select count(*)from public.leave_import_items where company_id=m.company_id)>5000 then raise exception 'Leave import history exceeds the supported view; no rows were truncated.'using errcode='54000';end if;
 select coalesce(jsonb_agg(to_jsonb(b)-'request_payload'order by b.created_at desc,b.id),'[]')into batches from public.leave_import_batches b where b.company_id=m.company_id;
 select coalesce(jsonb_agg(to_jsonb(i)||jsonb_build_object('type_code',r.type_code,'type_name',r.type_name,'starts_on',r.starts_on,'ends_on',r.ends_on,'day_part',r.day_part,'charged_units',r.charged_units,'charged_dates',r.charged_dates,'policy_revision',r.policy_revision,'approval_steps',r.approval_steps,'status',r.status,'request_revision',r.revision)order by b.created_at desc,i.source_row),'[]')into items
 from public.leave_import_items i join public.leave_import_batches b on b.id=i.batch_id join public.leave_requests r on r.company_id=i.company_id and r.id=i.request_id where i.company_id=m.company_id;
 return jsonb_build_object('version','leave-import-context-v1','company_id',m.company_id,'user_id',m.user_id,'role',m.role,'as_of',today,'timezone',zone,'batches',batches,'items',items);
end;$$;
create function public.preview_leave_import(p_batch jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.preview_leave_import(p_batch);$$;
create function public.post_leave_import(p_batch jsonb,p_fingerprint text)returns jsonb language sql security invoker set search_path=''as $$select hris_private.post_leave_import(p_batch,p_fingerprint);$$;
create function public.leave_import_context()returns jsonb language sql security invoker set search_path=''as $$select hris_private.leave_import_context();$$;
revoke all on function hris_private.leave_import_immutable(),hris_private.leave_import_member(),hris_private.leave_import_input(jsonb),hris_private.leave_import_basis(jsonb),hris_private.preview_leave_import(jsonb),hris_private.leave_import_receipt(uuid),hris_private.post_leave_import(jsonb,text),hris_private.leave_import_context(),public.preview_leave_import(jsonb),public.post_leave_import(jsonb,text),public.leave_import_context()from public,anon,authenticated;
grant execute on function hris_private.preview_leave_import(jsonb),hris_private.post_leave_import(jsonb,text),hris_private.leave_import_context(),public.preview_leave_import(jsonb),public.post_leave_import(jsonb,text),public.leave_import_context()to authenticated;
