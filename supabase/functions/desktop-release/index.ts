import "jsr:@supabase/functions-js/edge-runtime.d.ts";
import { createReleaseHandler } from "./handler.mjs";
import { UPDATE_PUBLIC_KEY } from "./verification-key.ts";

Deno.serve(createReleaseHandler({
  url: Deno.env.get("SUPABASE_URL"), serviceKey: Deno.env.get("SUPABASE_SERVICE_ROLE_KEY"),
  anonKey: Deno.env.get("SUPABASE_ANON_KEY"), publicKey: UPDATE_PUBLIC_KEY,
}));
