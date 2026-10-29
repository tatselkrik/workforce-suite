begin;
-- Optional managed scheduling extensions; local test engines need no network.
do $$begin
 if exists(select 1 from pg_available_extensions where name='pg_cron')then execute 'create extension if not exists pg_cron';end if;
 if exists(select 1 from pg_available_extensions where name='pg_net')then execute 'create extension if not exists pg_net with schema extensions';end if;
 if exists(select 1 from pg_available_extensions where name='supabase_vault')then execute 'create extension if not exists supabase_vault';end if;
end;$$;
create table hris_private.device_push_config(
 singleton boolean primary key default true check(singleton),enabled boolean not null default false,
 daily_attempt_cap integer not null default 1000 check(daily_attempt_cap between 1 and 1000),
 max_devices integer not null default 2000 check(max_devices between 1 and 2000));
insert into hris_private.device_push_config(singleton)values(true);
create table hris_private.device_push_devices(
 id uuid primary key default gen_random_uuid(),user_id uuid not null references auth.users(id)on delete cascade,
 company_id uuid not null references public.companies(id),employee_id uuid not null references public.employees(id),
 token_hash text not null unique check(token_hash~'^[0-9a-f]{64}$'),token text,
 enabled boolean not null default true,created_at timestamptz not null default now(),updated_at timestamptz not null default now(),
 confirmed_at timestamptz not null default now(),scanned_at timestamptz not null default '-infinity',
 last_sent_at timestamptz,last_error text check(last_error in('source_budget','source_unavailable','provider_auth','temporary','unregistered')),
 check(not enabled or token is not null),check(token is null or(length(token)between 40 and 4096 and token!~'[^A-Za-z0-9_:.-]')));
create index device_push_owner on hris_private.device_push_devices(user_id,company_id,employee_id);
create index device_push_live_scan on hris_private.device_push_devices(scanned_at,confirmed_at)where enabled;
create table hris_private.device_push_seen(
 user_id uuid not null references auth.users(id)on delete cascade,company_id uuid not null,employee_id uuid not null,
 key_hash text not null check(key_hash~'^[0-9a-f]{64}$'),seen_at timestamptz not null default now(),
 primary key(user_id,company_id,employee_id,key_hash));
create table hris_private.device_push_jobs(
 id uuid primary key default gen_random_uuid(),device_id uuid not null references hris_private.device_push_devices(id)on delete cascade,
 user_id uuid not null,company_id uuid not null,employee_id uuid not null,
 kind text not null check(kind in('notice','test')),state text not null default 'pending'check(state in('pending','sending','sent','cancelled','failed')),
 attempts integer not null default 0 check(attempts between 0 and 5),created_at timestamptz not null default now(),
 next_attempt_at timestamptz not null default now(),lease_id uuid,lease_until timestamptz,finished_at timestamptz,
 last_result text check(last_result in('sent','temporary','provider_auth','unregistered','cancelled')));
create unique index device_push_one_pending_notice on hris_private.device_push_jobs(device_id)where kind='notice'and state='pending';
create unique index device_push_one_pending_test on hris_private.device_push_jobs(device_id)where kind='test'and state='pending';
create index device_push_due on hris_private.device_push_jobs(next_attempt_at,created_at)where state in('pending','sending');
create index device_push_job_owner on hris_private.device_push_jobs(user_id,created_at);
create table hris_private.device_push_test_receipts(
 user_id uuid not null references auth.users(id)on delete cascade,request_id uuid not null,
 token_hash text not null,job_id uuid not null references hris_private.device_push_jobs(id)on delete cascade,
 created_at timestamptz not null default now(),primary key(user_id,request_id));
create index device_push_test_daily on hris_private.device_push_test_receipts(user_id,created_at);
create table hris_private.device_push_budget(day date primary key,attempts integer not null default 0 check(attempts between 0 and 1000));
create table hris_private.device_push_events(
 id bigint generated always as identity primary key,user_id uuid,device_id uuid,job_id uuid,
 kind text not null check(kind in('register','disable','test','claim','sent','temporary','provider_auth','unregistered','cancelled')),
 occurred_at timestamptz not null default now());
