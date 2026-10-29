-- Compatible with installed http1.6: use the typed header array directly.
begin;
create or replace function hris_private.device_push_tick()returns bigint
language plpgsql security definer set search_path=''as $$
declare endpoint text;secret text;status_code integer;result_text text;
begin
 -- No push lock or data writes here: the HTTP worker takes its own transactions.
 if not exists(select 1 from hris_private.device_push_config where singleton and enabled)
 or not exists(select 1 from hris_private.device_push_devices where enabled)then return null;end if;
 execute 'select decrypted_secret from vault.decrypted_secrets where name=$1'into endpoint using'hris_device_push_url';
 execute 'select decrypted_secret from vault.decrypted_secrets where name=$1'into secret using'hris_device_push_worker';
 if endpoint is null or endpoint!~'^https://[a-z0-9]+\.supabase\.co/functions/v1/device-push$'or secret is null or length(secret)<40 then
  raise exception 'Device sender is not configured.'using errcode='23514';
 end if;
 perform pg_catalog.set_config('http.curlopt_timeout_ms','20000',true);
 perform pg_catalog.set_config('http.curlopt_connecttimeout_ms','3000',true);
 perform pg_catalog.set_config('http.curlopt_ssl_verifypeer','1',true);
 perform pg_catalog.set_config('http.curlopt_ssl_verifyhost','2',true);
 begin
  execute $call$select r.status,r.content from extensions.http(('POST',$1,
   array[row('Authorization'::varchar,('Bearer '||$2)::varchar)::extensions.http_header],'application/json','{"operation":"worker"}')::extensions.http_request)r$call$
   into status_code,result_text using endpoint,secret;
  if status_code<>200 or result_text::jsonb->>'version'is distinct from'device-push-result-v1'then
   raise exception 'Unexpected sender outcome.';
  end if;
 exception when others then
  raise exception 'Device sender call failed; leased jobs retry safely.'using errcode='58000';
 end;
 return status_code;
end;$$;
revoke all on function hris_private.device_push_tick()from public,anon,authenticated,service_role;
commit;
