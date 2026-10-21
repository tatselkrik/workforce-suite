-- Translate stale-form conflicts at the public API boundary.
-- PostgREST 14 retries serialization_failure; PT409 returns an immediate conflict.
-- https://supabase.com/docs/guides/troubleshooting/high-cpu-and-infinite-transaction-retries-when-using-custom-error-codes-in-rpc-functions-77326b
begin;
create or replace function public.correct_attendance(p_change jsonb) returns uuid
language plpgsql security invoker set search_path='' as $$
begin
    return hris_private.correct_attendance(p_change);
exception when serialization_failure then
    raise sqlstate 'PT409' using message=sqlerrm;
end;
$$;
commit;