create index device_push_events_actor on hris_private.device_push_events(user_id,kind,occurred_at);
alter table hris_private.device_push_config enable row level security;
alter table hris_private.device_push_devices enable row level security;
alter table hris_private.device_push_seen enable row level security;
alter table hris_private.device_push_jobs enable row level security;
alter table hris_private.device_push_test_receipts enable row level security;
alter table hris_private.device_push_budget enable row level security;
alter table hris_private.device_push_events enable row level security;
revoke all on hris_private.device_push_config,hris_private.device_push_devices,hris_private.device_push_seen,
 hris_private.device_push_jobs,hris_private.device_push_test_receipts,hris_private.device_push_budget,hris_private.device_push_events
 from public,anon,authenticated,service_role;

create function hris_private.push_lock()returns void language sql set search_path=''as
 $$select pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended('hris/device-push/v1',0));$$;
create function hris_private.push_token_hash(p_token text)returns text language plpgsql immutable set search_path=''as $$
begin
 if p_token is null or length(p_token)not between 40 and 4096 or p_token~'[^A-Za-z0-9_:.-]'then raise exception 'Invalid device address.'using errcode='23514';end if;
 return pg_catalog.encode(pg_catalog.sha256(pg_catalog.convert_to(p_token,'UTF8')),'hex');
end;$$;
create function hris_private.push_actor()returns public.company_members language plpgsql security definer set search_path=''as $$
declare m public.company_members;claims jsonb;sid text;
begin
 claims:=coalesce(nullif(current_setting('request.jwt.claims',true),''),'{}')::jsonb;
 sid:=coalesce(nullif(current_setting('request.jwt.claim.session_id',true),''),claims->>'session_id');
 if sid is null or sid!~'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'then raise exception 'Current employee sign-in is required.'using errcode='42501';end if;
 select x.*into m from public.company_members x join public.employees e on e.id=x.employee_id and e.company_id=x.company_id
 join auth.users u on u.id=x.user_id join auth.sessions s on s.user_id=u.id and s.id=sid::uuid
 where x.user_id=(select auth.uid())and x.active and e.status='active'and u.deleted_at is null
 and not coalesce(u.is_anonymous,false)and(u.banned_until is null or u.banned_until<=now())for share of x,e,u,s;
 if not found then raise exception 'Current employee sign-in is required.'using errcode='42501';end if;
 return m;
end;$$;
create function hris_private.push_device_current(p_id uuid)returns boolean language plpgsql stable security definer set search_path=''as $$
begin
 return exists(select 1 from hris_private.device_push_devices d
 join public.company_members m on m.user_id=d.user_id and m.company_id=d.company_id and m.employee_id=d.employee_id and m.active
 join public.employees e on e.id=d.employee_id and e.company_id=d.company_id and e.status='active'
 join auth.users u on u.id=d.user_id
 where d.id=p_id and d.enabled and d.token is not null and d.confirmed_at>now()-interval'30 days'
 and u.deleted_at is null and not coalesce(u.is_anonymous,false)and(u.banned_until is null or u.banned_until<=now()));
end;$$;
-- Exactly the existing own portal sources, with no caller impersonation/content export.
create function hris_private.push_notice_keys(p_user uuid,p_company uuid,p_employee uuid)
returns table(key_hash text,is_read boolean)language plpgsql stable security definer set search_path=''as $$
declare zone text;today date;old_keys text[];new_keys text[];
begin
 select timezone into strict zone from public.companies where id=p_company;today:=(now()at time zone zone)::date;
 select read_keys into old_keys from public.portal_notification_read_state where user_id=p_user and company_id=p_company;
 select read_keys into new_keys from public.lifecycle_notification_read_state where user_id=p_user and company_id=p_company and surface='portal';
 return query select pg_catalog.encode(pg_catalog.sha256(pg_catalog.convert_to(x.alert_key,'UTF8')),'hex'),x.was_read from(
 select r.alert_key,r.alert_key=any(coalesce(old_keys,'{}'))was_read from hris_private.portal_notification_rows(p_company,p_employee,today,now())r
 union all select r.alert_key,r.alert_key=any(coalesce(new_keys,'{}'))from hris_private.lifecycle_notification_rows(p_company,p_employee,'employee',today,now())r)x limit 5001;
