-- Personal read markers only. HR content remains in its original protected tables.
begin;
create function hris_private.valid_notification_keys(p_keys text[],p_limit integer) returns boolean
language sql immutable set search_path='' as $$
 select p_keys is not null and coalesce(array_ndims(p_keys),1)=1 and cardinality(p_keys)<=p_limit
 and not exists(select 1 from unnest(p_keys) k where k is null or length(k)>130 or
 k !~ '^(leave-review|leave-status|leave-upcoming|correction-review|correction-status|overtime-review|overtime-status|document-expiry|contract-end|checkout):[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}:[0-9]{1,19}(:soon|:today|:expired)?$'
 or split_part(k,':',2)='00000000-0000-0000-0000-000000000000'
 or case when split_part(k,':',3) ~ '^[0-9]{1,19}$' then split_part(k,':',3)::numeric>9223372036854775807 else false end)
 and cardinality(p_keys)=(select count(distinct k) from unnest(p_keys) k);
$$;
revoke all on function hris_private.valid_notification_keys(text[],integer) from public,anon,authenticated;

create table public.notification_read_state (
 user_id uuid not null references auth.users(id) on delete cascade,
 company_id uuid not null references public.companies(id) on delete cascade,
 read_keys text[] not null default '{}',
 updated_at timestamptz not null default clock_timestamp(),
 primary key(user_id,company_id),
 constraint notification_read_keys_valid check(hris_private.valid_notification_keys(read_keys,5000))
);
create index notification_read_state_company_idx on public.notification_read_state(company_id);
alter table public.notification_read_state enable row level security;
revoke all on public.notification_read_state from public,anon,authenticated;
grant select on public.notification_read_state to authenticated;
grant all on public.notification_read_state to service_role;
create policy notification_read_state_own on public.notification_read_state for select to authenticated using(
 user_id=(select auth.uid()) and hris_private.company_role(company_id) in ('administrator','hr_generalist','manager')
);

create function hris_private.set_notification_reads(p_company_id uuid,p_keys text[],p_read boolean,p_reset boolean default false) returns jsonb
language plpgsql security definer set search_path='' as $$
declare subject uuid:=(select auth.uid());saved public.notification_read_state;next_keys text[];
begin
 if not exists(select 1 from public.company_members where user_id=subject and company_id=p_company_id and active and role in ('administrator','hr_generalist','manager')) then
  raise exception 'Active desktop staff membership is required for notifications.' using errcode='42501';end if;
 if p_read is null or p_reset is null or not hris_private.valid_notification_keys(p_keys,100)
 or (not p_reset and cardinality(p_keys)=0) or (p_reset and (cardinality(p_keys)<>0 or p_read)) then
  raise exception 'The notification read operation is invalid.' using errcode='23514';end if;
 insert into public.notification_read_state(user_id,company_id) values(subject,p_company_id) on conflict(user_id,company_id) do nothing;
 select * into saved from public.notification_read_state where user_id=subject and company_id=p_company_id for update;
 if p_reset then next_keys:='{}';
 elsif p_read then select array_agg(k order by k) into next_keys from (select distinct unnest(saved.read_keys||p_keys) k) s;
 else select coalesce(array_agg(k order by k),'{}'::text[]) into next_keys from unnest(saved.read_keys) k where not(k=any(p_keys));end if;
 if cardinality(next_keys)>5000 then raise exception 'Read history is full. Reset read history in Notifications before marking more items.' using errcode='23514';end if;
 update public.notification_read_state set read_keys=next_keys,updated_at=clock_timestamp() where user_id=subject and company_id=p_company_id returning * into saved;
 return to_jsonb(saved);
end;
$$;
revoke all on function hris_private.set_notification_reads(uuid,text[],boolean,boolean) from public,anon;
grant execute on function hris_private.set_notification_reads(uuid,text[],boolean,boolean) to authenticated;
create function public.set_notification_reads(p_company_id uuid,p_keys text[],p_read boolean,p_reset boolean default false) returns jsonb
language sql security invoker set search_path='' as $$select hris_private.set_notification_reads(p_company_id,p_keys,p_read,p_reset);$$;
revoke all on function public.set_notification_reads(uuid,text[],boolean,boolean) from public,anon;
grant execute on function public.set_notification_reads(uuid,text[],boolean,boolean) to authenticated;
commit;
