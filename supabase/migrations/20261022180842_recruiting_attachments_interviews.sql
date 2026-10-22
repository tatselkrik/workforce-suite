begin;
-- Private applicant attachments. Storage bytes are immutable once filed; archive retains them.

create table public.applicant_documents (
    id uuid primary key,
    company_id uuid not null references public.companies(id) on delete restrict,
    applicant_id uuid not null,
    title text not null check(title=btrim(title) and char_length(title) between 1 and 120 and title !~ '[[:cntrl:]]'),
    category text not null check(category in ('contract','identification','certificate','hr_form','acknowledgment','other')),
    notes text not null default '' check(char_length(notes)<=1000),
    issued_on date check(issued_on between date '1900-01-01' and date '2200-12-31'),
    expires_on date check(expires_on between date '1900-01-01' and date '2200-12-31'),
    contract_start date check(contract_start between date '1900-01-01' and date '2200-12-31'),
    contract_end date check(contract_end between date '1900-01-01' and date '2200-12-31'),
    original_filename text not null check(char_length(original_filename) between 1 and 160
        and original_filename !~ '[[:cntrl:]:<>"|?*]' and position('/' in original_filename)=0
        and position(chr(92) in original_filename)=0 and original_filename !~ '[. ]$'
        and upper(split_part(original_filename,'.',1)) !~ '^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$'),
    mime_type text not null check(mime_type in ('application/pdf','image/png','image/jpeg')),
    byte_size bigint not null check(byte_size between 1 and 10485760),
    sha256 text not null check(sha256 ~ '^[0-9a-f]{64}$'),
    object_name text generated always as (company_id::text||'/'||applicant_id::text||'/'||id::text||'/file.'||
        case mime_type when 'application/pdf' then 'pdf' when 'image/png' then 'png' else 'jpg' end) stored unique,
    state text not null default 'pending' check(state in ('pending','ready','discarded')),
    archived boolean not null default false,
    revision bigint not null default 0 check(revision>=0),
    created_by uuid not null references auth.users(id) on delete restrict,
    created_by_name text not null,
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),
    unique(company_id,applicant_id,id),
    foreign key(company_id,applicant_id) references public.applicants(company_id,id) on delete restrict,
    check(expires_on is null or issued_on is null or expires_on>=issued_on),
    check(contract_end is null or (contract_start is not null and contract_end>=contract_start)),
    check(category='contract' or (contract_start is null and contract_end is null)),
    check(not archived or state='ready'),
    check((mime_type='application/pdf' and lower(original_filename) ~ '\.pdf$') or
          (mime_type='image/png' and lower(original_filename) ~ '\.png$') or
          (mime_type='image/jpeg' and lower(original_filename) ~ '\.jpe?g$'))
);
create index applicant_documents_person_idx on public.applicant_documents(company_id,applicant_id,created_at desc);
create index applicant_documents_creator_idx on public.applicant_documents(created_by);
alter table public.applicant_documents enable row level security;
revoke all on public.applicant_documents from public,anon,authenticated;
grant select on public.applicant_documents to authenticated;
create policy applicant_documents_read on public.applicant_documents for select to authenticated
    using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));

create table public.applicant_document_events (
    id uuid primary key default gen_random_uuid(),
    sequence bigint generated always as identity unique,
    company_id uuid not null,
    applicant_id uuid not null,
    document_id uuid not null,
    kind text not null check(kind in ('uploaded','details_updated','archived','restored')),
    actor_id uuid not null references auth.users(id) on delete restrict,
    actor_name text not null,
    actor_role text not null check(actor_role in ('administrator','hr_generalist')),
    before_record jsonb,
    after_record jsonb not null,
    recorded_at timestamptz not null default clock_timestamp(),
    foreign key(company_id,applicant_id,document_id) references public.applicant_documents(company_id,applicant_id,id) on delete restrict
);
create index applicant_document_events_person_idx on public.applicant_document_events(company_id,applicant_id,document_id,sequence desc);
create index applicant_document_events_actor_idx on public.applicant_document_events(actor_id);
alter table public.applicant_document_events enable row level security;
revoke all on public.applicant_document_events from public,anon,authenticated;
grant select on public.applicant_document_events to authenticated;
create policy applicant_document_events_read on public.applicant_document_events for select to authenticated
    using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));