end;$$;
create function hris_private.push_identity(m public.company_members)returns jsonb language sql stable set search_path=''as $$
 select jsonb_build_object('version','portal-device-push-v1','user_id',m.user_id,'company_id',m.company_id,'employee_id',m.employee_id);$$;
create function hris_private.portal_push_status(p_token text)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;h text;d hris_private.device_push_devices;ready boolean;
begin
 m:=hris_private.push_actor();h:=hris_private.push_token_hash(p_token);
 select enabled into strict ready from hris_private.device_push_config where singleton;
 select *into d from hris_private.device_push_devices where token_hash=h and user_id=m.user_id and company_id=m.company_id and employee_id=m.employee_id;
 return hris_private.push_identity(m)||jsonb_build_object('available',ready,'enabled',ready and coalesce(d.enabled and d.confirmed_at>now()-interval'30 days',false),
 'expires_at',case when d.enabled then d.confirmed_at+interval'30 days'else null end,'last_sent_at',d.last_sent_at,'last_error',d.last_error);
end;$$;
create function hris_private.portal_push_register(p_token text)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;h text;d hris_private.device_push_devices;cap integer;first_device boolean;keys text[];
begin
 perform hris_private.push_lock();m:=hris_private.push_actor();h:=hris_private.push_token_hash(p_token);
 select max_devices into cap from hris_private.device_push_config where singleton and enabled;
 if cap is null then raise exception 'Device alerts are unavailable.'using errcode='42501';end if;
 if(select count(*)from hris_private.device_push_events where user_id=m.user_id and kind='register'and occurred_at>now()-interval'1 hour')>=20 then raise exception 'Try device setup later.'using errcode='PT429';end if;
 select *into d from hris_private.device_push_devices where token_hash=h for update;
 if not coalesce(d.enabled and d.user_id=m.user_id and d.company_id=m.company_id and d.employee_id=m.employee_id and d.confirmed_at>now()-interval'30 days',false)then
  if(select count(*)from hris_private.device_push_devices where user_id=m.user_id and enabled and confirmed_at>now()-interval'30 days')>=3 then raise exception 'Disable an old device before adding another.'using errcode='23514';end if;
  if(select count(*)from hris_private.device_push_devices where enabled and confirmed_at>now()-interval'30 days')>=cap then raise exception 'Device enrollment is at capacity.'using errcode='54000';end if;
 end if;
 first_device:=not exists(select 1 from hris_private.device_push_devices where user_id=m.user_id and company_id=m.company_id and employee_id=m.employee_id and enabled and confirmed_at>now()-interval'30 days');
 if first_device then
  select coalesce(array_agg(distinct key_hash),'{}')into keys from hris_private.push_notice_keys(m.user_id,m.company_id,m.employee_id);
  if cardinality(keys)>5000 then raise exception 'Personal notice history exceeds the device budget.'using errcode='54000';end if;
  insert into hris_private.device_push_seen(user_id,company_id,employee_id,key_hash)select m.user_id,m.company_id,m.employee_id,k from unnest(keys)k on conflict do nothing;
 end if;
 insert into hris_private.device_push_devices(user_id,company_id,employee_id,token_hash,token)values(m.user_id,m.company_id,m.employee_id,h,p_token)
 on conflict(token_hash)do update set user_id=excluded.user_id,company_id=excluded.company_id,employee_id=excluded.employee_id,
 token=excluded.token,enabled=true,confirmed_at=now(),updated_at=now(),last_error=null returning *into d;
 update hris_private.device_push_jobs set state='cancelled',finished_at=now(),last_result='cancelled'
 where device_id=d.id and state in('pending','sending')and(user_id<>m.user_id or company_id<>m.company_id or employee_id<>m.employee_id);
 insert into hris_private.device_push_events(user_id,device_id,kind)values(m.user_id,d.id,'register');
 return hris_private.portal_push_status(p_token);
