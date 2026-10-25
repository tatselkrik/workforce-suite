-- Private deterministic arithmetic only. No ledger, public RPC, grants or active
-- balance behavior is enabled by this foundation. Protected posting follows.
begin;
create function hris_private.opening_booking_totals(p_rows jsonb)returns jsonb
language plpgsql immutable security invoker set search_path=''as $$
declare r jsonb;total bigint:=0;early bigint:=0;used bigint:=0;ids uuid[]:='{}';rid uuid;units integer;before_expiry integer;
begin
 if jsonb_typeof(p_rows)is distinct from'array'or octet_length(p_rows::text)>300000 then raise exception 'Invalid opening bookings.'using errcode='23514';end if;
 if jsonb_array_length(p_rows)>732 then raise exception 'Too many opening bookings.'using errcode='23514';end if;
 for r in select value from jsonb_array_elements(p_rows)loop
  if jsonb_typeof(r)is distinct from'object'then raise exception 'Invalid booking object.'using errcode='23514';end if;
  if(select array_agg(k order by k)from jsonb_object_keys(r)k)is distinct from array['approved','early_units','request_id','units']::text[]
   or jsonb_typeof(r->'approved')is distinct from'boolean'or jsonb_typeof(r->'request_id')is distinct from'string'
   or jsonb_typeof(r->'units')is distinct from'number'or coalesce(r->>'units','')!~'^[0-9]{1,3}$'
   or jsonb_typeof(r->'early_units')is distinct from'number'or coalesce(r->>'early_units','')!~'^[0-9]{1,3}$'then raise exception 'Invalid saved booking fields.'using errcode='23514';end if;
  rid:=(r->>'request_id')::uuid;units:=(r->>'units')::integer;before_expiry:=(r->>'early_units')::integer;
  if rid='00000000-0000-0000-0000-000000000000'or rid=any(ids)or units not between 1 and 732 or before_expiry not between 0 and units then raise exception 'Invalid or duplicate saved booking.'using errcode='23514';end if;
  ids:=array_append(ids,rid);total:=total+units;early:=early+before_expiry;if(r->>'approved')::boolean then used:=used+units;end if;
 end loop;
 if total>732 then raise exception 'Year bookings exceed a calendar year.'using errcode='23514';end if;
 return jsonb_build_object('total_units',total,'early_units',early,'approved_units',used);
exception when invalid_text_representation or numeric_value_out_of_range then raise exception 'Invalid opening booking values.'using errcode='23514';
end;$$;

