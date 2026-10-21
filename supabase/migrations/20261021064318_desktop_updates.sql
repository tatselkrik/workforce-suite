-- Private, signed Windows updates; only trusted deployment tooling can publish.
begin;
create table public.desktop_releases (
    version text primary key check(version ~ '^(0|[1-9][0-9]{0,5})\.(0|[1-9][0-9]{0,5})\.(0|[1-9][0-9]{0,5})$'),
    platform text not null check(platform='windows-x64'),
    requires_schema text not null check(requires_schema ~ '^[0-9]{14}$'),
    envelope jsonb not null check(jsonb_typeof(envelope)='object' and octet_length(envelope::text)<=16000),
    enabled boolean not null default false,
    published_at timestamptz not null default clock_timestamp()
);
alter table public.desktop_releases enable row level security;
revoke all on public.desktop_releases from public,anon,authenticated;
grant select,insert,update on public.desktop_releases to service_role;

insert into storage.buckets(id,name,public,file_size_limit,allowed_mime_types)
    values('desktop-updates','desktop-updates',false,67108864,array['application/octet-stream']);
create policy hris_desktop_updates_download on storage.objects for select to authenticated using(
    bucket_id='desktop-updates' and exists(select 1 from public.company_members m where m.user_id=(select auth.uid())
        and m.active and m.role in ('administrator','hr_generalist','manager')));
-- No authenticated storage write/delete policies or catalog-write grants.

create function hris_private.desktop_update() returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare latest_schema text;
begin
    if not exists(select 1 from public.company_members where user_id=(select auth.uid()) and active
        and role in ('administrator','hr_generalist','manager')) then
        raise exception 'An active desktop staff account is required to check updates.' using errcode='42501';
    end if;
    select max(version) into latest_schema from supabase_migrations.schema_migrations;
    return jsonb_build_object('schema_version',latest_schema,'release',
        (select envelope from public.desktop_releases where enabled and platform='windows-x64'
        and requires_schema<=latest_schema order by string_to_array(version,'.')::integer[] desc limit 1));
end;
$$;
revoke all on function hris_private.desktop_update() from public,anon,authenticated;
grant execute on function hris_private.desktop_update() to authenticated;
create function public.desktop_update() returns jsonb language sql stable security invoker set search_path='' as $$
    select hris_private.desktop_update();
$$;
revoke all on function public.desktop_update() from public,anon,authenticated;
grant execute on function public.desktop_update() to authenticated;
commit;