end;$$;
create function hris_private.portal_push_disable(p_token text)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;h text;did uuid;
begin
 perform hris_private.push_lock();m:=hris_private.push_actor();h:=hris_private.push_token_hash(p_token);
 update hris_private.device_push_devices set enabled=false,token=null,updated_at=now()where token_hash=h and user_id=m.user_id and company_id=m.company_id and employee_id=m.employee_id returning id into did;
 if did is not null then
  update hris_private.device_push_jobs set state='cancelled',last_result='cancelled',finished_at=now()where device_id=did and state in('pending','sending');
  insert into hris_private.device_push_events(user_id,device_id,kind)values(m.user_id,did,'disable');
 end if;
 return hris_private.push_identity(m)||jsonb_build_object('enabled',false);
end;$$;
create function hris_private.portal_push_test(p_token text,p_request_id uuid)returns jsonb language plpgsql security definer set search_path=''as $$
declare m public.company_members;h text;d hris_private.device_push_devices;r hris_private.device_push_test_receipts;j hris_private.device_push_jobs;
begin
 perform hris_private.push_lock();m:=hris_private.push_actor();h:=hris_private.push_token_hash(p_token);
 if p_request_id is null or p_request_id='00000000-0000-0000-0000-000000000000'then raise exception 'A request identity is required.'using errcode='23514';end if;
 if not exists(select 1 from hris_private.device_push_config where singleton and enabled)then raise exception 'Device alerts are unavailable.'using errcode='42501';end if;
 select *into d from hris_private.device_push_devices where token_hash=h and user_id=m.user_id and company_id=m.company_id and employee_id=m.employee_id and enabled and confirmed_at>now()-interval'30 days';
 if not found then raise exception 'Enable alerts on this device first.'using errcode='42501';end if;
 select *into r from hris_private.device_push_test_receipts where user_id=m.user_id and request_id=p_request_id;
 if found then
  if r.token_hash<>h then raise exception 'Request identity was already used.'using errcode='PT409';end if;
  select *into strict j from hris_private.device_push_jobs where id=r.job_id;
 else
  if exists(select 1 from hris_private.device_push_test_receipts where user_id=m.user_id and created_at>now()-interval'1 minute')or
   (select count(*)from hris_private.device_push_test_receipts where user_id=m.user_id and created_at>=date_trunc('day',now()at time zone'UTC')at time zone'UTC')>=5 then raise exception 'Try a test alert later.'using errcode='PT429';end if;
  select *into j from hris_private.device_push_jobs where device_id=d.id and kind='test'and state='pending';
  if not found then insert into hris_private.device_push_jobs(device_id,user_id,company_id,employee_id,kind)values(d.id,m.user_id,m.company_id,m.employee_id,'test')returning *into j;end if;
  insert into hris_private.device_push_test_receipts(user_id,request_id,token_hash,job_id)values(m.user_id,p_request_id,h,j.id);
  insert into hris_private.device_push_events(user_id,device_id,job_id,kind)values(m.user_id,d.id,j.id,'test');
 end if;
 return hris_private.push_identity(m)||jsonb_build_object('job_id',j.id,'state',j.state);
end;$$;

create function public.portal_push_status(p_token text)returns jsonb language sql security invoker set search_path=''as $$select hris_private.portal_push_status(p_token);$$;
create function public.portal_push_register(p_token text)returns jsonb language sql security invoker set search_path=''as $$select hris_private.portal_push_register(p_token);$$;
create function public.portal_push_disable(p_token text)returns jsonb language sql security invoker set search_path=''as $$select hris_private.portal_push_disable(p_token);$$;
create function public.portal_push_test(p_token text,p_request_id uuid)returns jsonb language sql security invoker set search_path=''as $$select hris_private.portal_push_test(p_token,p_request_id);$$;
revoke all on function hris_private.push_lock(),hris_private.push_token_hash(text),hris_private.push_actor(),hris_private.push_device_current(uuid),
 hris_private.push_notice_keys(uuid,uuid,uuid),hris_private.push_identity(public.company_members)from public,anon,authenticated,service_role;
