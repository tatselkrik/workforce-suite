-- Applicants are private HR records, separate from employees and Auth accounts.
begin;
create table public.applicants (
 id uuid primary key check(id<>'00000000-0000-0000-0000-000000000000'),
 company_id uuid not null references public.companies(id),
 full_name text not null check(char_length(full_name) between 1 and 120 and full_name=btrim(full_name) and full_name !~ '[[:cntrl:]]'),
 email text not null default '' check(octet_length(email)<=254 and email !~ '[[:cntrl:][:space:]]'),
 phone text not null default '' check(char_length(phone)<=40 and phone !~ '[[:cntrl:]]'),
 position text not null check(char_length(position) between 1 and 100 and position=btrim(position) and position !~ '[[:cntrl:]]'),
 source text not null default '' check(char_length(source)<=80 and source !~ '[[:cntrl:]]'),
 applied_on date not null,
 notes text not null default '' check(char_length(notes)<=2000 and replace(notes,E'\n','') !~ '[[:cntrl:]]'),
 stage text not null default 'applied' check(stage in ('applied','screening','interview','offer','rejected','withdrawn')),
 archived boolean not null default false,
 revision bigint not null default 0 check(revision>=0),
 created_by uuid not null,created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id)
);
create index applicants_company_date_idx on public.applicants(company_id,applied_on desc,id);
create table public.applicant_events (
 id uuid primary key default gen_random_uuid(),
 company_id uuid not null references public.companies(id),
 applicant_id uuid not null,
 revision bigint not null check(revision>=0),
 kind text not null check(kind in ('created','saved','stage_changed','archived','restored')),
 reason text not null check(char_length(btrim(reason)) between 1 and 500 and reason !~ '[[:cntrl:]]'),
 actor_id uuid not null,actor_name text not null,actor_role text not null check(actor_role in ('administrator','hr_generalist')),
 before_record jsonb,after_record jsonb not null,
 recorded_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,applicant_id) references public.applicants(company_id,id),
 unique(applicant_id,revision)
);
create index applicant_events_company_record_idx on public.applicant_events(company_id,applicant_id,revision);
alter table public.applicants enable row level security;
alter table public.applicant_events enable row level security;
revoke all on public.applicants,public.applicant_events from public,anon,authenticated,service_role;
grant select on public.applicants,public.applicant_events to authenticated,service_role;
create policy applicants_hr_read on public.applicants for select to authenticated using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));
create policy applicant_events_hr_read on public.applicant_events for select to authenticated using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));
create function hris_private.applicant_history_immutable() returns trigger
language plpgsql set search_path='' as $$begin raise exception 'Application history is retained and cannot be edited or deleted.' using errcode='42501';end;$$;
revoke all on function hris_private.applicant_history_immutable() from public,anon,authenticated;
create trigger applicant_events_immutable before update or delete on public.applicant_events for each row execute function hris_private.applicant_history_immutable();

create function hris_private.manage_applicant(p_company_id uuid,p_operation text,p_record jsonb) returns jsonb
language plpgsql security definer set search_path='' as $$
declare subject uuid:=(select auth.uid());member public.company_members;old public.applicants;saved public.applicants;prior public.applicant_events;
 candidate uuid;reason text;next_stage text;kind text;fields jsonb;key text;actor text;today date;expected bigint;
