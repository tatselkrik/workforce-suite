-- Photos are private and scoped to the authenticated company administrator.
-- Never use user-editable account metadata to determine access.
begin;

insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('profile-photos', 'profile-photos', false, 2097152, array['image/png']);

create policy hris_profile_photos_select on storage.objects
for select to authenticated
using (
    bucket_id = 'profile-photos'
    and name = (select auth.uid())::text || '/avatar.png'
    and owner_id = (select auth.uid())::text
    and exists (select 1 from public.companies where owner_id = (select auth.uid()))
);

create policy hris_profile_photos_insert on storage.objects
for insert to authenticated
with check (
    bucket_id = 'profile-photos'
    and name = (select auth.uid())::text || '/avatar.png'
    and owner_id = (select auth.uid())::text
    and exists (select 1 from public.companies where owner_id = (select auth.uid()))
);

create policy hris_profile_photos_update on storage.objects
for update to authenticated
using (
    bucket_id = 'profile-photos'
    and name = (select auth.uid())::text || '/avatar.png'
    and owner_id = (select auth.uid())::text
    and exists (select 1 from public.companies where owner_id = (select auth.uid()))
)
with check (
    bucket_id = 'profile-photos'
    and name = (select auth.uid())::text || '/avatar.png'
    and owner_id = (select auth.uid())::text
    and exists (select 1 from public.companies where owner_id = (select auth.uid()))
);

create policy hris_profile_photos_delete on storage.objects
for delete to authenticated
using (
    bucket_id = 'profile-photos'
    and name = (select auth.uid())::text || '/avatar.png'
    and owner_id = (select auth.uid())::text
    and exists (select 1 from public.companies where owner_id = (select auth.uid()))
);

commit;
