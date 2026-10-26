-- Minimal BIR-only current identity roster beside the protected annual ledger.
-- Local files never enter this RPC. No tax calculation or registration mutation.
begin;
create function hris_private.government_bir_identities(p_year integer) returns jsonb
language plpgsql stable security invoker set search_path='' as $$
declare ledger jsonb;cid uuid;people jsonb;employer jsonb;n integer;
begin
 ledger:=hris_private.government_annual_ledger(p_year);cid:=(ledger->>'company_id')::uuid;
 select coalesce(jsonb_agg(q.person order by q.employee_id),'[]'),count(*) into people,n from(
  select e.id employee_id,jsonb_build_object('label',jsonb_build_object('employee_id',e.id,
   'employee_code',e.employee_code,'employee_name',e.full_name),'registration',case when r.id is null then null else
   jsonb_build_object('id',r.id,'company_id',r.company_id,'employee_id',r.employee_id,'revision',r.revision,
    'tin',coalesce(r.details->>'bir_tin',''),'branch',coalesce(r.details->>'bir_branch',''))end)person
  from public.employees e left join public.government_registration_records r
   on r.company_id=e.company_id and r.employee_id=e.id
  where e.company_id=cid order by e.id limit 20001
 )q;
 if n>20000 then raise exception 'Current BIR identity roster exceeds the review limit. No identities were truncated.'using errcode='23514';end if;
 select jsonb_build_object('id',r.id,'company_id',r.company_id,'employee_id',r.employee_id,'revision',r.revision,
  'tin',coalesce(r.details->>'bir_tin',''),'branch',coalesce(r.details->>'bir_branch',''))into employer
 from public.government_registration_records r where r.company_id=cid and r.employee_id is null;
 return jsonb_build_object('version','government-bir-identities-v1','ledger',ledger,'employer',employer,'people',people);
end;$$;
revoke all on function hris_private.government_bir_identities(integer)from public,anon,authenticated;
grant execute on function hris_private.government_bir_identities(integer)to authenticated;
create function public.government_bir_identities(p_year integer)returns jsonb
language sql stable security invoker set search_path=''as $$select hris_private.government_bir_identities(p_year);$$;
revoke all on function public.government_bir_identities(integer)from public,anon,authenticated;
grant execute on function public.government_bir_identities(integer)to authenticated;
commit;
