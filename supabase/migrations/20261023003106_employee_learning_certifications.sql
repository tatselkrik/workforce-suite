-- Employee learning records and certification metadata. File evidence stays HR-only.
begin;
create table hris_private.learning_birth(
 company_id uuid not null references public.companies(id),kind text not null check(kind in('training','certificate','evidence')),
 record_id uuid not null,actor_id uuid not null references auth.users(id),payload jsonb not null,
 primary key(company_id,kind,record_id)
);
create index learning_birth_actor_idx on hris_private.learning_birth(actor_id);
alter table hris_private.learning_birth enable row level security;
revoke all on hris_private.learning_birth from public,anon,authenticated;
create table public.employee_training(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid not null,
 employee_name text not null,employee_code text not null,
 title text not null check(title=btrim(title) and char_length(title) between 1 and 120 and title !~ '[[:cntrl:]]'),
 provider text not null default '' check(char_length(provider)<=120 and provider !~ '[[:cntrl:]]'),
 starts_on date check(starts_on between date '1900-01-01' and date '2200-12-31'),
 ends_on date check(ends_on between date '1900-01-01' and date '2200-12-31'),
 due_on date check(due_on between date '1900-01-01' and date '2200-12-31'),required boolean not null default false,
 status text not null default 'planned' check(status in('planned','in_progress','completed','cancelled')),
 progress_note text not null default '' check(char_length(progress_note)<=1000),
 completed_on date check(completed_on between date '1900-01-01' and date '2200-12-31'),
 learning_minutes integer check(learning_minutes between 0 and 600000),
 outcome text not null default '' check(char_length(outcome)<=2000),revision bigint not null default 0 check(revision>=0),
 created_by uuid not null references auth.users(id),created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),foreign key(company_id,employee_id) references public.employees(company_id,id),
 check(ends_on is null or starts_on is null or ends_on>=starts_on),
 check(due_on is null or starts_on is null or due_on>=starts_on),
 check(completed_on is null or starts_on is null or completed_on>=starts_on),
 check((status='completed' and completed_on is not null and btrim(outcome)<>'')or(status<>'completed' and completed_on is null and outcome=''))
);
create index employee_training_employee_idx on public.employee_training(company_id,employee_id,status);
create index employee_training_creator_idx on public.employee_training(created_by);
create table public.employee_certifications(
 id uuid primary key,company_id uuid not null references public.companies(id),employee_id uuid not null,employee_name text not null,employee_code text not null,
 title text not null check(title=btrim(title) and char_length(title) between 1 and 120 and title !~ '[[:cntrl:]]'),
 issuer text not null default '' check(char_length(issuer)<=120 and issuer !~ '[[:cntrl:]]'),
 reference_number text not null default '' check(char_length(reference_number)<=120 and reference_number !~ '[[:cntrl:]]'),
 issued_on date not null check(issued_on between date '1900-01-01' and date '2200-12-31'),
 expires_on date check(expires_on between date '1900-01-01' and date '2200-12-31' and expires_on>=issued_on),
 status text not null default 'valid' check(status in('valid','revoked')),archived boolean not null default false,
 revision bigint not null default 0 check(revision>=0),created_by uuid not null references auth.users(id),
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),unique(company_id,employee_id,id),foreign key(company_id,employee_id) references public.employees(company_id,id)
);
create index employee_certifications_employee_idx on public.employee_certifications(company_id,employee_id,expires_on);
create index employee_certifications_creator_idx on public.employee_certifications(created_by);
create table public.certification_evidence(
 id uuid primary key,company_id uuid not null,employee_id uuid not null,certificate_id uuid not null,document_id uuid,
 revision bigint not null default 0 check(revision>=0),created_by uuid not null references auth.users(id),
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 unique(company_id,id),unique(company_id,certificate_id),
 foreign key(company_id,employee_id,certificate_id) references public.employee_certifications(company_id,employee_id,id),
 foreign key(company_id,employee_id,document_id) references public.employee_documents(company_id,employee_id,id)
);
create index certification_evidence_person_idx on public.certification_evidence(company_id,employee_id,certificate_id);
create index certification_evidence_creator_idx on public.certification_evidence(created_by);
create index certification_evidence_document_idx on public.certification_evidence(company_id,employee_id,document_id);
create table public.learning_events(
 id uuid primary key default gen_random_uuid(),company_id uuid not null references public.companies(id),training_id uuid,certificate_id uuid,evidence_id uuid,
 kind text not null,reason text not null check(char_length(reason) between 1 and 500),actor_id uuid not null references auth.users(id),actor_name text not null,actor_role text not null,
 before_record jsonb,after_record jsonb not null,recorded_at timestamptz not null default clock_timestamp(),
 check(training_id is not null or certificate_id is not null),check(evidence_id is null or certificate_id is not null),
 foreign key(company_id,training_id) references public.employee_training(company_id,id),
 foreign key(company_id,certificate_id) references public.employee_certifications(company_id,id),
 foreign key(company_id,evidence_id) references public.certification_evidence(company_id,id)
);
create index learning_events_training_idx on public.learning_events(company_id,training_id,recorded_at);
create index learning_events_certificate_idx on public.learning_events(company_id,certificate_id,recorded_at);
create index learning_events_evidence_idx on public.learning_events(company_id,evidence_id);
create index learning_events_actor_idx on public.learning_events(actor_id);
create trigger learning_events_immutable before update or delete on public.learning_events for each row execute function hris_private.employment_history_immutable();
create function hris_private.learning_scope(p_company uuid,p_employee uuid) returns boolean language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=p_employee
 where m.company_id=p_company and m.user_id=(select auth.uid()) and m.active and(m.role in('administrator','hr_generalist')or m.employee_id=e.id or(m.role='manager'and e.reports_to=m.employee_id)));
