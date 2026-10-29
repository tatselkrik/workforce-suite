import {SafeError,responseJson,uuid,deviceToken} from "./http.js";
import {createFcmSender} from "./fcm.js";
const reply=(body,status=200)=>new Response(JSON.stringify(body),{status,headers:{"Content-Type":"application/json","Cache-Control":"no-store","X-Content-Type-Options":"nosniff",...(status===429?{"Retry-After":"60"}:{})}});
async function equalSecret(a,b){if(typeof a!=="string"||typeof b!=="string"||b.length<40)return false;const [x,y]=await Promise.all([crypto.subtle.digest("SHA-256",new TextEncoder().encode(a)),crypto.subtle.digest("SHA-256",new TextEncoder().encode(b))]);const u=new Uint8Array(x),v=new Uint8Array(y);let diff=0;for(let i=0;i<u.length;i++)diff|=u[i]^v[i];return diff===0}
export function createPushHandler(options){
 const{url,serviceKey,anonKey,workerSecret,projectId,accountJson,fetch:fetcher=globalThis.fetch}=options;let sender=options.sender;
 async function request(path,key,bearer,body){
  if(!url?.startsWith("https://")||!key)throw new SafeError();
  let response;try{response=await fetcher(url.replace(/\/$/u,"")+path,{method:body===undefined?"GET":"POST",redirect:"error",signal:AbortSignal.timeout(20000),headers:{apikey:key,Authorization:"Bearer "+bearer,...(body===undefined?{}:{"Content-Type":"application/json"})},...(body===undefined?{}:{body:JSON.stringify(body)})})}catch{throw new SafeError()}
  const value=await responseJson(response);
  if(!response.ok)throw new SafeError(response.status===429?429:response.status===409?409:[401,403].includes(response.status)?403:503);
  return value;
 }
 const rpc=(name,body,bearer=serviceKey,key=serviceKey)=>request("/rest/v1/rpc/"+name,key,bearer,body);
 async function deliver(only=null){
  const claims=await rpc("push_dispatch_claim",{p_limit:only?1:50,p_only_job:only});
  if(claims?.version!=="device-push-claims-v1"||!Array.isArray(claims.items)||claims.items.length>50||typeof claims.capped!=="boolean")throw new SafeError();
  for(const job of claims.items)if(!uuid(job.job_id)||!uuid(job.lease_id)||!deviceToken(job.token))throw new SafeError();
  let at=0;const totals={claimed:claims.items.length,sent:0,pending:0,failed:0,unconfirmed:0,capped:claims.capped};
  async function next(){for(;;){const job=claims.items[at++];if(!job)return;let result;
   try{sender??=createFcmSender({account:JSON.parse(accountJson||"null"),projectId,fetch:fetcher});result=await sender.send(job.token)}catch{result="provider_auth"}
   if(!["sent","temporary","provider_auth","unregistered"].includes(result))result="temporary";
   try{const ack=await rpc("push_dispatch_finish",{p_job_id:job.job_id,p_lease_id:job.lease_id,p_result:result});
    if(!["sent","pending","failed"].includes(ack?.state))throw new SafeError();totals[ack.state==="sent"?"sent":ack.state==="failed"?"failed":"pending"]++;
   }catch{totals.unconfirmed++}
  }}
  await Promise.all(Array.from({length:Math.min(8,claims.items.length)},next));return totals;
 }
 return async requestIn=>{
  try{
   if(requestIn.method!=="POST")return reply({error:"POST required."},405);
   if(!requestIn.headers.get("content-type")?.toLowerCase().startsWith("application/json"))return reply({error:"JSON required."},415);
   let body;try{body=await responseJson(requestIn,8192)}catch{throw new SafeError(400)}if(!body||Array.isArray(body))throw new SafeError(400);
   const bearer=requestIn.headers.get("authorization")?.match(/^Bearer ([^\s]+)$/u)?.[1];if(!bearer||bearer.length>8192)throw new SafeError(401);
   if(body.operation==="worker"){
    if(Object.keys(body).some(k=>k!=="operation")||!await equalSecret(bearer,workerSecret))throw new SafeError(401);
    const scan=await rpc("push_worker_collect",{p_limit:100});if(scan?.version!=="device-push-worker-v1"||typeof scan.enabled!=="boolean")throw new SafeError();
    const result=scan.enabled?await deliver():{claimed:0,sent:0,pending:0,failed:0,unconfirmed:0,capped:false};
    return reply({version:"device-push-result-v1",enabled:scan.enabled,...result});
   }
   if(body.operation!=="test"||Object.keys(body).some(k=>!["operation","token","request_id"].includes(k))||!deviceToken(body.token)||!uuid(body.request_id))throw new SafeError(400);
   const user=await request("/auth/v1/user",anonKey,bearer);
   if(!uuid(user?.id)||user.is_anonymous===true)throw new SafeError(403);
   const own=await rpc("portal_push_test",{p_token:body.token,p_request_id:body.request_id},bearer,anonKey);
   if(own?.version!=="portal-device-push-v1"||own.user_id!==user.id||!uuid(own.company_id)||!uuid(own.employee_id)||!uuid(own.job_id)||!["pending","sending","sent","cancelled","failed"].includes(own.state))throw new SafeError(403);
   if(["sent","failed","cancelled"].includes(own.state))return reply({version:"device-push-test-v1",state:own.state});
   const result=await deliver(own.job_id);return reply({version:"device-push-test-v1",state:result.sent?"sent":result.failed?"failed":"queued"});
  }catch(error){const status=error instanceof SafeError?error.status:503;return reply({error:status===401?"Sign in or use the configured worker.":status===403?"Current device access is required.":status===429?"Try a test alert later.":status===400?"Invalid device request.":status===409?"Device request changed. Reopen device alerts.":"Device alerts are unavailable right now."},status)}
 }
}
