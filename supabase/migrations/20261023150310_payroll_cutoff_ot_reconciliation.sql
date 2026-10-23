-- Preserve issued facts while preventing reuse of cross-cutoff OT authorization.
begin;
create function hris_private.cutoff_ot_limit(b jsonb,part integer)returns integer language plpgsql immutable security invoker set search_path=''as $$
declare cap integer;used integer:=0;begin
 if part is null or part not in(1,2)then raise exception 'Choose cutoff1 or2.'using errcode='23514';end if;
 select coalesce(sum((a->>'effective_ot')::integer),0)into cap from jsonb_array_elements(coalesce(nullif(b->case when part=2 then 'month_overtime'else 'attendance'end,'null'::jsonb),'[]'))a;
 if part=2 then select coalesce(sum((d->>'overtime_minutes')::integer),0)into used from jsonb_array_elements(coalesce(b#>'{first_cutoff,item,worksheet,days}','[]'))d;end if;
 return greatest(cap-used,0);
end;$$;
create or replace function hris_private.cutoff_basis(cid uuid,eid uuid,mon date,part integer)returns jsonb language plpgsql stable security invoker set search_path=''as $$
declare b jsonb;month_ot jsonb;s public.payroll_cutoff_slips;lo date;hi date;zone text;begin
 b:=hris_private.payroll_basis(cid,eid,mon);zone:=b->>'timezone';
 lo:=case when part=1 then mon else mon+15 end;hi:=case when part=1 then mon+14 else(mon+interval '1month -1day')::date end;
 if(b#>>'{employee,joined_on}')::date>hi then raise exception 'Employee joined after this cutoff.'using errcode='23514';end if;
 if part=2 then select coalesce(jsonb_agg(jsonb_build_object('id',a#>'{record,id}','effective_ot',a->'effective_ot')order by a#>>'{record,id}'),'[]')into month_ot from jsonb_array_elements(b->'attendance')a where(a->>'effective_ot')::integer>0;end if;
 b:=b||jsonb_build_object('month_overtime',month_ot,'attendance',coalesce((select jsonb_agg(a order by a#>>'{record,id}')from jsonb_array_elements(b->'attendance')a where(a#>>'{record,work_date}')::date between lo and hi or((a#>>'{record,checked_in_at}')::timestamptz<((hi+1)::timestamp at time zone zone)and coalesce(a#>>'{record,checked_out_at}',a#>>'{record,checked_in_at}')::timestamptz>=(lo::timestamp at time zone zone))),'[]'),
 'leave',coalesce((select jsonb_agg(l order by l->>'id')from jsonb_array_elements(b->'leave')l where(l->>'starts_on')::date<=hi and(l->>'ends_on')::date>=lo),'[]'),
 'holidays',coalesce((select jsonb_agg(h order by h->>'date')from jsonb_array_elements(b->'holidays')h where(h->>'date')::date between lo and hi),'[]'),
 'leave_holidays',coalesce((select jsonb_agg(h order by h->>'date')from jsonb_array_elements(b->'leave_holidays')h where(h->>'date')::date between lo and hi),'[]'));
 if part=2 then select ps.* into s from public.payroll_cutoff_slips ps where ps.company_id=cid and ps.employee_id=eid and ps.month=mon and ps.part=1;end if;
 return b||jsonb_build_object('first_cutoff',case when s.id is null then null else to_jsonb(s)end);
end;$$;

create or replace function hris_private.cutoff_check_item(i public.payroll_cutoff_items,r public.payroll_cutoff_runs)returns void language plpgsql stable security invoker set search_path=''as $$
declare fresh jsonb;d jsonb;cap integer;ot integer;begin
 fresh:=hris_private.cutoff_basis(i.company_id,i.employee_id,r.month,r.part);
 if fresh is distinct from i.source_basis then raise exception 'Cutoff source facts changed. Refresh source and review again.'using errcode='PT409';end if;
 if coalesce((i.worksheet->>'reviewed')::boolean,false)is not true or btrim(coalesce(i.worksheet->>'review_note',''))=''then raise exception 'Review every cutoff worksheet before submitting.'using errcode='23514';end if;
 if r.part=2 and fresh->>'first_cutoff'is null and coalesce((fresh#>>'{employee,joined_on}')::date,r.month)<=r.month+14 then raise exception 'Issue this employee''s first cutoff before reconciling the second.'using errcode='23514';end if;
 if(fresh#>>'{employee,joined_on}')::date>r.starts_on and i.profile#>>'{pay,basis}'='monthly'and i.worksheet->>'basic_override_cents'is null then raise exception 'A mid-cutoff joiner needs reviewed basic proration.'using errcode='23514';end if;
 if exists(select 1 from jsonb_array_elements(fresh->'leave_holidays')h where not exists(select 1 from jsonb_array_elements(fresh->'holidays')p where p->>'date'=h->>'date'))then raise exception 'Classify this cutoff''s leave-calendar holidays in Payroll holidays.'using errcode='23514';end if;
 for d in select value from jsonb_array_elements(i.worksheet->'days')loop
  if(d->>'date')::date<(fresh#>>'{employee,joined_on}')::date then raise exception 'Worksheet dates cannot precede employment.'using errcode='23514';end if;
  if exists(select 1 from jsonb_array_elements(fresh->'holidays')h where h->>'date'=d->>'date'and h->>'kind'<>d->>'kind')then raise exception 'A worksheet day differs from the payroll holiday classification.'using errcode='23514';end if;
 end loop;
 select coalesce(sum((wd.value->>'overtime_minutes')::integer),0)into ot from jsonb_array_elements(i.worksheet->'days')as wd(value);
 cap:=hris_private.cutoff_ot_limit(fresh,r.part);
 if ot>cap and btrim(coalesce(i.worksheet->>'exception_note',''))=''then raise exception 'Review overtime exceeding current authorizations after issued first-cutoff usage.'using errcode='23514';end if;
 if(i.totals->>'net')::bigint<0 then raise exception 'Correct deductions or advances before approving negative net pay.'using errcode='23514';end if;
 if i.calculation is distinct from hris_private.cutoff_compute(i.profile->'pay',i.worksheet,r.month,r.part,fresh#>'{first_cutoff,calculation}')or i.totals is distinct from i.calculation->'totals'then raise exception 'Cutoff calculation does not reconcile.'using errcode='23514';end if;
 if exists(select 1 from public.payroll_slips where company_id=i.company_id and employee_id=i.employee_id and month=r.month)then raise exception 'This employee already has an issued monthly register. Do not issue overlapping cutoff pay.'using errcode='23514';end if;
end;$$;

revoke all on function hris_private.cutoff_ot_limit(jsonb,integer)from public,anon,authenticated;
commit;
