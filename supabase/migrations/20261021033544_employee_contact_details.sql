-- Private HR contacts are not part of the manager-readable employee directory.
begin;

create table public.employee_contacts (
    id uuid primary key default gen_random_uuid(),
    company_id uuid not null references public.companies(id) on delete restrict,
    employee_id uuid not null,
    phone text not null default '',
    personal_email text not null default '',
    address text not null default '' check (char_length(address) <= 500),
    emergency_name text not null default '' check (char_length(emergency_name) <= 120),
    emergency_relationship text not null default '' check (char_length(emergency_relationship) <= 80),
    emergency_phone text not null default '',
    emergency_alternate_phone text not null default '',
    updated_at timestamptz not null default clock_timestamp(),
    unique(company_id,employee_id),
    foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict,
    check (personal_email='' or (octet_length(personal_email)<=254
        and char_length(personal_email)-char_length(replace(personal_email,'@',''))=1
        and split_part(personal_email,'@',1)<>'' and split_part(personal_email,'@',2) like '%.%'
        and split_part(personal_email,'@',2) not like '.%' and split_part(personal_email,'@',2) not like '%.'
        and personal_email !~ '[[:space:][:cntrl:]]')),
    check ((emergency_name='' and emergency_relationship='' and emergency_phone='' and emergency_alternate_phone='')
        or (emergency_name<>'' and emergency_relationship<>'' and emergency_phone<>''))
);

create function hris_private.validate_employee_contacts() returns trigger
language plpgsql security invoker set search_path='' as $$
declare number text;
begin
    new.phone:=btrim(new.phone); new.personal_email:=btrim(new.personal_email); new.address:=btrim(new.address);
    new.emergency_name:=btrim(new.emergency_name); new.emergency_relationship:=btrim(new.emergency_relationship);
    new.emergency_phone:=btrim(new.emergency_phone); new.emergency_alternate_phone:=btrim(new.emergency_alternate_phone);
    foreach number in array array[new.phone,new.emergency_phone,new.emergency_alternate_phone] loop
        if number<>'' and (char_length(number)>40 or number !~ '^[0-9 +()./xX#-]+$'
            or char_length(regexp_replace(number,'[^0-9]','','g'))<3) then
            raise exception 'Use phone numbers up to 40 characters with at least 3 digits.' using errcode='23514';
        end if;
    end loop;
    new.updated_at:=clock_timestamp();
    return new;
end;
$$;
revoke all on function hris_private.validate_employee_contacts() from public,anon,authenticated;
create trigger employee_contacts_validate before insert or update on public.employee_contacts
for each row execute function hris_private.validate_employee_contacts();

alter table public.employee_contacts enable row level security;
revoke all on public.employee_contacts from public,anon,authenticated;
grant select on public.employee_contacts to authenticated;
grant insert (company_id,employee_id,phone,personal_email,address,emergency_name,emergency_relationship,
    emergency_phone,emergency_alternate_phone) on public.employee_contacts to authenticated;
grant update (phone,personal_email,address,emergency_name,emergency_relationship,emergency_phone,
    emergency_alternate_phone) on public.employee_contacts to authenticated;

create policy employee_contacts_read on public.employee_contacts for select to authenticated using (
    hris_private.company_role(company_id) in ('administrator','hr_generalist') or exists (
        select 1 from public.company_members m where m.company_id=employee_contacts.company_id
            and m.user_id=(select auth.uid()) and m.active and m.employee_id=employee_contacts.employee_id));
create policy employee_contacts_create on public.employee_contacts for insert to authenticated
    with check (hris_private.company_role(company_id) in ('administrator','hr_generalist'));
create policy employee_contacts_update on public.employee_contacts for update to authenticated
    using (hris_private.company_role(company_id) in ('administrator','hr_generalist'))
    with check (hris_private.company_role(company_id) in ('administrator','hr_generalist'));

-- No client DELETE, identity reassignment or client-supplied update timestamps.
commit;
