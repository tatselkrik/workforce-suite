-- Add Word OOXML files without changing any existing document's object path or bytes.
begin;
alter table public.employee_documents drop constraint employee_documents_mime_type_check;
alter table public.employee_documents add constraint employee_documents_mime_type_check
    check (mime_type in ('application/pdf','image/png','image/jpeg',
        'application/vnd.openxmlformats-officedocument.wordprocessingml.document'));
alter table public.employee_documents drop constraint employee_documents_check4;
alter table public.employee_documents add constraint employee_documents_filename_mime_check
    check ((mime_type='application/pdf' and lower(original_filename) ~ '\.pdf$') or
           (mime_type='image/png' and lower(original_filename) ~ '\.png$') or
           (mime_type='image/jpeg' and lower(original_filename) ~ '\.jpe?g$') or
           (mime_type='application/vnd.openxmlformats-officedocument.wordprocessingml.document'
               and lower(original_filename) ~ '\.docx$'));
alter table public.employee_documents alter column object_name set expression as (
    company_id::text||'/'||employee_id::text||'/'||id::text||'/file.'||case mime_type
        when 'application/pdf' then 'pdf' when 'image/png' then 'png'
        when 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' then 'docx'
        else 'jpg' end);
update storage.buckets set allowed_mime_types=array['application/pdf','image/png','image/jpeg',
    'application/vnd.openxmlformats-officedocument.wordprocessingml.document']
    where id='employee-documents';
analyze public.employee_documents;
commit;
