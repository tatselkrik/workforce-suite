-- Free, local-only payroll exchange. No source payroll row is changed.
begin;
create table public.payroll_exchange_profiles(
 id uuid not null,company_id uuid not null references public.companies(id),revision integer not null check(revision between 0 and 99),
 request_id uuid not null unique,actor_id uuid not null references auth.users(id),payload jsonb not null,
 name text not null,enabled boolean not null,columns jsonb not null,created_at timestamptz not null default transaction_timestamp(),
 primary key(id,revision),unique(company_id,id,revision)
);
create index payroll_exchange_profiles_company on public.payroll_exchange_profiles(company_id,id,revision desc);
create table public.payroll_exchanges(
 id uuid primary key,company_id uuid not null references public.companies(id),actor_id uuid not null references auth.users(id),
 payload jsonb not null,basis jsonb not null,created_at timestamptz not null default transaction_timestamp()
);
create index payroll_exchanges_company on public.payroll_exchanges(company_id,created_at desc,id);
create table public.payroll_exchange_ack_batches(
 id uuid primary key,company_id uuid not null references public.companies(id),exchange_id uuid not null references public.payroll_exchanges(id),
 actor_id uuid not null references auth.users(id),payload jsonb not null,created_at timestamptz not null default transaction_timestamp()
);
create index payroll_exchange_ack_batches_parent on public.payroll_exchange_ack_batches(company_id,exchange_id);
create table public.payroll_exchange_retired_attempts(
 kind text not null check(kind in('profile','prepare','ack')),id uuid not null,company_id uuid not null references public.companies(id),
 actor_id uuid not null references auth.users(id),payload_sha256 text not null check(payload_sha256~'^[0-9a-f]{64}$'),
 created_at timestamptz not null default transaction_timestamp(),primary key(kind,id)
);
create index payroll_exchange_retired_company on public.payroll_exchange_retired_attempts(company_id);
create table public.payroll_exchange_ack_events(
 exchange_id uuid not null references public.payroll_exchanges(id),record_key text not null,revision integer not null check(revision between 1 and 100),
 batch_id uuid not null references public.payroll_exchange_ack_batches(id),status text not null check(status in('accepted','rejected','pending')),
 reference text not null,note text not null,created_at timestamptz not null default transaction_timestamp(),primary key(exchange_id,record_key,revision)
);
alter table public.payroll_exchange_profiles enable row level security;
alter table public.payroll_exchanges enable row level security;
alter table public.payroll_exchange_ack_batches enable row level security;
alter table public.payroll_exchange_ack_events enable row level security;
alter table public.payroll_exchange_retired_attempts enable row level security;
revoke all on public.payroll_exchange_profiles,public.payroll_exchanges,public.payroll_exchange_ack_batches,public.payroll_exchange_ack_events,public.payroll_exchange_retired_attempts from public,anon,authenticated;
create function hris_private.payroll_exchange_immutable()returns trigger language plpgsql security invoker set search_path=''as $$begin
 raise exception 'Payroll exchange facts and history are immutable.'using errcode='23514';end;$$;
create trigger payroll_exchange_profiles_immutable before update or delete on public.payroll_exchange_profiles for each row execute function hris_private.payroll_exchange_immutable();
create trigger payroll_exchanges_immutable before update or delete on public.payroll_exchanges for each row execute function hris_private.payroll_exchange_immutable();
create trigger payroll_exchange_ack_batches_immutable before update or delete on public.payroll_exchange_ack_batches for each row execute function hris_private.payroll_exchange_immutable();
create trigger payroll_exchange_ack_events_immutable before update or delete on public.payroll_exchange_ack_events for each row execute function hris_private.payroll_exchange_immutable();
create trigger payroll_exchange_retired_immutable before update or delete on public.payroll_exchange_retired_attempts for each row execute function hris_private.payroll_exchange_immutable();
create function hris_private.payroll_exchange_assert_open(p_kind text,p_id uuid)returns void language plpgsql security definer set search_path=''as $$begin
 if exists(select 1 from public.payroll_exchange_retired_attempts where kind=p_kind and id=p_id)then raise exception 'This unused attempt identity was closed during recovery. Review a new draft.'using errcode='PT409';end if;
