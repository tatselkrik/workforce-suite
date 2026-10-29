import {createPushHandler} from "./handler.js";
Deno.serve(createPushHandler({url:Deno.env.get("SUPABASE_URL"),serviceKey:Deno.env.get("SUPABASE_SERVICE_ROLE_KEY"),anonKey:Deno.env.get("SUPABASE_ANON_KEY"),workerSecret:Deno.env.get("HRIS_PUSH_WORKER_KEY"),projectId:Deno.env.get("HRIS_FCM_PROJECT_ID"),accountJson:Deno.env.get("HRIS_FCM_SERVICE_ACCOUNT_JSON")}));
