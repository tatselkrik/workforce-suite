-- Create-only, all-or-nothing employee CSV imports. Raw source files are not retained.
begin;
create table public.employee_import_batches (
 id uuid primary key,company_id uuid not null references public.companies(id) on delete restrict,
 actor_id uuid not null references auth.users(id) on delete restrict,actor_name text not null,
 actor_role text not null check(actor_role in ('administrator','hr_generalist')),
 row_count integer not null check(row_count between 1 and 500),
 payload_sha256 text not null check(payload_sha256 ~ '^[0-9a-f]{64}$'),
 reason text not null check(char_length(btrim(reason)) between 1 and 500 and reason !~ '[[:cntrl:]]'),
 created_at timestamptz not null default clock_timestamp(),unique(company_id,id)
);
create index employee_import_batches_company_idx on public.employee_import_batches(company_id,created_at desc,id);
create index employee_import_batches_actor_idx on public.employee_import_batches(actor_id);
create table public.employee_import_items (
 id uuid primary key default gen_random_uuid(),company_id uuid not null,batch_id uuid not null,employee_id uuid not null,
 source_row integer not null check(source_row between 2 and 2097152),
 foreign key(company_id,batch_id) references public.employee_import_batches(company_id,id) on delete restrict,
 foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict,
 unique(company_id,batch_id,source_row),unique(company_id,batch_id,employee_id)
);
create index employee_import_items_employee_idx on public.employee_import_items(company_id,employee_id);
alter table public.employee_import_batches enable row level security;
alter table public.employee_import_items enable row level security;
revoke all on public.employee_import_batches,public.employee_import_items from public,anon,authenticated;
grant select on public.employee_import_batches,public.employee_import_items to authenticated;
create policy employee_import_batches_read on public.employee_import_batches for select to authenticated using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));
create policy employee_import_items_read on public.employee_import_items for select to authenticated using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));

create function hris_private.import_employees(p_batch_id uuid,p_rows jsonb,p_reason text) returns uuid
language plpgsql security definer set search_path='' as $$
declare m public.company_members;old public.employee_import_batches;r jsonb;e jsonb;contacts jsonb;refs jsonb;actual jsonb;
 manager public.employees;choice record;person uuid;source integer;note text:=btrim(p_reason);fingerprint text;label text;
