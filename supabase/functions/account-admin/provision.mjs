// Versioned native provisioning. Passwords/tokens stay in request memory only.
const roleOrder=['administrator','hr_generalist','recruiter','learning_performance_officer','payroll_officer','manager','employee'];
const uuid=v=>typeof v==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v)&&v!=='00000000-0000-0000-0000-000000000000';
const canonicalRoles=roles=>{
 if(!Array.isArray(roles)||roles.length<1||roles.length>6||new Set(roles).size!==roles.length||roles.some(r=>!roleOrder.includes(r))||(roles.includes('employee')&&roles.length!==1))return null;
 return roleOrder.filter(r=>roles.includes(r));
};
const compatibleBase=roles=>roles.includes('administrator')?'administrator':
 ['hr_generalist','recruiter','learning_performance_officer','payroll_officer'].every(r=>roles.includes(r))?'hr_generalist':roles.includes('manager')?'manager':'employee';
const only=(value,keys)=>value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).every(k=>keys.includes(k));
const validReceipt=(r,request,intent,access)=>{
 const expected=canonicalRoles(request.roles);
 return only(r,['schema_version','id','company_id','actor_id','user_id','employee_id','active','roles','revision','base_role','member_updated_at'])&&
 r.schema_version===1&&r.id===request.id&&r.company_id===access.company_id&&r.actor_id===access.user_id&&r.user_id===intent.user_id&&
 uuid(r.employee_id)&&(request.employee.id==null||r.employee_id===request.employee.id)&&r.active===request.active&&r.revision===1&&
 JSON.stringify(r.roles)===JSON.stringify(expected)&&r.base_role===compatibleBase(expected)&&typeof r.member_updated_at==='string'&&Number.isFinite(Date.parse(r.member_updated_at));
};
const validIntent=(i,request,access)=>only(i,['schema_version','id','company_id','actor_id','user_id','email','state','receipt'])&&i.schema_version===1&&
 i.id===request.id&&i.company_id===access.company_id&&i.actor_id===access.user_id&&uuid(i.user_id)&&i.email===request.email.trim().toLowerCase()&&
 ['reserved','committed'].includes(i.state)&&(i.state!=='committed'||validReceipt(i.receipt,request,i,access));
const validUser=(user,intent)=>user&&user.id===intent.user_id&&typeof user.email==='string'&&user.email.toLowerCase()===intent.email;
const json=async response=>response.json().catch(()=>null);

export async function provisionStaff({body,access,call,answer}){
 const request=body.request;
 if(!only(body,['operation','request','password'])||!only(request,['id','email','roles','active','reason','employee','contacts','expected_source','supersedes'])||
 !uuid(request.id)||typeof request.email!=='string'||request.email.length>254||!canonicalRoles(request.roles)||typeof request.active!=='boolean'||
 typeof request.reason!=='string'||request.reason!==request.reason.trim()||[...request.reason].length<1||[...request.reason].length>500||/[\u0000-\u001f\u007f]/u.test(request.reason)||
 !request.employee||typeof request.employee!=='object'||Array.isArray(request.employee)||request.employee.company_id!==access.company_id||
 typeof request.employee.full_name!=='string'||!request.contacts||request.contacts.company_id!==access.company_id||
 (body.password!==undefined&&(typeof body.password!=='string'||[...body.password].length<12||[...body.password].length>128))){
  return answer(400,'Enter valid reviewed staff, employment and contact details. Use a12–128character initial password when one is needed.');
 }
 if(access.staff_contract_version!==1||!uuid(access.company_id))return answer(409,'Install the verified Account Manager contract before creating staff.');
 try{
  // The caller, never the service key, reserves and commits this immutable request.
  const reserved=await call('/rest/v1/rpc/begin_staff_provision','POST',{p_request:request});
  if(!reserved.ok)return answer([401,403].includes(reserved.status)?403:409,'The account attempt could not be reserved. Refresh and review the original request.');
  const intent=await json(reserved);
  if(!validIntent(intent,request,access))return answer(502,'The reserved account response could not be verified. Retain this exact request.');
  if(intent.state==='committed')return answer(200,'Employee and staff access were already saved.',{receipt:intent.receipt,confirmation_sent:false,replayed:true});
  // A fixed server-reserved UUID makes unknown Auth delivery recoverable after restart.
  const lookup=await call('/auth/v1/admin/users/'+intent.user_id,'GET',undefined,true);
  let user;
  if(lookup.ok){user=await json(lookup);if(!validUser(user,intent))return answer(502,'The reserved Auth identity does not match. Retain this request for review.');}
  else if(lookup.status!==404)return answer(503,'The reserved Auth identity could not be checked. Retry the same request later.');
  else{
   if(body.password===undefined)return answer(400,'An initial password is needed because the reserved Auth identity has not been created.');
   const created=await call('/auth/v1/admin/users','POST',{id:intent.user_id,email:intent.email,password:body.password,email_confirm:false,
    user_metadata:{hris_profile:{full_name:request.employee.full_name}}},true);
   if(created.ok)user=await json(created);
   else{
    // Concurrent exact attempts may race at Auth. Never reset/delete any identity.
    const failure=await json(created);const recovered=await call('/auth/v1/admin/users/'+intent.user_id,'GET',undefined,true);
    if(recovered.ok)user=await json(recovered);
    else return answer(['email_exists','user_already_exists'].includes(failure?.error_code)?409:502,'Auth creation could not be confirmed. Keep this attempt; do not create a different identity to retry.');
   }
   if(!validUser(user,intent))return answer(502,'Auth creation returned another identity. Keep this original attempt for review.');
  }
  const committed=await call('/rest/v1/rpc/commit_staff_provision','POST',{p_request:request});
  if(!committed.ok){
   const failure=await json(committed);
   if([401,403].includes(committed.status))return answer(403,'Your Administrator access changed. The reserved identity was retained without deleting it.');
   if(failure?.code==='23514')return answer(409,'Employment or access details changed. Review a linked replacement attempt; the employee and roles were not partially saved.',{provision_state:'needs_review',intent_id:request.id});
   return answer(502,'The commit outcome is unresolved. Retry this exact request to recover its recorded receipt.');
  }
  const receipt=await json(committed);
  if(!validReceipt(receipt,request,intent,access))return answer(502,'The saved account receipt could not be verified. Keep the exact retry.');
  // Confirmation is an explicit Administrator action, avoiding duplicate mail on retries.
  return answer(201,'Employee and staff access saved. Use Send confirmation to deliver account setup.',{receipt,confirmation_sent:false,replayed:false});
 }catch{
  // There is deliberately no Auth DELETE or password UPDATE on any uncertain path.
  return answer(502,'Account delivery is uncertain. Retry this same attempt; its reserved identity and any recorded result are retained.');
 }
}