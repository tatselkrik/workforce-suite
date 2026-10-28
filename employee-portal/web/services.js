// Own request drafts and exact retry payloads live only in this page's memory.
const serviceState={data:null,selected:null,form:null,pending:null,rejected:false,dirty:false};
const serviceStatuses={submitted:'Submitted',in_progress:'In progress',waiting_on_employee:'Waiting on employee',completed:'Completed',cancelled:'Cancelled'};
const serviceCategories={employment_document:'Employment document',information_update:'Employee information update',benefits_question:'Benefits question',general_support:'General HR support'};
function resetServices(){
 serviceState.data=null;serviceState.selected=null;serviceState.form=null;serviceState.pending=null;serviceState.rejected=false;serviceState.dirty=false;
 $('services-form').reset();$('services-form').hidden=true;$('services-detail').hidden=true;
 $('services-list').replaceChildren();$('services-detail').replaceChildren();$('services-summary').textContent='';$('services-save-status').textContent='';$('services-form-context').textContent='';serviceGuards();
}
function serviceGuards(){
 const f=serviceState.form,p=serviceState.pending;
 const current=f&&serviceState.data?.requests.find(t=>t.id===f.request_id);
 const stale=f&&f.kind!=='submit'&&current&&current.revision!==f.revision;
 $('services-fields').disabled=state.busy||!!p;
 for(const id of['services-title','services-category','services-priority'])$(id).disabled=state.busy||!!p||f?.kind!=='submit';
 blocked($('services-new'),!serviceState.data||!!f||state.busy);
 blocked($('services-submit'),!f||state.busy||serviceState.rejected||(!p&&!!stale));
 blocked($('services-discard'),state.busy||(!!p&&!serviceState.rejected));
 blocked($('services-refresh'),state.busy);
 $('services-review').hidden=!(f&&(serviceState.rejected||(!p&&stale)));
 blocked($('services-review'),state.busy||(!serviceState.rejected&&!!p));
 $('services-submit').textContent=p?'Retry same save':f?.kind==='reply'?'Send reply':f?.kind==='cancel'?'Cancel request':'Submit request';
 $('services-save-status').textContent=p?(serviceState.rejected?'The save was rejected. Refresh, then review the current version before preparing another save.':'Awaiting confirmation. Retry this exact save; its identity and message stay fixed.'):(stale?'This request changed. Review the current version before saving.':'Your message is shared with you and current handling staff.');
}
function openServiceForm(kind,ticket){
 if(state.busy)return;
 if(serviceState.dirty){notify('Save or discard the current service draft first.',true);return;}
 serviceState.form={kind,request_id:ticket?.id||crypto.randomUUID(),revision:ticket?.revision||0};
 serviceState.pending=null;serviceState.rejected=false;serviceState.dirty=true;
 $('services-form').reset();$('services-form').hidden=false;$('services-new-fields').hidden=kind!=='submit';
 $('services-title').required=kind==='submit';
 $('services-form-heading').textContent=kind==='reply'?'Reply to request':kind==='cancel'?'Cancel service request':'New service request';
 $('services-form-context').textContent=ticket?`${ticket.title} · ${serviceStatuses[ticket.status]} · Revision ${ticket.revision}`:'Your own employee request. HR assigns the handling staff.';
 serviceGuards();$('services-body').focus();
}
function renderServices(){
 const data=serviceState.data,list=$('services-list'),detail=$('services-detail');list.replaceChildren();detail.replaceChildren();detail.hidden=true;
 if(!data){list.append(node('p','empty','Open this page or refresh to load your service requests.'));serviceGuards();return;}
 const active=data.requests.filter(t=>!['completed','cancelled'].includes(t.status)).length;
 $('services-summary').textContent=`${data.requests.length} requests · ${active} active`;
 for(const t of data.requests){
  const card=node('article','card');card.append(node('h2','',t.title),node('p','',`${serviceStatuses[t.status]} · ${serviceCategories[t.category]}`),node('p','fine',`${t.code} · Revision ${t.revision}`));
  const open=node('button','secondary','View request');open.onclick=()=>{if(state.busy)return;if(serviceState.dirty){notify('Save or discard your service draft before opening another request.',true);return;}serviceState.selected=t.id;renderServices();};card.append(open);list.append(card);
 }
 if(!data.requests.length)list.append(node('p','empty','No service requests yet. Use New service request to ask HR for help.'));
 const t=data.requests.find(t=>t.id===serviceState.selected);
 if(t){
  detail.hidden=false;detail.append(node('h2','',t.title),node('p','',`${serviceStatuses[t.status]} · ${serviceCategories[t.category]}`),node('p','',`Handler: ${t.handler_name||'HR service queue'}`),node('p','fine',`${t.code} · Revision ${t.revision}${t.due_on?' · Due '+day(t.due_on):''}`));
  const actions=node('div','actions');
  if(t.can_reply){const reply=node('button','primary','Reply');reply.onclick=()=>openServiceForm('reply',t);actions.append(reply);}
  if(t.can_cancel){const cancel=node('button','secondary','Cancel request');cancel.onclick=()=>openServiceForm('cancel',t);actions.append(cancel);}
  detail.append(actions,node('h3','','Shared request history'));
  for(const m of t.messages){const item=node('div','service-message');item.append(node('strong','',`${m.actor_name} · ${m.kind==='submit'?'Submitted':m.kind==='reply'?'Reply':m.kind==='route'?'Routed':m.kind==='cancel'?'Cancelled':'Status updated'}`),node('p','fine',`Revision ${m.revision} · ${day(m.created_at)} · ${serviceStatuses[m.status]}`),node('p','service-body',m.body));detail.append(item);}
 }
 serviceGuards();
}
async function refreshServices(){
 try{const data=await api('/api/services');if(data.schema_version!==1||!Array.isArray(data.requests))throw Error('The service request view could not be verified.');serviceState.data=data;renderServices();}
 catch(e){if(e.status===401||e.status===403)resetServices();throw e;}
}
function servicePayload(){
 if(serviceState.pending)return serviceState.pending;
 const f=serviceState.form;if(!f)throw Error('Open a service request form first.');
 const body=$('services-body').value.trim();if(Array.from(body).length<3||Array.from(body).length>12000)throw Error('Enter a shared message of 3–12,000 characters.');
 const change={kind:f.kind,body};if(f.kind==='submit'){change.title=$('services-title').value.trim();change.category=$('services-category').value;change.priority=$('services-priority').value;if(change.title.length<3)throw Error('Enter a request title of at least three characters.');}
 const payload={id:crypto.randomUUID(),request_id:f.request_id,expected_revision:f.kind==='submit'?0:f.revision,change};
 if(new TextEncoder().encode(JSON.stringify(payload)).length>23000)throw Error('The message exceeds this request’s byte limit. Shorten it before saving.');
 Object.freeze(change);serviceState.pending=Object.freeze(payload);return serviceState.pending;
}
async function saveService(){
 const payload=servicePayload();serviceGuards();
 try{
  const r=await api('/api/services/save',payload);
  if(r.schema_version!==1||r.id!==payload.id||r.request_id!==payload.request_id||r.revision!==payload.expected_revision+1)throw Error('The response did not confirm this exact save. Retry the same save.');
  serviceState.selected=r.request_id;serviceState.form=null;serviceState.pending=null;serviceState.rejected=false;serviceState.dirty=false;$('services-form').reset();$('services-form').hidden=true;
  try{await refreshServices();}catch{throw Error('Your request was saved. Refresh requests to see the current history.');}
  notify('Your service request was saved with its shared history.');
 }catch(e){if([400,409,413,422].includes(e.status))serviceState.rejected=true;if([401,403].includes(e.status))resetServices();serviceGuards();throw e;}
}
function reviewService(){
 if(!serviceState.form||!serviceState.data)return;
 if(serviceState.pending&&!serviceState.rejected){notify('Retry the pending save to confirm it before preparing another change.',true);return;}
 const f=serviceState.form,t=serviceState.data.requests.find(t=>t.id===f.request_id);
 if(f.kind==='submit'&&t){notify('This request already exists. Inspect its history before preparing a new request.',true);return;}
 if(f.kind!=='submit'&&!t){notify('This request is unavailable. Refresh your requests or contact HR.',true);return;}
 f.revision=t?.revision||0;serviceState.pending=null;serviceState.rejected=false;
 if(t)$('services-form-context').textContent=`${t.title} · ${serviceStatuses[t.status]} · Revision ${t.revision}`;
 serviceGuards();
}
function initServices(){
 $('services-new').onclick=()=>openServiceForm('submit');
 $('services-refresh').onclick=()=>busy(refreshServices);
 $('services-form').onsubmit=e=>{e.preventDefault();busy(saveService);};
 $('services-form').oninput=()=>{serviceState.dirty=true;};
 $('services-review').onclick=reviewService;
 $('services-discard').onclick=()=>{if(serviceState.pending&&!serviceState.rejected){notify('Confirm the pending save by retrying it before discarding.',true);return;}serviceState.form=null;serviceState.pending=null;serviceState.rejected=false;serviceState.dirty=false;$('services-form').reset();$('services-form').hidden=true;renderServices();};
}