begin
 select * into m from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist');
 if not found then raise exception 'Active HR or Administrator access required for employee imports.' using errcode='42501';end if;
 perform pg_advisory_xact_lock(hashtextextended(m.company_id::text,7));
 select * into m from public.company_members where user_id=(select auth.uid()) and active and role in ('administrator','hr_generalist');
 if not found then raise exception 'Your access changed.' using errcode='42501';end if;
 if p_batch_id is null or jsonb_typeof(p_rows) is distinct from 'array' or jsonb_array_length(p_rows) not between 1 and 500 or octet_length(p_rows::text)>2097152 then raise exception 'Import between 1 and 500 valid employee rows, within 2 MiB.' using errcode='23514';end if;
 if coalesce(char_length(note),0) not between 1 and 500 or note ~ '[[:cntrl:]]' then raise exception 'Enter an import reason of 1–500 characters without control characters.' using errcode='23514';end if;
 fingerprint:=encode(sha256(convert_to(jsonb_build_object('rows',p_rows,'reason',note)::text,'UTF8')),'hex');
 select * into old from public.employee_import_batches where company_id=m.company_id and id=p_batch_id;
 if found then
  if old.actor_id=m.user_id and old.payload_sha256=fingerprint then return old.id;end if;
  raise exception 'This import ID already belongs to different details. Reopen the source file.' using errcode='PT409';
 end if;
 select full_name into label from public.employees where company_id=m.company_id and id=m.employee_id;
 insert into public.employee_import_batches(id,company_id,actor_id,actor_name,actor_role,row_count,payload_sha256,reason)
 values(p_batch_id,m.company_id,m.user_id,coalesce(label,'HR staff'),m.role,jsonb_array_length(p_rows),fingerprint,note);
 for r in select value from jsonb_array_elements(p_rows) loop
  e:=r->'employee';contacts:=r->'contacts';refs:=r->'references';
  if jsonb_typeof(r) is distinct from 'object' or jsonb_typeof(e) is distinct from 'object' or jsonb_typeof(contacts) is distinct from 'object' or jsonb_typeof(refs) is distinct from 'object' or coalesce(r->>'source_row','') !~ '^[0-9]{1,7}$' then raise exception 'A preview row is invalid. Reopen the source file.' using errcode='23514';end if;
  source:=(r->>'source_row')::integer;
  if exists(select 1 from public.employees where company_id=m.company_id and upper(btrim(employee_code))=upper(btrim(e->>'employee_code'))) then raise exception 'An employee code already exists. Imports create new people only; refresh the preview.' using errcode='PT409';end if;
  if nullif(e->>'reports_to','') is not null then
   select * into manager from public.employees where company_id=m.company_id and id=(e->>'reports_to')::uuid and status='active' for share;
   if not found then raise exception 'A reporting manager changed. Refresh the preview.' using errcode='PT409';end if;
   actual:=jsonb_build_object('id',manager.id,'employee_code',manager.employee_code,'full_name',manager.full_name,'status',manager.status);
  else actual:='null'::jsonb;end if;
  if actual is distinct from coalesce(refs->'manager','null'::jsonb) then raise exception 'A reporting manager changed. Refresh the preview.' using errcode='PT409';end if;
  if nullif(e->>'employment_type_id','') is not null then
   select * into choice from public.employment_types where company_id=m.company_id and id=(e->>'employment_type_id')::uuid and active for share;
   if not found then raise exception 'An employment type changed. Refresh the preview.' using errcode='PT409';end if;
   actual:=jsonb_build_object('id',choice.id,'name',choice.name,'revision',choice.revision,'active',choice.active);
  else actual:='null'::jsonb;end if;
  if actual is distinct from coalesce(refs->'employment_type','null'::jsonb) then raise exception 'An employment type changed. Refresh the preview.' using errcode='PT409';end if;
  if nullif(e->>'work_location_id','') is not null then
   select * into choice from public.work_locations where company_id=m.company_id and id=(e->>'work_location_id')::uuid and active for share;
   if not found then raise exception 'A work location changed. Refresh the preview.' using errcode='PT409';end if;
   actual:=jsonb_build_object('id',choice.id,'name',choice.name,'revision',choice.revision,'active',choice.active);
  else actual:='null'::jsonb;end if;
  if actual is distinct from coalesce(refs->'work_location','null'::jsonb) then raise exception 'A work location changed. Refresh the preview.' using errcode='PT409';end if;
  -- The existing employee/contact triggers and history enforce field rules.
  -- Never accept a source employee ID, company, archived status or account role.
  person:=public.save_employee_record(e||jsonb_build_object('id',null,'company_id',m.company_id,'status','active'),contacts);
  insert into public.employee_import_items(company_id,batch_id,employee_id,source_row) values(m.company_id,p_batch_id,person,source);
 end loop;
 return p_batch_id;
exception when unique_violation then raise exception 'Import details or employee codes changed. No partial import was saved; refresh the preview.' using errcode='PT409';
end;$$;
create function public.import_employees(p_batch_id uuid,p_rows jsonb,p_reason text) returns uuid
language sql security invoker set search_path='' as $$select hris_private.import_employees(p_batch_id,p_rows,p_reason);$$;
revoke all on function public.import_employees(uuid,jsonb,text),hris_private.import_employees(uuid,jsonb,text) from public,anon,authenticated;
grant execute on function public.import_employees(uuid,jsonb,text),hris_private.import_employees(uuid,jsonb,text) to authenticated;
commit;