$$;
revoke all on function hris_private.learning_scope(uuid,uuid) from public,anon,authenticated;
grant execute on function hris_private.learning_scope(uuid,uuid) to authenticated;
alter table public.employee_training enable row level security;
alter table public.employee_certifications enable row level security;
alter table public.certification_evidence enable row level security;
alter table public.learning_events enable row level security;
revoke all on public.employee_training,public.employee_certifications,public.certification_evidence,public.learning_events from public,anon,authenticated;
grant select on public.employee_training,public.employee_certifications,public.certification_evidence,public.learning_events to authenticated;
create policy employee_training_read on public.employee_training for select to authenticated using(hris_private.learning_scope(company_id,employee_id));
create policy employee_certifications_read on public.employee_certifications for select to authenticated using(hris_private.learning_scope(company_id,employee_id));
create policy certification_evidence_read on public.certification_evidence for select to authenticated using(hris_private.company_role(company_id)in('administrator','hr_generalist'));
create policy learning_events_read on public.learning_events for select to authenticated using(
 hris_private.company_role(company_id)in('administrator','hr_generalist')or(evidence_id is null and(
 exists(select 1 from public.employee_training t where t.company_id=learning_events.company_id and t.id=learning_events.training_id and hris_private.learning_scope(t.company_id,t.employee_id))or
 exists(select 1 from public.employee_certifications c where c.company_id=learning_events.company_id and c.id=learning_events.certificate_id and hris_private.learning_scope(c.company_id,c.employee_id)))));
create function hris_private.learning_retry(p_company uuid,p_kind text,p_id uuid,p_actor uuid,p_record jsonb)returns boolean language plpgsql security definer set search_path='' as $$
declare b hris_private.learning_birth;begin
 select * into b from hris_private.learning_birth where company_id=p_company and kind=p_kind and record_id=p_id;
 if b.record_id is null then return false;end if;
 if b.actor_id=p_actor and b.payload=p_record then return true;end if;
 raise exception 'This record ID exists. Refresh and reopen it.' using errcode='PT409';
end;$$;
revoke all on function hris_private.learning_retry(uuid,text,uuid,uuid,jsonb)from public,anon,authenticated;
create function hris_private.manage_learning(p_operation text,p_record jsonb)returns jsonb language plpgsql security definer set search_path='' as $$
declare m public.company_members;person public.employees;t public.employee_training;c public.employee_certifications;e public.certification_evidence;d public.employee_documents;
 v_id uuid;expected bigint;why text;actor_name text;today date;old_record jsonb;result jsonb;kind text;next_status text;create_kind text;
 event_training uuid;event_certificate uuid;event_evidence uuid;doc_id uuid;