begin
 if hris_private.company_role(p_company_id) is null or hris_private.company_role(p_company_id) not in ('administrator','hr_generalist') then
  raise exception 'Only active HR staff and Administrators can manage applicants.' using errcode='42501';end if;
 perform 1 from public.companies where id=p_company_id for update;
 select * into member from public.company_members where company_id=p_company_id and user_id=subject and active and role in ('administrator','hr_generalist');
 if not found then raise exception 'Your applicant access changed. Sign in again.' using errcode='42501';end if;
 if p_operation is null or p_operation not in ('create','edit','stage','archive','restore') or jsonb_typeof(p_record) is distinct from 'object' or octet_length(p_record::text)>16384 then
  raise exception 'The application operation is invalid.' using errcode='23514';end if;
 for key in select jsonb_object_keys(p_record) loop
  if not(key=any(case when p_operation in ('create','edit') then array['id','expected_revision','full_name','email','phone','position','source','applied_on','notes','reason']
     when p_operation='stage' then array['id','expected_revision','stage','reason'] else array['id','expected_revision','reason'] end)) then
   raise exception 'The application includes a protected or unknown field.' using errcode='23514';end if;
 end loop;
 if jsonb_typeof(p_record->'id') is distinct from 'string' or coalesce(p_record->>'id','') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' then
  raise exception 'The applicant identifier is invalid.' using errcode='23514';end if;
 candidate:=(p_record->>'id')::uuid;
 if candidate='00000000-0000-0000-0000-000000000000' then raise exception 'The applicant identifier is invalid.' using errcode='23514';end if;
 if jsonb_typeof(p_record->'reason') is distinct from 'string' then raise exception 'Enter a change reason.' using errcode='23514';end if;
 reason:=btrim(p_record->>'reason');
 if char_length(reason) not between 1 and 500 or reason ~ '[[:cntrl:]]' then raise exception 'Enter a change reason of 1-500 characters.' using errcode='23514';end if;
 select * into old from public.applicants where id=candidate and company_id=p_company_id for update;
 if p_operation in ('create','edit') then
  for key in select unnest(array['full_name','email','phone','position','source','applied_on','notes']) loop
   if jsonb_typeof(p_record->key) is distinct from 'string' then raise exception 'Application fields must be text.' using errcode='23514';end if;
  end loop;
  fields:=jsonb_build_object('id',candidate,'full_name',btrim(p_record->>'full_name'),'email',btrim(p_record->>'email'),'phone',btrim(p_record->>'phone'),
   'position',btrim(p_record->>'position'),'source',btrim(p_record->>'source'),'applied_on',btrim(p_record->>'applied_on'),'notes',btrim(p_record->>'notes'));
  if (fields->>'applied_on') !~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}$' then raise exception 'Enter an ISO application date.' using errcode='23514';end if;
  select (clock_timestamp() at time zone timezone)::date into today from public.companies where id=p_company_id;
  if (fields->>'applied_on')::date>today then raise exception 'Application date cannot be in the future.' using errcode='23514';end if;
  if fields->>'email'<>'' and (length(fields->>'email')-length(replace(fields->>'email','@',''))<>1 or split_part(fields->>'email','@',1)='' or position('.' in split_part(fields->>'email','@',2))=0
    or left(split_part(fields->>'email','@',2),1)='.' or right(split_part(fields->>'email','@',2),1)='.') then raise exception 'Enter a valid email address.' using errcode='23514';end if;
 end if;
 if p_operation='create' then
  if p_record ? 'expected_revision' then raise exception 'Create does not accept a prior revision.' using errcode='23514';end if;
  if old.id is not null then
   select ev.* into prior from public.applicant_events ev where ev.applicant_id=candidate and ev.company_id=p_company_id and ev.revision=0 and ev.kind='created';
   if prior.actor_id=subject and prior.reason=reason and (select jsonb_object_agg(k,prior.after_record->k) from jsonb_object_keys(fields) k)=fields then return to_jsonb(old);end if;
   raise exception 'This application identifier already exists. Refresh before retrying.' using errcode='23505';
  end if;
  insert into public.applicants(id,company_id,full_name,email,phone,position,source,applied_on,notes,created_by)
   values(candidate,p_company_id,fields->>'full_name',fields->>'email',fields->>'phone',fields->>'position',fields->>'source',(fields->>'applied_on')::date,fields->>'notes',subject) returning * into saved;
  kind:='created';
 else
  if old.id is null then raise exception 'Application not found in this company.' using errcode='42501';end if;
  if jsonb_typeof(p_record->'expected_revision') is distinct from 'number' or coalesce(p_record->>'expected_revision','') !~ '^[0-9]{1,19}$' then raise exception 'The prior application revision is invalid.' using errcode='23514';end if;
  expected:=(p_record->>'expected_revision')::bigint;
  if expected<>old.revision then raise exception 'The application changed. Refresh and reopen it before saving.' using errcode='PT409';end if;
  if old.archived and p_operation<>'restore' then raise exception 'Restore this application before editing it.' using errcode='23514';end if;
  if p_operation='edit' then
   update public.applicants set full_name=fields->>'full_name',email=fields->>'email',phone=fields->>'phone',position=fields->>'position',source=fields->>'source',applied_on=(fields->>'applied_on')::date,notes=fields->>'notes',revision=revision+1,updated_at=clock_timestamp()
    where id=candidate and company_id=p_company_id returning * into saved;kind:='saved';
  elsif p_operation='stage' then
   next_stage:=p_record->>'stage';
   if jsonb_typeof(p_record->'stage') is distinct from 'string' or next_stage is null or next_stage=old.stage or not(
    (old.stage in ('rejected','withdrawn') and next_stage='applied') or
    (old.stage not in ('rejected','withdrawn') and (next_stage in ('rejected','withdrawn') or (old.stage='applied' and next_stage in ('screening','interview')) or (old.stage='screening' and next_stage in ('interview','offer')) or (old.stage='interview' and next_stage='offer')))) then
    raise exception 'This application stage change is not available.' using errcode='23514';end if;
   update public.applicants set stage=next_stage,revision=revision+1,updated_at=clock_timestamp() where id=candidate and company_id=p_company_id returning * into saved;kind:='stage_changed';
  else
   if old.archived=(p_operation='archive') then raise exception 'The application is already in that archive state.' using errcode='23514';end if;
   update public.applicants set archived=(p_operation='archive'),revision=revision+1,updated_at=clock_timestamp() where id=candidate and company_id=p_company_id returning * into saved;
   kind:=case when p_operation='archive' then 'archived' else 'restored' end;
  end if;
 end if;
 select full_name into actor from public.employees where id=member.employee_id and company_id=p_company_id;
 insert into public.applicant_events(company_id,applicant_id,revision,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)
  values(p_company_id,candidate,saved.revision,kind,reason,subject,coalesce(nullif(actor,''),'HR staff'),member.role,case when old.id is null then null else to_jsonb(old) end,to_jsonb(saved));
 return to_jsonb(saved);
end;$$;
revoke all on function hris_private.manage_applicant(uuid,text,jsonb) from public,anon;
grant execute on function hris_private.manage_applicant(uuid,text,jsonb) to authenticated;
create function public.manage_applicant(p_company_id uuid,p_operation text,p_record jsonb) returns jsonb
language sql security invoker set search_path='' as $$select hris_private.manage_applicant(p_company_id,p_operation,p_record);$$;
revoke all on function public.manage_applicant(uuid,text,jsonb) from public,anon;
grant execute on function public.manage_applicant(uuid,text,jsonb) to authenticated;
commit;
