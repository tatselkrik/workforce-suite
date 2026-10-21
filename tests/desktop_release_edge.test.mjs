import test from 'node:test';
import assert from 'node:assert/strict';
import {generateKeyPairSync,sign,createHash,webcrypto} from 'node:crypto';
import {createReleaseHandler} from '../supabase/functions/desktop-release/handler.mjs';
const key=generateKeyPairSync('ed25519');
const publicKey=key.publicKey.export({format:'der',type:'spki'}).subarray(-32).toString('base64');
const id='00000000-0000-0000-0000-000000000901';
const bytes=Buffer.from('MZsynthetic release test');
const data={application_id:'io.github.tatselkrik.hrisdesktop',version:'0.1.6',platform:'windows-x64',requires_schema:'20261021054356',
  artifact:'0.1.6/HRIS-Desktop-Update-0.1.6.exe',sha256:createHash('sha256').update(bytes).digest('hex'),
  executable_sha256:'a'.repeat(64),size:bytes.length,notes:'Synthetic release test'};
function envelope(value=data) {
 const payload=Buffer.from(JSON.stringify(value));return {payload:payload.toString('base64'),signature:sign(null,payload,key.privateKey).toString('base64')};
}
function req(signed=envelope(),body=bytes,extras={}) {
 return new Request('https://function.example.invalid',{method:'POST',headers:{Authorization:'Bearer actor-token',
  'Content-Type':'application/octet-stream','X-HRIS-Manifest':Buffer.from(JSON.stringify(signed)).toString('base64'),...extras},body});
}
function fixture(overrides={}) {
 const requests=[];
 const fetcher=async(url,init)=>{
  requests.push({url,init});const path=new URL(url).pathname;
  if(path==='/auth/v1/user')return Response.json({id});
  if(path.endsWith('/my_hris_access'))return Response.json({user_id:id,role:overrides.role||'administrator'});
  if(path.endsWith('/desktop_update'))return Response.json({schema_version:overrides.schema||'20261021054356',release:null});
  if(path==='/rest/v1/desktop_releases' && init.method==='GET') return Response.json(overrides.existing||[]);
  if(path.startsWith('/storage/'))return new Response(null,{status:overrides.storageStatus||200});
  return new Response(null,{status:201});
 };
 return {requests,handler:createReleaseHandler({url:'https://project.example.invalid',serviceKey:'synthetic-server-key',anonKey:'synthetic-client-key',publicKey},fetcher,webcrypto)};
}
test('signed publication stores an immutable package before enabling its catalog entry',async()=>{
 const{handler,requests}=fixture();const response=await handler(req());assert.equal(response.status,201);
 assert.equal((await response.json()).published,true);
 const privileged=requests.filter(r=>r.init.headers.apikey==='synthetic-server-key');
 assert.equal(privileged.length,4);
 assert.equal(JSON.parse(privileged[1].init.body).enabled,false);
 assert.equal(privileged[2].init.headers['x-upsert'],'false');
 assert.deepEqual(privileged[2].init.body,new Uint8Array(bytes));
 assert.equal(JSON.parse(privileged[3].init.body).enabled,true);
 assert.ok(requests.slice(0,3).every(r=>r.init.headers.Authorization==='Bearer actor-token'));
});
for(const role of ['hr_generalist','manager','employee'])test(`${role} cannot publish even with a valid signature`,async()=>{
 const{handler,requests}=fixture({role});assert.equal((await handler(req())).status,403);
 assert.ok(requests.every(r=>r.init.headers.apikey!=='synthetic-server-key'));
});
test('tampered signature, other app/path/platform and future schema cannot upload',async()=>{
 for(const field of ['signature','application_id','platform','artifact','requires_schema']) {
  const{handler,requests}=fixture();let signed;
  if(field==='signature'){signed=envelope();signed.signature=Buffer.alloc(64).toString('base64');}
  else signed=envelope({...data,[field]:field==='requires_schema'?'99990101000000':'wrong-value'});
  assert.ok([400,409].includes((await handler(req(signed))).status));
  assert.ok(requests.every(r=>r.init.headers.apikey!=='synthetic-server-key'));
 }
});
test('oversized, truncated and modified downloads remain unpublished',async()=>{
 for(const body of [Buffer.concat([bytes,Buffer.from('extra')]),bytes.subarray(1),Buffer.from('MZchanged release bytes!')]) {
  const{handler,requests}=fixture();assert.ok([400,413].includes((await handler(req(envelope(),body))).status));
  assert.ok(requests.every(r=>r.init.headers.apikey!=='synthetic-server-key'));
 }
});
test('existing release contents cannot be replaced and storage failure keeps catalog disabled',async()=>{
 const conflict=fixture({existing:[{envelope:envelope({...data,notes:'different'}),enabled:true}]});
 assert.equal((await conflict.handler(req())).status,409);
 const failed=fixture({storageStatus:500});assert.equal((await failed.handler(req())).status,502);
 assert.ok(!failed.requests.some(r=>r.init.method==='PATCH'));
});
test('anonymous and browser requests never reach upstream services',async()=>{
 const{handler,requests}=fixture();
 assert.equal((await handler(new Request('https://function.example.invalid',{method:'POST',headers:{'Content-Type':'application/octet-stream'}}))).status,401);
 assert.equal((await handler(req(envelope(),bytes,{Origin:'https://browser.example.invalid'}))).status,403);
 assert.equal(requests.length,0);
});
