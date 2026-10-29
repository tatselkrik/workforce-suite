import {SafeError,responseJson} from "./http.js";
const encode=bytes=>btoa(String.fromCharCode(...bytes)).replaceAll("+","-").replaceAll("/","_").replace(/=+$/u,"");
const text64=s=>encode(new TextEncoder().encode(s));
export function createFcmSender({account,projectId,fetch:fetcher=globalThis.fetch,clock=Date.now,crypto=globalThis.crypto}){
 if(!account||account.type!=="service_account"||account.project_id!==projectId||!/^[-a-z0-9]{6,30}$/.test(projectId)||!account.client_email?.endsWith("@"+projectId+".iam.gserviceaccount.com")||typeof account.private_key!=="string")throw new SafeError(503,"provider_auth");
 let key,access,expiry=0,refreshing;
 async function refresh(){
  try{
   if(!key){const b64=account.private_key.replace(/-----BEGIN PRIVATE KEY-----|-----END PRIVATE KEY-----|\s/gu,"");const bytes=Uint8Array.from(atob(b64),c=>c.charCodeAt(0));key=await crypto.subtle.importKey("pkcs8",bytes,{name:"RSASSA-PKCS1-v1_5",hash:"SHA-256"},false,["sign"])}
   const now=Math.floor(clock()/1000);const header=text64(JSON.stringify({alg:"RS256",typ:"JWT",...(account.private_key_id?{kid:account.private_key_id}:{})}));
   const body=text64(JSON.stringify({iss:account.client_email,scope:"https://www.googleapis.com/auth/firebase.messaging",aud:"https://oauth2.googleapis.com/token",iat:now,exp:now+3600}));const unsigned=header+"."+body;
   const signature=await crypto.subtle.sign("RSASSA-PKCS1-v1_5",key,new TextEncoder().encode(unsigned));
   const response=await fetcher("https://oauth2.googleapis.com/token",{method:"POST",redirect:"error",signal:AbortSignal.timeout(10000),headers:{"Content-Type":"application/x-www-form-urlencoded"},body:new URLSearchParams({grant_type:"urn:ietf:params:oauth:grant-type:jwt-bearer",assertion:unsigned+"."+encode(new Uint8Array(signature))})});
   const value=await responseJson(response);if(!response.ok||typeof value.access_token!=="string"||value.access_token.length<10||value.token_type!=="Bearer"||!Number.isInteger(value.expires_in)||value.expires_in<120||value.expires_in>7200)throw new SafeError(503,"provider_auth");
   access=value.access_token;expiry=clock()+value.expires_in*1000;
  }catch{access=undefined;expiry=0;throw new SafeError(503,"provider_auth")}
 }
 async function token(){if(!access||expiry-clock()<60000){refreshing??=refresh().finally(()=>{refreshing=undefined});await refreshing}return access}
 return{async send(address){
  try{
   const bearer=await token();const response=await fetcher("https://fcm.googleapis.com/v1/projects/"+encodeURIComponent(projectId)+"/messages:send",{method:"POST",redirect:"error",signal:AbortSignal.timeout(10000),headers:{Authorization:"Bearer "+bearer,"Content-Type":"application/json"},body:JSON.stringify({message:{token:address,data:{hris_hint:"inbox"},android:{priority:"HIGH",ttl:"3600s",collapse_key:"hris-inbox"}}})});
   const body=await responseJson(response);
   if(response.ok&&typeof body.name==="string")return"sent";
   const reason=body?.error?.details?.find(x=>x?.["@type"]==="type.googleapis.com/google.firebase.fcm.v1.FcmError")?.errorCode;
   if(reason==="UNREGISTERED"||reason==="SENDER_ID_MISMATCH")return"unregistered";
   if(response.status===401||response.status===403){access=undefined;expiry=0;return"provider_auth"}
   return"temporary";
  }catch(error){return error instanceof SafeError&&error.kind==="provider_auth"?"provider_auth":"temporary"}
 }}
}
