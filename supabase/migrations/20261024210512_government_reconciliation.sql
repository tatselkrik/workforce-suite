-- Reviewed contribution coverage and recorded government remittances.
-- Never changes an issued payroll slip, employee deduction or actual bank balance.
begin;
create table public.government_allocation_reviews(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid not null,
 source_kind text not null check(source_kind in('thirteenth','amendment','final_contribution')),
 source_id uuid not null,agency text not null check(agency in('sss','philhealth','pagibig')),
 status text not null default'draft'check(status in('draft','submitted','approved','cancelled')),
 revision bigint not null default 0 check(revision>=0),previous_id uuid,
 source_basis jsonb not null,allocations jsonb not null,reference text not null,note text not null,
 created_by uuid not null references auth.users(id),prepared_by uuid not null references auth.users(id),
 contributors uuid[]not null,submitted_by uuid references auth.users(id),approved_by uuid references auth.users(id),approved_at timestamptz,
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),foreign key(company_id,employee_id)references public.employees(company_id,id),
 foreign key(company_id,previous_id)references public.government_allocation_reviews(company_id,id),
 check(octet_length(source_basis::text)<=8192 and octet_length(allocations::text)<=16384),
 check(cardinality(contributors)between 1 and 128 and array_position(contributors,null)is null and created_by=any(contributors)and prepared_by=any(contributors)),
 check((status in('submitted','approved')and submitted_by is not null)or(status in('draft','cancelled')and submitted_by is null)),
 check((status='approved'and approved_by is not null and approved_at is not null and not(approved_by=any(contributors)))or(status<>'approved'and approved_by is null and approved_at is null))
);
create index government_allocation_source_idx on public.government_allocation_reviews(company_id,source_kind,source_id,agency,status);
create unique index government_allocation_pending_idx on public.government_allocation_reviews(company_id,source_kind,source_id,agency)where status in('draft','submitted');
create unique index government_allocation_successor_idx on public.government_allocation_reviews(company_id,previous_id)where status='approved';
create unique index government_allocation_root_idx on public.government_allocation_reviews(company_id,source_kind,source_id,agency)where status='approved'and previous_id is null;
create index government_allocation_employee_idx on public.government_allocation_reviews(company_id,employee_id);
create index government_allocation_previous_idx on public.government_allocation_reviews(company_id,previous_id);
create index government_allocation_creator_idx on public.government_allocation_reviews(created_by);
create index government_allocation_preparer_idx on public.government_allocation_reviews(prepared_by);
create index government_allocation_submitter_idx on public.government_allocation_reviews(submitted_by);
create index government_allocation_approver_idx on public.government_allocation_reviews(approved_by);

create table public.government_remittances(
 id uuid primary key,company_id uuid not null references public.companies(id),
 agency text not null check(agency in('bir','sss','philhealth','pagibig')),
 paid_on date not null,amount_cents bigint not null check(amount_cents between 1 and 100000000000000),
 allocations jsonb not null check(octet_length(allocations::text)<=16384),
 reference text not null,reference_key text generated always as(lower(btrim(reference)))stored,
 evidence text not null,note text not null,replaces_id uuid,
 recorded_by uuid not null references auth.users(id),recorded_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),foreign key(company_id,replaces_id)references public.government_remittances(company_id,id)
);
create unique index government_remittance_root_reference_idx on public.government_remittances(company_id,agency,reference_key)where replaces_id is null;
create unique index government_remittance_successor_idx on public.government_remittances(company_id,replaces_id)where replaces_id is not null;
create index government_remittance_reference_idx on public.government_remittances(company_id,agency,reference_key);
create index government_remittance_recorder_idx on public.government_remittances(recorded_by);
create table public.government_remittance_voids(
 id uuid primary key,company_id uuid not null,remittance_id uuid not null,
 reason text not null,recorded_by uuid not null references auth.users(id),recorded_at timestamptz not null default clock_timestamp(),
 unique(company_id,remittance_id),foreign key(company_id,remittance_id)references public.government_remittances(company_id,id)
);
create index government_remittance_void_recorder_idx on public.government_remittance_voids(recorded_by);
create table public.government_events(
 id uuid primary key default gen_random_uuid(),company_id uuid not null references public.companies(id),record_id uuid not null,
 kind text not null,actor_id uuid not null references auth.users(id),actor_name text not null,
 before_record jsonb,after_record jsonb not null,recorded_at timestamptz not null default clock_timestamp()
);
create index government_events_record_idx on public.government_events(company_id,record_id,recorded_at);
create index government_events_actor_idx on public.government_events(actor_id);
alter table public.government_allocation_reviews enable row level security;
alter table public.government_remittances enable row level security;
alter table public.government_remittance_voids enable row level security;
alter table public.government_events enable row level security;
revoke all on public.government_allocation_reviews,public.government_remittances,public.government_remittance_voids,public.government_events from public,anon,authenticated;
grant select on public.government_allocation_reviews,public.government_remittances,public.government_remittance_voids,public.government_events to authenticated;
create policy government_allocation_hr on public.government_allocation_reviews for select to authenticated using(hris_private.payroll_hr(company_id));
create policy government_remittance_hr on public.government_remittances for select to authenticated using(hris_private.payroll_hr(company_id));
create policy government_remittance_void_hr on public.government_remittance_voids for select to authenticated using(hris_private.payroll_hr(company_id));
create policy government_events_hr on public.government_events for select to authenticated using(hris_private.payroll_hr(company_id));
create trigger government_remittances_immutable before update or delete on public.government_remittances for each row execute function hris_private.employment_history_immutable();
create trigger government_remittance_voids_immutable before update or delete on public.government_remittance_voids for each row execute function hris_private.employment_history_immutable();
create trigger government_events_immutable before update or delete on public.government_events for each row execute function hris_private.employment_history_immutable();