begin
 select * into m from public.company_members where user_id=(select auth.uid())and active and role in('administrator','hr_generalist','manager');
 if m.user_id is null then raise exception 'Active desktop staff access is required.' using errcode='42501';end if;
 perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(m.company_id::text,7));
 perform 1 from public.companies where id=m.company_id for update;
 select * into m from public.company_members where user_id=(select auth.uid())and active and role in('administrator','hr_generalist','manager')for update;
 if m.user_id is null then raise exception 'Your access changed. Refresh.' using errcode='42501';end if;
 select full_name into actor_name from public.employees where company_id=m.company_id and id=m.employee_id;
 select(now()at time zone timezone)::date into today from public.companies where id=m.company_id;
 if jsonb_typeof(p_record)is distinct from 'object'or octet_length(p_record::text)>32768 then raise exception 'Enter valid bounded learning details.'using errcode='23514';end if;
 v_id:=(p_record->>'id')::uuid;expected:=(p_record->>'expected_revision')::bigint;why:=btrim(p_record->>'reason');
 if v_id is null or why is null or char_length(why)not between 1 and 500 or why~'[[:cntrl:]]'then raise exception 'Choose a record and enter a reason of 1-500 characters.'using errcode='23514';end if;
 if p_operation='training_save'then
  if m.role not in('administrator','hr_generalist')then raise exception 'Only HR or Administrators manage training definitions.'using errcode='42501';end if;
  if p_record-array['id','expected_revision','employee_id','title','provider','starts_on','ends_on','due_on','required','reason']<>'{}'::jsonb then raise exception 'Unsupported training fields.'using errcode='23514';end if;
  select * into t from public.employee_training where company_id=m.company_id and id=v_id for update;
  if t.id is not null and expected is null then if hris_private.learning_retry(m.company_id,'training',v_id,m.user_id,p_record)then return to_jsonb(t);end if;raise exception 'Training ID exists.'using errcode='PT409';end if;
  select * into person from public.employees where company_id=m.company_id and id=(p_record->>'employee_id')::uuid;
  if person.id is null then raise exception 'Choose an employee in this company.'using errcode='42501';end if;
  if t.id is null then
   if expected is not null then raise exception 'Training not found. Refresh.'using errcode='PT409';end if;
   if person.status<>'active'then raise exception 'New training requires an active employee.'using errcode='23514';end if;
   insert into public.employee_training(id,company_id,employee_id,employee_name,employee_code,title,provider,starts_on,ends_on,due_on,required,created_by)
   values(v_id,m.company_id,person.id,person.full_name,person.employee_code,btrim(p_record->>'title'),btrim(coalesce(p_record->>'provider','')),nullif(p_record->>'starts_on','')::date,nullif(p_record->>'ends_on','')::date,nullif(p_record->>'due_on','')::date,coalesce((p_record->>'required')::boolean,false),m.user_id)returning * into t;create_kind:='training';kind:='training_created';
  else
   if t.revision is distinct from expected then raise exception 'Training changed. Refresh and reopen it.'using errcode='PT409';end if;
   if t.employee_id<>person.id then raise exception 'The employee cannot change on saved training.'using errcode='23514';end if;
   old_record:=to_jsonb(t);
   update public.employee_training set title=btrim(p_record->>'title'),provider=btrim(coalesce(p_record->>'provider','')),starts_on=nullif(p_record->>'starts_on','')::date,ends_on=nullif(p_record->>'ends_on','')::date,due_on=nullif(p_record->>'due_on','')::date,required=coalesce((p_record->>'required')::boolean,false),revision=revision+1,updated_at=clock_timestamp()where id=t.id returning * into t;kind:='training_edited';
  end if;
  result:=to_jsonb(t);event_training:=t.id;
 elsif p_operation in('training_progress','training_status')then
  if p_record-(case when p_operation='training_progress'then array['id','expected_revision','progress_note','reason']else array['id','expected_revision','status','completed_on','learning_minutes','outcome','reason']end)<>'{}'::jsonb then raise exception 'Unsupported training progress fields.'using errcode='23514';end if;
  select * into t from public.employee_training where company_id=m.company_id and id=v_id for update;
  if t.id is null or not hris_private.learning_scope(m.company_id,t.employee_id)then raise exception 'Training is outside your current access.'using errcode='42501';end if;
  if t.revision is distinct from expected then raise exception 'Training changed. Refresh and reopen it.'using errcode='PT409';end if;
  select * into person from public.employees where company_id=m.company_id and id=t.employee_id;old_record:=to_jsonb(t);
  if p_operation='training_progress'then
   if person.status<>'active'or t.status not in('planned','in_progress')then raise exception 'Only active planned/in-progress training accepts progress.'using errcode='23514';end if;
   update public.employee_training set progress_note=btrim(coalesce(p_record->>'progress_note','')),revision=revision+1,updated_at=clock_timestamp()where id=t.id returning * into t;kind:='training_progress';
  else
   if m.role not in('administrator','hr_generalist')then raise exception 'HR or Administrators record completion and training status.'using errcode='42501';end if;
   next_status:=p_record->>'status';
   if next_status is null or next_status not in('planned','in_progress','completed','cancelled')or(next_status=t.status and next_status<>'completed')then raise exception 'Choose a different training status.'using errcode='23514';end if;
   if person.status<>'active'and next_status in('planned','in_progress')then raise exception 'Archived employees cannot restart training.'using errcode='23514';end if;
   if next_status='completed'and(nullif(p_record->>'completed_on','')::date is null or nullif(p_record->>'completed_on','')::date>today or btrim(coalesce(p_record->>'outcome',''))='')then raise exception 'Completion needs a factual date on/before today and an outcome.'using errcode='23514';end if;
   if next_status<>'completed'and(nullif(p_record->>'completed_on','')is not null or coalesce(p_record->>'outcome','')<>'')then raise exception 'Completion details belong only to completed training.'using errcode='23514';end if;
   update public.employee_training set status=next_status,completed_on=case when next_status='completed'then(p_record->>'completed_on')::date end,learning_minutes=nullif(p_record->>'learning_minutes','')::integer,outcome=case when next_status='completed'then btrim(p_record->>'outcome')else ''end,revision=revision+1,updated_at=clock_timestamp()where id=t.id returning * into t;kind:='training_'||next_status;
  end if;
  result:=to_jsonb(t);event_training:=t.id;
 elsif p_operation='certificate_save'then
  if m.role not in('administrator','hr_generalist')then raise exception 'Only HR or Administrators maintain certifications.'using errcode='42501';end if;
  if p_record-array['id','expected_revision','employee_id','title','issuer','reference_number','issued_on','expires_on','reason']<>'{}'::jsonb then raise exception 'Unsupported certification fields.'using errcode='23514';end if;
  select * into c from public.employee_certifications where company_id=m.company_id and id=v_id for update;
  if c.id is not null and expected is null then if hris_private.learning_retry(m.company_id,'certificate',v_id,m.user_id,p_record)then return to_jsonb(c);end if;raise exception 'Certification ID exists.'using errcode='PT409';end if;
  select * into person from public.employees where company_id=m.company_id and id=(p_record->>'employee_id')::uuid;
  if person.id is null then raise exception 'Choose an employee in this company.'using errcode='42501';end if;
  if (p_record->>'issued_on')::date>today then raise exception 'Issue date cannot be in the future.'using errcode='23514';end if;
  if c.id is null then
   if expected is not null then raise exception 'Certification not found. Refresh.'using errcode='PT409';end if;
   if person.status<>'active'then raise exception 'New certifications require an active employee.'using errcode='23514';end if;
   insert into public.employee_certifications(id,company_id,employee_id,employee_name,employee_code,title,issuer,reference_number,issued_on,expires_on,created_by)
   values(v_id,m.company_id,person.id,person.full_name,person.employee_code,btrim(p_record->>'title'),btrim(coalesce(p_record->>'issuer','')),btrim(coalesce(p_record->>'reference_number','')),(p_record->>'issued_on')::date,nullif(p_record->>'expires_on','')::date,m.user_id)returning * into c;create_kind:='certificate';kind:='certificate_created';
  else
   if c.revision is distinct from expected then raise exception 'Certification changed. Refresh and reopen it.'using errcode='PT409';end if;
   if c.employee_id<>person.id or c.archived then raise exception 'Restore the certification before editing; employee cannot change.'using errcode='23514';end if;
   old_record:=to_jsonb(c);
   update public.employee_certifications set title=btrim(p_record->>'title'),issuer=btrim(coalesce(p_record->>'issuer','')),reference_number=btrim(coalesce(p_record->>'reference_number','')),issued_on=(p_record->>'issued_on')::date,expires_on=nullif(p_record->>'expires_on','')::date,revision=revision+1,updated_at=clock_timestamp()where id=c.id returning * into c;kind:='certificate_edited';
  end if;
  result:=to_jsonb(c);event_certificate:=c.id;
 elsif p_operation in('certificate_status','certificate_archive')then
  if m.role not in('administrator','hr_generalist')then raise exception 'Only HR or Administrators manage certification status.'using errcode='42501';end if;
  if p_record-(case when p_operation='certificate_status'then array['id','expected_revision','status','reason']else array['id','expected_revision','archived','reason']end)<>'{}'::jsonb then raise exception 'Unsupported certification status fields.'using errcode='23514';end if;
  select * into c from public.employee_certifications where company_id=m.company_id and id=v_id for update;
  if c.id is null then raise exception 'Certification not found.'using errcode='42501';end if;
  if c.revision is distinct from expected then raise exception 'Certification changed. Refresh.'using errcode='PT409';end if;
  old_record:=to_jsonb(c);
  if p_operation='certificate_status'then
   next_status:=p_record->>'status';
   if c.archived or next_status is null or next_status not in('valid','revoked')or next_status=c.status then raise exception 'Choose a different status on an unarchived certification.'using errcode='23514';end if;
   update public.employee_certifications set status=next_status,revision=revision+1,updated_at=clock_timestamp()where id=c.id returning * into c;kind:='certificate_'||next_status;
  else
   if (p_record->>'archived')::boolean is null or (p_record->>'archived')::boolean=c.archived then raise exception 'Choose archive or restore.'using errcode='23514';end if;
   update public.employee_certifications set archived=(p_record->>'archived')::boolean,revision=revision+1,updated_at=clock_timestamp()where id=c.id returning * into c;kind:=case when c.archived then 'certificate_archived'else 'certificate_restored'end;
  end if;
  result:=to_jsonb(c);event_certificate:=c.id;
 elsif p_operation='evidence_save'then
  if m.role not in('administrator','hr_generalist')then raise exception 'Certification files are private to HR and Administrators.'using errcode='42501';end if;
  if p_record-array['id','expected_revision','certificate_id','document_id','reason']<>'{}'::jsonb then raise exception 'Unsupported evidence fields.'using errcode='23514';end if;
  select * into c from public.employee_certifications where company_id=m.company_id and id=(p_record->>'certificate_id')::uuid for update;
  if c.id is null then raise exception 'Certification not found.'using errcode='42501';end if;
  select * into e from public.certification_evidence where company_id=m.company_id and id=v_id for update;
  if e.id is not null and expected is null then if hris_private.learning_retry(m.company_id,'evidence',v_id,m.user_id,p_record)then return to_jsonb(e);end if;raise exception 'Evidence ID exists.'using errcode='PT409';end if;
  if c.archived then raise exception 'Restore the certification before changing its evidence.'using errcode='23514';end if;
  doc_id:=(p_record->>'document_id')::uuid;
  if doc_id is not null then
   select * into d from public.employee_documents where company_id=m.company_id and employee_id=c.employee_id and id=doc_id for update;
   if d.id is null or d.state<>'ready'or d.archived then raise exception 'Choose a completed, unarchived 201 document for this employee.'using errcode='23514';end if;
  end if;
  if e.id is null then
   if expected is not null then raise exception 'Evidence not found. Refresh.'using errcode='PT409';end if;
   if exists(select 1 from public.certification_evidence where company_id=m.company_id and certificate_id=c.id)then raise exception 'Evidence exists. Refresh and reopen it.'using errcode='PT409';end if;
   insert into public.certification_evidence(id,company_id,employee_id,certificate_id,document_id,created_by)values(v_id,m.company_id,c.employee_id,c.id,doc_id,m.user_id)returning * into e;create_kind:='evidence';kind:='evidence_linked';
  else
   if e.revision is distinct from expected then raise exception 'Evidence changed. Refresh.'using errcode='PT409';end if;
   if e.certificate_id<>c.id then raise exception 'The certification cannot change on saved evidence.'using errcode='23514';end if;
   old_record:=to_jsonb(e);update public.certification_evidence set document_id=doc_id,revision=revision+1,updated_at=clock_timestamp()where id=e.id returning * into e;kind:=case when doc_id is null then 'evidence_unlinked'else 'evidence_replaced'end;
  end if;
  result:=to_jsonb(e);event_certificate:=c.id;event_evidence:=e.id;
 else raise exception 'Unknown learning operation.'using errcode='23514';end if;
 if create_kind is not null then insert into hris_private.learning_birth(company_id,kind,record_id,actor_id,payload)values(m.company_id,create_kind,v_id,m.user_id,p_record);end if;
 insert into public.learning_events(company_id,training_id,certificate_id,evidence_id,kind,reason,actor_id,actor_name,actor_role,before_record,after_record)
 values(m.company_id,event_training,event_certificate,event_evidence,kind,why,m.user_id,actor_name,m.role,old_record,result);
 return result;
end;$$;
revoke all on function hris_private.manage_learning(text,jsonb)from public,anon,authenticated;
grant execute on function hris_private.manage_learning(text,jsonb)to authenticated;
create function public.manage_learning(p_operation text,p_record jsonb)returns jsonb language sql security invoker set search_path=''as $$select hris_private.manage_learning(p_operation,p_record);$$;
revoke all on function public.manage_learning(text,jsonb)from public,anon,authenticated;
grant execute on function public.manage_learning(text,jsonb)to authenticated;
commit;
