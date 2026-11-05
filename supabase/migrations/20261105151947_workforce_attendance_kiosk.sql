-- Restricted per-launch station grants, private evidence and opt-in method enforcement.
begin;
do $$begin
 if exists(select 1 from pg_available_extensions where name='pgcrypto')then
  create extension if not exists pgcrypto with schema extensions;
 end if;
end $$;
create table hris_private.attendance_stations(
 id uuid primary key,company_id uuid not null references public.companies(id),name text not null check(char_length(name)between 1 and 120),
 active boolean not null default true,revision bigint not null default 0,enrolled_by uuid not null,verified_at timestamptz,created_at timestamptz not null default clock_timestamp()
);
create table hris_private.attendance_station_sessions(
 token_hash text primary key,station_id uuid not null references hris_private.attendance_stations(id),unlocked_by uuid not null,
 expires_at timestamptz not null,revoked_at timestamptz,created_at timestamptz not null default clock_timestamp()
);
create index attendance_station_sessions_station on hris_private.attendance_station_sessions(station_id,expires_at);
create table hris_private.attendance_pins(
 company_id uuid not null,employee_id uuid primary key,pin_hash text not null,failed_count integer not null default 0,
 locked_until timestamptz,updated_at timestamptz not null default clock_timestamp(),foreign key(company_id,employee_id)references public.employees(company_id,id)
);
create table hris_private.kiosk_punch_intents(
 id uuid primary key,company_id uuid not null,employee_id uuid not null,station_id uuid not null,token_hash text not null,
 operation text not null check(operation in('check_in','check_out')),photo_hash text not null check(photo_hash~'^[a-f0-9]{64}$'),
 object_path text not null unique,photo_bytes integer not null check(photo_bytes between 100 and 65536),
 attendance_id uuid not null,expected_revision bigint,policy_snapshot jsonb,work_date date not null,
 status text not null default 'pending' check(status in('pending','committed','expired')),result jsonb,purged_at timestamptz,purged_by uuid,
 created_at timestamptz not null default clock_timestamp(),expires_at timestamptz not null default(clock_timestamp()+interval '3 minutes')
);
create index kiosk_punch_intents_employee on hris_private.kiosk_punch_intents(company_id,employee_id,created_at desc);
create table hris_private.attendance_policy_snapshots(
 attendance_id uuid primary key references public.attendance(id),company_id uuid not null,employee_id uuid not null,
 method text not null check(method in('kiosk','remote')),policy jsonb,created_at timestamptz not null default clock_timestamp()
);
create table hris_private.kiosk_write_context(transaction_id bigint primary key,attendance_id uuid not null,station_id uuid not null);
do $$declare n text;begin
 foreach n in array array['attendance_stations','attendance_station_sessions','attendance_pins','kiosk_punch_intents','attendance_policy_snapshots','kiosk_write_context']loop
  execute format('alter table hris_private.%I enable row level security',n);
  execute format('revoke all on hris_private.%I from public,anon,authenticated,service_role',n);
 end loop;
end $$;
insert into storage.buckets(id,name,public,file_size_limit,allowed_mime_types)values('attendance-evidence','attendance-evidence',false,65536,array['image/jpeg']);

