-- Run as the project administrator after creating the first HR Auth user.
-- This provisions configuration only, never demo employees or credentials.
-- Additional Auth users require explicit selection instead of guessing an owner.
begin;
do $$
declare administrator_id uuid;
begin
    if exists (select 1 from public.companies) then
        raise exception 'The company is already configured; setup did not change it.';
    end if;
    if (select count(*) from auth.users) <> 1 then
        raise exception 'Create exactly one HR administrator Auth user before initial setup.';
    end if;
    select id into administrator_id from auth.users;
    insert into public.companies (owner_id, name, timezone, annual_leave_days)
        values (administrator_id, 'HRIS Company', 'Asia/Manila', 15);
end;
$$;
commit;
