-- Staff imports submit normal pending correction requests. They never impersonate
-- employees, modify attendance or approve a correction. Keep the self API intact.
create table public.attendance_import_batches (
 id uuid primary key,company_id uuid not null references public.companies(id) on delete restrict,
 actor_id uuid not null references auth.users(id) on delete restrict,actor_name text not null,
 actor_role text not null check(actor_role in('administrator','hr_generalist')),
 timezone text not null,row_count integer not null check(row_count between 1 and 100),
 reason text not null check(char_length(btrim(reason)) between 1 and 500),
 reference text not null check(char_length(reference)<=200),
 source_fingerprint text not null check(source_fingerprint~'^[0-9a-f]{64}$'),
 payload_sha256 text not null check(payload_sha256~'^[0-9a-f]{64}$'),request_payload jsonb not null,
 created_at timestamptz not null default clock_timestamp(),unique(company_id,id)
);
create index attendance_import_batches_company_idx on public.attendance_import_batches(company_id,created_at desc,id);
create index attendance_import_batches_actor_idx on public.attendance_import_batches(actor_id);
create table public.attendance_import_items (
 id uuid primary key,company_id uuid not null,batch_id uuid not null,employee_id uuid not null,
 request_id uuid not null unique,source_row integer not null check(source_row between 2 and 501),
 employee_code text not null,employee_name text not null,reference text not null check(char_length(reference)<=200),
 foreign key(company_id,batch_id)references public.attendance_import_batches(company_id,id)on delete restrict,
 foreign key(company_id,employee_id)references public.employees(company_id,id)on delete restrict,
 foreign key(company_id,request_id)references public.attendance_requests(company_id,id)on delete restrict,
 unique(batch_id,source_row),unique(company_id,id)
);
create index attendance_import_items_employee_idx on public.attendance_import_items(company_id,employee_id);
create table public.attendance_import_withdrawals (
 id uuid primary key,company_id uuid not null,item_id uuid not null unique,
 actor_id uuid not null references auth.users(id)on delete restrict,actor_name text not null,
 actor_role text not null check(actor_role in('administrator','hr_generalist')),
 reason text not null check(char_length(btrim(reason))between 1 and 500),request_payload jsonb not null,
 created_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,item_id)references public.attendance_import_items(company_id,id)on delete restrict
);
create index attendance_import_withdrawals_company_idx on public.attendance_import_withdrawals(company_id,created_at,id);
create index attendance_import_withdrawals_actor_idx on public.attendance_import_withdrawals(actor_id);
alter table public.attendance_import_batches enable row level security;
alter table public.attendance_import_items enable row level security;
alter table public.attendance_import_withdrawals enable row level security;
revoke all on public.attendance_import_batches,public.attendance_import_items,public.attendance_import_withdrawals from public,anon,authenticated;

create function hris_private.attendance_import_immutable()returns trigger language plpgsql security invoker set search_path=''as $$begin
 raise exception 'Attendance import history is immutable.'using errcode='23514';end;$$;
create trigger attendance_import_batch_immutable before update or delete on public.attendance_import_batches for each row execute function hris_private.attendance_import_immutable();
create trigger attendance_import_item_immutable before update or delete on public.attendance_import_items for each row execute function hris_private.attendance_import_immutable();
create trigger attendance_import_withdrawal_immutable before update or delete on public.attendance_import_withdrawals for each row execute function hris_private.attendance_import_immutable();

create function hris_private.attendance_import_member()returns public.company_members language plpgsql security definer set search_path=''as $$
declare m public.company_members;begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and role in('administrator','hr_generalist');
 if not found then raise exception 'Current HR or Administrator access is required for attendance imports.'using errcode='42501';end if;return m;
end;$$;

