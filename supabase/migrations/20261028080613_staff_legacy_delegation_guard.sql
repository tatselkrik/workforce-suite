-- v1 cannot express Employee-base specialist delegates. Require an explicit
-- upgrade rather than returning malformed or silently incomplete audit history.
begin;
create or replace function public.approval_delegation_context()returns jsonb
language plpgsql security invoker set search_path=''as $$
declare c jsonb;
begin
 c:=hris_private.approval_delegation_context();
 if exists(select 1 from jsonb_array_elements(c->'grants')g
  where g->>'principal_role'='employee'or g->>'delegate_role'='employee')then
  raise exception 'Update HRIS to view delegations involving specialist staff accounts; original history is retained.'using errcode='PT426';
 end if;
 return c;
end;$$;
notify pgrst,'reload schema';
commit;
