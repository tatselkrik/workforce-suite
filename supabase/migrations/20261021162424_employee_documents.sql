-- Private employee 201 files. Storage bytes are immutable once filed; archive retains them.
begin;
create table public.employee_documents (
    id uuid primary key,
    company_id uuid not null references public.companies(id) on delete restrict,
    employee_id uuid not null,
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
    object_name text generated always as (company_id::text||'/'||employee_id::text||'/'||id::text||'/file.'||
        case mime_type when 'application/pdf' then 'pdf' when 'image/png' then 'png' else 'jpg' end) stored unique,
    state text not null default 'pending' check(state in ('pending','ready','discarded')),
    archived boolean not null default false,
    revision bigint not null default 0 check(revision>=0),
    created_by uuid not null references auth.users(id) on delete restrict,
    created_by_name text not null,
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),
    unique(company_id,employee_id,id),
    foreign key(company_id,employee_id) references public.employees(company_id,id) on delete restrict,
    check(expires_on is null or issued_on is null or expires_on>=issued_on),
    check(contract_end is null or (contract_start is not null and contract_end>=contract_start)),
    check(category='contract' or (contract_start is null and contract_end is null)),
    check(not archived or state='ready'),
    check((mime_type='application/pdf' and lower(original_filename) ~ '\.pdf$') or
          (mime_type='image/png' and lower(original_filename) ~ '\.png$') or
          (mime_type='image/jpeg' and lower(original_filename) ~ '\.jpe?g$'))
);
create index employee_documents_person_idx on public.employee_documents(company_id,employee_id,created_at desc);
create index employee_documents_creator_idx on public.employee_documents(created_by);
alter table public.employee_documents enable row level security;
revoke all on public.employee_documents from public,anon,authenticated;
grant select on public.employee_documents to authenticated;
create policy employee_documents_read on public.employee_documents for select to authenticated
    using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));

create table public.employee_document_events (
    id uuid primary key default gen_random_uuid(),
    sequence bigint generated always as identity unique,
    company_id uuid not null,
    employee_id uuid not null,
    document_id uuid not null,
    kind text not null check(kind in ('uploaded','details_updated','archived','restored')),
    actor_id uuid not null references auth.users(id) on delete restrict,
    actor_name text not null,
    actor_role text not null check(actor_role in ('administrator','hr_generalist')),
    before_record jsonb,
    after_record jsonb not null,
    recorded_at timestamptz not null default clock_timestamp(),
    foreign key(company_id,employee_id,document_id) references public.employee_documents(company_id,employee_id,id) on delete restrict
);
create index employee_document_events_person_idx on public.employee_document_events(company_id,employee_id,document_id,sequence desc);
create index employee_document_events_actor_idx on public.employee_document_events(actor_id);
alter table public.employee_document_events enable row level security;
revoke all on public.employee_document_events from public,anon,authenticated;
grant select on public.employee_document_events to authenticated;
create policy employee_document_events_read on public.employee_document_events for select to authenticated
    using(hris_private.company_role(company_id) in ('administrator','hr_generalist'));

create function hris_private.guard_employee_document() returns trigger
language plpgsql security invoker set search_path='' as $$
begin
    if tg_op='DELETE' then raise exception 'Document records are retained. Archive filed documents instead.' using errcode='42501'; end if;
    if (new.id,new.company_id,new.employee_id,new.original_filename,new.mime_type,new.byte_size,new.sha256,new.created_by,new.created_by_name,new.created_at)
        is distinct from (old.id,old.company_id,old.employee_id,old.original_filename,old.mime_type,old.byte_size,old.sha256,old.created_by,old.created_by_name,old.created_at) then
        raise exception 'A document file and its original attribution cannot be replaced.' using errcode='42501';
    end if;
    if not ((old.state='pending' and new.state in ('ready','discarded')) or (old.state='ready' and new.state='ready')) then
        raise exception 'This upload state cannot be changed.' using errcode='42501';
    end if;
    if new.revision<>old.revision+1 then raise exception 'Invalid document revision.' using errcode='PT409'; end if;
    return new;
end;
$$;
revoke all on function hris_private.guard_employee_document() from public,anon,authenticated;
create trigger employee_documents_guard before update or delete on public.employee_documents
    for each row execute function hris_private.guard_employee_document();

create function hris_private.record_employee_document_event() returns trigger
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
    insert into public.employee_document_events(company_id,employee_id,document_id,kind,actor_id,actor_name,actor_role,before_record,after_record)
        values(new.company_id,new.employee_id,new.id,event_kind,actor,actor_label,actor_kind,
            case when old.state='ready' then to_jsonb(old) else null end,to_jsonb(new));
    return new;
end;
$$;
revoke all on function hris_private.record_employee_document_event() from public,anon,authenticated;
create trigger employee_documents_record_event after update on public.employee_documents
    for each row execute function hris_private.record_employee_document_event();
create trigger employee_document_events_immutable before update or delete on public.employee_document_events
    for each row execute function hris_private.employment_history_immutable();

-- Only this narrow private writer can change metadata. The public wrapper remains an invoker.
create function hris_private.save_employee_document(p_operation text,p_document jsonb) returns public.employee_documents
language plpgsql security definer set search_path='' as $$
declare company uuid; actor uuid; actor_label text; person uuid; doc public.employee_documents;
    candidate public.employee_documents; object_meta jsonb; object_owner text;
