import test from'node:test';import assert from'node:assert/strict';
import{createAccountHandler}from'../supabase/functions/account-admin/handler.mjs';
const id=n=>'20000000-0000-4000-8000-'+String(n).padStart(12,'0');
const owner=id(1),company=id(2),user=id(3),person=id(4),attempt=id(5);
const config={url:'https://company.example.invalid',serviceKey:'fictional-server-only-key',anonKey:'fictional-public-key'};
const access={user_id:owner,company_id:company,role:'administrator',staff_contract_version:1};
const request={id:attempt,email:'person@example.invalid',roles:['recruiter','learning_performance_officer'],active:true,reason:'Reviewed combined responsibilities',
 employee:{id:null,company_id:company,employee_code:'000004',full_name:'Fictional Staff',department:'Prototype',job_title:'Specialist',joined_on:null,annual_leave_days:15,status:'active',reports_to:null,employment_type_id:null,work_location_id:null},
 contacts:{company_id:company,employee_id:'00000000-0000-0000-0000-000000000000',phone:'000 123 4567',personal_email:'',address:'',emergency_name:'Fictional Contact',emergency_relationship:'Sibling',emergency_phone:'000 123 4568',emergency_alternate_phone:'',updated_at:null},expected_source:null,supersedes:null};
const body={operation:'create_staff_v2',request,password:'fictional initial password'};
const receipt={schema_version:1,id:attempt,company_id:company,actor_id:owner,user_id:user,employee_id:person,active:true,roles:request.roles,revision:1,base_role:'employee',member_updated_at:'2026-10-27T12:00:00Z'};
const intent={schema_version:1,id:attempt,company_id:company,actor_id:owner,user_id:user,auth_preexisting:false,email:request.email,state:'reserved',receipt:null};
const authUser={id:user,email:request.email,email_confirmed_at:null,user_metadata:{role:'administrator'}};
const authenticated=(...tail)=>[{body:{id:owner}},{body:access},...tail];
const req=b=>new Request(config.url+'/functions/v1/account-admin',{method:'POST',headers:{Authorization:'Bearer fictional-actor-token','Content-Type':'application/json'},body:JSON.stringify(b)});
function fixture(replies){const requests=[];const handler=createAccountHandler(config,async(url,options)=>{requests.push({url,...options,json:options.body?JSON.parse(options.body):null});const r=replies.shift();assert(r,'Unexpected upstream request');if(r instanceof Error)throw r;return Response.json(r.body??{}, {status:r.status??200});});return{handler,requests};}
const fresh=()=>authenticated({body:intent},{status:404},{body:authUser},{body:receipt});
const protectedCalls=requests=>{for(const r of requests.filter(r=>r.url.includes('/rest/v1/'))){assert.equal(r.headers.Authorization,'Bearer fictional-actor-token');assert(!JSON.stringify(r.json).includes(body.password));}assert(requests.every(r=>r.method!=='DELETE'&&r.method!=='PUT'&&r.redirect==='error'));assert(requests.every(r=>!r.url.endsWith('/resend')));};

