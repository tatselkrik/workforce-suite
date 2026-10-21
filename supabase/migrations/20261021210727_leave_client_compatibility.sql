-- A read compatibility filter, never an authorization grant. Older clients
-- cannot decode custom/cancelled rows, but must still sign in to get updates.
begin;
create function hris_private.readable_leave_contract(p_kind text,p_status text) returns boolean
language sql stable security invoker set search_path='' as $$
 select (p_kind in ('annual','sick','unpaid') and p_status in ('pending','approved','rejected'))
  or coalesce(nullif(current_setting('request.headers',true),'')::jsonb->>'x-hris-leave-contract','')='2';
$$;
revoke all on function hris_private.readable_leave_contract(text,text) from public,anon,authenticated;
grant execute on function hris_private.readable_leave_contract(text,text) to authenticated;
alter policy leave_read on public.leave_requests using(
 hris_private.readable_leave_contract(kind,status) and
 (hris_private.can_read_employee(company_id,employee_id) or hris_private.can_read_leave_receipt(company_id,approval_steps)
  or (status='pending' and hris_private.can_act_leave(company_id,employee_id,approval_steps))));
commit;
