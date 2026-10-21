import test from 'node:test';
import assert from 'node:assert/strict';
import {createAccountHandler} from '../supabase/functions/account-admin/handler.mjs';

const owner='00000000-0000-0000-0000-000000000001';
const user='00000000-0000-0000-0000-000000000002';
const person='00000000-0000-0000-0000-000000000003';
const config={url:'https://company.example.invalid',serviceKey:'synthetic-server-secret',anonKey:'synthetic-anon'};
const create={operation:'create',email:'person@example.invalid',password:'synthetic initial password',role:'employee',employee_id:person};
const identity={id:owner};
const access={user_id:owner,role:'administrator'};
const req=(body=create,headers={})=>new Request('https://company.example.invalid/functions/v1/account-admin',{
  method:'POST',headers:{Authorization:'Bearer synthetic-user-token','Content-Type':'application/json',...headers},body:JSON.stringify(body),
});
function fixture(replies) {
  const requests=[];
  const handler=createAccountHandler(config,async(url,options)=>{
    requests.push({url, ...options, json:options.body?JSON.parse(options.body):null});
    const next=replies.shift();
    assert.ok(next,'Unexpected upstream request');
    if(next instanceof Error)throw next;
    return Response.json(next.body??{}, {status:next.status??200});
  });
  return {handler,requests};
}
const authenticated=(...tail)=>[{body:identity},{body:access},...tail];

test('new account is unconfirmed; protected assignment uses caller JWT and no credentials leave the response',async()=>{
  const {handler,requests}=fixture(authenticated({body:{id:user}},{body:user},{body:{}}));
  const response=await handler(req());
  assert.equal(response.status,201);
  const text=await response.text();
  assert.equal(JSON.parse(text).confirmation_sent,true);
  for(const secret of [config.serviceKey,create.password,'synthetic-user-token'])assert.ok(!text.includes(secret));
  assert.equal(requests.length,5);
  assert.equal(requests[2].url,config.url+'/auth/v1/admin/users');
  assert.deepEqual(requests[2].json,{email:create.email,password:create.password,email_confirm:false});
  assert.equal(requests[2].headers.Authorization,'Bearer '+config.serviceKey);
  for(const index of [0,1,3,4]) {
    assert.equal(requests[index].headers.Authorization,'Bearer synthetic-user-token');
    assert.equal(requests[index].headers.apikey,config.anonKey);
    assert.equal(requests[index].redirect,'error');
  }
  assert.equal(requests[3].json.p_user_id,user);
  assert.equal(requests[3].json.p_role,'employee');
  assert.equal(requests[3].json.p_active,true);
  assert.deepEqual(requests[4].json,{type:'signup',email:create.email});
  assert.equal(response.headers.get('Cache-Control'),'no-store');
});

test('new disabled account retains disabled access without requiring an employee link',async()=>{
  const {handler,requests}=fixture(authenticated({body:{id:user}},{body:user},{body:{}}));
  const response=await handler(req({...create,active:false,employee_id:null}));
  assert.equal(response.status,201);
  assert.equal(requests[3].json.p_active,false);
  assert.equal(requests[3].json.p_employee_id,null);
});

for(const role of ['hr_generalist','manager','employee'])test(role+' cannot provision accounts',async()=>{
  const {handler,requests}=fixture([{body:identity},{body:{...access,role}}]);
  assert.equal((await handler(req())).status,403);
  assert.equal(requests.length,2);
  assert.ok(requests.every(r=>r.headers.apikey!==config.serviceKey));
});
test('revoked membership cannot use provisioning even with a valid Auth bearer',async()=>{
  const {handler,requests}=fixture([{body:identity},{status:403}]);
  assert.equal((await handler(req())).status,403);assert.equal(requests.length,2);
});
test('unverified forged bearer fails before querying privileges',async()=>{
  const {handler,requests}=fixture([{status:401}]);
  assert.equal((await handler(req())).status,401);assert.equal(requests.length,1);
});
test('mismatched access subject fails closed',async()=>{
  const {handler}=fixture([{body:identity},{body:{...access,user_id:user}}]);
  assert.equal((await handler(req())).status,403);
});
test('failed assignment cleans up only the new Auth identity',async()=>{
  const {handler,requests}=fixture(authenticated({body:{id:user}},{status:403},{body:{}}));
  assert.equal((await handler(req())).status,409);
  assert.equal(requests.at(-1).url,config.url+'/auth/v1/admin/users/'+user);
  assert.equal(requests.at(-1).method,'DELETE');
});
test('an existing Auth identity is not deleted or implicitly granted',async()=>{
  const {handler,requests}=fixture(authenticated({status:422,body:{error_code:'email_exists'}}));
  assert.equal((await handler(req())).status,409);assert.equal(requests.length,3);
  assert.ok(!requests.some(r=>r.method==='DELETE'));
});
test('email rate limiting preserves created membership for later resend',async()=>{
  const {handler,requests}=fixture(authenticated({body:{id:user}},{body:user},{status:429}));
  const response=await handler(req());assert.equal(response.status,201);
  assert.equal((await response.json()).confirmation_sent,false);
  assert.ok(!requests.some(r=>r.method==='DELETE'));
});
test('email transport exception after assignment also preserves new account',async()=>{
  const {handler,requests}=fixture(authenticated({body:{id:user}},{body:user},new Error('synthetic transport failure')));
  const response=await handler(req());assert.equal(response.status,201);
  assert.equal((await response.json()).confirmation_sent,false);
  assert.ok(!requests.some(r=>r.method==='DELETE'));
});
test('resend uses the stored member email rather than client-supplied email',async()=>{
  const {handler,requests}=fixture(authenticated({body:[{user_id:user,email:create.email,active:true,email_confirmed:false}]},{body:{}}));
  assert.equal((await handler(req({operation:'resend',user_id:user,email:'injected@example.invalid'}))).status,200);
  assert.equal(requests.at(-1).json.email,create.email);
});
test('resend cannot target an unrelated account',async()=>{
  const {handler,requests}=fixture(authenticated({body:[]}));
  assert.equal((await handler(req({operation:'resend',user_id:user}))).status,400);
  assert.equal(requests.length,3);
});
for(const change of [{role:'root'},{active:'false'},{password:'short'},{employee_id:null},{manager_user_id:'not-a-uuid'},{email:'invalid'}]) {
  test('invalid provisioning input is rejected before using the server credential: '+Object.keys(change)[0],async()=>{
    const {handler,requests}=fixture(authenticated());
    assert.equal((await handler(req({...create,...change}))).status,400);
    assert.equal(requests.length,2);
  });
}
test('oversized body is rejected without any upstream request',async()=>{
  const {handler,requests}=fixture([]);
  assert.equal((await handler(req({...create,extra:'x'.repeat(9000)}))).status,413);assert.equal(requests.length,0);
});
test('missing credentials and browser origins cannot reach provisioning',async()=>{
  const {handler,requests}=fixture([]);
  assert.equal((await handler(req(create,{Authorization:''}))).status,401);
  assert.equal((await handler(req(create,{Origin:'https://untrusted.example.invalid'}))).status,403);
  assert.equal(requests.length,0);
});