test('versioned creation uses reserved Auth UUID and one atomic caller-JWT commit without sending mail',async()=>{
 const{handler,requests}=fixture(fresh());const response=await handler(req(body));assert.equal(response.status,201);const text=await response.text();assert.deepEqual(JSON.parse(text).receipt,receipt);
 assert.equal(requests.length,6);assert.deepEqual(requests[2].json,{p_request:request});assert.equal(requests[3].url,config.url+'/auth/v1/admin/users/'+user);
 assert.deepEqual(requests[4].json,{id:user,email:request.email,password:body.password,email_confirm:false,user_metadata:{hris_profile:{full_name:'Fictional Staff'}}});
 assert.equal(requests[4].headers.Authorization,'Bearer '+config.serviceKey);assert.deepEqual(requests[5].json,{p_request:request});
 assert(!text.includes(body.password));assert(!text.includes(config.serviceKey));assert(!text.includes('fictional-actor-token'));assert.equal(JSON.parse(text).confirmation_sent,false);protectedCalls(requests);
});
test('completed exact retry returns its frozen receipt before any privileged Auth access',async()=>{
 const{handler,requests}=fixture(authenticated({body:{...intent,state:'committed',receipt}}));const response=await handler(req(body));assert.equal(response.status,200);assert.equal((await response.json()).replayed,true);assert.equal(requests.length,3);protectedCalls(requests);
});
test('previously created unconfirmed identity resumes without an initial password or password reset',async()=>{
 const{handler,requests}=fixture(authenticated({body:intent},{body:authUser},{body:receipt}));const{password,...resume}=body;
 const response=await handler(req(resume));assert.equal(response.status,201);assert.equal(requests.length,5);assert(!requests.some(r=>r.url.endsWith('/admin/users')));protectedCalls(requests);
});
test('unknown Auth creation delivery keeps its reservation and retry recovers the same identity',async()=>{
 const first=fixture(authenticated({body:intent},{status:404},new Error('Fictional unknown Auth delivery')));const original=structuredClone(body);assert.equal((await first.handler(req(body))).status,502);
 const again=fixture(authenticated({body:intent},{body:authUser},{body:receipt}));assert.equal((await again.handler(req(body))).status,201);assert.equal(first.requests[4].json.id,user);assert.equal(again.requests[3].url,config.url+'/auth/v1/admin/users/'+user);assert.deepEqual(body,original);protectedCalls(first.requests);protectedCalls(again.requests);
});
test('concurrent exact Auth creation rechecks only its reserved UUID and commits once',async()=>{
 const{handler,requests}=fixture(authenticated({body:intent},{status:404},{status:409,body:{error_code:'user_already_exists'}},{body:authUser},{body:receipt}));assert.equal((await handler(req(body))).status,201);assert.equal(requests.length,7);assert.equal(requests[5].url,config.url+'/auth/v1/admin/users/'+user);protectedCalls(requests);
});
test('email conflict never attaches or deletes a different existing Auth identity',async()=>{
 const{handler,requests}=fixture(authenticated({body:intent},{status:404},{status:409,body:{error_code:'email_exists'}},{status:404}));assert.equal((await handler(req(body))).status,409);assert.equal(requests.length,6);protectedCalls(requests);
});
test('confirmed database source rejection requests linked review and retains the Auth identity',async()=>{
 const{handler,requests}=fixture(authenticated({body:intent},{body:authUser},{status:400,body:{code:'23514',message:'Fictional stale employee'}}));const response=await handler(req(body));assert.equal(response.status,409);assert.deepEqual(await response.json(),{message:'Employment or access details changed. Review a linked replacement attempt; the employee and roles were not partially saved.',provision_state:'needs_review',intent_id:attempt});protectedCalls(requests);
});
for(const status of[403,500])test('commit status '+status+' retains Auth and exposes no private upstream error',async()=>{
 const{handler,requests}=fixture(authenticated({body:intent},{body:authUser},{status,body:{message:'private upstream details'}}));const response=await handler(req(body));assert.equal(response.status,status===403?403:502);assert(!(await response.text()).includes('private upstream details'));protectedCalls(requests);
});
for(const patch of[{company_id:id(99)},{actor_id:id(99)},{id:id(99)},{email:'different@example.invalid'},{schema_version:2},{state:'unknown'}])test('foreign or unsupported reserved intent is rejected before using server key: '+Object.keys(patch)[0],async()=>{
 const{handler,requests}=fixture(authenticated({body:{...intent,...patch}}));assert.equal((await handler(req(body))).status,502);assert.equal(requests.length,3);assert(requests.every(r=>r.headers.apikey!==config.serviceKey));
});
for(const patch of[{id:id(99)},{email:'different@example.invalid'}])test('Auth lookup cannot substitute another identity: '+Object.keys(patch)[0],async()=>{
 const{handler,requests}=fixture(authenticated({body:intent},{body:{...authUser,...patch}}));assert.equal((await handler(req(body))).status,502);assert.equal(requests.length,4);protectedCalls(requests);
});
for(const patch of[{actor_id:id(99)},{active:false},{roles:['administrator']},{revision:2},{employee_id:'00000000-0000-0000-0000-000000000000'},{unexpected_token:'must not leave server'}])test('corrupt commit receipt is never forwarded: '+Object.keys(patch)[0],async()=>{
 const{handler,requests}=fixture(authenticated({body:intent},{body:authUser},{body:{...receipt,...patch}}));const response=await handler(req(body));assert.equal(response.status,502);assert(!(await response.text()).includes('must not leave server'));protectedCalls(requests);
});
for(const patch of[{roles:['employee','recruiter']},{roles:['recruiter','recruiter']},{roles:['learning_officer']},{roles:[]},{reason:''},{active:'true'},{password:'unexpected persisted credential'}])test('invalid credential-free request fails before reservation: '+Object.keys(patch)[0],async()=>{
 const{handler,requests}=fixture(authenticated());assert.equal((await handler(req({...body,request:{...request,...patch}}))).status,400);assert.equal(requests.length,2);
});
for(const roles of[['hr_generalist'],['manager','payroll_officer'],['employee'],['hr_generalist','recruiter','learning_performance_officer','payroll_officer']])test('role combination uses its narrow compatible base: '+roles.join(','),async()=>{
 const canonical=['administrator','hr_generalist','recruiter','learning_performance_officer','payroll_officer','manager','employee'].filter(r=>roles.includes(r));const base=roles.length===4?'hr_generalist':roles.includes('manager')?'manager':'employee';
 const{handler,requests}=fixture(authenticated({body:intent},{body:authUser},{body:{...receipt,roles:canonical,base_role:base}}));assert.equal((await handler(req({...body,request:{...request,roles}}))).status,201);protectedCalls(requests);
});
test('missing password is allowed for recovery but cannot create a missing Auth identity',async()=>{
 const{handler,requests}=fixture(authenticated({body:intent},{status:404}));const{password,...resume}=body;assert.equal((await handler(req(resume))).status,400);assert.equal(requests.length,4);protectedCalls(requests);
});
test('unverified contract and server-side reservation denial stop before privileged calls',async()=>{
 for(const version of[0,2]){const{handler,requests}=fixture([{body:{id:owner}},{body:{...access,staff_contract_version:version}}]);assert.equal((await handler(req(body))).status,409);assert.equal(requests.length,2);}
 const{handler,requests}=fixture(authenticated({status:403}));assert.equal((await handler(req(body))).status,403);assert.equal(requests.length,3);assert(requests.every(r=>r.headers.apikey!==config.serviceKey));
});
for(const state of[null,'reserved','cancelled','superseded'])test('confirmed prepare rejection resolves prior status before releasing a retry: '+state,async()=>{
 const status=state?{...intent,state}:null;const{handler,requests}=fixture(authenticated({status:400,body:{code:'23514'}},{body:status}));
 // The generic fixture's nullish body defaults to {}; preserve an explicit JSON null here.
 const actual=state?handler:createAccountHandler(config,async(url,options)=>{
  requests.push({url,...options,json:options.body?JSON.parse(options.body):null});
  if(url.endsWith('/auth/v1/user'))return Response.json({id:owner});if(url.endsWith('/my_hris_access'))return Response.json(access);
  if(url.endsWith('/begin_staff_provision'))return Response.json({code:'23514'},{status:400});return Response.json(null);
 });
 const response=await actual(req(body));assert.equal(response.status,409);assert.equal((await response.json()).provision_state,state==='superseded'?'closed':state?'needs_review':'not_started');assert.equal(requests.length,4);assert(requests.every(r=>r.headers.apikey!==config.serviceKey));
});
test('temporary reservation error keeps uncertain identity without claiming a rejection',async()=>{
 const{handler,requests}=fixture(authenticated({status:503}));const response=await handler(req(body));assert.equal(response.status,502);assert.equal((await response.json()).provision_state,undefined);assert.equal(requests.length,3);
});

test('malformed reservation status is uncertain rather than a confirmed missing reservation',async()=>{
 const requests=[];const handler=createAccountHandler(config,async(url,options)=>{
  requests.push({url,...options,json:options.body?JSON.parse(options.body):null});
  if(url.endsWith('/auth/v1/user'))return Response.json({id:owner});if(url.endsWith('/my_hris_access'))return Response.json(access);
  if(url.endsWith('/begin_staff_provision'))return Response.json({code:'23514'},{status:400});return new Response('invalid JSON',{status:200});
 });
 const response=await handler(req(body));assert.equal(response.status,502);assert.equal((await response.json()).provision_state,undefined);assert.equal(requests.length,4);assert(requests.every(r=>r.headers.apikey!==config.serviceKey));
});
test('an adopted existing sign-in identity is never recreated after deletion',async()=>{
 const{handler,requests}=fixture(authenticated({body:{...intent,auth_preexisting:true}},{status:404}));const response=await handler(req(body));assert.equal(response.status,409);assert.equal((await response.json()).provision_state,'needs_review');assert.equal(requests.length,4);assert(requests.every(r=>!r.url.endsWith('/admin/users')));protectedCalls(requests);
});