revoke all on function hris_private.portal_push_status(text),hris_private.portal_push_register(text),hris_private.portal_push_disable(text),hris_private.portal_push_test(text,uuid),
 public.portal_push_status(text),public.portal_push_register(text),public.portal_push_disable(text),public.portal_push_test(text,uuid)from public,anon,authenticated,service_role;
grant execute on function hris_private.portal_push_status(text),hris_private.portal_push_register(text),hris_private.portal_push_disable(text),hris_private.portal_push_test(text,uuid),
 public.portal_push_status(text),public.portal_push_register(text),public.portal_push_disable(text),public.portal_push_test(text,uuid)to authenticated;

create function hris_private.push_maintenance()returns void language plpgsql security definer set search_path=''as $$
begin
 perform hris_private.push_lock();
 update hris_private.device_push_devices set enabled=false,token=null,updated_at=now()
 where enabled and not hris_private.push_device_current(id);
 update hris_private.device_push_jobs j set state='cancelled',last_result='cancelled',finished_at=now()
 where j.state in('pending','sending')and not exists(select 1 from hris_private.device_push_devices d
 where d.id=j.device_id and d.enabled and d.user_id=j.user_id and d.company_id=j.company_id and d.employee_id=j.employee_id);
 -- Operational address/queue retention only. Canonical HR facts/history untouched.
 delete from hris_private.device_push_jobs where state in('sent','cancelled','failed')and finished_at<now()-interval'30 days';
 delete from hris_private.device_push_devices where not enabled and updated_at<now()-interval'90 days';
 delete from hris_private.device_push_seen x where seen_at<now()-interval'90 days'and not exists(select 1 from hris_private.device_push_devices d where d.user_id=x.user_id and d.company_id=x.company_id and d.employee_id=x.employee_id and d.enabled);
 delete from hris_private.device_push_events where occurred_at<now()-interval'90 days';
 delete from hris_private.device_push_budget where day<(now()at time zone'UTC')::date-90;
end;$$;
create function hris_private.push_worker_collect(p_limit integer default 100)returns jsonb language plpgsql security definer set search_path=''as $$
declare g record;d record;all_keys text[];unread_keys text[];added integer;scanned integer:=0;queued integer:=0;errors integer:=0;
begin
 if p_limit is null or p_limit not between 1 and 250 then raise exception 'Choose a bounded device scan.'using errcode='23514';end if;
 perform hris_private.push_lock();perform hris_private.push_maintenance();
 if not exists(select 1 from hris_private.device_push_config where singleton and enabled)then return jsonb_build_object('version','device-push-worker-v1','enabled',false,'scanned',0,'queued',0,'errors',0);end if;
 for g in select user_id,company_id,employee_id,min(scanned_at)last_scan from hris_private.device_push_devices
 where enabled group by user_id,company_id,employee_id order by min(scanned_at),user_id limit p_limit loop
  begin
   select coalesce(array_agg(distinct key_hash),'{}'),coalesce(array_agg(distinct key_hash)filter(where not is_read),'{}')
   into all_keys,unread_keys from hris_private.push_notice_keys(g.user_id,g.company_id,g.employee_id);
   if cardinality(all_keys)>5000 then raise exception 'Notice source budget exceeded.'using errcode='54000';end if;
   if exists(select 1 from unnest(unread_keys)k where not exists(select 1 from hris_private.device_push_seen x
     where x.user_id=g.user_id and x.company_id=g.company_id and x.employee_id=g.employee_id and x.key_hash=k))then
    for d in select *from hris_private.device_push_devices where enabled and user_id=g.user_id and company_id=g.company_id and employee_id=g.employee_id loop
     insert into hris_private.device_push_jobs(device_id,user_id,company_id,employee_id,kind)values(d.id,g.user_id,g.company_id,g.employee_id,'notice')
     on conflict(device_id)where kind='notice'and state='pending'do nothing;
     get diagnostics added=row_count;queued:=queued+added;
    end loop;
   end if;
   if (select count(*)from hris_private.device_push_seen x where x.user_id=g.user_id and x.company_id=g.company_id and x.employee_id=g.employee_id)+(select count(*)from unnest(all_keys)k where not exists(select 1 from hris_private.device_push_seen x where x.user_id=g.user_id and x.company_id=g.company_id and x.employee_id=g.employee_id and x.key_hash=k))>10000 then raise exception 'Notice history budget exceeded.'using errcode='54000';end if;
   insert into hris_private.device_push_seen(user_id,company_id,employee_id,key_hash)
   select g.user_id,g.company_id,g.employee_id,k from unnest(all_keys)k on conflict do nothing;
   update hris_private.device_push_devices set last_error=null where enabled and user_id=g.user_id and company_id=g.company_id and employee_id=g.employee_id and last_error in('source_budget','source_unavailable');
  exception when others then
   update hris_private.device_push_devices set last_error=case when sqlstate='54000'then'source_budget'else'source_unavailable'end
   where user_id=g.user_id and company_id=g.company_id and employee_id=g.employee_id;
   errors:=errors+1;
  end;
  update hris_private.device_push_devices set scanned_at=now()where user_id=g.user_id and company_id=g.company_id and employee_id=g.employee_id;
  scanned:=scanned+1;
 end loop;
 return jsonb_build_object('version','device-push-worker-v1','enabled',true,'scanned',scanned,'queued',queued,'errors',errors);