create function hris_private.opening_project(p_opening jsonb,p_asof date,p_base_units integer,p_current jsonb)returns jsonb
language plpgsql immutable security invoker set search_path=''as $$
declare baseline jsonb;current_totals jsonb;field text;on_date date;expires date;old_base integer;target integer;remaining integer;old_consumed integer;initial_total integer;total integer;early integer;used integer;baseline_early integer;baseline_carry integer;consumed integer;gross integer;unspent integer;virtual_base bigint;unused_base bigint;available bigint;
begin
 if jsonb_typeof(p_opening)is distinct from'object'or octet_length(p_opening::text)>350000 then raise exception 'Invalid opening object.'using errcode='23514';end if;
 if(select array_agg(k order by k)from jsonb_object_keys(p_opening)k)is distinct from array['base_earned_at_opening','baseline','carry_expires_on','effective_on','employee_id','id','previously_consumed_carry_units','remaining_carry_units','target_available_units','type_code']::text[]then raise exception 'Invalid opening basis.'using errcode='23514';end if;
 foreach field in array array['base_earned_at_opening','target_available_units','remaining_carry_units','previously_consumed_carry_units']loop
  if jsonb_typeof(p_opening->field)is distinct from'number'or coalesce(p_opening->>field,'')!~'^[0-9]{1,10}$'then raise exception 'Opening quantities must be whole half-day units.'using errcode='23514';end if;
 end loop;
 foreach field in array array['id','employee_id']loop
  if jsonb_typeof(p_opening->field)is distinct from'string'or(p_opening->>field)::uuid='00000000-0000-0000-0000-000000000000'then raise exception 'Invalid opening identity.'using errcode='23514';end if;
 end loop;
 if jsonb_typeof(p_opening->'type_code')is distinct from'string'or coalesce(p_opening->>'type_code','')!~'^[a-z0-9_]{1,30}$'then raise exception 'Invalid opening type.'using errcode='23514';end if;
 foreach field in array array['effective_on','carry_expires_on']loop
  if jsonb_typeof(p_opening->field)is distinct from'string'or coalesce(p_opening->>field,'')!~'^[0-9]{4}-[0-9]{2}-[0-9]{2}$'then raise exception 'Invalid opening date.'using errcode='23514';end if;
 end loop;
 on_date:=(p_opening->>'effective_on')::date;expires:=(p_opening->>'carry_expires_on')::date;
 old_base:=(p_opening->>'base_earned_at_opening')::integer;target:=(p_opening->>'target_available_units')::integer;remaining:=(p_opening->>'remaining_carry_units')::integer;old_consumed:=(p_opening->>'previously_consumed_carry_units')::integer;
 baseline:=hris_private.opening_booking_totals(p_opening->'baseline');current_totals:=hris_private.opening_booking_totals(p_current);
 initial_total:=(baseline->>'total_units')::integer;total:=(current_totals->>'total_units')::integer;early:=(current_totals->>'early_units')::integer;used:=(current_totals->>'approved_units')::integer;
 if p_asof is null or p_base_units is null or p_base_units not between -4000000 and 4000000 or extract(year from on_date)not between 1900 and 2200
  or extract(year from expires)<>extract(year from on_date)or extract(day from expires+1)<>1 or p_asof<on_date or extract(year from p_asof)<>extract(year from on_date)
  or old_base not between 0 and 4000000 or target not between 0 and 732 or remaining not between 0 and target or old_consumed not between 0 and(baseline->>'early_units')::integer
  or old_consumed+remaining>732 or(on_date>expires and remaining<>0)then raise exception 'Unsupported opening dates or quantities.'using errcode='23514';end if;
 if exists(select 1 from jsonb_array_elements(p_current)c join jsonb_array_elements(p_opening->'baseline')b on(c->>'request_id')::uuid=(b->>'request_id')::uuid where c->'units'is distinct from b->'units'or c->'early_units'is distinct from b->'early_units')then raise exception 'A baseline request changed its saved charges.'using errcode='23514';end if;
 select coalesce(sum((c->>'early_units')::integer),0)::integer into baseline_early from jsonb_array_elements(p_current)c join jsonb_array_elements(p_opening->'baseline')b on(c->>'request_id')::uuid=(b->>'request_id')::uuid;
 gross:=old_consumed+remaining;baseline_carry:=least(old_consumed,baseline_early);consumed:=baseline_carry+least(early-baseline_early,gross-baseline_carry);
 virtual_base:=(target-remaining)::bigint+p_base_units::bigint-old_base::bigint+initial_total-old_consumed;unused_base:=virtual_base-total+consumed;
 unspent:=case when p_asof<=expires then gross-consumed else 0 end;available:=unused_base+unspent;
 return jsonb_build_object('balance',jsonb_build_object('earned_units',virtual_base,'carryover_units',case when p_asof<=expires then gross else 0 end,'used_units',used,'reserved_units',total-used,'available_units',available),'unused_base_units',unused_base,'unspent_carry_units',unspent,'consumed_carry_units',consumed);
exception when invalid_text_representation or invalid_datetime_format or datetime_field_overflow or numeric_value_out_of_range then raise exception 'Invalid opening arithmetic input.'using errcode='23514';
end;$$;
revoke all on function hris_private.opening_booking_totals(jsonb),hris_private.opening_project(jsonb,date,integer,jsonb)from public,anon,authenticated;
commit;