create function hris_private.attendance_import_input(p_batch jsonb)returns uuid language plpgsql immutable security invoker set search_path=''as $$
declare bid uuid;r jsonb;rid uuid;line integer;ids uuid[]:='{}';lines integer[]:='{}';begin
 if jsonb_typeof(p_batch)is distinct from'object'then raise exception 'Use an attendance import object.'using errcode='23514';end if;
 if jsonb_typeof(p_batch->'rows')is distinct from'array'then raise exception 'Include the attendance import rows.'using errcode='23514';end if;
 if jsonb_typeof(p_batch)is distinct from'object'or octet_length(p_batch::text)>262144
  or(select count(*)from jsonb_object_keys(p_batch))<>5 or not(p_batch?&array['id','expected_timezone','reason','reference','rows'])
  or jsonb_typeof(p_batch->'id')is distinct from'string'or jsonb_typeof(p_batch->'expected_timezone')is distinct from'string'
  or jsonb_typeof(p_batch->'reason')is distinct from'string'or jsonb_typeof(p_batch->'reference')is distinct from'string'
  or char_length(btrim(p_batch->>'reason'))not between 1 and 500 or(p_batch->>'reason')~'[[:cntrl:]]'
  or char_length(p_batch->>'reference')>200 or(p_batch->>'reference')~'[[:cntrl:]]'
  or char_length(p_batch->>'expected_timezone')not between 1 and 100 or jsonb_typeof(p_batch->'rows')is distinct from'array'
  or jsonb_array_length(p_batch->'rows')not between 1 and 100 then raise exception 'Use a reviewed attendance import with 1–100 rows, a reason and optional reference.'using errcode='23514';end if;
 bid:=(p_batch->>'id')::uuid;if bid is null or bid='00000000-0000-0000-0000-000000000000'::uuid then raise exception 'An import identity is required.'using errcode='23514';end if;
 for r in select value from jsonb_array_elements(p_batch->'rows')loop
  if jsonb_typeof(r)is distinct from'object'then raise exception 'Use attendance import row objects.'using errcode='23514';end if;
  if jsonb_typeof(r)is distinct from'object'or(select count(*)from jsonb_object_keys(r))<>8
   or not(r?&array['id','source_row','employee_code','work_date','checked_in_at','checked_out_at','reason','reference'])
   or jsonb_typeof(r->'id')is distinct from'string'or jsonb_typeof(r->'source_row')is distinct from'number'
   or coalesce(r->>'source_row','')!~'^[0-9]{1,3}$'
   or jsonb_typeof(r->'employee_code')is distinct from'string'or char_length(r->>'employee_code')not between 1 and 50
   or(r->>'employee_code')~'[[:cntrl:]]'or jsonb_typeof(r->'work_date')is distinct from'string'
   or char_length(r->>'work_date')>20 or jsonb_typeof(r->'checked_in_at')is distinct from'string'or char_length(r->>'checked_in_at')>40
   or jsonb_typeof(r->'checked_out_at')not in('null','string')or char_length(r->>'checked_out_at')>40
   or jsonb_typeof(r->'reason')is distinct from'string'or jsonb_typeof(r->'reference')is distinct from'string'
   or char_length(r->>'reason')>500 or char_length(r->>'reference')>200 then raise exception 'Use the attendance template fields and bounded source rows.'using errcode='23514';end if;
  rid:=(r->>'id')::uuid;line:=(r->>'source_row')::integer;
  if rid is null or rid='00000000-0000-0000-0000-000000000000'::uuid or rid=any(ids)or line not between 2 and 501 or line=any(lines)then raise exception 'Import row identities and source row numbers must be distinct.'using errcode='23514';end if;
  ids:=array_append(ids,rid);lines:=array_append(lines,line);
 end loop;return bid;
exception when invalid_text_representation or numeric_value_out_of_range then raise exception 'Use valid import/row identities and source row numbers.'using errcode='23514';end;$$;