end;$$;
create function hris_private.push_dispatch_claim(p_limit integer default 25,p_only_job uuid default null)returns jsonb
language plpgsql security definer set search_path=''as $$
declare today date:=(now()at time zone'UTC')::date;used integer;cap integer;j record;lease uuid;items jsonb:='[]'::jsonb;
begin
 if p_limit is null or p_limit not between 1 and 50 then raise exception 'Choose a bounded dispatch batch.'using errcode='23514';end if;
 perform hris_private.push_lock();perform hris_private.push_maintenance();
 select daily_attempt_cap into cap from hris_private.device_push_config where singleton and enabled;
 if cap is null then return jsonb_build_object('version','device-push-claims-v1','capped',false,'items',items);end if;
 insert into hris_private.device_push_budget(day)values(today)on conflict do nothing;
 select attempts into strict used from hris_private.device_push_budget where day=today for update;
 for j in select x.*,d.token from hris_private.device_push_jobs x join hris_private.device_push_devices d on d.id=x.device_id
 where(p_only_job is null or x.id=p_only_job)and((x.state='pending'and x.next_attempt_at<=now())or(x.state='sending'and x.lease_until<=now()))
 and x.attempts<5 and hris_private.push_device_current(d.id)and d.user_id=x.user_id and d.company_id=x.company_id and d.employee_id=x.employee_id
 order by x.created_at,x.id limit least(p_limit,greatest(cap-used,0))for update of x skip locked loop
  lease:=gen_random_uuid();
  update hris_private.device_push_jobs set state='sending',attempts=attempts+1,lease_id=lease,lease_until=now()+interval'3 minutes'where id=j.id;
  insert into hris_private.device_push_events(user_id,device_id,job_id,kind)values(j.user_id,j.device_id,j.id,'claim');
  items:=items||jsonb_build_array(jsonb_build_object('job_id',j.id,'lease_id',lease,'token',j.token));
 end loop;
 update hris_private.device_push_budget set attempts=attempts+jsonb_array_length(items)where day=today;
 return jsonb_build_object('version','device-push-claims-v1','capped',used+jsonb_array_length(items)>=cap,'items',items);
