-- Saved layouts are private preferences, never authorization claims or JWT data.
begin;
create function hris_private.valid_report_views(p_company uuid,p_views jsonb) returns boolean
language plpgsql immutable set search_path='' as $$
declare v jsonb;
begin
 if p_company is null or p_views is null or jsonb_typeof(p_views)<>'array' then return false;end if;
 if jsonb_array_length(p_views)>20 or octet_length(p_views::text)>65536 then return false;end if;
 for v in select value from jsonb_array_elements(p_views) loop
  if jsonb_typeof(v)<>'object' or jsonb_typeof(v->'name') is distinct from 'string'
   or char_length(btrim(v->>'name')) not between 1 and 80 or v->>'name' ~ '[[:cntrl:]]'
   or v->>'company_id' is distinct from p_company::text
   or coalesce(v->>'id','') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
   or v->>'id'='00000000-0000-0000-0000-000000000000'
   or jsonb_typeof(v->'filter') is distinct from 'object' then return false;end if;
 end loop;
 return (select count(*)=count(distinct value->>'id') and count(*)=count(distinct lower(btrim(value->>'name'))) from jsonb_array_elements(p_views));
end;
$$;
revoke all on function hris_private.valid_report_views(uuid,jsonb) from public,anon,authenticated;

create table public.report_preferences (
 user_id uuid not null references auth.users(id) on delete cascade,
 company_id uuid not null references public.companies(id) on delete cascade,
 views jsonb not null default '[]'::jsonb,
 revision bigint not null default 0 check(revision>=0),
 updated_at timestamptz not null default clock_timestamp(),
 primary key(user_id,company_id),
 constraint report_preferences_valid_views check(hris_private.valid_report_views(company_id,views))
);
create index report_preferences_company_idx on public.report_preferences(company_id);
alter table public.report_preferences enable row level security;
revoke all on public.report_preferences from public,anon,authenticated;
grant select on public.report_preferences to authenticated;
grant all on public.report_preferences to service_role;
create policy report_preferences_read on public.report_preferences for select to authenticated using(
 user_id=(select auth.uid()) and hris_private.company_role(company_id) in ('administrator','hr_generalist','manager')
);

-- Copy existing same-company preferences once; preserve legacy Auth metadata
-- and other profile fields so older installations remain able to sign in.
insert into public.report_preferences(user_id,company_id,views,revision)
select m.user_id,m.company_id,v.views,1 from public.company_members m join auth.users u on u.id=m.user_id
cross join lateral (
 select coalesce(jsonb_agg(e.value),'[]'::jsonb) views from jsonb_array_elements(
  case when jsonb_typeof(u.raw_user_meta_data->'hris_report_views')='array' then u.raw_user_meta_data->'hris_report_views' else '[]'::jsonb end
 )e where e.value->>'company_id'=m.company_id::text
)v
where m.active and m.role in ('administrator','hr_generalist','manager')
 and jsonb_array_length(v.views)>0 and hris_private.valid_report_views(m.company_id,v.views);

create function hris_private.save_report_preferences(p_company_id uuid,p_expected_revision bigint,p_views jsonb) returns jsonb
language plpgsql security definer set search_path='' as $$
declare subject uuid:=(select auth.uid());saved public.report_preferences;
begin
 if not exists(select 1 from public.company_members where user_id=subject and company_id=p_company_id and active and role in ('administrator','hr_generalist','manager')) then
  raise exception 'Active desktop staff membership is required for saved reports.' using errcode='42501';end if;
 if p_expected_revision is null or p_expected_revision<0 or not hris_private.valid_report_views(p_company_id,p_views) then
  raise exception 'The saved report views are invalid or exceed their limits.' using errcode='23514';end if;
 insert into public.report_preferences(user_id,company_id) values(subject,p_company_id) on conflict(user_id,company_id) do nothing;
 select * into saved from public.report_preferences where user_id=subject and company_id=p_company_id for update;
 if saved.revision<>p_expected_revision then raise exception 'Saved report views changed in another session. Reload views and review your draft before saving again.' using errcode='PT409';end if;
 update public.report_preferences set views=p_views,revision=revision+1,updated_at=clock_timestamp() where user_id=subject and company_id=p_company_id returning * into saved;
 return to_jsonb(saved);
end;
$$;
revoke all on function hris_private.save_report_preferences(uuid,bigint,jsonb) from public,anon;
grant execute on function hris_private.save_report_preferences(uuid,bigint,jsonb) to authenticated;
create function public.save_report_preferences(p_company_id uuid,p_expected_revision bigint,p_views jsonb) returns jsonb
language sql security invoker set search_path='' as $$select hris_private.save_report_preferences(p_company_id,p_expected_revision,p_views);$$;
revoke all on function public.save_report_preferences(uuid,bigint,jsonb) from public,anon;
grant execute on function public.save_report_preferences(uuid,bigint,jsonb) to authenticated;
commit;