-- A pure proposal check for an explicitly authorized subject. The public self-only
-- attendance_request endpoint is not changed or called with a forged identity.
create function hris_private.attendance_import_row(cid uuid,p_row jsonb,p_now timestamptz)returns jsonb language plpgsql stable security definer set search_path=''as $$
declare e public.employees;a public.attendance;zone text;day date;tin timestamptz;tout timestamptz;plan jsonb;pending jsonb;breaks jsonb;overlap_records jsonb;
 problem text;errors jsonb:='[]';result jsonb;begin
 select timezone into strict zone from public.companies where id=cid;
 begin
  if btrim(p_row->>'reason')=''or(p_row->>'reason')~'[[:cntrl:]]'or(p_row->>'reference')~'[[:cntrl:]]'then raise exception 'Enter a row reason and a reference without control characters.'using errcode='23514';end if;
  select *into e from public.employees where company_id=cid and upper(employee_code)=upper(btrim(p_row->>'employee_code'));
  if not found or e.status<>'active'then raise exception 'Employee code must match one active employee in this company.'using errcode='23514';end if;
  if(p_row->>'work_date')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'
   or(p_row->>'checked_in_at')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]([.][0-9]{1,6})?(Z|[+-](0[0-9]|1[0-3]):[0-5][0-9]|[+-]14:00)$'
   or(p_row->>'checked_in_at')~'-00:00$'
   or(p_row->>'checked_out_at'is not null and((p_row->>'checked_out_at')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]([.][0-9]{1,6})?(Z|[+-](0[0-9]|1[0-3]):[0-5][0-9]|[+-]14:00)$'or(p_row->>'checked_out_at')~'-00:00$'))then raise exception 'Use an ISO work date and timestamps with seconds, at most six fractional digits and a known offset within 14 hours.'using errcode='23514';end if;
  day:=(p_row->>'work_date')::date;tin:=(p_row->>'checked_in_at')::timestamptz;tout:=(p_row->>'checked_out_at')::timestamptz;
  select *into a from public.attendance where company_id=cid and employee_id=e.id and work_date=day;
  plan:=case when a.id is null then hris_private.shift_plan_at(cid,e.id,day)else a.planned_shift end;
  select coalesce(jsonb_agg(to_jsonb(r)order by r.id),'[]')into pending from public.attendance_requests r where r.company_id=cid and r.employee_id=e.id and r.work_date=day and r.status='pending';
  select coalesce(jsonb_agg(to_jsonb(b)order by b.id),'[]')into breaks from public.attendance_breaks b where b.attendance_id=a.id and not b.voided;
  if day<date'1900-01-01'or day>(p_now at time zone zone)::date or tin>p_now or tout>p_now or tout<tin
   or((tin at time zone zone)::date<>day and not coalesce(hris_private.shift_contains_clock(plan,day,tin),false))then raise exception 'Use valid past/current times in the work date or its dated overnight shift; checkout must follow check-in.'using errcode='23514';end if;
  if e.joined_on>day then raise exception 'Attendance cannot precede the employee hire date.'using errcode='23514';end if;
  select coalesce(jsonb_agg(to_jsonb(other)order by other.id),'[]')into overlap_records from public.attendance other where other.company_id=cid and other.employee_id=e.id and other.id is distinct from a.id
   and(other.work_date=day or(other.checked_out_at is null and tout is null)or tstzrange(other.checked_in_at,other.checked_out_at,'[)')&&tstzrange(tin,tout,'[)'));
  if a.id is not null and a.checked_in_at=tin and a.checked_out_at is not distinct from tout then raise exception 'Change the check-in or checkout before requesting a correction.'using errcode='23514';end if;
  if jsonb_array_length(pending)>0 then raise exception 'A pending correction request already exists for this employee and date.'using errcode='23514';end if;
  if jsonb_array_length(overlap_records)>0 then raise exception 'Proposed times overlap another attendance record or open shift.'using errcode='23514';end if;
  if exists(select 1 from public.attendance_breaks b where b.attendance_id=a.id and not b.voided and(b.started_at<tin or(tout is not null and(b.ended_at is null or b.ended_at>tout))))then raise exception 'Resolve break times outside the proposed attendance first.'using errcode='23514';end if;
 exception when check_violation then get stacked diagnostics problem=message_text;errors:=jsonb_build_array(problem);
  when invalid_text_representation or invalid_datetime_format or datetime_field_overflow or numeric_value_out_of_range or data_exception then errors:=jsonb_build_array('Use valid calendar dates and explicit ISO timestamp offsets.');
 end;
 result:=jsonb_build_object('id',(p_row->>'id')::uuid,'source_row',(p_row->>'source_row')::integer,'employee_code',coalesce(e.employee_code,upper(btrim(p_row->>'employee_code'))),'employee_id',e.id,'employee_name',e.full_name,
  'work_date',day,'proposed_in',tin,'proposed_out',tout,'base_revision',a.revision,'errors',errors);
 return jsonb_build_object('preview',result,'employee',to_jsonb(e),'base_record',case when a.id is null then null else to_jsonb(a)end,'planned_shift',plan,'pending',pending,'breaks',breaks,'overlap_records',overlap_records);
end;$$;

create function hris_private.attendance_import_basis(p_batch jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;bid uuid;zone text;today date;rows jsonb:='[]';r jsonb;item jsonb;other jsonb;errors jsonb;public_rows jsonb:='[]';ready boolean:=true;body jsonb;fingerprint text;begin
 m:=hris_private.attendance_import_member();bid:=hris_private.attendance_import_input(p_batch);
 select timezone into strict zone from public.companies where id=m.company_id;today:=(clock_timestamp()at time zone zone)::date;
 if p_batch->>'expected_timezone'is distinct from zone then raise exception 'Company time zone changed. Refresh and reopen the import.'using errcode='PT409';end if;
 if(select count(*)from public.attendance_import_batches where company_id=m.company_id)>=500 or(select count(*)from public.attendance_import_items where company_id=m.company_id)+jsonb_array_length(p_batch->'rows')>5000 then raise exception 'Import history limit reached. Review retained history before adding batches.'using errcode='23514';end if;
 for r in select value from jsonb_array_elements(p_batch->'rows')loop rows:=rows||jsonb_build_array(hris_private.attendance_import_row(m.company_id,r,clock_timestamp()));end loop;
 for item in select value from jsonb_array_elements(rows)loop
  errors:=item#>'{preview,errors}';
  for other in select value from jsonb_array_elements(rows)where value#>>'{preview,id}'<>item#>>'{preview,id}'loop
   if item#>>'{preview,employee_id}'is not null and item#>>'{preview,employee_id}'=other#>>'{preview,employee_id}'then
    if item#>>'{preview,work_date}'is not null and item#>>'{preview,work_date}'=other#>>'{preview,work_date}'then errors:=errors||jsonb_build_array('Duplicate employee and work date in this file.');exit;end if;
    if jsonb_array_length(item#>'{preview,errors}')=0 and jsonb_array_length(other#>'{preview,errors}')=0 and item#>>'{preview,proposed_in}'is not null and other#>>'{preview,proposed_in}'is not null
     and tstzrange((item#>>'{preview,proposed_in}')::timestamptz,(item#>>'{preview,proposed_out}')::timestamptz,'[)')&&tstzrange((other#>>'{preview,proposed_in}')::timestamptz,(other#>>'{preview,proposed_out}')::timestamptz,'[)')then errors:=errors||jsonb_build_array('Proposed intervals overlap another row in this file.');exit;end if;
   end if;
  end loop;
  if jsonb_array_length(errors)>0 then ready:=false;end if;
  public_rows:=public_rows||jsonb_build_array(jsonb_set(item->'preview','{errors}',errors));
 end loop;
 body:=jsonb_build_object('batch',p_batch,'member',to_jsonb(m),'timezone',zone,'as_of',today,'rows',rows,'public_rows',public_rows);
 fingerprint:=encode(sha256(convert_to(body::text,'UTF8')),'hex');
 return jsonb_build_object('public',jsonb_build_object('version','attendance-import-v1','company_id',m.company_id,'user_id',m.user_id,'role',m.role,'as_of',today,'timezone',zone,'batch_id',bid,'fingerprint',fingerprint,'ready',ready,'rows',public_rows),'private_rows',rows);
end;$$;
create function hris_private.preview_attendance_import(p_batch jsonb)returns jsonb language sql security definer set search_path=''as $$select hris_private.attendance_import_basis(p_batch)->'public';$$;

create function hris_private.attendance_import_receipt(bid uuid)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;b public.attendance_import_batches;items jsonb;begin
 m:=hris_private.attendance_import_member();select *into b from public.attendance_import_batches where id=bid and company_id=m.company_id;
 if not found or b.actor_id<>m.user_id then raise exception 'This import receipt belongs to its original staff actor.'using errcode='42501';end if;
 select coalesce(jsonb_agg(jsonb_build_object('id',i.id,'source_row',i.source_row,'employee_id',i.employee_id,'request_id',i.request_id)order by i.source_row),'[]')into items from public.attendance_import_items i where i.batch_id=b.id;
 return jsonb_build_object('version','attendance-import-receipt-v1','company_id',b.company_id,'user_id',m.user_id,'role',m.role,'batch_id',b.id,'payload_sha256',b.payload_sha256,'row_count',b.row_count,'items',items);
end;$$;
create function hris_private.post_attendance_import(p_batch jsonb,p_fingerprint text)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;bid uuid;old public.attendance_import_batches;basis jsonb;item jsonb;input jsonb;pr jsonb;rid uuid;actor text;begin
 m:=hris_private.attendance_import_member();bid:=hris_private.attendance_import_input(p_batch);
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));m:=hris_private.attendance_import_member();
 select *into old from public.attendance_import_batches where id=bid;
 if found then
  if old.company_id<>m.company_id or old.actor_id<>m.user_id or old.request_payload is distinct from p_batch then raise exception 'Import identity was already used. Reopen the file for a new reviewed batch.'using errcode='PT409';end if;
  return hris_private.attendance_import_receipt(bid);
 end if;
 basis:=hris_private.attendance_import_basis(p_batch);
 if p_fingerprint is null or p_fingerprint!~'^[0-9a-f]{64}$'or p_fingerprint is distinct from basis#>>'{public,fingerprint}'then raise exception 'Employee, attendance, break, shift, pending-request or company sources changed. Refresh and preview every row again.'using errcode='PT409';end if;
 if not(basis#>>'{public,ready}')::boolean then raise exception 'Every row must pass its protected preview. No requests were imported.'using errcode='23514';end if;
 select coalesce((select full_name from public.employees where id=m.employee_id and company_id=m.company_id),'Staff')into actor;
 insert into public.attendance_import_batches(id,company_id,actor_id,actor_name,actor_role,timezone,row_count,reason,reference,source_fingerprint,payload_sha256,request_payload)
 values(bid,m.company_id,m.user_id,actor,m.role,p_batch->>'expected_timezone',jsonb_array_length(p_batch->'rows'),btrim(p_batch->>'reason'),btrim(p_batch->>'reference'),p_fingerprint,encode(sha256(convert_to(p_batch::text,'UTF8')),'hex'),p_batch);
 for item in select value from jsonb_array_elements(basis->'private_rows')loop
  pr:=item->'preview';select value into strict input from jsonb_array_elements(p_batch->'rows')where value->>'id'=pr->>'id';
  insert into public.attendance_requests(company_id,employee_id,employee_name,employee_code,submitted_by,work_date,timezone,base_record,planned_shift,proposed_in,proposed_out,reason)
  values(m.company_id,(pr->>'employee_id')::uuid,pr->>'employee_name',pr->>'employee_code',m.user_id,(pr->>'work_date')::date,p_batch->>'expected_timezone',nullif(item->'base_record','null'),nullif(item->'planned_shift','null'),(pr->>'proposed_in')::timestamptz,(pr->>'proposed_out')::timestamptz,btrim(input->>'reason'))returning id into rid;
  insert into public.attendance_request_events(company_id,request_id,employee_id,actor_id,actor_name,actor_role,status,reason)values(m.company_id,rid,(pr->>'employee_id')::uuid,m.user_id,actor,m.role,'pending',btrim(input->>'reason'));
  insert into public.attendance_import_items(id,company_id,batch_id,employee_id,request_id,source_row,employee_code,employee_name,reference)
  values((pr->>'id')::uuid,m.company_id,bid,(pr->>'employee_id')::uuid,rid,(pr->>'source_row')::integer,pr->>'employee_code',pr->>'employee_name',btrim(input->>'reference'));
 end loop;return hris_private.attendance_import_receipt(bid);
exception when unique_violation or serialization_failure then raise exception 'An import, request or source changed. Refresh before retrying this exact batch.'using errcode='PT409';end;$$;

create function hris_private.attendance_import_context()returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;zone text;today date;batches jsonb;items jsonb;withdrawals jsonb;begin
 m:=hris_private.attendance_import_member();select timezone into strict zone from public.companies where id=m.company_id;today:=(clock_timestamp()at time zone zone)::date;
 if(select count(*)from public.attendance_import_batches where company_id=m.company_id)>500 or(select count(*)from public.attendance_import_items where company_id=m.company_id)>5000 then raise exception 'Import history exceeds the bounded desktop view; no rows were truncated.'using errcode='23514';end if;
 select coalesce(jsonb_agg(to_jsonb(b)-'request_payload'order by b.created_at desc,b.id),'[]')into batches from public.attendance_import_batches b where b.company_id=m.company_id;
 select coalesce(jsonb_agg(to_jsonb(i)||jsonb_build_object('work_date',r.work_date,'proposed_in',r.proposed_in,'proposed_out',r.proposed_out,'status',r.status,'request_revision',r.revision)order by b.created_at desc,i.source_row),'[]')into items
  from public.attendance_import_items i join public.attendance_import_batches b on b.id=i.batch_id join public.attendance_requests r on r.company_id=i.company_id and r.id=i.request_id where i.company_id=m.company_id;
 select coalesce(jsonb_agg(to_jsonb(w)-'request_payload'order by w.created_at,w.id),'[]')into withdrawals from public.attendance_import_withdrawals w where w.company_id=m.company_id;
 return jsonb_build_object('version','attendance-import-context-v1','company_id',m.company_id,'user_id',m.user_id,'role',m.role,'as_of',today,'timezone',zone,'batches',batches,'items',items,'withdrawals',withdrawals);
end;$$;

create function hris_private.withdraw_attendance_import(p_change jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;i public.attendance_import_items;b public.attendance_import_batches;r public.attendance_requests;w public.attendance_import_withdrawals;wid uuid;actor text;begin
 m:=hris_private.attendance_import_member();
 if jsonb_typeof(p_change)is distinct from'object'or(select count(*)from jsonb_object_keys(p_change))<>4 or not(p_change?&array['id','item_id','expected_revision','reason'])or octet_length(p_change::text)>2000
  or jsonb_typeof(p_change->'id')is distinct from'string'or jsonb_typeof(p_change->'item_id')is distinct from'string'
  or jsonb_typeof(p_change->'expected_revision')is distinct from'number'or(p_change->>'expected_revision')!~'^[0-9]{1,12}$'
  or jsonb_typeof(p_change->'reason')is distinct from'string'or char_length(btrim(p_change->>'reason'))not between 1 and 500 or(p_change->>'reason')~'[[:cntrl:]]'then raise exception 'Choose a pending imported request and enter a withdrawal reason.'using errcode='23514';end if;
 wid:=(p_change->>'id')::uuid;if wid='00000000-0000-0000-0000-000000000000'::uuid or(p_change->>'item_id')::uuid='00000000-0000-0000-0000-000000000000'::uuid then raise exception 'Choose valid withdrawal and item identities.'using errcode='23514';end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));m:=hris_private.attendance_import_member();
 select *into w from public.attendance_import_withdrawals where id=wid;
 if found then
  if w.company_id<>m.company_id or w.actor_id<>m.user_id or w.request_payload is distinct from p_change then raise exception 'Withdrawal identity was already used.'using errcode='PT409';end if;
  return jsonb_build_object('id',w.id,'item_id',w.item_id);
 end if;
 select *into i from public.attendance_import_items where company_id=m.company_id and id=(p_change->>'item_id')::uuid;
 select *into b from public.attendance_import_batches where company_id=m.company_id and id=i.batch_id;
 if b.id is null or b.actor_id<>m.user_id then raise exception 'Only the original active HR import actor may withdraw its pending request.'using errcode='42501';end if;
 select *into r from public.attendance_requests where company_id=m.company_id and id=i.request_id for update;
 if r.status<>'pending'or r.revision is distinct from(p_change->>'expected_revision')::bigint or r.submitted_by<>m.user_id then raise exception 'This request changed or was already reviewed. Refresh its history.'using errcode='PT409';end if;
 select coalesce((select full_name from public.employees where id=m.employee_id and company_id=m.company_id),'Staff')into actor;
 update public.attendance_requests set status='cancelled',revision=revision+1 where id=r.id;
 insert into public.attendance_request_events(company_id,request_id,employee_id,actor_id,actor_name,actor_role,status,reason)values(m.company_id,r.id,r.employee_id,m.user_id,actor,m.role,'cancelled',btrim(p_change->>'reason'));
 insert into public.attendance_import_withdrawals(id,company_id,item_id,actor_id,actor_name,actor_role,reason,request_payload)values(wid,m.company_id,i.id,m.user_id,actor,m.role,btrim(p_change->>'reason'),p_change);
 return jsonb_build_object('id',wid,'item_id',i.id);
exception when invalid_text_representation or numeric_value_out_of_range or unique_violation then raise exception 'Withdrawal changed or its identity was already used. Refresh before retrying.'using errcode='PT409';end;$$;

create function public.preview_attendance_import(p_batch jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.preview_attendance_import(p_batch);$$;
create function public.post_attendance_import(p_batch jsonb,p_fingerprint text)returns jsonb language sql security invoker set search_path=''as $$select hris_private.post_attendance_import(p_batch,p_fingerprint);$$;
create function public.attendance_import_context()returns jsonb language sql security invoker set search_path=''as $$select hris_private.attendance_import_context();$$;
create function public.withdraw_attendance_import(p_change jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.withdraw_attendance_import(p_change);$$;
revoke all on function hris_private.attendance_import_immutable(),hris_private.attendance_import_member(),hris_private.attendance_import_input(jsonb),hris_private.attendance_import_row(uuid,jsonb,timestamptz),hris_private.attendance_import_basis(jsonb),hris_private.attendance_import_receipt(uuid),hris_private.preview_attendance_import(jsonb),hris_private.post_attendance_import(jsonb,text),hris_private.attendance_import_context(),hris_private.withdraw_attendance_import(jsonb),public.preview_attendance_import(jsonb),public.post_attendance_import(jsonb,text),public.attendance_import_context(),public.withdraw_attendance_import(jsonb)from public,anon,authenticated;
grant execute on function hris_private.preview_attendance_import(jsonb),hris_private.post_attendance_import(jsonb,text),hris_private.attendance_import_context(),hris_private.withdraw_attendance_import(jsonb),public.preview_attendance_import(jsonb),public.post_attendance_import(jsonb,text),public.attendance_import_context(),public.withdraw_attendance_import(jsonb)to authenticated;