create function hris_private.guard_applicant_document() returns trigger
language plpgsql security invoker set search_path='' as $$
begin
    if tg_op='DELETE' then raise exception 'Document records are retained. Archive filed documents instead.' using errcode='42501'; end if;
    if (new.id,new.company_id,new.applicant_id,new.original_filename,new.mime_type,new.byte_size,new.sha256,new.created_by,new.created_by_name,new.created_at)
        is distinct from (old.id,old.company_id,old.applicant_id,old.original_filename,old.mime_type,old.byte_size,old.sha256,old.created_by,old.created_by_name,old.created_at) then
        raise exception 'A document file and its original attribution cannot be replaced.' using errcode='42501';
    end if;
    if not ((old.state='pending' and new.state in ('ready','discarded')) or (old.state='ready' and new.state='ready')) then
        raise exception 'This upload state cannot be changed.' using errcode='42501';
    end if;
    if new.revision<>old.revision+1 then raise exception 'Invalid document revision.' using errcode='PT409'; end if;
    return new;
end;
$$;
revoke all on function hris_private.guard_applicant_document() from public,anon,authenticated;
create trigger applicant_documents_guard before update or delete on public.applicant_documents
    for each row execute function hris_private.guard_applicant_document();

create function hris_private.record_applicant_document_event() returns trigger
language plpgsql security definer set search_path='' as $$
declare actor uuid; actor_label text; actor_kind text; event_kind text;
begin
    if new.state<>'ready' then return new; end if;
    select m.user_id,e.full_name,m.role into actor,actor_label,actor_kind from public.company_members m
        join public.employees e on e.company_id=m.company_id and e.id=m.employee_id
        where m.company_id=new.company_id and m.user_id=(select auth.uid()) and m.active
          and m.role in ('administrator','hr_generalist');
    if actor is null then raise exception 'Active HR access is required.' using errcode='42501'; end if;
    event_kind:=case when old.state='pending' then 'uploaded' when old.archived is distinct from new.archived
        then case when new.archived then 'archived' else 'restored' end else 'details_updated' end;
    insert into public.applicant_document_events(company_id,applicant_id,document_id,kind,actor_id,actor_name,actor_role,before_record,after_record)
        values(new.company_id,new.applicant_id,new.id,event_kind,actor,actor_label,actor_kind,
            case when old.state='ready' then to_jsonb(old) else null end,to_jsonb(new));
    return new;
end;
$$;
revoke all on function hris_private.record_applicant_document_event() from public,anon,authenticated;
create trigger applicant_documents_record_event after update on public.applicant_documents
    for each row execute function hris_private.record_applicant_document_event();
create trigger applicant_document_events_immutable before update or delete on public.applicant_document_events
    for each row execute function hris_private.employment_history_immutable();

-- Only this narrow private writer can change metadata. The public wrapper remains an invoker.
create function hris_private.save_applicant_document(p_operation text,p_document jsonb) returns public.applicant_documents
language plpgsql security definer set search_path='' as $$
declare company uuid; actor uuid; actor_label text; person uuid; doc public.applicant_documents;
    candidate public.applicant_documents; object_meta jsonb; object_owner text;
