-- Operator-only configuration, not an application endpoint.
-- Run only via the linked CLI after verifying the private SQL target guard,
-- exact installed/signed release, publication and complete permission evidence.
-- No Auth, employment, source pay, immutable history or grants are changed.
begin;
do $activation$
declare c public.companies;
begin
 if(select count(*)from public.companies)<>1 then raise exception 'One company is required';end if;
 select *into strict c from public.companies;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(c.id::text,7));
 if not exists(select 1 from public.company_members m join public.employees e
 on e.company_id=m.company_id and e.id=m.employee_id
 where m.company_id=c.id and m.user_id=c.owner_id and m.active and m.role='administrator'and e.status='active')
 then raise exception 'Preserve the active owner Administrator';end if;
 if exists(select 1 from public.company_members m left join hris_private.staff_role_assignments a
 on a.company_id=m.company_id and a.user_id=m.user_id where m.company_id=c.id
 and(a.user_id is null or a.base_role<>m.role or a.roles<>hris_private.normalize_staff_roles(a.roles)
 or hris_private.compatible_staff_base_role(a.roles)<>m.role))
 then raise exception 'Reconcile all protected assignments before activation';end if;
 if not exists(select 1 from supabase_migrations.schema_migrations where version='20261028080613')
 then raise exception 'Complete the staff safeguard migrations first';end if;
 if not exists(select 1 from public.desktop_releases where version='0.1.63'and enabled)
 then raise exception 'Deliver the verified staff release first';end if;
 update hris_private.staff_role_runtime_policy set runtime_ready=true where company_id=c.id;
 if not found then raise exception 'Missing protected rollout policy';end if;
end;$activation$;
commit;