create function hris_private.government_allocation_guard()returns trigger language plpgsql security invoker set search_path=''as $$
begin
 if tg_op='DELETE'or old.status in('approved','cancelled')then raise exception 'Terminal government reviews are immutable; prepare a linked replacement.'using errcode='42501';end if;
 if(new.id,new.company_id,new.employee_id,new.source_kind,new.source_id,new.agency,new.previous_id,new.source_basis,new.created_by,new.created_at)
  is distinct from(old.id,old.company_id,old.employee_id,old.source_kind,old.source_id,old.agency,old.previous_id,old.source_basis,old.created_by,old.created_at)
 then raise exception 'Pinned government source/creator cannot change.'using errcode='42501';end if;
 return new;
end;$$;
revoke all on function hris_private.government_allocation_guard()from public,anon,authenticated;
create trigger government_allocation_guard before update or delete on public.government_allocation_reviews for each row execute function hris_private.government_allocation_guard();

create function hris_private.government_money(j jsonb,key text,signed boolean default false)returns bigint
language plpgsql immutable security invoker set search_path=''as $$
declare n numeric;
begin
 if jsonb_typeof(j->key)is distinct from'number'then raise exception 'Enter exact centavo amounts.'using errcode='23514';end if;
 n:=(j->>key)::numeric;
 if n<>trunc(n)or abs(n)>100000000000000 or(not signed and n<0)then raise exception 'Government centavos exceed the supported range.'using errcode='23514';end if;
 return n::bigint;
end;$$;
revoke all on function hris_private.government_money(jsonb,text,boolean)from public,anon,authenticated;

create function hris_private.government_allocation_source(cid uuid,k text,sid uuid,a text)returns jsonb
language plpgsql stable security invoker set search_path=''as $$
declare j jsonb;begin
 if a not in('sss','philhealth','pagibig')or k not in('thirteenth','amendment','final_contribution')then return null;end if;
 if k='thirteenth'then
  select jsonb_build_object('company_id',s.company_id,'employee_id',s.employee_id,'id',s.id,'kind',k,'agency',a,
   'starts_on',make_date(s.year,1,1),'ends_on',s.as_of,'contribution_month',null,
   'parts',jsonb_build_object('employee_cents',s.item->'totals'->(a||'_adjustment_cents'),'employer_cents',null,'ec_cents',case when a='sss'then null::jsonb else'0'::jsonb end))into j
  from public.payroll_thirteenth_slips s join public.payroll_thirteenth_runs r on r.company_id=s.company_id and r.id=s.run_id
  where s.company_id=cid and s.id=sid and r.status in('approved','paid');
 elsif k='amendment'then
  select jsonb_build_object('company_id',s.company_id,'employee_id',s.employee_id,'id',s.id,'kind',k,'agency',a,
   'starts_on',s.starts_on,'ends_on',s.ends_on,'contribution_month',case when s.target_kind in('monthly','cutoff')then date_trunc('month',s.starts_on)::date end,
   'parts',jsonb_build_object('employee_cents',r.worksheet->(a||'_cents'),'employer_cents',null,'ec_cents',case when a='sss'then null::jsonb else'0'::jsonb end))into j
  from public.payroll_amendment_slips s join public.payroll_amendment_cases r on r.company_id=s.company_id and r.id=s.id
  where s.company_id=cid and s.id=sid and s.purpose='payout'and r.status in('approved','paid');
 else
  select jsonb_build_object('company_id',s.company_id,'employee_id',s.employee_id,'id',s.id,'kind',k,'agency',a,
   'starts_on',s.employment_start,'ends_on',s.service_end,'contribution_month',null,
   'parts',jsonb_build_object('employee_cents',r.worksheet->(a||'_adjustment_cents'),'employer_cents',null,'ec_cents',case when a='sss'then null::jsonb else'0'::jsonb end))into j
  from public.payroll_final_slips s join public.payroll_final_cases r on r.company_id=s.company_id and r.id=s.id
  where s.company_id=cid and s.id=sid and r.status in('approved','settled');
 end if;return j;