create function hris_private.attendance_station(p_request jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members:=hris_private.work_activity_member();op text:=p_request->>'operation';s hris_private.attendance_stations;
 token text;eid uuid;station uuid:=(p_request->>'id')::uuid;pid text;rid uuid;receipt hris_private.work_activity_receipts;
begin
 if octet_length(p_request::text)>10000 then raise exception 'Station request is too large.'using errcode='23514';end if;
 if op='pin_reset' then
  if not hris_private.has_staff_capability(m.company_id,'attendance_administration')then raise exception 'HR attendance access is required.'using errcode='42501';end if;
  eid:=(p_request->>'employee_id')::uuid;pid:=p_request->>'pin';
  if pid is null or pid!~'^[0-9]{6,12}$'or not exists(select 1 from public.employees where company_id=m.company_id and id=eid and status='active')then raise exception 'Use an active employee and a 6–12 digit personal PIN.'using errcode='23514';end if;
  insert into hris_private.attendance_pins(company_id,employee_id,pin_hash)values(m.company_id,eid,extensions.crypt(pid,extensions.gen_salt('bf',10)))
  on conflict(employee_id)do update set pin_hash=excluded.pin_hash,failed_count=0,locked_until=null,updated_at=clock_timestamp();
  insert into hris_private.work_activity_events(company_id,employee_id,record_id,kind,actor_id,body)values(m.company_id,eid,eid,'pin_reset',m.user_id,'{}');
  return jsonb_build_object('saved',true);
 end if;
 if not hris_private.has_staff_capability(m.company_id,'account_administration')then raise exception 'An Administrator must enroll or unlock a station.'using errcode='42501';end if;
 if op='list'then return coalesce((select jsonb_agg(to_jsonb(v)order by v.name)from hris_private.attendance_stations v where v.company_id=m.company_id),'[]');end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,521));
 if op='save'then
  rid:=(p_request->>'request_id')::uuid;
  if rid is null then raise exception 'A stable station request identifier is required.'using errcode='23514';end if;
  select *into receipt from hris_private.work_activity_receipts where id=rid;
  if found then
   if receipt.company_id<>m.company_id or receipt.actor_id<>m.user_id or receipt.request is distinct from p_request then raise exception 'Station retry does not match.'using errcode='PT409';end if;
   return receipt.result;
  end if;
 end if;
 select *into s from hris_private.attendance_stations where id=station for update;
 if op='save'then
  if station is null then raise exception 'Station identifier required.'using errcode='23514';end if;
  if found then
   if s.company_id<>m.company_id then raise exception 'Station unavailable.'using errcode='42501';end if;
   if s.revision is distinct from(p_request->>'expected_revision')::bigint then raise exception 'Station changed. Refresh before saving.'using errcode='PT409';end if;
   update hris_private.attendance_stations set name=btrim(p_request->>'name'),active=(p_request->>'active')::boolean,revision=revision+1 where id=station returning *into s;
   if not s.active then update hris_private.attendance_station_sessions set revoked_at=clock_timestamp()where station_id=station and revoked_at is null;end if;
  else
   if(p_request->>'expected_revision')::bigint is distinct from -1 then raise exception 'Station baseline changed.'using errcode='PT409';end if;
   insert into hris_private.attendance_stations(id,company_id,name,enrolled_by)values(station,m.company_id,btrim(p_request->>'name'),m.user_id)returning *into s;
  end if;
  insert into hris_private.work_activity_receipts values(rid,m.company_id,m.user_id,p_request,to_jsonb(s));
  insert into hris_private.work_activity_events(company_id,employee_id,record_id,kind,actor_id,body)values(m.company_id,m.employee_id,s.id,'station_save',m.user_id,jsonb_build_object('name',s.name,'active',s.active,'revision',s.revision));
  return to_jsonb(s);
 end if;
 if not found or s.company_id<>m.company_id or not s.active then raise exception 'Station unavailable.'using errcode='42501';end if;
 if op='unlock'then
  update hris_private.attendance_station_sessions set revoked_at=clock_timestamp()where station_id=station and revoked_at is null;
  token:=replace(gen_random_uuid()::text,'-','')||replace(gen_random_uuid()::text,'-','');
  insert into hris_private.attendance_station_sessions(token_hash,station_id,unlocked_by,expires_at)values(encode(sha256(convert_to(token,'UTF8')),'hex'),station,m.user_id,clock_timestamp()+interval '16 hours');
  return jsonb_build_object('token',token,'station_id',station,'name',s.name,'expires_at',clock_timestamp()+interval '16 hours');
 elsif op='close'then
  update hris_private.attendance_station_sessions set revoked_at=clock_timestamp()where station_id=station and revoked_at is null;
  return jsonb_build_object('closed',true);
 end if;
 raise exception 'Unknown station operation.'using errcode='23514';