begin
    select m.company_id,m.user_id,e.full_name into company,actor,actor_label from public.company_members m
        join public.employees e on e.company_id=m.company_id and e.id=m.employee_id
        where m.user_id=(select auth.uid()) and m.active and m.role in ('administrator','hr_generalist');
    if actor is null then raise exception 'Only active HR and Administrators can manage applicant attachments.' using errcode='42501'; end if;
    perform 1 from public.companies where id=company for update;
    perform 1 from public.company_members where company_id=company and user_id=actor and active and role in ('administrator','hr_generalist') for update;
    if not found then raise exception 'Your applicant access changed.' using errcode='42501';end if;
    if jsonb_typeof(p_document) is distinct from 'object' or p_document->>'id' is null then
        raise exception 'Enter valid document details.' using errcode='23514'; end if;
    -- Serialize each document, including two concurrent retries before the first row exists.
    perform pg_advisory_xact_lock(hashtextextended((p_document->>'id'),73));
    select * into doc from public.applicant_documents where id=(p_document->>'id')::uuid and company_id=company for update;
    if p_operation='reserve' then
        person:=(p_document->>'applicant_id')::uuid;
        if not exists(select 1 from public.applicants where id=person and company_id=company and not archived) then
            raise exception 'Active application not found in this company.' using errcode='42501'; end if;
        candidate:=jsonb_populate_record(null::public.applicant_documents,p_document);
        if doc.id is not null then
            if doc.created_by<>actor or doc.applicant_id<>person or doc.state='discarded' or
                (doc.title,doc.category,doc.notes,doc.issued_on,doc.expires_on,doc.contract_start,doc.contract_end,doc.original_filename,doc.mime_type,doc.byte_size,doc.sha256)
                is distinct from (candidate.title,candidate.category,candidate.notes,candidate.issued_on,candidate.expires_on,candidate.contract_start,candidate.contract_end,candidate.original_filename,candidate.mime_type,candidate.byte_size,candidate.sha256) then
                raise exception 'This upload ID already belongs to another document. Refresh and start a new upload.' using errcode='PT409'; end if;
            return doc;
        end if;
        insert into public.applicant_documents(id,company_id,applicant_id,title,category,notes,issued_on,expires_on,contract_start,contract_end,
            original_filename,mime_type,byte_size,sha256,created_by,created_by_name)
            values(candidate.id,company,person,btrim(candidate.title),candidate.category,btrim(coalesce(candidate.notes,'')),candidate.issued_on,candidate.expires_on,candidate.contract_start,candidate.contract_end,
                candidate.original_filename,candidate.mime_type,candidate.byte_size,candidate.sha256,actor,actor_label) returning * into doc;
        return doc;
    end if;
    if doc.id is null then raise exception 'Document not found in this company.' using errcode='42501'; end if;
    if p_operation='finalize' then
        if doc.state='ready' then return doc; end if;
        if doc.state<>'pending' or doc.created_by<>actor then raise exception 'This upload cannot be completed.' using errcode='42501'; end if;
        select metadata,owner_id into object_meta,object_owner from storage.objects where bucket_id='applicant-documents' and name=doc.object_name;
        if object_meta is null or object_owner is distinct from actor::text or object_meta->>'mimetype' is distinct from doc.mime_type
            or object_meta->>'size' is distinct from doc.byte_size::text then
            raise exception 'Upload incomplete or the stored file does not match its reserved size/type. Retry the same file or discard the incomplete upload.' using errcode='23514'; end if;
        update public.applicant_documents set state='ready',revision=revision+1,updated_at=clock_timestamp() where id=doc.id returning * into doc;
    elsif p_operation='discard' then
        if doc.state='ready' then raise exception 'Filed documents are retained. Archive this document instead.' using errcode='42501'; end if;
        if doc.state='discarded' then return doc; end if;
        -- A permanent tombstone precedes Storage API cleanup, so cleanup can never race finalization.
        update public.applicant_documents set state='discarded',revision=revision+1,updated_at=clock_timestamp() where id=doc.id returning * into doc;
    elsif p_operation in ('edit','archive','restore') then
        if doc.state<>'ready' then raise exception 'Only filed documents have editable details.' using errcode='23514'; end if;
        if (p_document->>'revision')::bigint is distinct from doc.revision then
            raise exception 'This document changed. Refresh and reopen it before saving.' using errcode='PT409'; end if;
        candidate:=doc;
        if p_operation='edit' then
            candidate.title:=btrim(p_document->>'title');candidate.category:=p_document->>'category';candidate.notes:=btrim(coalesce(p_document->>'notes',''));
            candidate.issued_on:=nullif(p_document->>'issued_on','')::date;candidate.expires_on:=nullif(p_document->>'expires_on','')::date;
            candidate.contract_start:=nullif(p_document->>'contract_start','')::date;candidate.contract_end:=nullif(p_document->>'contract_end','')::date;
        else candidate.archived:=(p_operation='archive'); end if;
        if candidate=doc then return doc; end if;
        update public.applicant_documents set title=candidate.title,category=candidate.category,notes=candidate.notes,issued_on=candidate.issued_on,
            expires_on=candidate.expires_on,contract_start=candidate.contract_start,contract_end=candidate.contract_end,archived=candidate.archived,
            revision=revision+1,updated_at=clock_timestamp() where id=doc.id returning * into doc;
    else raise exception 'Unknown document operation.' using errcode='23514'; end if;
    return doc;