end;$$;
create function hris_private.push_dispatch_finish(p_job_id uuid,p_lease_id uuid,p_result text)returns jsonb
language plpgsql security definer set search_path=''as $$
declare j hris_private.device_push_jobs;next_state text;
begin
 if p_result is null or p_result not in('sent','temporary','provider_auth','unregistered')then raise exception 'Invalid dispatch outcome.'using errcode='23514';end if;
 perform hris_private.push_lock();select *into j from hris_private.device_push_jobs where id=p_job_id for update;
 if not found or p_lease_id is null or j.lease_id is distinct from p_lease_id then raise exception 'Dispatch lease changed.'using errcode='PT409';end if;
 if j.state in('sent','failed')and j.last_result=p_result then return jsonb_build_object('state',j.state);end if;
 if j.state<>'sending'or j.lease_until<=now()then raise exception 'Dispatch lease expired or was cancelled.'using errcode='PT409';end if;
 if p_result='sent'then next_state:='sent';
 elsif p_result='unregistered'or j.attempts>=5 then next_state:='failed';else next_state:='pending';end if;
 update hris_private.device_push_jobs set state=next_state,last_result=p_result,
  finished_at=case when next_state in('sent','failed')then now()else null end,
  next_attempt_at=now()+make_interval(secs=>least(3600,60*(2^j.attempts)::integer)),lease_until=null where id=j.id;
 if p_result='sent'then
  update hris_private.device_push_devices set last_sent_at=now(),last_error=null where id=j.device_id and user_id=j.user_id and company_id=j.company_id and employee_id=j.employee_id;
 elsif p_result='unregistered'then
  update hris_private.device_push_devices set enabled=false,token=null,last_error='unregistered',updated_at=now()
   where id=j.device_id and user_id=j.user_id and company_id=j.company_id and employee_id=j.employee_id;
 else
  update hris_private.device_push_devices set last_error=p_result where id=j.device_id and user_id=j.user_id and company_id=j.company_id and employee_id=j.employee_id;
 end if;
 insert into hris_private.device_push_events(user_id,device_id,job_id,kind)values(j.user_id,j.device_id,j.id,p_result);
 return jsonb_build_object('state',next_state);
end;$$;
-- Only a service worker can obtain the actual address; clients get own status only.
create function public.push_worker_collect(p_limit integer default 100)returns jsonb language sql security invoker set search_path=''as $$select hris_private.push_worker_collect(p_limit);$$;
create function public.push_dispatch_claim(p_limit integer default 25,p_only_job uuid default null)returns jsonb language sql security invoker set search_path=''as $$select hris_private.push_dispatch_claim(p_limit,p_only_job);$$;
create function public.push_dispatch_finish(p_job_id uuid,p_lease_id uuid,p_result text)returns jsonb language sql security invoker set search_path=''as $$select hris_private.push_dispatch_finish(p_job_id,p_lease_id,p_result);$$;
revoke all on function hris_private.push_maintenance(),hris_private.push_worker_collect(integer),hris_private.push_dispatch_claim(integer,uuid),hris_private.push_dispatch_finish(uuid,uuid,text),
 public.push_worker_collect(integer),public.push_dispatch_claim(integer,uuid),public.push_dispatch_finish(uuid,uuid,text)from public,anon,authenticated,service_role;
grant usage on schema hris_private to service_role;
grant execute on function hris_private.push_worker_collect(integer),hris_private.push_dispatch_claim(integer,uuid),hris_private.push_dispatch_finish(uuid,uuid,text),
 public.push_worker_collect(integer),public.push_dispatch_claim(integer,uuid),public.push_dispatch_finish(uuid,uuid,text)to service_role;

-- Vault/pg_net are intentionally used only by the owner-operated scheduled tick.
create function hris_private.device_push_tick()returns bigint language plpgsql security definer set search_path=''as $$
declare endpoint text;secret text;request_id bigint;
begin
 perform hris_private.push_maintenance();
 if not exists(select 1 from hris_private.device_push_config where singleton and enabled)
 or not exists(select 1 from hris_private.device_push_devices where enabled)then return null;end if;
 execute 'select decrypted_secret from vault.decrypted_secrets where name=$1'into endpoint using'hris_device_push_url';
 execute 'select decrypted_secret from vault.decrypted_secrets where name=$1'into secret using'hris_device_push_worker';
 if endpoint is null or endpoint!~'^https://[a-z0-9]+\.supabase\.co/functions/v1/device-push$'or secret is null or length(secret)<40 then raise exception 'Device sender is not configured.'using errcode='23514';end if;
 execute 'select net.http_post(url:=$1,headers:=$2,body:=$3,timeout_milliseconds:=120000)'into request_id
 using endpoint,jsonb_build_object('Content-Type','application/json','Authorization','Bearer '||secret),jsonb_build_object('operation','worker');
 return request_id;
end;$$;
revoke all on function hris_private.device_push_tick()from public,anon,authenticated,service_role;
commit;
