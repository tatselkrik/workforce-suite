-- Own-only portal projections. Existing staff RPCs and installed clients remain
-- unchanged. Compute balances with the established charge/accrual implementation.
begin;
create function hris_private.portal_leave_context(p_asof date) returns jsonb
language plpgsql stable security definer set search_path='' as $$
declare m public.company_members;p public.leave_policies;t jsonb;asof date;options jsonb:='[]';
begin
 select * into m from public.company_members where user_id=(select auth.uid()) and active;
 if not found or m.employee_id is null or not exists(select 1 from public.employees where company_id=m.company_id and id=m.employee_id and status='active')then raise exception 'Active employee access is required.'using errcode='42501';end if;
 select coalesce(p_asof,(now() at time zone timezone)::date) into asof from public.companies where id=m.company_id;
 if asof is null or not isfinite(asof)or asof<date '1900-01-01'or asof>date '2200-12-31'then raise exception 'Use a valid balance date.'using errcode='23514';end if;
 select * into strict p from public.leave_policies where company_id=m.company_id;
 for t in select value from jsonb_array_elements(p.types)where(value->>'active')::boolean loop
  options:=options||jsonb_build_array(jsonb_build_object('code',t->>'code','name',t->>'name','paid',(t->>'paid')::boolean,'unlimited',t->>'accrual'='unlimited','balance',case when t->>'accrual'='unlimited'then null else hris_private.leave_balance(m.employee_id,t,extract(year from asof)::integer,asof)end));
 end loop;
 return jsonb_build_object('company_id',m.company_id,'employee_id',m.employee_id,'policy_revision',p.revision,'asof',asof,'workweek',p.workweek,'holidays',p.holidays,'types',options);
end;$$;
create function hris_private.portal_submit_leave(p_type_code text,p_starts date,p_ends date,p_part text,p_reason text,p_policy_revision bigint)returns uuid
language plpgsql security definer set search_path='' as $$
declare m public.company_members;current_revision bigint;
begin
 select * into m from public.company_members where user_id=(select auth.uid())and active;
 if not found or m.employee_id is null then raise exception 'Active employee access is required.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 if not exists(select 1 from public.company_members where user_id=m.user_id and company_id=m.company_id and employee_id=m.employee_id and active)then raise exception 'Your access changed. Sign in again.'using errcode='42501';end if;
 select revision into strict current_revision from public.leave_policies where company_id=m.company_id;
 if p_policy_revision is distinct from current_revision then raise exception 'The leave policy changed. Refresh and review the dates and charge before filing.'using errcode='PT409';end if;
 return hris_private.submit_leave(m.employee_id,p_type_code,p_starts,p_ends,p_part,p_reason);
end;$$;
create function hris_private.portal_cancel_leave(p_id uuid,p_revision bigint,p_reason text)returns uuid
language plpgsql security definer set search_path='' as $$
declare m public.company_members;r public.leave_requests;today date;
begin
 select * into m from public.company_members where user_id=(select auth.uid())and active;
 if not found or m.employee_id is null then raise exception 'Active employee access is required.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 if not exists(select 1 from public.company_members where user_id=m.user_id and company_id=m.company_id and employee_id=m.employee_id and active)then raise exception 'Your access changed. Sign in again.'using errcode='42501';end if;
 select * into r from public.leave_requests where id=p_id and company_id=m.company_id and employee_id=m.employee_id for update;
 if not found then raise exception 'Only your own leave is available here.'using errcode='42501';end if;
 select(now() at time zone timezone)::date into today from public.companies where id=m.company_id;
 if r.status='approved'and r.starts_on<today then raise exception 'Ask HR to review cancellation of approved leave that has already begun.'using errcode='23514';end if;
 return hris_private.cancel_leave(p_id,p_revision,p_reason);
end;$$;

create function public.portal_leave_context(p_asof date default null)returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.portal_leave_context(p_asof);$$;
create function public.portal_submit_leave(p_type_code text,p_starts date,p_ends date,p_part text,p_reason text,p_policy_revision bigint)returns uuid language sql security invoker set search_path=''as $$select hris_private.portal_submit_leave(p_type_code,p_starts,p_ends,p_part,p_reason,p_policy_revision);$$;
create function public.portal_cancel_leave(p_id uuid,p_revision bigint,p_reason text)returns uuid language sql security invoker set search_path=''as $$select hris_private.portal_cancel_leave(p_id,p_revision,p_reason);$$;
revoke all on function hris_private.portal_leave_context(date),hris_private.portal_submit_leave(text,date,date,text,text,bigint),hris_private.portal_cancel_leave(uuid,bigint,text),public.portal_leave_context(date),public.portal_submit_leave(text,date,date,text,text,bigint),public.portal_cancel_leave(uuid,bigint,text)from public,anon,authenticated;
grant execute on function hris_private.portal_leave_context(date),hris_private.portal_submit_leave(text,date,date,text,text,bigint),hris_private.portal_cancel_leave(uuid,bigint,text),public.portal_leave_context(date),public.portal_submit_leave(text,date,date,text,text,bigint),public.portal_cancel_leave(uuid,bigint,text)to authenticated;
commit;