end;
$$;
revoke all on function hris_private.save_applicant_document(text,jsonb) from public,anon,authenticated;
grant execute on function hris_private.save_applicant_document(text,jsonb) to authenticated;
create function public.save_applicant_document(p_operation text,p_document jsonb) returns jsonb
language sql security invoker set search_path='' as $$ select to_jsonb(hris_private.save_applicant_document(p_operation,p_document)); $$;
revoke all on function public.save_applicant_document(text,jsonb) from public,anon,authenticated;
grant execute on function public.save_applicant_document(text,jsonb) to authenticated;

insert into storage.buckets(id,name,public,file_size_limit,allowed_mime_types)
    values('applicant-documents','applicant-documents',false,10485760,array['application/pdf','image/png','image/jpeg']);
create policy hris_applicant_documents_select on storage.objects for select to authenticated using(
    bucket_id='applicant-documents' and exists(select 1 from public.applicant_documents d where d.object_name=name));
create policy hris_applicant_documents_insert on storage.objects for insert to authenticated with check(
    bucket_id='applicant-documents' and owner_id=(select auth.uid())::text and exists(
        select 1 from public.applicant_documents d where d.object_name=name and d.state='pending' and d.created_by=(select auth.uid())));
-- No UPDATE policy: the same path cannot be overwritten, including with upsert.
create policy hris_applicant_documents_cleanup on storage.objects for delete to authenticated using(
    bucket_id='applicant-documents' and exists(select 1 from public.applicant_documents d where d.object_name=name and d.state='discarded'));


-- Add Word OOXML files without changing any existing document's object path or bytes.

alter table public.applicant_documents drop constraint applicant_documents_mime_type_check;
alter table public.applicant_documents add constraint applicant_documents_mime_type_check
    check (mime_type in ('application/pdf','image/png','image/jpeg',
        'application/vnd.openxmlformats-officedocument.wordprocessingml.document'));
alter table public.applicant_documents drop constraint applicant_documents_check4;
alter table public.applicant_documents add constraint applicant_documents_filename_mime_check
    check ((mime_type='application/pdf' and lower(original_filename) ~ '\.pdf$') or
           (mime_type='image/png' and lower(original_filename) ~ '\.png$') or
           (mime_type='image/jpeg' and lower(original_filename) ~ '\.jpe?g$') or
           (mime_type='application/vnd.openxmlformats-officedocument.wordprocessingml.document'
               and lower(original_filename) ~ '\.docx$'));
alter table public.applicant_documents alter column object_name set expression as (
    company_id::text||'/'||applicant_id::text||'/'||id::text||'/file.'||case mime_type
        when 'application/pdf' then 'pdf' when 'image/png' then 'png'
        when 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' then 'docx'
        else 'jpg' end);
update storage.buckets set allowed_mime_types=array['application/pdf','image/png','image/jpeg',
    'application/vnd.openxmlformats-officedocument.wordprocessingml.document']
    where id='applicant-documents';
analyze public.applicant_documents;