begin
    select m.company_id,m.user_id,e.full_name into company,actor,actor_label from public.company_members m
        join public.employees e on e.company_id=m.company_id and e.id=m.employee_id
        where m.user_id=(select auth.uid()) and m.active and m.role in ('administrator','hr_generalist');
    if actor is null then raise exception 'Only active HR and Administrators can manage employee documents.' using errcode='42501'; end if;
    if jsonb_typeof(p_document) is distinct from 'object' or p_document->>'id' is null then
        raise exception 'Enter valid document details.' using errcode='23514'; end if;
    -- Serialize each document, including two concurrent retries before the first row exists.
    perform pg_advisory_xact_lock(hashtextextended((p_document->>'id'),73));
    select * into doc from public.employee_documents where id=(p_document->>'id')::uuid and company_id=company for update;
    if p_operation='reserve' then
        person:=(p_document->>'employee_id')::uuid;
        if not exists(select 1 from public.employees where id=person and company_id=company) then
            raise exception 'Employee not found in this company.' using errcode='42501'; end if;
        candidate:=jsonb_populate_record(null::public.employee_documents,p_document);
        if doc.id is not null then
            if doc.created_by<>actor or doc.employee_id<>person or doc.state='discarded' or
                (doc.title,doc.category,doc.notes,doc.issued_on,doc.expires_on,doc.contract_start,doc.contract_end,doc.original_filename,doc.mime_type,doc.byte_size,doc.sha256)
                is distinct from (candidate.title,candidate.category,candidate.notes,candidate.issued_on,candidate.expires_on,candidate.contract_start,candidate.contract_end,candidate.original_filename,candidate.mime_type,candidate.byte_size,candidate.sha256) then
                raise exception 'This upload ID already belongs to another document. Refresh and start a new upload.' using errcode='PT409'; end if;
            return doc;
        end if;
        insert into public.employee_documents(id,company_id,employee_id,title,category,notes,issued_on,expires_on,contract_start,contract_end,
            original_filename,mime_type,byte_size,sha256,created_by,created_by_name)
            values(candidate.id,company,person,btrim(candidate.title),candidate.category,btrim(coalesce(candidate.notes,'')),candidate.issued_on,candidate.expires_on,candidate.contract_start,candidate.contract_end,
                candidate.original_filename,candidate.mime_type,candidate.byte_size,candidate.sha256,actor,actor_label) returning * into doc;
        return doc;
    end if;
    if doc.id is null then raise exception 'Document not found in this company.' using errcode='42501'; end if;
    if p_operation='finalize' then
        if doc.state='ready' then return doc; end if;
        if doc.state<>'pending' or doc.created_by<>actor then raise exception 'This upload cannot be completed.' using errcode='42501'; end if;
        select metadata,owner_id into object_meta,object_owner from storage.objects where bucket_id='employee-documents' and name=doc.object_name;
        if object_meta is null or object_owner is distinct from actor::text or object_meta->>'mimetype' is distinct from doc.mime_type
            or object_meta->>'size' is distinct from doc.byte_size::text then
            raise exception 'Upload incomplete or the stored file does not match its reserved size/type. Retry the same file or discard the incomplete upload.' using errcode='23514'; end if;
        update public.employee_documents set state='ready',revision=revision+1,updated_at=clock_timestamp() where id=doc.id returning * into doc;
    elsif p_operation='discard' then
        if doc.state='ready' then raise exception 'Filed documents are retained. Archive this document instead.' using errcode='42501'; end if;
        if doc.state='discarded' then return doc; end if;
        -- A permanent tombstone precedes Storage API cleanup, so cleanup can never race finalization.
        update public.employee_documents set state='discarded',revision=revision+1,updated_at=clock_timestamp() where id=doc.id returning * into doc;
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
        update public.employee_documents set title=candidate.title,category=candidate.category,notes=candidate.notes,issued_on=candidate.issued_on,
            expires_on=candidate.expires_on,contract_start=candidate.contract_start,contract_end=candidate.contract_end,archived=candidate.archived,
            revision=revision+1,updated_at=clock_timestamp() where id=doc.id returning * into doc;
    else raise exception 'Unknown document operation.' using errcode='23514'; end if;
    return doc;
end;
$$;
revoke all on function hris_private.save_employee_document(text,jsonb) from public,anon,authenticated;
grant execute on function hris_private.save_employee_document(text,jsonb) to authenticated;
create function public.save_employee_document(p_operation text,p_document jsonb) returns jsonb
language sql security invoker set search_path='' as $$ select to_jsonb(hris_private.save_employee_document(p_operation,p_document)); $$;
revoke all on function public.save_employee_document(text,jsonb) from public,anon,authenticated;
grant execute on function public.save_employee_document(text,jsonb) to authenticated;

insert into storage.buckets(id,name,public,file_size_limit,allowed_mime_types)
    values('employee-documents','employee-documents',false,10485760,array['application/pdf','image/png','image/jpeg']);
create policy hris_employee_documents_select on storage.objects for select to authenticated using(
    bucket_id='employee-documents' and exists(select 1 from public.employee_documents d where d.object_name=name));
create policy hris_employee_documents_insert on storage.objects for insert to authenticated with check(
    bucket_id='employee-documents' and owner_id=(select auth.uid())::text and exists(
        select 1 from public.employee_documents d where d.object_name=name and d.state='pending' and d.created_by=(select auth.uid())));
-- No UPDATE policy: the same path cannot be overwritten, including with upsert.
create policy hris_employee_documents_cleanup on storage.objects for delete to authenticated using(
    bucket_id='employee-documents' and exists(select 1 from public.employee_documents d where d.object_name=name and d.state='discarded'));
commit;
