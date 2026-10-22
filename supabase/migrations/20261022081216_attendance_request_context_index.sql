-- Cover the private transaction-proof foreign key flagged by the hosted advisor.
create index attendance_request_context_request_idx on hris_private.attendance_request_context(request_id);