create table public.recruiting_interviews (
 id uuid primary key check(id<>'00000000-0000-0000-0000-000000000000'),
 company_id uuid not null references public.companies(id),applicant_id uuid not null,
 interviewer_employee_id uuid not null,applicant_name text not null,position text not null,
 title text not null check(char_length(btrim(title)) between 1 and 120 and title !~ '[[:cntrl:]]'),
 starts_at timestamptz not null,ends_at timestamptz not null,timezone text not null,
 location text not null default '' check(char_length(location)<=240 and location !~ '[[:cntrl:]]'),
 brief text not null default '' check(char_length(brief)<=1000 and replace(brief,E'\n','') !~ '[[:cntrl:]]'),
 status text not null default 'scheduled' check(status in ('scheduled','completed','cancelled')),
 revision bigint not null default 0 check(revision>=0),created_by uuid not null,
 created_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 check(ends_at>starts_at and ends_at<=starts_at+interval '8 hours'),
 foreign key(company_id,applicant_id) references public.applicants(company_id,id),
 foreign key(company_id,interviewer_employee_id) references public.employees(company_id,id),unique(company_id,id)
);
create index recruiting_interviews_applicant_idx on public.recruiting_interviews(company_id,applicant_id,starts_at);
create index recruiting_interviews_assignee_idx on public.recruiting_interviews(company_id,interviewer_employee_id,starts_at);
create table public.interview_feedback (
 id uuid primary key default gen_random_uuid(),company_id uuid not null,interview_id uuid not null,
 author_id uuid not null,author_name text not null,author_role text not null check(author_role in ('administrator','hr_generalist','manager')),
 rating integer not null check(rating between 1 and 5),recommendation text not null check(recommendation in ('proceed','hold','not_recommended')),
 assessment text not null check(char_length(btrim(assessment)) between 1 and 2000 and replace(assessment,E'\n','') !~ '[[:cntrl:]]'),
 revision bigint not null default 0 check(revision>=0),submitted_at timestamptz not null default clock_timestamp(),updated_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,interview_id) references public.recruiting_interviews(company_id,id),unique(interview_id,author_id)
);
create index interview_feedback_company_interview_idx on public.interview_feedback(company_id,interview_id);
create table public.interview_events (
 id uuid primary key default gen_random_uuid(),company_id uuid not null,interview_id uuid not null,
 kind text not null check(kind in ('scheduled','rescheduled','cancelled','completed','feedback_saved')),
 actor_id uuid not null,actor_name text not null,actor_role text not null,
 reason text not null check(char_length(btrim(reason)) between 1 and 500 and reason !~ '[[:cntrl:]]'),
 before_record jsonb,after_record jsonb not null,recorded_at timestamptz not null default clock_timestamp(),
 foreign key(company_id,interview_id) references public.recruiting_interviews(company_id,id)
);
create index interview_events_company_interview_idx on public.interview_events(company_id,interview_id,recorded_at);
create trigger interview_events_immutable before update or delete on public.interview_events for each row execute function hris_private.applicant_history_immutable();
alter table public.recruiting_interviews enable row level security;
alter table public.interview_feedback enable row level security;
alter table public.interview_events enable row level security;
revoke all on public.recruiting_interviews,public.interview_feedback,public.interview_events from public,anon,authenticated;
grant select on public.recruiting_interviews,public.interview_feedback,public.interview_events to authenticated;
create function hris_private.can_read_interview(p_company uuid,p_interview uuid) returns boolean
language sql stable security definer set search_path='' as $$
 select exists(select 1 from public.company_members m join public.recruiting_interviews i on i.company_id=m.company_id
 join public.applicants a on a.company_id=i.company_id and a.id=i.applicant_id
 where m.company_id=p_company and i.id=p_interview and m.user_id=(select auth.uid()) and m.active
 and (m.role in ('administrator','hr_generalist') or (m.role='manager' and m.employee_id=i.interviewer_employee_id and not a.archived)));
