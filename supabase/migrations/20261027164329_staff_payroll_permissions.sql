-- Payroll admission only. Calculations, source guards and independent approval
-- remain unchanged. Runtime gate remains FALSE pending full module verification.
begin;
create or replace function hris_private.payroll_hr(p_company uuid)returns boolean
language sql stable security definer set search_path=''as $$
 select hris_private.has_staff_capability(p_company,'payroll_preparation')
$$;
-- Exact reviewed membership predicates; preserve OIDs, grants, locked rechecks,
-- approval exclusions and original immutable financial/receipt history.
do $permission_patch$
declare spec record;definition text;source text;needle text:='role in(''administrator'',''hr_generalist'')';matches integer;
begin
 for spec in select *from(values
 ('hris_private.government_actor()',2,'government_preparation'),
 ('hris_private.government_payroll_sources(date,text)',1,'government_preparation'),
 ('hris_private.government_registration(uuid)',1,'government_preparation'),
 ('hris_private.manage_payroll(text,jsonb)',2,'payroll_preparation'),
 ('hris_private.manage_payroll_amendment(text,jsonb)',2,'payroll_preparation'),
 ('hris_private.manage_payroll_cutoff(text,jsonb)',2,'payroll_preparation'),
 ('hris_private.manage_payroll_final(text,jsonb)',2,'payroll_preparation'),
 ('hris_private.manage_payroll_tax(text,jsonb)',2,'payroll_preparation'),
 ('hris_private.manage_payroll_thirteenth(text,jsonb)',2,'payroll_preparation'),
 ('hris_private.manage_payroll_wage(text,jsonb)',2,'payroll_preparation'),
 ('hris_private.payroll_exchange_member()',1,'payroll_preparation'),
 ('hris_private.payroll_report_before_amendments(jsonb)',1,'payroll_reports'),
 ('hris_private.payroll_report_v2(jsonb)',1,'payroll_reports'),
 ('hris_private.wage_payroll_guard()',2,'payroll_preparation')
 )as reviewed(identity,expected,capability)loop
  select p.prosrc,pg_catalog.pg_get_functiondef(p.oid)into source,definition
   from pg_catalog.pg_proc p where p.oid=spec.identity::regprocedure;
  matches:=(length(source)-length(replace(source,needle,'')))/length(needle);
  if matches<>spec.expected then raise exception 'Payroll permission source changed: %',spec.identity;end if;
  execute replace(definition,needle,'hris_private.has_staff_capability(company_id,'||quote_literal(spec.capability)||')');
 end loop;
end;$permission_patch$;
comment on function hris_private.payroll_hr(uuid)is 'Protected payroll preparation; legacy rights retained while rollout disabled. Own issued slips and independent Administrator approval/payment remain separately guarded.';
commit;