end;$$;
revoke all on function hris_private.government_allocation_source(uuid,text,uuid,text)from public,anon,authenticated;

create function hris_private.government_validate_allocation(src jsonb,plan jsonb,today date)returns void
language plpgsql immutable security invoker set search_path=''as $$
declare x jsonb;mon date;seen date[]:='{}';ee bigint:=0;e bigint;er bigint;ec bigint;expected bigint;
begin
 if src is null or jsonb_typeof(plan)is distinct from'array'or jsonb_array_length(plan)not between 1 and 24 then raise exception 'Use 1–24 explicit contribution months for an available source.'using errcode='23514';end if;
 expected:=hris_private.government_money(src->'parts','employee_cents',true);
 for x in select value from jsonb_array_elements(plan)loop
  perform hris_private.final_pay_object(x,array['month','employee_cents','employer_cents','ec_cents']);
  if jsonb_typeof(x->'month')is distinct from'string'then raise exception 'Choose each contribution month.'using errcode='23514';end if;
  mon:=(x->>'month')::date;
  if mon<date'2025-01-01'or mon>today or extract(day from mon)<>1 or mon=any(seen)
   or mon<greatest(date'2025-01-01',date_trunc('month',(src->>'starts_on')::date)::date)
   or mon>date_trunc('month',(src->>'ends_on')::date)::date
   or(src->>'contribution_month'is not null and mon<>(src->>'contribution_month')::date)
  then raise exception 'Contribution months must be unique, supported and within the original service/known coverage.'using errcode='23514';end if;
  e:=hris_private.government_money(x,'employee_cents',true);er:=hris_private.government_money(x,'employer_cents',true);ec:=hris_private.government_money(x,'ec_cents',true);
  if(src->>'agency'<>'sss'and ec<>0)or(expected>=0 and e<0)or(expected<=0 and e>0)then raise exception 'Do not invent EC or redistribute charges as offsetting employee refunds.'using errcode='23514';end if;
  ee:=ee+e;seen:=array_append(seen,mon);
 end loop;
 if ee<>expected then raise exception 'Allocated employee centavos must exactly retain the issued deduction/adjustment.'using errcode='23514';end if;
end;$$;
revoke all on function hris_private.government_validate_allocation(jsonb,jsonb,date)from public,anon,authenticated;

create function hris_private.government_actor()returns public.company_members
language plpgsql volatile security invoker set search_path=''as $$
declare m public.company_members;begin
 select *into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist');
 if m.user_id is null then raise exception 'Current HR/Admin access required.'using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 perform 1 from public.companies where id=m.company_id for update;
 select *into m from public.company_members where user_id=(select auth.uid())and active and employee_id is not null and role in('administrator','hr_generalist')for update;
 if m.user_id is null then raise exception 'Your government-review access changed.'using errcode='42501';end if;return m;
end;$$;
revoke all on function hris_private.government_actor()from public,anon,authenticated;

create function hris_private.government_effective_facts(base jsonb,reviews jsonb,mon date)returns jsonb
language plpgsql immutable security invoker set search_path=''as $$
declare f jsonb;r jsonb;x jsonb;result jsonb:='[]';begin
 for f in select value from jsonb_array_elements(base)loop
  r:=null;
  if f->>'state'not in('withdrawn','review_only')then
   select q into r from jsonb_array_elements(reviews)q where q->>'status'='approved'
    and q->>'source_kind'=f->>'kind'and q->>'source_id'=f->>'id'and q->>'agency'=f->>'agency'
    and not exists(select 1 from jsonb_array_elements(reviews)n where n->>'status'='approved'and n->>'previous_id'=q->>'id');
  end if;
  if r is not null and r->'source_basis'=f-array['state','received_on']then
   select a into x from jsonb_array_elements(r->'allocations')a where(a->>'month')::date=mon;
   if x is null then continue;end if;
   f:=f||jsonb_build_object('contribution_month',mon,'parts',x-'month');
  end if;
  result:=result||jsonb_build_array(f);
 end loop;return result;