$$;
revoke all on function hris_private.can_read_interview(uuid,uuid) from public,anon;
grant execute on function hris_private.can_read_interview(uuid,uuid) to authenticated;
create policy recruiting_interviews_read on public.recruiting_interviews for select to authenticated using(hris_private.can_read_interview(company_id,id));
create policy interview_feedback_read on public.interview_feedback for select to authenticated using(hris_private.can_read_interview(company_id,interview_id) and (hris_private.company_role(company_id) in ('administrator','hr_generalist') or author_id=(select auth.uid())));
create policy interview_events_read on public.interview_events for select to authenticated using(hris_private.can_read_interview(company_id,interview_id) and (kind<>'feedback_saved' or hris_private.company_role(company_id) in ('administrator','hr_generalist') or actor_id=(select auth.uid())));
create function hris_private.recruiting_interviewers() returns jsonb language sql stable security definer set search_path='' as $$
 select coalesce(jsonb_agg(jsonb_build_object('employee_id',e.id,'full_name',e.full_name,'role',m.role) order by e.full_name,e.id),'[]'::jsonb)
 from public.company_members m join public.employees e on e.company_id=m.company_id and e.id=m.employee_id
 where m.active and m.role in ('administrator','hr_generalist','manager') and e.status='active'
 and hris_private.company_role(m.company_id) in ('administrator','hr_generalist');
$$;
revoke all on function hris_private.recruiting_interviewers() from public,anon;
grant execute on function hris_private.recruiting_interviewers() to authenticated;
create function public.recruiting_interviewers() returns jsonb language sql security invoker set search_path='' as $$select hris_private.recruiting_interviewers();$$;
revoke all on function public.recruiting_interviewers() from public,anon;
grant execute on function public.recruiting_interviewers() to authenticated;

create function hris_private.manage_interview(p_operation text,p_record jsonb) returns jsonb
language plpgsql security definer set search_path='' as $$
declare m public.company_members;app public.applicants;old public.recruiting_interviews;saved public.recruiting_interviews;
 f public.interview_feedback;nf public.interview_feedback;prior public.interview_events;
 subject uuid:=(select auth.uid());v_id uuid;v_app uuid;v_interviewer uuid;v_reason text;v_actor text;v_kind text;
 v_title text;v_start timestamptz;v_end timestamptz;v_timezone text;v_location text;v_brief text;v_expected bigint;k text;
