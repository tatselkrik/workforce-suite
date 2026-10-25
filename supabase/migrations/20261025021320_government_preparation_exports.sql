-- Single-statement HR-only preparation projection. No filing/payment mutation.
begin;
create function hris_private.government_preparation_keys(agency text,person boolean)
returns text[] language sql immutable security invoker set search_path=''as $$
 select(case when person then array['last_name','first_name','middle_name','name_suffix']else array['registered_name']end)
 ||case agency when'bir'then array['bir_tin','bir_branch']when'sss'then array['sss_number']
 when'philhealth'then array['philhealth_number']when'pagibig'then array['pagibig_number']else array[]::text[]end
 ||case when person and agency='philhealth'then array['birth_date','sex']when person and agency='bir'then array['nationality']else array[]::text[]end;
$$;
revoke all on function hris_private.government_preparation_keys(text,boolean)from public,anon,authenticated;
create function hris_private.government_preparation(p_month date,p_agency text)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare b jsonb;cid uuid;identities jsonb;n integer;begin
 b:=hris_private.government_reconciliation(p_month,p_agency);cid:=(b->'source'->>'company_id')::uuid;
 select coalesce(jsonb_agg(q.j order by q.j->>'employee_id'),'[]'),count(*)into identities,n from(
  select jsonb_build_object('id',r.id,'company_id',r.company_id,'employee_id',r.employee_id,'revision',r.revision,'details',
   (select coalesce(jsonb_object_agg(k,v),'{}')from jsonb_each_text(r.details)d(k,v)
    where k=any(hris_private.government_preparation_keys(p_agency,r.employee_id is not null))))j
  from public.government_registration_records r where r.company_id=cid
   and(r.employee_id is null or r.employee_id in(select(e->>'employee_id')::uuid from jsonb_array_elements(b->'employees')e))limit 20002
 )q;
 if n>20001 then raise exception 'Too many registration identities; no preparation data was truncated.'using errcode='23514';end if;
 return jsonb_build_object('version','government-preparation-v1','review',b,'registrations',identities);
end;$$;
revoke all on function hris_private.government_preparation(date,text)from public,anon,authenticated;
grant execute on function hris_private.government_preparation(date,text)to authenticated;
create function public.government_preparation(p_month date,p_agency text)returns jsonb
language sql stable security invoker set search_path=''as $$select hris_private.government_preparation(p_month,p_agency);$$;
revoke all on function public.government_preparation(date,text)from public,anon,authenticated;
grant execute on function public.government_preparation(date,text)to authenticated;
commit;
