import {createHandler} from './handler.mjs';
const url=Deno.env.get('SUPABASE_URL')!;
const key=Deno.env.get('SUPABASE_SERVICE_ROLE_KEY')!;
const headers={apikey:key,authorization:`Bearer ${key}`};
Deno.serve(createHandler({
 async rpc(name:string,args:unknown){
  const response=await fetch(`${url}/rest/v1/rpc/${name}`,{method:'POST',headers:{...headers,'content-type':'application/json'},body:JSON.stringify(args),signal:AbortSignal.timeout(15000)});
  if(!response.ok){
   const failure=await response.json().catch(()=>null);
   // These RPC failures roll back their transaction; network/storage errors stay uncertain.
   if([400,403,409].includes(response.status)&&['PT409','42501','23514','23505'].includes(failure?.code)){
    const messages:Record<string,string>={
     'Capture expired. Take a new photo.':'Capture expired. Take a new photo.',
     'Station is locked.':'Station is locked. Ask an Administrator to unlock it.',
     'Attendance policy changed. Take a new photo.':'Attendance policy changed. Take a new photo.',
     'Attendance changed. Refresh before another punch.':'Attendance changed. Ask HR to review it before another punch.',
     'Employee access changed.':'Employee access changed. Contact HR.',
    };
    throw Object.assign(Error(messages[failure.message]||'Attendance was rejected. Ask HR to review this punch.'),{definitive:true,status:response.status});
   }
   throw Error('Protected attendance operation failed.');
  }
  return response.json();
 },
 async upload(path:string,bytes:Uint8Array){
  const response=await fetch(`${url}/storage/v1/object/attendance-evidence/${path}`,{method:'POST',headers:{...headers,'content-type':'image/jpeg','x-upsert':'false'},body:bytes,signal:AbortSignal.timeout(15000)});
  if(!response.ok && response.status!==409){
   const value=await response.json().catch(()=>null);
   if(value?.statusCode!=='409'&&value?.error!=='Duplicate')throw Error('Camera evidence upload failed.');
  }
 }
}));