begin
 select * into m from public.company_members where user_id=subject and active and role in ('administrator','hr_generalist','manager');
 if m.user_id is null then raise exception 'Active staff access is required.' using errcode='42501';end if;
 perform 1 from public.companies where id=m.company_id for update;
 select * into m from public.company_members where user_id=subject and active and role in ('administrator','hr_generalist','manager') for update;
 if m.user_id is null then raise exception 'Your interview access changed.' using errcode='42501';end if;
 if p_operation is null or p_operation not in ('save','status','feedback') or jsonb_typeof(p_record) is distinct from 'object' or octet_length(p_record::text)>16384 then raise exception 'Invalid interview operation.' using errcode='23514';end if;
 if p_operation<>'feedback' and m.role not in ('administrator','hr_generalist') then raise exception 'Only HR can schedule or close interviews.' using errcode='42501';end if;
 for k in select jsonb_object_keys(p_record) loop
  if not(k=any(case p_operation when 'save' then array['id','expected_revision','applicant_id','interviewer_employee_id','title','starts_at','ends_at','timezone','location','brief','reason']
   when 'feedback' then array['id','expected_revision','rating','recommendation','assessment','reason'] else array['id','expected_revision','status','reason'] end)) then raise exception 'Unknown or protected interview field.' using errcode='23514';end if;
 end loop;
 if jsonb_typeof(p_record->'id') is distinct from 'string' or coalesce(p_record->>'id','') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' then raise exception 'Invalid interview identifier.' using errcode='23514';end if;
 v_id:=(p_record->>'id')::uuid;
 if v_id='00000000-0000-0000-0000-000000000000' then raise exception 'Invalid interview identifier.' using errcode='23514';end if;
 v_reason:=btrim(p_record->>'reason');
 if jsonb_typeof(p_record->'reason') is distinct from 'string' or char_length(v_reason) not between 1 and 500 or v_reason ~ '[[:cntrl:]]' then raise exception 'Enter a change reason of 1-500 characters.' using errcode='23514';end if;
 select * into old from public.recruiting_interviews where id=v_id and company_id=m.company_id for update;
 select full_name into v_actor from public.employees where id=m.employee_id and company_id=m.company_id;
 if p_record ? 'expected_revision' then
  if jsonb_typeof(p_record->'expected_revision') is distinct from 'number' or coalesce(p_record->>'expected_revision','') !~ '^[0-9]{1,18}$' then raise exception 'Invalid prior interview revision.' using errcode='23514';end if;
  v_expected:=(p_record->>'expected_revision')::bigint;
 end if;
 if p_operation='feedback' then
  if old.id is null or not hris_private.can_read_interview(m.company_id,v_id) or old.interviewer_employee_id is distinct from m.employee_id then raise exception 'Only the currently assigned interviewer can write their feedback.' using errcode='42501';end if;
  if old.status='cancelled' or old.starts_at>clock_timestamp() then raise exception 'Feedback is available after the interview starts and before cancellation.' using errcode='23514';end if;
  select * into f from public.interview_feedback where interview_id=v_id and author_id=subject for update;
  if v_expected is distinct from (case when f.id is null then 0 else f.revision end) then raise exception 'Feedback changed. Refresh and reopen it.' using errcode='PT409';end if;
  if jsonb_typeof(p_record->'rating') is distinct from 'number' or coalesce(p_record->>'rating','') !~ '^[1-5]$' or jsonb_typeof(p_record->'recommendation') is distinct from 'string' or jsonb_typeof(p_record->'assessment') is distinct from 'string' then raise exception 'Enter a rating, recommendation and written assessment.' using errcode='23514';end if;
  insert into public.interview_feedback(company_id,interview_id,author_id,author_name,author_role,rating,recommendation,assessment,revision)
   values(m.company_id,v_id,subject,coalesce(v_actor,'Staff'),m.role,(p_record->>'rating')::int,p_record->>'recommendation',btrim(p_record->>'assessment'),1)
   on conflict(interview_id,author_id) do update set rating=excluded.rating,recommendation=excluded.recommendation,assessment=excluded.assessment,revision=public.interview_feedback.revision+1,updated_at=clock_timestamp() returning * into nf;
  insert into public.interview_events(company_id,interview_id,kind,actor_id,actor_name,actor_role,reason,before_record,after_record)
   values(m.company_id,v_id,'feedback_saved',subject,coalesce(v_actor,'Staff'),m.role,v_reason,case when f.id is null then null else to_jsonb(f) end,to_jsonb(nf));return to_jsonb(nf);
 elsif p_operation='save' then
  for k in select unnest(array['applicant_id','interviewer_employee_id','title','starts_at','ends_at','timezone','location','brief']) loop
   if jsonb_typeof(p_record->k) is distinct from 'string' then raise exception 'Interview schedule fields must be text.' using errcode='23514';end if;
  end loop;
  v_app:=(p_record->>'applicant_id')::uuid;v_interviewer:=(p_record->>'interviewer_employee_id')::uuid;
  select * into app from public.applicants where id=v_app and company_id=m.company_id and not archived;
  if app.id is null then raise exception 'Open an active application before scheduling.' using errcode='42501';end if;
  if not exists(select 1 from public.company_members cm join public.employees e on e.id=cm.employee_id and e.company_id=cm.company_id where cm.company_id=m.company_id and cm.employee_id=v_interviewer and cm.active and cm.role in ('administrator','hr_generalist','manager') and e.status='active') then raise exception 'Choose active staff with desktop access as interviewer.' using errcode='23514';end if;
  v_title:=btrim(p_record->>'title');v_start:=(p_record->>'starts_at')::timestamptz;v_end:=(p_record->>'ends_at')::timestamptz;v_timezone:=p_record->>'timezone';v_location:=btrim(p_record->>'location');v_brief:=btrim(p_record->>'brief');
  if v_timezone is distinct from (select timezone from public.companies where id=m.company_id) or not isfinite(v_start) or not isfinite(v_end) or v_end<=v_start or v_end>v_start+interval '8 hours' then raise exception 'Check the company time zone and an interview duration up to eight hours.' using errcode='23514';end if;
  if old.id is not null and v_expected is null then
   select ev.* into prior from public.interview_events ev where ev.interview_id=v_id and ev.kind='scheduled' order by ev.recorded_at limit 1;
   if prior.actor_id=subject and prior.reason=v_reason and (prior.after_record->>'applicant_id')::uuid=v_app and (prior.after_record->>'interviewer_employee_id')::uuid=v_interviewer and prior.after_record->>'title'=v_title and (prior.after_record->>'starts_at')::timestamptz=v_start and (prior.after_record->>'ends_at')::timestamptz=v_end and prior.after_record->>'timezone'=v_timezone and prior.after_record->>'location'=v_location and prior.after_record->>'brief'=v_brief then return to_jsonb(old);end if;
   raise exception 'This interview identifier already exists.' using errcode='PT409';end if;
  if old.id is not null and (v_expected<>old.revision or v_app<>old.applicant_id) then raise exception 'The interview changed. Refresh and reopen it.' using errcode='PT409';end if;
  if old.id is not null and old.status<>'scheduled' then raise exception 'Only a scheduled interview may be rescheduled.' using errcode='23514';end if;
  if exists(select 1 from public.recruiting_interviews i where i.company_id=m.company_id and i.id<>v_id and i.interviewer_employee_id=v_interviewer and i.status='scheduled' and tstzrange(i.starts_at,i.ends_at,'[)') && tstzrange(v_start,v_end,'[)')) then raise exception 'The interviewer already has a scheduled interview during that time.' using errcode='23P01';end if;
  if old.id is null then
   if v_expected is not null then raise exception 'Create does not accept a prior revision.' using errcode='23514';end if;
   insert into public.recruiting_interviews(id,company_id,applicant_id,interviewer_employee_id,applicant_name,position,title,starts_at,ends_at,timezone,location,brief,created_by)
    values(v_id,m.company_id,v_app,v_interviewer,app.full_name,app.position,v_title,v_start,v_end,v_timezone,v_location,v_brief,subject) returning * into saved;v_kind:='scheduled';
  else
   update public.recruiting_interviews set interviewer_employee_id=v_interviewer,title=v_title,starts_at=v_start,ends_at=v_end,timezone=v_timezone,location=v_location,brief=v_brief,revision=revision+1,updated_at=clock_timestamp() where id=v_id and company_id=m.company_id returning * into saved;v_kind:='rescheduled';
  end if;
 else
  if old.id is null then raise exception 'Interview not found.' using errcode='42501';end if;
  if v_expected is distinct from old.revision then raise exception 'The interview changed. Refresh and reopen it.' using errcode='PT409';end if;
  if old.status<>'scheduled' or jsonb_typeof(p_record->'status') is distinct from 'string' or coalesce(p_record->>'status','') not in ('completed','cancelled') then raise exception 'Choose complete or cancel for a scheduled interview.' using errcode='23514';end if;
  if p_record->>'status'='completed' and old.starts_at>clock_timestamp() then raise exception 'The interview has not started yet.' using errcode='23514';end if;
  update public.recruiting_interviews set status=p_record->>'status',revision=revision+1,updated_at=clock_timestamp() where id=v_id and company_id=m.company_id returning * into saved;v_kind:=saved.status;
 end if;
 insert into public.interview_events(company_id,interview_id,kind,actor_id,actor_name,actor_role,reason,before_record,after_record)
  values(m.company_id,v_id,v_kind,subject,coalesce(v_actor,'Staff'),m.role,v_reason,case when old.id is null then null else to_jsonb(old) end,to_jsonb(saved));return to_jsonb(saved);
end;$$;
revoke all on function hris_private.manage_interview(text,jsonb) from public,anon;
grant execute on function hris_private.manage_interview(text,jsonb) to authenticated;
create function public.manage_interview(p_operation text,p_record jsonb) returns jsonb language sql security invoker set search_path='' as $$select hris_private.manage_interview(p_operation,p_record);$$;
revoke all on function public.manage_interview(text,jsonb) from public,anon;
grant execute on function public.manage_interview(text,jsonb) to authenticated;
commit;