end;$$;
revoke all on function hris_private.payroll_exchange_assert_open(text,uuid)from public,anon,authenticated;
create function hris_private.payroll_exchange_capacity(cid uuid,extra integer)returns void language plpgsql security definer set search_path=''as $$
declare used bigint;begin
 select coalesce((select sum(octet_length(basis::text)+octet_length(payload::text)+512)from public.payroll_exchanges where company_id=cid),0)
 +coalesce((select sum(octet_length(payload::text)*2+512)from public.payroll_exchange_profiles where company_id=cid),0)
 +coalesce((select sum(octet_length(payload::text)*3+512)from public.payroll_exchange_ack_batches where company_id=cid),0)
 +(select count(*)*512 from public.payroll_exchange_retired_attempts where company_id=cid)into used;
 -- Reserve512KiB within32MiB for up to1000 fixed-size recovery identities.
 if extra<0 or used+extra>33030144 then raise exception 'The free prototype exchange-history budget is32MiB, including reserved recovery space. No history was removed; no paid upgrade was started.'using errcode='54000';end if;
end;$$;
revoke all on function hris_private.payroll_exchange_capacity(uuid,integer)from public,anon,authenticated;

create function hris_private.payroll_exchange_member()returns public.company_members language plpgsql security definer set search_path=''as $$
declare m public.company_members;begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and role in('administrator','hr_generalist');
 if not found or not exists(select 1 from public.employees where company_id=m.company_id and id=m.employee_id and status='active')then raise exception 'Current active HR/Admin access is required for payroll exchange.'using errcode='42501';end if;return m;
end;$$;
create function hris_private.payroll_exchange_keys()returns text[] language sql immutable security invoker set search_path=''as $$
 select array['employee_name','department','period_start','period_end','issued_on','received_on','status','basic_cents','gross_cents','employee_contributions_cents','employer_contributions_cents','withholding_cents','net_cents','contained_source_net_cents','cash_cents','offset_cents','tax_adjustment_cents','outstanding_cents','reserved_cents','rule_version','linked_kind','linked_id','review_amount_cents'];$$;
create function hris_private.payroll_exchange_quote(v text, literal boolean default true)returns text language plpgsql immutable security invoker set search_path=''as $$
declare cleaned text;spaces text:='';n integer;begin
 v:=coalesce(v,'');foreach n in array array[9,10,11,12,13,32,133,160,5760,8192,8193,8194,8195,8196,8197,8198,8199,8200,8201,8202,8232,8233,8239,8287,12288,65279]loop spaces:=spaces||chr(n);end loop;
 cleaned:=ltrim(v,spaces);
 return '"'||case when literal and(left(cleaned,1)in('=','+','-','@')or left(v,1)in(chr(9),chr(10),chr(13)))then chr(39)else''end||replace(v,'"','""')||'"';
end;$$;
create function hris_private.payroll_exchange_profile_json(p public.payroll_exchange_profiles)returns jsonb language sql stable security invoker set search_path=''as $$
 select jsonb_build_object('id',p.id,'revision',p.revision,'name',p.name,'enabled',p.enabled,'columns',p.columns,'created_at',p.created_at);$$;
create function hris_private.payroll_exchange_state()returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;profiles jsonb;history jsonb;begin
 m:=hris_private.payroll_exchange_member();
 select coalesce(jsonb_agg(hris_private.payroll_exchange_profile_json(p)order by lower(p.name),p.id),'[]')into profiles
 from(select distinct on(id)*from public.payroll_exchange_profiles where company_id=m.company_id order by id,revision desc)p;
 select coalesce(jsonb_agg(jsonb_build_object('id',b.id,'profile_name',b.basis->'profile'->>'name','profile_revision',(b.basis->'profile'->>'revision')::integer,
 'row_count',jsonb_array_length(b.basis->'controls'),'sha256',b.basis->>'sha256','created_at',b.created_at,
 'accepted',(select count(*)from(select distinct on(record_key)status from public.payroll_exchange_ack_events where exchange_id=b.id order by record_key,revision desc)a where a.status='accepted'),
 'rejected',(select count(*)from(select distinct on(record_key)status from public.payroll_exchange_ack_events where exchange_id=b.id order by record_key,revision desc)a where a.status='rejected'))order by b.created_at desc,b.id),'[]')into history
 from public.payroll_exchanges b where b.company_id=m.company_id;
 return jsonb_build_object('version','payroll-exchange-state-v1','company_id',m.company_id,'user_id',m.user_id,'role',m.role,'profiles',profiles,'history',history);