end $$;
create function public.attendance_station(p_request jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.attendance_station(p_request);$$;

create function hris_private.kiosk_session(p_token text)returns hris_private.attendance_station_sessions
language plpgsql security definer set search_path=''as $$
declare s hris_private.attendance_station_sessions;cid uuid;
begin
 if p_token is null or p_token!~'^[a-f0-9]{64}$'then return null;end if;
 select x.*into s from hris_private.attendance_station_sessions x join hris_private.attendance_stations a on a.id=x.station_id
 where x.token_hash=encode(sha256(convert_to(p_token,'UTF8')),'hex')and x.expires_at>clock_timestamp()and x.revoked_at is null and a.active;
 if not found then return null;end if;
 select company_id into cid from hris_private.attendance_stations where id=s.station_id;
 if not coalesce('account_administration'=any(hris_private.staff_capabilities_for(cid,s.unlocked_by)),false)then return null;end if;
 return s;
end $$;

create function hris_private.kiosk_punch_prepare(p_request jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare s hris_private.attendance_station_sessions:=hris_private.kiosk_session(p_request->>'token');
 cid uuid;emp public.employees;pin hris_private.attendance_pins;a public.attendance;i hris_private.kiosk_punch_intents;
 ident uuid:=(p_request->>'request_id')::uuid;op text:=p_request->>'operation';hash text:=p_request->>'photo_hash';
 policy jsonb;day date;zone text;bytes integer:=(p_request->>'photo_bytes')::integer;method text;
begin
 if s.token_hash is null then return jsonb_build_object('accepted',false,'error','Station is locked. Ask an Administrator to unlock it.');end if;
 select st.company_id into cid from hris_private.attendance_stations st where st.id=s.station_id;
 if(select count(*)from public.employees where company_id=cid and lower(employee_code)=lower(btrim(p_request->>'employee_code'))and status='active')<>1 then
  return jsonb_build_object('accepted',false,'error','Employee code or PIN is incorrect.');end if;
 select e.*into emp from public.employees e where e.company_id=cid and lower(e.employee_code)=lower(btrim(p_request->>'employee_code'))and e.status='active';
 select *into pin from hris_private.attendance_pins where employee_id=emp.id for update;
 if pin.locked_until>clock_timestamp()then return jsonb_build_object('accepted',false,'error','PIN attempts are temporarily locked. Contact HR or try later.');end if;
 if emp.id is null or pin.pin_hash is null or coalesce(p_request->>'pin','')!~'^[0-9]{6,12}$'or extensions.crypt(p_request->>'pin',pin.pin_hash)is distinct from pin.pin_hash then
  if pin.employee_id is not null then update hris_private.attendance_pins set failed_count=case when locked_until<=clock_timestamp()then 1 else failed_count+1 end,
    locked_until=case when locked_until<=clock_timestamp()then null when failed_count>=4 then clock_timestamp()+interval '15 minutes'else locked_until end where employee_id=emp.id;end if;
  return jsonb_build_object('accepted',false,'error','Employee code or PIN is incorrect.');
 end if;
 update hris_private.attendance_pins set failed_count=0,locked_until=null where employee_id=emp.id;
 if ident is null or op not in('check_in','check_out')or hash is null or hash!~'^[a-f0-9]{64}$'or bytes not between 100 and 65536 then
  return jsonb_build_object('accepted',false,'error','A fresh bounded JPEG capture and attendance action are required.');end if;
 perform pg_advisory_xact_lock(hashtextextended(emp.id::text,522));
 select *into i from hris_private.kiosk_punch_intents where id=ident;
 if found then
  if i.company_id<>cid or i.employee_id<>emp.id or i.station_id<>s.station_id or i.token_hash<>s.token_hash or i.operation<>op or i.photo_hash<>hash or i.photo_bytes<>bytes then
   return jsonb_build_object('accepted',false,'error','Retry does not match the original punch.');end if;
  if i.status='committed'then return jsonb_build_object('accepted',true,'result',i.result);end if;
  if i.expires_at<=clock_timestamp()or i.status<>'pending'then return jsonb_build_object('accepted',false,'error','Capture expired. Take a new photo.');end if;
  return jsonb_build_object('accepted',true,'object_path',i.object_path);
 end if;
 select timezone into zone from public.companies where id=cid;
 if op='check_out'then
  select *into a from public.attendance where company_id=cid and employee_id=emp.id and checked_out_at is null for update;
  if not found then return jsonb_build_object('accepted',false,'error','No open attendance record. Ask HR to review a missed punch.');end if;
  day:=a.work_date;
  select ps.policy into policy from hris_private.attendance_policy_snapshots ps where ps.attendance_id=a.id;
 else
  if exists(select 1 from public.attendance where company_id=cid and employee_id=emp.id and checked_out_at is null)then return jsonb_build_object('accepted',false,'error','Your previous shift is still open. Check out or contact HR.');end if;
  day:=hris_private.attendance_work_date_at(cid,emp.id,clock_timestamp());policy:=hris_private.work_arrangement_at(cid,emp.id,day);
 end if;
 if policy is not null and(policy->>'arrangement'='remote'or(policy->>'arrangement'='hybrid'and not(policy->'office_days'@>jsonb_build_array(extract(isodow from day)::integer))))then
  return jsonb_build_object('accepted',false,'error','Your approved work date uses remote attendance in Workforce Employee.');end if;
 -- Bound retained and pending evidence within the prototype storage budget.
 perform pg_advisory_xact_lock(hashtextextended(cid::text,523));
 if(select coalesce(sum(photo_bytes),0)from hris_private.kiosk_punch_intents where company_id=cid and purged_at is null)+bytes>134217728 then
  return jsonb_build_object('accepted',false,'error','Attendance photo storage needs HR review before another capture.');end if;
 insert into hris_private.kiosk_punch_intents(id,company_id,employee_id,station_id,token_hash,operation,photo_hash,photo_bytes,object_path,attendance_id,expected_revision,policy_snapshot,work_date)
 values(ident,cid,emp.id,s.station_id,s.token_hash,op,hash,bytes,cid::text||'/'||ident::text||'.jpg',coalesce(a.id,gen_random_uuid()),a.revision,policy,day)returning *into i;
 return jsonb_build_object('accepted',true,'object_path',i.object_path);
end $$;

create function hris_private.kiosk_punch_complete(p_token text,p_id uuid)returns jsonb language plpgsql security definer set search_path=''as $$
declare s hris_private.attendance_station_sessions:=hris_private.kiosk_session(p_token);i hris_private.kiosk_punch_intents;
 a public.attendance;current_policy jsonb;v_result jsonb;
begin
 if s.token_hash is null then raise exception 'Station is locked.'using errcode='42501';end if;
 select *into i from hris_private.kiosk_punch_intents where id=p_id for update;
 if not found or i.token_hash<>s.token_hash then raise exception 'Punch unavailable.'using errcode='42501';end if;
 if i.status='committed'then return i.result;end if;
 if i.status<>'pending'or i.expires_at<=clock_timestamp()then raise exception 'Capture expired. Take a new photo.'using errcode='PT409';end if;
 perform pg_advisory_xact_lock(hashtextextended(i.employee_id::text,522));
 if not exists(select 1 from public.employees where company_id=i.company_id and id=i.employee_id and status='active')then raise exception 'Employee access changed.'using errcode='42501';end if;
 if not exists(select 1 from storage.objects where bucket_id='attendance-evidence'and name=i.object_path and(metadata->>'size')::integer=i.photo_bytes and metadata->>'mimetype'='image/jpeg')then
  raise exception 'Required attendance photo is not ready.'using errcode='23514';end if;
 if i.operation='check_in'then
  current_policy:=hris_private.work_arrangement_at(i.company_id,i.employee_id,hris_private.attendance_work_date_at(i.company_id,i.employee_id,clock_timestamp()));
  if current_policy is distinct from i.policy_snapshot then raise exception 'Attendance policy changed. Take a new photo.'using errcode='PT409';end if;
 else
  select *into a from public.attendance where id=i.attendance_id for update;
  if not found or a.revision is distinct from i.expected_revision or a.checked_out_at is not null then raise exception 'Attendance changed. Refresh before another punch.'using errcode='PT409';end if;
 end if;
 insert into hris_private.kiosk_write_context values(txid_current(),i.attendance_id,s.station_id);
 if i.operation='check_in'then
  insert into public.attendance(id,company_id,employee_id)values(i.attendance_id,i.company_id,i.employee_id)returning *into a;
 else update public.attendance set checked_out_at=clock_timestamp()where id=i.attendance_id returning *into a;end if;
 delete from hris_private.kiosk_write_context where transaction_id=txid_current();
 v_result:=jsonb_build_object('saved',true,'request_id',i.id,'operation',i.operation,'work_date',a.work_date,'recorded_at',case when i.operation='check_in'then a.checked_in_at else a.checked_out_at end);
 update hris_private.kiosk_punch_intents set status='committed',result=v_result where id=i.id;
 update hris_private.attendance_stations set verified_at=coalesce(verified_at,clock_timestamp())where id=s.station_id;
 return v_result;
end $$;

create function hris_private.kiosk_station_close(p_token text)returns boolean language plpgsql security definer set search_path=''as $$
begin
 if p_token is null or p_token!~'^[a-f0-9]{64}$'then return false;end if;
 update hris_private.attendance_station_sessions set revoked_at=clock_timestamp()where token_hash=encode(sha256(convert_to(p_token,'UTF8')),'hex')and revoked_at is null;
 return true;
end $$;
create function public.kiosk_station_close(p_token text)returns boolean language sql security invoker set search_path=''as $$select hris_private.kiosk_station_close(p_token);$$;
revoke all on function hris_private.kiosk_station_close(text),public.kiosk_station_close(text)from public,anon,authenticated,service_role;
grant execute on function hris_private.kiosk_station_close(text),public.kiosk_station_close(text)to service_role;

create function hris_private.work_punch_guard()returns trigger language plpgsql security definer set search_path=''as $$
declare policy jsonb;kiosk boolean;office boolean;own_employee uuid;
begin
 if hris_private.is_attendance_correction(new.id)then return new;end if;
 if tg_op='UPDATE'and new.checked_out_at is not distinct from old.checked_out_at then return new;end if;
 select exists(select 1 from hris_private.kiosk_write_context where transaction_id=txid_current()and attendance_id=new.id)into kiosk;
 if tg_op='INSERT'then policy:=hris_private.work_arrangement_at(new.company_id,new.employee_id,new.work_date);
 else select ps.policy into policy from hris_private.attendance_policy_snapshots ps where ps.attendance_id=new.id;end if;
 if policy is null then return new;end if;
 office:=policy->>'arrangement'='onsite'or(policy->>'arrangement'='hybrid'and policy->'office_days'@>jsonb_build_array(extract(isodow from new.work_date)::integer));
 if office and not kiosk then raise exception 'This work date requires the registered office station. Use a reasoned HR correction for an exception.'using errcode='42501';end if;
 if not office and kiosk then raise exception 'This work date uses remote attendance.'using errcode='42501';end if;
 if not kiosk then
  select employee_id into own_employee from public.company_members where user_id=(select auth.uid())and company_id=new.company_id and active;
  if own_employee is distinct from new.employee_id then raise exception 'Routine remote punches are own-only. Use a reasoned HR correction for an exception.'using errcode='42501';end if;
 end if;
 return new;
end $$;
create trigger zz_work_punch_guard before insert or update on public.attendance for each row execute function hris_private.work_punch_guard();
create function hris_private.capture_work_punch_policy()returns trigger language plpgsql security definer set search_path=''as $$
declare policy jsonb;station uuid;
begin
 if hris_private.is_attendance_correction(new.id)then return new;end if;
 policy:=hris_private.work_arrangement_at(new.company_id,new.employee_id,new.work_date);
 select station_id into station from hris_private.kiosk_write_context where transaction_id=txid_current()and attendance_id=new.id;
 if policy is not null or station is not null then insert into hris_private.attendance_policy_snapshots(attendance_id,company_id,employee_id,method,policy)
 values(new.id,new.company_id,new.employee_id,case when station is null then 'remote'else 'kiosk'end,policy);end if;
 return new;
end $$;
create trigger attendance_work_policy_snapshot after insert on public.attendance for each row execute function hris_private.capture_work_punch_policy();

-- Preserve the existing calculation capture and current-member guard. The sole new
-- exception is the private, transaction-bound context established after PIN/photo validation.
create or replace function hris_private.record_attendance_calculation()returns trigger
language plpgsql security definer set search_path=''as $$begin
 if not exists(select 1 from hris_private.kiosk_write_context where transaction_id=txid_current()and attendance_id=new.id)
 and not exists(select 1 from public.company_members m where m.user_id=(select auth.uid())and m.active and m.company_id=new.company_id
 and(hris_private.has_staff_capability(m.company_id,'attendance_administration')or m.employee_id=new.employee_id))then
  raise exception 'Active membership and attendance access are required.'using errcode='42501';end if;
 new.calculation_snapshot:=hris_private.capture_attendance_calculation(new);return new;
end $$;

create function hris_private.work_attendance_status()returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members:=hris_private.work_activity_member();a public.attendance;policy jsonb;day date;method text;
begin
 select *into a from public.attendance where company_id=m.company_id and employee_id=m.employee_id and checked_out_at is null;
 if found then day:=a.work_date;select p.policy into policy from hris_private.attendance_policy_snapshots p where p.attendance_id=a.id;
 else day:=hris_private.attendance_work_date_at(m.company_id,m.employee_id,clock_timestamp());policy:=hris_private.work_arrangement_at(m.company_id,m.employee_id,day);end if;
 method:=case when policy is null then 'legacy'when policy->>'arrangement'='onsite'or(policy->>'arrangement'='hybrid'and policy->'office_days'@>jsonb_build_array(extract(isodow from day)::integer))then 'kiosk'else 'remote'end;
 return jsonb_build_object('version',1,'company_id',m.company_id,'employee_id',m.employee_id,'user_id',m.user_id,'work_date',day,'method',method,'arrangement',coalesce(policy->>'arrangement','legacy'));
end $$;
create function public.work_attendance_status()returns jsonb language sql security invoker set search_path=''as $$select hris_private.work_attendance_status();$$;
revoke all on function hris_private.work_attendance_status(),public.work_attendance_status()from public,anon,authenticated,service_role;
grant execute on function hris_private.work_attendance_status(),public.work_attendance_status()to authenticated;

create function hris_private.can_purge_attendance_photo(p_name text)returns boolean language sql stable security definer set search_path=''as $$
 select exists(select 1 from hris_private.kiosk_punch_intents i where i.object_path=p_name and i.purged_at is null
 and(i.created_at<statement_timestamp()-interval '7 days'or(i.status='pending'and i.expires_at<statement_timestamp()))
 and hris_private.has_staff_capability(i.company_id,'attendance_administration'))
$$;
create function hris_private.can_read_attendance_photo(p_name text)returns boolean language sql stable security definer set search_path=''as $$
 select exists(select 1 from hris_private.kiosk_punch_intents i where i.object_path=p_name and i.status='committed'and i.purged_at is null
 and hris_private.has_staff_capability(i.company_id,'attendance_administration'))
$$;
create policy attendance_evidence_hr_read on storage.objects for select to authenticated using(bucket_id='attendance-evidence'and(hris_private.can_read_attendance_photo(name)or hris_private.can_purge_attendance_photo(name)));

create function hris_private.attendance_evidence_context(p_id uuid)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members:=hris_private.work_activity_member();
begin
 if not hris_private.has_staff_capability(m.company_id,'attendance_administration')then raise exception 'Attendance evidence is HR-private.'using errcode='42501';end if;
 return coalesce((select jsonb_agg(jsonb_build_object('id',i.id,'company_id',i.company_id,'employee_id',i.employee_id,'attendance_id',i.attendance_id,'operation',i.operation,'recorded_at',i.result->>'recorded_at','station',s.name,'object_path',i.object_path,'photo_hash',i.photo_hash,'purged_at',i.purged_at))
 from hris_private.kiosk_punch_intents i join hris_private.attendance_stations s on s.id=i.station_id where i.company_id=m.company_id and i.attendance_id=p_id and i.status='committed'),'[]');
end $$;
create function public.attendance_evidence_context(p_id uuid)returns jsonb language sql security invoker set search_path=''as $$select hris_private.attendance_evidence_context(p_id);$$;
create function public.kiosk_punch_prepare(p_request jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.kiosk_punch_prepare(p_request);$$;
create function public.kiosk_punch_complete(p_token text,p_id uuid)returns jsonb language sql security invoker set search_path=''as $$select hris_private.kiosk_punch_complete(p_token,p_id);$$;

create function hris_private.attendance_evidence_retention(p_ids uuid[] default null)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members:=hris_private.work_activity_member();
begin
 if not hris_private.has_staff_capability(m.company_id,'attendance_administration')then raise exception 'Attendance evidence is HR-private.'using errcode='42501';end if;
 if cardinality(p_ids)>100 then raise exception 'Clean at most 100 photos per batch.'using errcode='23514';end if;
 if p_ids is not null then
  update hris_private.kiosk_punch_intents i set purged_at=clock_timestamp(),purged_by=m.user_id,status=case when i.status='pending'then 'expired'else i.status end
   where i.id=any(p_ids)and i.company_id=m.company_id and i.purged_at is null
   and(i.created_at<clock_timestamp()-interval '7 days'or(i.status='pending'and i.expires_at<clock_timestamp()))
   and not exists(select 1 from storage.objects o where o.bucket_id='attendance-evidence'and o.name=i.object_path);
 end if;
 return coalesce((select jsonb_agg(jsonb_build_object('id',v.id,'object_path',v.object_path))from(
  select i.id,i.object_path from hris_private.kiosk_punch_intents i where i.company_id=m.company_id and i.purged_at is null
   and(i.created_at<clock_timestamp()-interval '7 days'or(i.status='pending'and i.expires_at<clock_timestamp()))order by i.created_at limit 100)v),'[]');
end $$;
create function public.attendance_evidence_retention(p_ids uuid[] default null)returns jsonb language sql security invoker set search_path=''as $$select hris_private.attendance_evidence_retention(p_ids);$$;
create policy attendance_evidence_hr_cleanup on storage.objects for delete to authenticated using(bucket_id='attendance-evidence'and hris_private.can_purge_attendance_photo(name));
revoke all on function hris_private.can_purge_attendance_photo(text),hris_private.attendance_evidence_retention(uuid[]),public.attendance_evidence_retention(uuid[])from public,anon,authenticated,service_role;
grant execute on function hris_private.can_purge_attendance_photo(text),hris_private.attendance_evidence_retention(uuid[]),public.attendance_evidence_retention(uuid[])to authenticated;

revoke all on function hris_private.attendance_station(jsonb),public.attendance_station(jsonb),hris_private.kiosk_session(text),hris_private.kiosk_punch_prepare(jsonb),hris_private.kiosk_punch_complete(text,uuid),hris_private.work_punch_guard(),hris_private.capture_work_punch_policy(),hris_private.can_read_attendance_photo(text),hris_private.attendance_evidence_context(uuid),public.attendance_evidence_context(uuid),public.kiosk_punch_prepare(jsonb),public.kiosk_punch_complete(text,uuid)from public,anon,authenticated,service_role;
grant execute on function hris_private.attendance_station(jsonb),public.attendance_station(jsonb),hris_private.can_read_attendance_photo(text),hris_private.attendance_evidence_context(uuid),public.attendance_evidence_context(uuid)to authenticated;
grant execute on function hris_private.kiosk_punch_prepare(jsonb),hris_private.kiosk_punch_complete(text,uuid),public.kiosk_punch_prepare(jsonb),public.kiosk_punch_complete(text,uuid)to service_role;
commit;
