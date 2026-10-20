-- Each deployment is dedicated to one company. Only the project administrator
-- provisions its company and assigns the first HR account through trusted setup.
begin;

-- The unique index fails safely if a non-dedicated database has multiple rows.
create unique index companies_single_installation on public.companies ((true));
revoke insert on public.companies from public, anon, authenticated;
revoke insert (owner_id, name, timezone, annual_leave_days)
    on public.companies from public, anon, authenticated;
alter policy companies_create on public.companies with check (false);

commit;