end;$$;
create function hris_private.save_payroll_exchange_profile(p_change jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;company uuid;rid uuid;pid uuid;prev public.payroll_exchange_profiles;saved public.payroll_exchange_profiles;expected integer;v jsonb;k text;h text;keys text[]:='{}';headers text[]:=array['exchange_id','record_key','row_sha256','employee_code','source_kind','payment_state','amount_unit'];begin
 m:=hris_private.payroll_exchange_member();company:=m.company_id;if m.role<>'administrator'then raise exception 'Only Administrators configure exchange profiles.'using errcode='42501';end if;
 if jsonb_typeof(p_change)is distinct from'object'or octet_length(p_change::text)>16384 then raise exception 'Use a bounded profile change.'using errcode='23514';end if;
 if(select count(*)from jsonb_object_keys(p_change))<>7 or not(p_change?&array['request_id','id','expected_revision','name','enabled','columns','reason'])
 or jsonb_typeof(p_change->'request_id')is distinct from'string'or jsonb_typeof(p_change->'id')is distinct from'string'
 or jsonb_typeof(p_change->'expected_revision')not in('number','null')or jsonb_typeof(p_change->'enabled')is distinct from'boolean'
 or jsonb_typeof(p_change->'name')is distinct from'string'or char_length(btrim(p_change->>'name'))not between 1 and 80 or(p_change->>'name')~'[[:cntrl:]]'
 or jsonb_typeof(p_change->'reason')is distinct from'string'or char_length(btrim(p_change->>'reason'))not between 1 and 500 or(p_change->>'reason')~'[[:cntrl:]]'
 or jsonb_typeof(p_change->'columns')is distinct from'array'then raise exception 'Use the profile fields and a private reason.'using errcode='23514';end if;
 if jsonb_array_length(p_change->'columns')not between 1 and 23 then raise exception 'Choose 1–23 distinct exchange fields.'using errcode='23514';end if;
 rid:=(p_change->>'request_id')::uuid;pid:=(p_change->>'id')::uuid;expected:=(p_change->>'expected_revision')::integer;
 if rid='00000000-0000-0000-0000-000000000000'::uuid or pid='00000000-0000-0000-0000-000000000000'::uuid or(expected is not null and expected not between 0 and 98)then raise exception 'Use valid profile/retry identities and revisions.'using errcode='23514';end if;
 for v in select value from jsonb_array_elements(p_change->'columns')loop
  if jsonb_typeof(v)is distinct from'object'then raise exception 'Use column objects.'using errcode='23514';end if;
  k:=v->>'key';h:=v->>'header';
  if(select count(*)from jsonb_object_keys(v))<>2 or not(v?&array['key','header'])or jsonb_typeof(v->'key')is distinct from'string'or jsonb_typeof(v->'header')is distinct from'string'
   or not(k=any(hris_private.payroll_exchange_keys()))or k=any(keys)or h!~'^[A-Za-z][A-Za-z0-9 _.-]{0,63}$'or lower(h)=any(headers)then raise exception 'Choose whitelisted fields and distinct safe column headers.'using errcode='23514';end if;
  keys:=array_append(keys,k);headers:=array_append(headers,lower(h));
 end loop;
 perform pg_advisory_xact_lock(hashtextextended(company::text,53));m:=hris_private.payroll_exchange_member();
 if m.company_id<>company or m.role<>'administrator'then raise exception 'Profile authority changed.'using errcode='42501';end if;
 perform hris_private.payroll_exchange_assert_open('profile',rid);
 select *into saved from public.payroll_exchange_profiles where request_id=rid;
 if found then if saved.company_id<>company or saved.actor_id<>m.user_id or saved.payload<>p_change then raise exception 'Retry identity belongs to another actor or payload.'using errcode='PT409';end if;return hris_private.payroll_exchange_state();end if;
 select *into prev from public.payroll_exchange_profiles where id=pid order by revision desc limit 1;
 if found then if prev.company_id<>company or expected is distinct from prev.revision then raise exception 'Profile changed. Refresh before editing.'using errcode='PT409';end if;
 else if expected is not null then raise exception 'Profile no longer matches this revision.'using errcode='PT409';end if;
  if(select count(distinct id)from public.payroll_exchange_profiles where company_id=company)>=50 then raise exception 'The company profile limit is50.'using errcode='54000';end if;
 end if;
 perform hris_private.payroll_exchange_capacity(company,octet_length(p_change::text)*2+512);
 insert into public.payroll_exchange_profiles(id,company_id,revision,request_id,actor_id,payload,name,enabled,columns)
 values(pid,company,coalesce(expected+1,0),rid,m.user_id,p_change,btrim(p_change->>'name'),(p_change->>'enabled')::boolean,p_change->'columns');
 return hris_private.payroll_exchange_state();
exception when invalid_text_representation or numeric_value_out_of_range then raise exception 'Use valid profile identities and integer revisions.'using errcode='23514';end;$$;

create function hris_private.payroll_exchange_basis(p_request jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;p public.payroll_exchange_profiles;eid uuid;pid uuid;report jsonb;controls jsonb:='[]';r jsonb;c jsonb;key text;digest text;csv text;fingerprint text;begin
 m:=hris_private.payroll_exchange_member();
 if jsonb_typeof(p_request)is distinct from'object'or octet_length(p_request::text)>4096 then raise exception 'Use a bounded exchange request.'using errcode='23514';end if;
 if(select count(*)from jsonb_object_keys(p_request))<>5 or not(p_request?&array['id','profile_id','expected_revision','filter','reason'])
 or jsonb_typeof(p_request->'id')is distinct from'string'or jsonb_typeof(p_request->'profile_id')is distinct from'string'
 or jsonb_typeof(p_request->'expected_revision')is distinct from'number'or coalesce(p_request->>'expected_revision','')!~'^[0-9]{1,2}$'
 or jsonb_typeof(p_request->'reason')is distinct from'string'or char_length(btrim(p_request->>'reason'))not between 1 and 500 or(p_request->>'reason')~'[[:cntrl:]]'then raise exception 'Choose a profile version, report filter and private export reason.'using errcode='23514';end if;
 eid:=(p_request->>'id')::uuid;pid:=(p_request->>'profile_id')::uuid;
 if eid='00000000-0000-0000-0000-000000000000'::uuid or pid='00000000-0000-0000-0000-000000000000'::uuid then raise exception 'Use valid exchange identities.'using errcode='23514';end if;
 select *into p from public.payroll_exchange_profiles where id=pid and company_id=m.company_id order by revision desc limit 1;
 if not found or not p.enabled or p.revision<>(p_request->>'expected_revision')::integer then raise exception 'Profile changed or is disabled. Refresh.'using errcode='PT409';end if;
 report:=hris_private.payroll_report_v2(p_request->'filter');
 if jsonb_array_length(report->'rows')not between 1 and 1000 then raise exception 'Use1–1000 report rows per exchange; narrow the filter.'using errcode='54000';end if;
 if exists(select 1 from jsonb_array_elements(report->'rows')v where jsonb_typeof(v.value->'employee_code')is distinct from'string'or jsonb_typeof(v.value->'employee_name')is distinct from'string'or jsonb_typeof(v.value->'department')is distinct from'string')then
  raise exception 'A historical source is missing saved employee labels. Review its history or narrow the filter to complete records before exchange.'using errcode='23514';
 end if;
 csv:='"exchange_id","record_key","row_sha256","employee_code","source_kind","payment_state","amount_unit"';
 for c in select value from jsonb_array_elements(p.columns)loop csv:=csv||','||hris_private.payroll_exchange_quote(c->>'header');end loop;csv:=csv||chr(10);
 for r in select value from jsonb_array_elements(report->'rows')loop
  key:=(r->>'kind')||':'||(r->>'id');digest:=encode(sha256(convert_to(r::text,'UTF8')),'hex');
  controls:=controls||jsonb_build_array(jsonb_build_object('record_key',key,'row_sha256',digest));
  csv:=csv||hris_private.payroll_exchange_quote(eid::text)||','||hris_private.payroll_exchange_quote(key)||','||hris_private.payroll_exchange_quote(digest)||','||hris_private.payroll_exchange_quote(r->>'employee_code')||','||hris_private.payroll_exchange_quote(r->>'kind')||','||hris_private.payroll_exchange_quote(r->>'payment_state')||',"PHP_CENTAVOS"';
  for c in select value from jsonb_array_elements(p.columns)loop csv:=csv||','||hris_private.payroll_exchange_quote(r->>(c->>'key'),right(c->>'key',6)<>'_cents');end loop;csv:=csv||chr(10);
 end loop;
 if octet_length(csv)>4194304 or octet_length(report::text)>8388608 then raise exception 'Exchange snapshot exceeds its byte limit; narrow the filter.'using errcode='54000';end if;
 fingerprint:=encode(sha256(convert_to(jsonb_build_object('request',p_request,'profile',hris_private.payroll_exchange_profile_json(p),'report',report-'generated_at','controls',controls)::text,'UTF8')),'hex');
 return jsonb_build_object('company_id',m.company_id,'exchange_id',eid,'profile',hris_private.payroll_exchange_profile_json(p),'report',report,'controls',controls,'csv',csv,'sha256',encode(sha256(convert_to(csv,'UTF8')),'hex'),'fingerprint',fingerprint);
exception when invalid_text_representation or numeric_value_out_of_range then raise exception 'Use valid exchange identities and revisions.'using errcode='23514';end;$$;
create function hris_private.preview_payroll_exchange(p_request jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;b jsonb;begin m:=hris_private.payroll_exchange_member();b:=hris_private.payroll_exchange_basis(p_request);
 return jsonb_build_object('version','payroll-exchange-preview-v1','company_id',m.company_id,'user_id',m.user_id,'role',m.role,'basis',b);end;$$;
create function hris_private.payroll_exchange_detail(p_id uuid)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;b public.payroll_exchanges;events jsonb;begin
 m:=hris_private.payroll_exchange_member();select *into b from public.payroll_exchanges where id=p_id and company_id=m.company_id;
 if not found then raise exception 'Exchange unavailable to this company.'using errcode='42501';end if;
 select coalesce(jsonb_agg(jsonb_build_object('record_key',e.record_key,'revision',e.revision,'batch_id',e.batch_id,'status',e.status,'reference',e.reference,'note',e.note,'created_at',e.created_at)order by e.created_at,e.record_key,e.revision),'[]')into events from public.payroll_exchange_ack_events e where exchange_id=p_id;
 return jsonb_build_object('version','payroll-exchange-detail-v1','company_id',m.company_id,'user_id',m.user_id,'role',m.role,'basis',b.basis,'created_at',b.created_at,'events',events);
end;$$;
create function hris_private.prepare_payroll_exchange(p_request jsonb,p_fingerprint text)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;company uuid;eid uuid;old public.payroll_exchanges;b jsonb;begin
 m:=hris_private.payroll_exchange_member();company:=m.company_id;
 if p_fingerprint is null or p_fingerprint!~'^[0-9a-f]{64}$'or octet_length(p_request::text)>4096 then raise exception 'Preview this bounded request before preparation.'using errcode='23514';end if;
 eid:=(p_request->>'id')::uuid;if eid is null then raise exception 'An exchange identity is required.'using errcode='23514';end if;
 perform pg_advisory_xact_lock(hashtextextended(company::text,53));m:=hris_private.payroll_exchange_member();if m.company_id<>company then raise exception 'Exchange company changed.'using errcode='42501';end if;
 perform hris_private.payroll_exchange_assert_open('prepare',eid);
 select *into old from public.payroll_exchanges where id=eid;
 if found then if old.company_id<>company or old.actor_id<>m.user_id or old.payload<>jsonb_build_object('request',p_request,'fingerprint',p_fingerprint)then raise exception 'Retry identity belongs to another actor or payload.'using errcode='PT409';end if;return hris_private.payroll_exchange_detail(eid);end if;
 if(select count(*)from public.payroll_exchanges where company_id=company)>=100 then raise exception 'The company exchange history limit is100. No records were removed.'using errcode='54000';end if;
 b:=hris_private.payroll_exchange_basis(p_request);if b->>'fingerprint'<>p_fingerprint then raise exception 'Report sources or profile changed. Preview again.'using errcode='PT409';end if;
 perform hris_private.payroll_exchange_capacity(company,octet_length(b::text)+octet_length(jsonb_build_object('request',p_request,'fingerprint',p_fingerprint)::text)+512);
 insert into public.payroll_exchanges(id,company_id,actor_id,payload,basis)values(eid,company,m.user_id,jsonb_build_object('request',p_request,'fingerprint',p_fingerprint),b);
 return hris_private.payroll_exchange_detail(eid);
exception when invalid_text_representation then raise exception 'Use a valid exchange identity.'using errcode='23514';end;$$;

create function hris_private.ack_payroll_exchange(p_batch jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;company uuid;bid uuid;eid uuid;old public.payroll_exchange_ack_batches;b public.payroll_exchanges;r jsonb;c jsonb;prev public.payroll_exchange_ack_events;
 key text;keys text[]:='{}';expected integer;status text;begin
 m:=hris_private.payroll_exchange_member();company:=m.company_id;
 if jsonb_typeof(p_batch)is distinct from'object'or octet_length(p_batch::text)>2097152 then raise exception 'Use a bounded acknowledgement batch.'using errcode='23514';end if;
 if(select count(*)from jsonb_object_keys(p_batch))<>4 or not(p_batch?&array['id','exchange_id','reason','rows'])or jsonb_typeof(p_batch->'id')is distinct from'string'or jsonb_typeof(p_batch->'exchange_id')is distinct from'string'
 or jsonb_typeof(p_batch->'reason')is distinct from'string'or char_length(btrim(p_batch->>'reason'))not between 1 and 500 or(p_batch->>'reason')~'[[:cntrl:]]'or jsonb_typeof(p_batch->'rows')is distinct from'array'then raise exception 'Use acknowledgement identities, rows and a private reason.'using errcode='23514';end if;
 if jsonb_array_length(p_batch->'rows')not between 1 and 1000 then raise exception 'Use1–1000 acknowledgement rows.'using errcode='23514';end if;
 bid:=(p_batch->>'id')::uuid;eid:=(p_batch->>'exchange_id')::uuid;
 if bid='00000000-0000-0000-0000-000000000000'::uuid or eid='00000000-0000-0000-0000-000000000000'::uuid then raise exception 'Use valid acknowledgement identities.'using errcode='23514';end if;
 perform pg_advisory_xact_lock(hashtextextended(company::text,53));m:=hris_private.payroll_exchange_member();if m.company_id<>company then raise exception 'Acknowledgement company changed.'using errcode='42501';end if;
 perform hris_private.payroll_exchange_assert_open('ack',bid);
 select *into old from public.payroll_exchange_ack_batches where id=bid;
 if found then if old.company_id<>company or old.actor_id<>m.user_id or old.payload<>p_batch then raise exception 'Retry identity belongs to another actor or payload.'using errcode='PT409';end if;return hris_private.payroll_exchange_detail(eid);end if;
 select *into b from public.payroll_exchanges where id=eid and company_id=company;if not found then raise exception 'Exchange unavailable to this company.'using errcode='42501';end if;
 if(select count(*)from public.payroll_exchange_ack_batches where exchange_id=eid)>=100 or(select count(*)from public.payroll_exchange_ack_events where exchange_id=eid)+jsonb_array_length(p_batch->'rows')>10000 then raise exception 'Acknowledgement history limit reached; no history was truncated.'using errcode='54000';end if;
 for r in select value from jsonb_array_elements(p_batch->'rows')loop
  if jsonb_typeof(r)is distinct from'object'then raise exception 'Use acknowledgement row objects.'using errcode='23514';end if;
  if(select count(*)from jsonb_object_keys(r))<>6 or not(r?&array['record_key','row_sha256','expected_revision','status','reference','note'])
   or jsonb_typeof(r->'record_key')is distinct from'string'or char_length(r->>'record_key')>60
   or jsonb_typeof(r->'row_sha256')is distinct from'string'or(r->>'row_sha256')!~'^[0-9a-f]{64}$'
   or jsonb_typeof(r->'expected_revision')is distinct from'number'or coalesce(r->>'expected_revision','')!~'^[0-9]{1,2}$'
   or jsonb_typeof(r->'status')is distinct from'string'or r->>'status'not in('accepted','rejected','pending')
   or jsonb_typeof(r->'reference')is distinct from'string'or char_length(r->>'reference')>200 or(r->>'reference')~'[[:cntrl:]]'
   or jsonb_typeof(r->'note')is distinct from'string'or char_length(r->>'note')>500 or(r->>'note')~'[[:cntrl:]]'then raise exception 'Use the bounded acknowledgement template fields.'using errcode='23514';end if;
  key:=r->>'record_key';expected:=(r->>'expected_revision')::integer;status:=r->>'status';
  if key=any(keys)then raise exception 'A source record occurs twice in this file.'using errcode='23514';end if;keys:=array_append(keys,key);
  select value into c from jsonb_array_elements(b.basis->'controls')where value->>'record_key'=key;
  if c is null or (c->>'row_sha256')<>(r->>'row_sha256') then
   raise exception 'Acknowledgement source/digest does not match this frozen export.'using errcode='23514';
  end if;
  select *into prev from public.payroll_exchange_ack_events where exchange_id=eid and record_key=key order by revision desc limit 1;
  if expected<>coalesce(prev.revision,0)then raise exception 'Acknowledgement changed. Reload its current revision.'using errcode='PT409';end if;
  if status='pending'then
   if m.role<>'administrator'or coalesce(prev.status,'pending')='pending'or btrim(r->>'note')=''then raise exception 'Only an Administrator can reset a terminal outcome with a row reason.'using errcode='42501';end if;
  elsif coalesce(prev.status,'pending')<>'pending'then raise exception 'A terminal outcome must first be reset by an Administrator.'using errcode='PT409';end if;
 end loop;
 perform hris_private.payroll_exchange_capacity(company,octet_length(p_batch::text)*3+512);
 insert into public.payroll_exchange_ack_batches(id,company_id,exchange_id,actor_id,payload)values(bid,company,eid,m.user_id,p_batch);
 for r in select value from jsonb_array_elements(p_batch->'rows')loop
  insert into public.payroll_exchange_ack_events(exchange_id,record_key,revision,batch_id,status,reference,note)
  values(eid,r->>'record_key',(r->>'expected_revision')::integer+1,bid,r->>'status',r->>'reference',r->>'note');
 end loop;
 return hris_private.payroll_exchange_detail(eid);
exception when invalid_text_representation or numeric_value_out_of_range then raise exception 'Use valid acknowledgement identities and integer revisions.'using errcode='23514';end;$$;

create function hris_private.payroll_exchange_attempt(p_kind text,p_id uuid,p_payload jsonb)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;target uuid;company uuid;owner uuid;payload jsonb;present boolean:=false;retired public.payroll_exchange_retired_attempts;digest text;begin
 m:=hris_private.payroll_exchange_member();
 if p_id is null or p_id='00000000-0000-0000-0000-000000000000'::uuid or p_kind is null or p_kind not in('profile','prepare','ack')or p_payload is null or octet_length(p_payload::text)>2097152 then raise exception 'Use a bounded retained attempt identity.'using errcode='23514';end if;
 if p_kind='profile'and m.role<>'administrator'then raise exception 'Only Administrators resolve profile changes.'using errcode='42501';end if;
 target:=m.company_id;perform pg_advisory_xact_lock(hashtextextended(target::text,53));m:=hris_private.payroll_exchange_member();if m.company_id<>target then raise exception 'Retained attempt company changed.'using errcode='42501';end if;
 if p_kind='profile'then select b.company_id,b.actor_id,b.payload into company,owner,payload from public.payroll_exchange_profiles b where request_id=p_id;
 elsif p_kind='prepare'then select b.company_id,b.actor_id,b.payload into company,owner,payload from public.payroll_exchanges b where id=p_id;
 else select b.company_id,b.actor_id,b.payload into company,owner,payload from public.payroll_exchange_ack_batches b where id=p_id;end if;
 present:=found;
 if present and(company<>m.company_id or owner<>m.user_id or payload<>p_payload)then raise exception 'Retained identity belongs to another actor or payload.'using errcode='PT409';end if;
 if not present then
  digest:=encode(sha256(convert_to(p_payload::text,'UTF8')),'hex');
  select *into retired from public.payroll_exchange_retired_attempts where kind=p_kind and id=p_id;
  if found then if retired.company_id<>m.company_id or retired.actor_id<>m.user_id or retired.payload_sha256<>digest then raise exception 'Retained identity belongs to another actor or payload.'using errcode='PT409';end if;
  else
   if(select count(*)from public.payroll_exchange_retired_attempts where company_id=m.company_id)>=1000 then raise exception 'Retained-attempt history limit reached.'using errcode='54000';end if;
   -- Recovery must remain possible when ordinary export history is full.
   insert into public.payroll_exchange_retired_attempts(kind,id,company_id,actor_id,payload_sha256)values(p_kind,p_id,m.company_id,m.user_id,digest);
  end if;
 end if;
 return jsonb_build_object('version','payroll-exchange-attempt-v1','company_id',m.company_id,'user_id',m.user_id,'role',m.role,'kind',p_kind,'id',p_id,'committed',present);
end;$$;
revoke all on function hris_private.payroll_exchange_attempt(text,uuid,jsonb)from public,anon,authenticated;
grant execute on function hris_private.payroll_exchange_attempt(text,uuid,jsonb)to authenticated;
create function public.payroll_exchange_attempt(p_kind text,p_id uuid,p_payload jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.payroll_exchange_attempt(p_kind,p_id,p_payload);$$;
revoke all on function public.payroll_exchange_attempt(text,uuid,jsonb)from public,anon,authenticated;
grant execute on function public.payroll_exchange_attempt(text,uuid,jsonb)to authenticated;
revoke all on function hris_private.payroll_exchange_immutable(),hris_private.payroll_exchange_member(),hris_private.payroll_exchange_keys(),hris_private.payroll_exchange_quote(text,boolean),hris_private.payroll_exchange_profile_json(public.payroll_exchange_profiles),hris_private.payroll_exchange_state(),hris_private.save_payroll_exchange_profile(jsonb),hris_private.payroll_exchange_basis(jsonb),hris_private.preview_payroll_exchange(jsonb),hris_private.payroll_exchange_detail(uuid),hris_private.prepare_payroll_exchange(jsonb,text),hris_private.ack_payroll_exchange(jsonb)from public,anon,authenticated;
grant execute on function hris_private.payroll_exchange_state(),hris_private.save_payroll_exchange_profile(jsonb),hris_private.preview_payroll_exchange(jsonb),hris_private.payroll_exchange_detail(uuid),hris_private.prepare_payroll_exchange(jsonb,text),hris_private.ack_payroll_exchange(jsonb)to authenticated;
create function public.payroll_exchange_state()returns jsonb language sql security invoker set search_path=''as $$select hris_private.payroll_exchange_state();$$;
create function public.save_payroll_exchange_profile(p_change jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.save_payroll_exchange_profile(p_change);$$;
create function public.preview_payroll_exchange(p_request jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.preview_payroll_exchange(p_request);$$;
create function public.payroll_exchange_detail(p_id uuid)returns jsonb language sql security invoker set search_path=''as $$select hris_private.payroll_exchange_detail(p_id);$$;
create function public.prepare_payroll_exchange(p_request jsonb,p_fingerprint text)returns jsonb language sql security invoker set search_path=''as $$select hris_private.prepare_payroll_exchange(p_request,p_fingerprint);$$;
create function public.ack_payroll_exchange(p_batch jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.ack_payroll_exchange(p_batch);$$;
revoke all on function public.payroll_exchange_state(),public.save_payroll_exchange_profile(jsonb),public.preview_payroll_exchange(jsonb),public.payroll_exchange_detail(uuid),public.prepare_payroll_exchange(jsonb,text),public.ack_payroll_exchange(jsonb)from public,anon,authenticated;
grant execute on function public.payroll_exchange_state(),public.save_payroll_exchange_profile(jsonb),public.preview_payroll_exchange(jsonb),public.payroll_exchange_detail(uuid),public.prepare_payroll_exchange(jsonb,text),public.ack_payroll_exchange(jsonb)to authenticated;
commit;
