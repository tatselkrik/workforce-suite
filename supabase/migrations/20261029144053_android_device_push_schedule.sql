-- Free bounded scheduling only. Dispatch remains disabled until verified rollout.
begin;
do $$begin
 if exists(select 1 from pg_extension where extname='pg_cron')then
  if exists(select 1 from cron.job where jobname='hris-device-alerts-v1')then
   raise exception 'Device-alert schedule already exists; reconcile its owner before changing it.';
  end if;
  perform cron.schedule('hris-device-alerts-v1','*/15 * * * *','select hris_private.device_push_tick();');
 end if;
end;$$;
commit;