end;$$;
revoke all on function hris_private.government_effective_facts(jsonb,jsonb,date)from public,anon,authenticated;

create function hris_private.government_allocation_floor(src jsonb,plan jsonb)returns void
language plpgsql stable security invoker set search_path=''as $$
declare x jsonb;f jsonb;facts jsonb;reviews jsonb;mon date;ee bigint;er bigint;ec bigint;cid uuid:=(src->>'company_id')::uuid;begin
 select coalesce(jsonb_agg(to_jsonb(q)),'[]')into reviews from public.government_allocation_reviews q
  where q.company_id=cid and q.agency=src->>'agency'and q.status='approved';
 for x in select value from jsonb_array_elements(plan)loop
  mon:=(x->>'month')::date;ee:=(x->>'employee_cents')::bigint;er:=(x->>'employer_cents')::bigint;ec:=(x->>'ec_cents')::bigint;
  facts:=hris_private.government_payroll_sources(mon,src->>'agency')->'facts';
  facts:=hris_private.government_effective_facts(facts,reviews,mon);
  for f in select value from jsonb_array_elements(facts)loop
   if f->>'employee_id'<>src->>'employee_id'or(f->>'kind'=src->>'kind'and f->>'id'=src->>'id')
    or f->>'state'in('withdrawn','review_only')or(f->>'contribution_month')::date is distinct from mon then continue;end if;
   ee:=ee+coalesce((f#>>'{parts,employee_cents}')::bigint,0);
   er:=er+coalesce((f#>>'{parts,employer_cents}')::bigint,0);
   ec:=ec+coalesce((f#>>'{parts,ec_cents}')::bigint,0);
  end loop;
  if ee<0 or er<0 or ec<0 then raise exception 'A negative correction needs a known recorded charge for this person, agency and original month; no automatic government credit is created.'using errcode='23514';end if;
 end loop;
end;$$;
revoke all on function hris_private.government_allocation_floor(jsonb,jsonb)from public,anon,authenticated;

create function hris_private.manage_government_allocation(p_operation text,p_record jsonb)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;r public.government_allocation_reviews;before_row jsonb;result jsonb;
 birth hris_private.payroll_birth;rid uuid;expected bigint;src jsonb;prev uuid;current_prev uuid;why text;allowed text[];today date;label text;
begin
 m:=hris_private.government_actor();
 select(now()at time zone c.timezone)::date into today from public.companies c where c.id=m.company_id;
 select e.full_name into label from public.employees e where e.company_id=m.company_id and e.id=m.employee_id;
 allowed:=case p_operation
  when'create'then array['id','expected_revision','source_kind','source_id','agency','previous_id','source_basis','allocations','reference','note','reason']
  when'save'then array['id','expected_revision','allocations','reference','note','reason']
  when'submit'then array['id','expected_revision','reason','reviewed']
  when'approve'then array['id','expected_revision','reason','reviewed']
  when'return'then array['id','expected_revision','reason']when'cancel'then array['id','expected_revision','reason']else null end;
 if allowed is null or octet_length(p_record::text)>65536 then raise exception 'Use a supported bounded government review operation.'using errcode='23514';end if;
 perform hris_private.final_pay_object(p_record,allowed);
 rid:=(p_record->>'id')::uuid;why:=hris_private.payroll_text(p_record,'reason',2000,true);
 if rid is null or rid='00000000-0000-0000-0000-000000000000'then raise exception 'Review identity required.'using errcode='23514';end if;
 if p_operation='create'then
  select *into birth from hris_private.payroll_birth where company_id=m.company_id and operation='government_allocation_create'and record_id=rid;
  if birth.record_id is not null then
   if birth.actor_id<>m.user_id or birth.payload<>p_record then raise exception 'This review identity was already used.'using errcode='PT409';end if;
   select to_jsonb(q)into result from public.government_allocation_reviews q where q.company_id=m.company_id and q.id=rid;return result;
  end if;
  if p_record->'expected_revision'is distinct from'null'::jsonb then raise exception 'A new review has no revision.'using errcode='23514';end if;
  src:=hris_private.government_allocation_source(m.company_id,p_record->>'source_kind',(p_record->>'source_id')::uuid,p_record->>'agency');
  if src is null or p_record->'source_basis'is distinct from src then raise exception 'The source is unavailable or changed. Refresh before preparing its coverage.'using errcode='PT409';end if;
  select q.id into current_prev from public.government_allocation_reviews q where q.company_id=m.company_id and q.source_kind=src->>'kind'and q.source_id=(src->>'id')::uuid and q.agency=src->>'agency'and q.status='approved'
   and not exists(select 1 from public.government_allocation_reviews n where n.company_id=q.company_id and n.previous_id=q.id and n.status='approved');
  prev:=(p_record->>'previous_id')::uuid;
  if prev is distinct from current_prev then raise exception 'The approved coverage version changed. Refresh.'using errcode='PT409';end if;
  perform hris_private.government_validate_allocation(src,p_record->'allocations',today);
  r.id:=rid;r.company_id:=m.company_id;r.employee_id:=(src->>'employee_id')::uuid;r.source_kind:=src->>'kind';r.source_id:=(src->>'id')::uuid;r.agency:=src->>'agency';r.previous_id:=prev;r.source_basis:=src;r.status:='draft';r.revision:=0;
  r.allocations:=p_record->'allocations';r.reference:=hris_private.payroll_text(p_record,'reference',1000,true);r.note:=hris_private.payroll_text(p_record,'note',2000,true);
  r.created_by:=m.user_id;r.prepared_by:=m.user_id;r.contributors:=array[m.user_id];r.created_at:=clock_timestamp();r.updated_at:=r.created_at;
  insert into public.government_allocation_reviews select(r).*;
  insert into hris_private.payroll_birth(company_id,operation,record_id,actor_id,payload)values(m.company_id,'government_allocation_create',rid,m.user_id,p_record);
 else
  if jsonb_typeof(p_record->'expected_revision')is distinct from'number'then raise exception 'Current revision required.'using errcode='23514';end if;
  expected:=hris_private.government_money(p_record,'expected_revision');
  select *into r from public.government_allocation_reviews where company_id=m.company_id and id=rid for update;
  if r.id is null then raise exception 'Review outside your company.'using errcode='42501';end if;
  if r.revision is distinct from expected then raise exception 'This review changed. Refresh; unsaved entries remain yours.'using errcode='PT409';end if;
  before_row:=to_jsonb(r);
  if r.status in('approved','cancelled')then raise exception 'Prepare a linked replacement for a terminal review.'using errcode='23514';end if;
  if p_operation in('save','submit','approve')then
   src:=hris_private.government_allocation_source(r.company_id,r.source_kind,r.source_id,r.agency);
   if src is null or src<>r.source_basis then raise exception 'The issued source changed or was withdrawn. Cancel and refresh this review.'using errcode='PT409';end if;
  end if;
  if p_operation='save'then
   if r.status<>'draft'then raise exception 'Return a submitted review before editing.'using errcode='23514';end if;
   perform hris_private.government_validate_allocation(src,p_record->'allocations',today);
   r.allocations:=p_record->'allocations';r.reference:=hris_private.payroll_text(p_record,'reference',1000,true);r.note:=hris_private.payroll_text(p_record,'note',2000,true);r.prepared_by:=m.user_id;
   select array_agg(distinct x)into r.contributors from unnest(r.contributors||m.user_id)x;
  elsif p_operation='submit'then
   if r.status<>'draft'or p_record->'reviewed'is distinct from'true'::jsonb then raise exception 'Review the coverage and explicit employer/EC shares before submitting.'using errcode='23514';end if;
   perform hris_private.government_validate_allocation(src,r.allocations,today);
   r.status:='submitted';r.submitted_by:=m.user_id;
   select array_agg(distinct x)into r.contributors from unnest(r.contributors||m.user_id)x;
  elsif p_operation='approve'then
   if m.role<>'administrator'or m.user_id=any(r.contributors)or m.employee_id=r.employee_id then raise exception 'An independent Administrator who did not prepare/submit this review and is not its employee must approve it.'using errcode='42501';end if;
   if r.status<>'submitted'or p_record->'reviewed'is distinct from'true'::jsonb then raise exception 'Review the submitted contribution evidence before approval.'using errcode='23514';end if;
   select q.id into current_prev from public.government_allocation_reviews q where q.company_id=r.company_id and q.source_kind=r.source_kind and q.source_id=r.source_id and q.agency=r.agency and q.status='approved'
    and not exists(select 1 from public.government_allocation_reviews n where n.company_id=q.company_id and n.previous_id=q.id and n.status='approved');
   if r.previous_id is distinct from current_prev then raise exception 'An approved version changed. Cancel and prepare a fresh replacement.'using errcode='PT409';end if;
   perform hris_private.government_validate_allocation(src,r.allocations,today);perform hris_private.government_allocation_floor(src,r.allocations);
   r.status:='approved';r.approved_by:=m.user_id;r.approved_at:=clock_timestamp();
  elsif p_operation='return'then
   if r.status<>'submitted'then raise exception 'Only a submitted review can be returned.'using errcode='23514';end if;
   r.status:='draft';r.submitted_by:=null;
  else r.status:='cancelled';r.submitted_by:=null;
  end if;
  r.revision:=r.revision+1;r.updated_at:=clock_timestamp();
  update public.government_allocation_reviews set status=r.status,revision=r.revision,allocations=r.allocations,reference=r.reference,note=r.note,
   prepared_by=r.prepared_by,contributors=r.contributors,submitted_by=r.submitted_by,approved_by=r.approved_by,approved_at=r.approved_at,updated_at=r.updated_at where company_id=m.company_id and id=rid;
 end if;
 result:=to_jsonb(r);
 insert into public.government_events(company_id,record_id,kind,actor_id,actor_name,before_record,after_record)
 values(m.company_id,rid,'allocation_'||p_operation,m.user_id,label,before_row,result||jsonb_build_object('action_reason',why));
 return result;
end;$$;
revoke all on function hris_private.manage_government_allocation(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_government_allocation(text,jsonb)to authenticated;
create function public.manage_government_allocation(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_government_allocation(p_operation,p_record);$$;
revoke all on function public.manage_government_allocation(text,jsonb)from public,anon,authenticated;
grant execute on function public.manage_government_allocation(text,jsonb)to authenticated;

create function hris_private.government_validate_remittance(plan jsonb,amount bigint,today date)returns void
language plpgsql immutable security invoker set search_path=''as $$
declare x jsonb;mon date;seen date[]:='{}';total bigint:=0;n bigint;begin
 if jsonb_typeof(plan)is distinct from'array'or jsonb_array_length(plan)not between 1 and 24 or amount<=0 then raise exception 'Allocate a positive recorded remittance across1–24months.'using errcode='23514';end if;
 for x in select value from jsonb_array_elements(plan)loop
  perform hris_private.final_pay_object(x,array['month','amount_cents']);
  if jsonb_typeof(x->'month')is distinct from'string'then raise exception 'Choose covered months.'using errcode='23514';end if;
  mon:=(x->>'month')::date;n:=hris_private.government_money(x,'amount_cents');
  if mon<date'2025-01-01'or mon>today or extract(day from mon)<>1 or mon=any(seen)or n<=0 then raise exception 'Use unique past/current contribution months and positive centavos.'using errcode='23514';end if;
  seen:=array_append(seen,mon);total:=total+n;
 end loop;
 if total<>amount then raise exception 'Month allocations must exactly equal the evidenced receipt total.'using errcode='23514';end if;
end;$$;
revoke all on function hris_private.government_validate_remittance(jsonb,bigint,date)from public,anon,authenticated;

create function hris_private.manage_government_remittance(p_operation text,p_record jsonb)returns jsonb
language plpgsql security definer set search_path=''as $$
declare m public.company_members;r public.government_remittances;v public.government_remittance_voids;
 old_row public.government_remittances;birth hris_private.payroll_birth;rid uuid;today date;why text;label text;result jsonb;
begin
 m:=hris_private.government_actor();
 if m.role<>'administrator'then raise exception 'Only Administrators record or void evidenced government remittances.'using errcode='42501';end if;
 select(now()at time zone c.timezone)::date into today from public.companies c where c.id=m.company_id;
 select full_name into label from public.employees where company_id=m.company_id and id=m.employee_id;
 if octet_length(p_record::text)>65536 or p_operation not in('record','void')then raise exception 'Use a bounded record/void operation.'using errcode='23514';end if;
 perform hris_private.final_pay_object(p_record,case when p_operation='record'then array['id','agency','paid_on','amount_cents','allocations','reference','evidence','note','replaces_id','received_confirmed','reason']else array['id','remittance_id','reason']end);
 rid:=(p_record->>'id')::uuid;why:=hris_private.payroll_text(p_record,'reason',2000,true);
 if rid is null or rid='00000000-0000-0000-0000-000000000000'then raise exception 'Government operation identity required.'using errcode='23514';end if;
 select *into birth from hris_private.payroll_birth where company_id=m.company_id and operation='government_remittance_'||p_operation and record_id=rid;
 if birth.record_id is not null then
  if birth.actor_id<>m.user_id or birth.payload<>p_record then raise exception 'This receipt/void identity was already used.'using errcode='PT409';end if;
  if p_operation='record'then select to_jsonb(q)-'reference_key'into result from public.government_remittances q where q.company_id=m.company_id and q.id=rid;
  else select to_jsonb(q)into result from public.government_remittance_voids q where q.company_id=m.company_id and q.id=rid;end if;return result;
 end if;
 if p_operation='record'then
  if p_record->'received_confirmed'is distinct from'true'::jsonb then raise exception 'Confirm an actual evidenced remittance; this action does not pay an agency.'using errcode='23514';end if;
  r.id:=rid;r.company_id:=m.company_id;r.agency:=hris_private.payroll_text(p_record,'agency',20,true);
  if r.agency not in('bir','sss','philhealth','pagibig')then raise exception 'Choose a supported agency.'using errcode='23514';end if;
  r.paid_on:=hris_private.thirteenth_date(p_record,'paid_on');r.amount_cents:=hris_private.government_money(p_record,'amount_cents');
  if r.paid_on<date'2025-01-01'or r.paid_on>today then raise exception 'Record the actual past/current payment date from2025 onward.'using errcode='23514';end if;
  r.allocations:=p_record->'allocations';perform hris_private.government_validate_remittance(r.allocations,r.amount_cents,today);
  r.reference:=btrim(hris_private.payroll_text(p_record,'reference',1000,true));
  if r.reference~'[[:cntrl:]]'then raise exception 'Use a single-line receipt/reference.'using errcode='23514';end if;
  r.evidence:=hris_private.payroll_text(p_record,'evidence',2000,true);r.note:=hris_private.payroll_text(p_record,'note',2000,false);
  r.replaces_id:=(p_record->>'replaces_id')::uuid;r.recorded_by:=m.user_id;r.recorded_at:=clock_timestamp();
  if exists(select 1 from public.government_remittances q where q.company_id=m.company_id and q.agency=r.agency and q.reference_key=lower(r.reference)
   and not exists(select 1 from public.government_remittance_voids z where z.company_id=q.company_id and z.remittance_id=q.id))then raise exception 'An active record already uses this agency/reference. Do not record the same payment twice.'using errcode='PT409';end if;
  if r.replaces_id is not null then
   select *into old_row from public.government_remittances q where q.company_id=m.company_id and q.id=r.replaces_id;
   if old_row.id is null or old_row.agency<>r.agency
    or not exists(select 1 from public.government_remittance_voids z where z.company_id=old_row.company_id and z.remittance_id=old_row.id)
    or exists(select 1 from public.government_remittances n where n.company_id=old_row.company_id and n.replaces_id=old_row.id)
   then raise exception 'Replace the last voided record of this same agency, preserving the original history.'using errcode='PT409';end if;
   if exists(with recursive ancestors as(select q.id,q.replaces_id from public.government_remittances q where q.company_id=m.company_id and q.id=old_row.id
      union all select q.id,q.replaces_id from public.government_remittances q join ancestors x on x.replaces_id=q.id where q.company_id=m.company_id)
     select 1 from public.government_remittances q where q.company_id=m.company_id and q.agency=r.agency and q.reference_key=lower(r.reference)
      and not exists(select 1 from ancestors x where x.id=q.id))
   then raise exception 'The corrected reference belongs to another payment history; review its own voided record.'using errcode='PT409';end if;
  elsif exists(select 1 from public.government_remittances q where q.company_id=m.company_id and q.agency=r.agency and q.reference_key=lower(r.reference))then
   raise exception 'Open the voided receipt and prepare its linked replacement.'using errcode='PT409';
  end if;
  insert into public.government_remittances(id,company_id,agency,paid_on,amount_cents,allocations,reference,evidence,note,replaces_id,recorded_by,recorded_at)
   values(r.id,r.company_id,r.agency,r.paid_on,r.amount_cents,r.allocations,r.reference,r.evidence,r.note,r.replaces_id,r.recorded_by,r.recorded_at);
  result:=to_jsonb(r)-'reference_key';
 else
  select *into r from public.government_remittances where company_id=m.company_id and id=(p_record->>'remittance_id')::uuid;
  if r.id is null then raise exception 'Remittance outside your company.'using errcode='42501';end if;
  v.id:=rid;v.company_id:=m.company_id;v.remittance_id:=r.id;v.reason:=why;v.recorded_by:=m.user_id;v.recorded_at:=clock_timestamp();
  if exists(select 1 from public.government_remittance_voids q where q.company_id=m.company_id and q.remittance_id=r.id)then raise exception 'This receipt is already voided. Refresh its history.'using errcode='PT409';end if;
  insert into public.government_remittance_voids select(v).*;result:=to_jsonb(v);
 end if;
 insert into hris_private.payroll_birth(company_id,operation,record_id,actor_id,payload)values(m.company_id,'government_remittance_'||p_operation,rid,m.user_id,p_record);
 insert into public.government_events(company_id,record_id,kind,actor_id,actor_name,before_record,after_record)
 values(m.company_id,case when p_operation='void'then r.id else rid end,'remittance_'||p_operation,m.user_id,label,case when p_operation='void'then to_jsonb(r)-'reference_key'else null end,result||jsonb_build_object('action_reason',why));
 return result;
end;$$;
revoke all on function hris_private.manage_government_remittance(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_government_remittance(text,jsonb)to authenticated;
create function public.manage_government_remittance(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_government_remittance(p_operation,p_record);$$;
revoke all on function public.manage_government_remittance(text,jsonb)from public,anon,authenticated;
grant execute on function public.manage_government_remittance(text,jsonb)to authenticated;

create function hris_private.government_reconciliation(p_month date,p_agency text)returns jsonb
language plpgsql stable security definer set search_path=''as $$
declare b jsonb;reviews jsonb;receipts jsonb;history jsonb;labels jsonb;n integer;cid uuid;begin
 b:=hris_private.government_payroll_sources(p_month,p_agency);cid:=(b->>'company_id')::uuid;
 select coalesce(jsonb_agg(to_jsonb(q)order by q.created_at,q.id),'[]'),count(*)into reviews,n
 from(select r.*from public.government_allocation_reviews r where r.company_id=cid and r.agency=p_agency
  and exists(select 1 from jsonb_array_elements(b->'facts')f where f->>'kind'=r.source_kind and f->>'id'=r.source_id::text)limit 10001)q;
 if n>10000 then raise exception 'Too many review versions; no financial totals were truncated.'using errcode='23514';end if;
 select coalesce(jsonb_agg(q.j order by q.j->>'paid_on',q.j->>'id'),'[]'),count(*)into receipts,n
 from(select(to_jsonb(r)-'reference_key')||jsonb_build_object('void_id',v.id,'void_reason',v.reason,'voided_by',v.recorded_by,'voided_at',v.recorded_at,
   'replacement_id',(select x.id from public.government_remittances x where x.company_id=r.company_id and x.replaces_id=r.id))j
  from public.government_remittances r left join public.government_remittance_voids v on v.company_id=r.company_id and v.remittance_id=r.id
  where r.company_id=cid and r.agency=p_agency and exists(select 1 from jsonb_array_elements(r.allocations)x where(x->>'month')::date=p_month)limit 5001)q;
 if n>5000 then raise exception 'Too many recorded remittances for this month; no totals were truncated.'using errcode='23514';end if;
 select coalesce(jsonb_agg(q.j order by q.j->>'recorded_at',q.j->>'id'),'[]'),count(*)into history,n
 from(select jsonb_build_object('id',e.id,'company_id',e.company_id,'record_id',e.record_id,'kind',e.kind,'actor_name',e.actor_name,'reason',e.after_record->>'action_reason','recorded_at',e.recorded_at)j
  from public.government_events e where e.company_id=cid and(
   exists(select 1 from jsonb_array_elements(reviews)r where r->>'id'=e.record_id::text)
   or exists(select 1 from jsonb_array_elements(receipts)r where r->>'id'=e.record_id::text))limit 20001)q;
 if n>20000 then raise exception 'Too much government audit history for this scope; nothing was truncated.'using errcode='23514';end if;
 select coalesce(jsonb_agg(jsonb_build_object('employee_id',e.id,'employee_code',e.employee_code,'employee_name',e.full_name)order by e.employee_code),'[]')into labels
 from public.employees e where e.company_id=cid and exists(select 1 from jsonb_array_elements(b->'facts')f where f->>'employee_id'=e.id::text);
 return jsonb_build_object('version','government-reconciliation-v1','source',b,'effective_facts',hris_private.government_effective_facts(b->'facts',reviews,p_month),
  'reviews',reviews,'remittances',receipts,'history',history,'employees',labels);
end;$$;
revoke all on function hris_private.government_reconciliation(date,text)from public,anon,authenticated;
grant execute on function hris_private.government_reconciliation(date,text)to authenticated;
create function public.government_reconciliation(p_month date,p_agency text)returns jsonb language sql stable security invoker set search_path=''as $$select hris_private.government_reconciliation(p_month,p_agency);$$;
revoke all on function public.government_reconciliation(date,text)from public,anon,authenticated;
grant execute on function public.government_reconciliation(date,text)to authenticated;
commit;
