-- The new pg_net request headers and responses are owner-only.
begin;
do $$begin
 if exists(select 1 from pg_extension where extname='pg_net')then
  revoke all on net.http_request_queue,net._http_response from public,anon,authenticated;
 end if;
end;$$;
commit;
