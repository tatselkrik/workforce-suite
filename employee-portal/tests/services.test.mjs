// Browser logic and a small DOM fixture; these checks do not establish visual layout.
import{readFile}from'node:fs/promises';import vm from'node:vm';import assert from'node:assert/strict';import test from'node:test';import{TextEncoder}from'node:util';
async function fixture(){
 const elements=new Map(),calls=[],messages=[];let nonce=1;
 const make=(tag='',text='')=>({tag,textContent:text,value:'',dataset:{},children:[],hidden:false,disabled:false,required:false,append(...v){this.children.push(...v)},replaceChildren(...v){this.children=v},reset(){},focus(){}});
 const $=id=>{if(!elements.has(id))elements.set(id,make());return elements.get(id)};
 const state={busy:false,page:'services',me:{company:{timezone:'Asia/Manila'}}},response={mode:'ok',requests:[],receiptWrong:false};
 const c=vm.createContext({$,state,TextEncoder,crypto:{randomUUID(){return'67000000-0000-4000-8000-'+String(nonce++).padStart(12,'0')}},node:(t,cl,text)=>make(t,text),day:String,blocked(e,d){e.disabled=d;e.dataset.blocked=String(d)},notify(v){messages.push(v)},async busy(fn){state.busy=true;try{return await fn()}finally{state.busy=false;c.serviceGuards()}},async api(path,body){
  calls.push({path,body});if(path==='/api/services')return{schema_version:1,requests:response.requests};
  if(response.mode!=='ok'){const e=Error('Fictional delivery outcome');if(response.mode!=='network')e.status=Number(response.mode);throw e;}
  return{schema_version:1,id:response.receiptWrong?'wrong':body.id,request_id:body.request_id,revision:body.expected_revision+1};
 }});
 const helpers=await readFile(new URL('../web/self-service.js',import.meta.url),'utf8');const start=helpers.indexOf('function when(');const end=helpers.indexOf('\n',start);vm.runInContext(helpers.slice(start,end),c);
 vm.runInContext(await readFile(new URL('../web/services.js',import.meta.url),'utf8'),c);c.initServices();
 function draft(){c.openServiceForm('submit');$('services-title').value='Fictional service';$('services-body').value='Fictional shared message';$('services-category').value='general_support';$('services-priority').value='normal';}
 return{c,$,state,response,calls,messages,draft};
}
const plain=v=>JSON.parse(JSON.stringify(v));
const value=(f,v)=>vm.runInContext(v,f.c);
const ticket=(revision=1)=>({id:'67000000-0000-4000-8000-000000000170',code:'SR-example',title:'Fictional request',category:'general_support',priority:'normal',status:'submitted',handler_name:'Fictional HR',due_on:null,revision,can_reply:true,can_cancel:true,messages:[{actor_name:'Fictional employee',kind:'submit',revision:1,body:'Shared literal <img src=x onerror=alert(1)>',status:'submitted',created_at:'2026-10-29T00:00:00Z'}]});
test('own payload is frozen and contains no employee, actor or routing authority',async()=>{
 const f=await fixture();f.draft();const first=f.c.servicePayload();f.$('services-body').value='An edit cannot change the pending message';const retry=f.c.servicePayload();
 assert.equal(retry,first);assert(Object.isFrozen(first)&&Object.isFrozen(first.change));assert.deepEqual(Object.keys(first).sort(),['change','expected_revision','id','request_id']);assert.deepEqual(Object.keys(first.change).sort(),['body','category','kind','priority','title']);assert.equal(first.change.body,'Fictional shared message');
});
test('uncertain delivery keeps exact identity and blocks discard until confirmation',async()=>{
 const f=await fixture();f.draft();f.response.mode='502';await assert.rejects(f.c.saveService());const pending=plain(value(f,'serviceState.pending'));
 assert.equal(value(f,'serviceState.dirty'),true);f.$('services-discard').onclick();assert.equal(value(f,'serviceState.dirty'),true);assert(f.messages.some(m=>m.includes('pending save')));
 f.response.mode='ok';await f.c.saveService();const posts=f.calls.filter(r=>r.path==='/api/services/save');assert.equal(posts.length,2);assert.deepEqual(plain(posts[0].body),pending);assert.deepEqual(plain(posts[1].body),pending);assert.equal(value(f,'serviceState.dirty'),false);assert.equal(value(f,'serviceState.pending'),null);
});
test('a mismatched receipt never acknowledges the pending save',async()=>{
 const f=await fixture();f.draft();f.response.receiptWrong=true;await assert.rejects(f.c.saveService(),/exact save/);assert(value(f,'serviceState.pending'));assert.equal(value(f,'serviceState.dirty'),true);assert.equal(f.calls.length,1);
});
test('confirmed rejection requires explicit current-version review before a new identity',async()=>{
 const f=await fixture();const t=ticket(2);f.response.requests=[t];await f.c.refreshServices();f.c.openServiceForm('reply',t);f.$('services-body').value='Preserve this reply for explicit review';
 f.response.mode='409';await assert.rejects(f.c.saveService());const old=plain(value(f,'serviceState.pending'));assert.equal(value(f,'serviceState.rejected'),true);
 f.response.requests=[ticket(5)];await f.c.refreshServices();assert.equal(value(f,'serviceState.form.revision'),2);assert.equal(value(f,'serviceState.pending.id'),old.id);
 f.c.reviewService();assert.equal(value(f,'serviceState.form.revision'),5);assert.equal(value(f,'serviceState.pending'),null);assert.equal(f.$('services-body').value,'Preserve this reply for explicit review');
 const next=f.c.servicePayload();assert.notEqual(next.id,old.id);assert.equal(next.expected_revision,5);
});
test('session loss clears thread, pending payload and all service draft fields',async()=>{
 const f=await fixture();f.draft();f.c.servicePayload();f.response.mode='403';await assert.rejects(f.c.saveService());assert.equal(value(f,'serviceState.pending'),null);assert.equal(value(f,'serviceState.form'),null);assert.equal(value(f,'serviceState.dirty'),false);assert.equal(value(f,'serviceState.data'),null);assert.equal(f.$('services-form').hidden,true);
});
const nodes=n=>[n,...n.children.flatMap(nodes)];
test('shared messages render as literal text and terminal requests have no action buttons',async()=>{
 const f=await fixture();const t=ticket();t.status='completed';t.can_reply=t.can_cancel=false;f.response.requests=[t];await f.c.refreshServices();vm.runInContext('serviceState.selected="'+t.id+'"',f.c);f.c.renderServices();
 const all=nodes(f.$('services-detail'));assert(all.some(n=>n.textContent.includes('<img src=x onerror=alert(1)>')));assert(!all.some(n=>n.tag==='button'));assert(!all.some(n=>'innerHTML'in n));
});
test('actual navigation code blocks leaving a service draft and never refreshes it implicitly',async()=>{
 const f=await fixture();f.draft();const source=await readFile(new URL('../web/app.js',import.meta.url),'utf8');
 Object.assign(f.c,{timeState:{dirtyKind:null},document:{querySelectorAll(){return[]}},window:{scrollTo(){}},busy:async()=>{throw Error('Unexpected draft refresh')}});
 vm.runInContext(source.slice(source.indexOf('function navigate('),source.indexOf('function clock(')),f.c);
 f.c.navigate('home');assert.equal(f.state.page,'services');assert(f.messages.some(m=>m.includes('draft')));f.c.navigate('services');assert.equal(f.state.page,'services');
});
test('the new services page uses no persisted tokens, drafts or HTML rendering',async()=>{
 const source=await readFile(new URL('../web/services.js',import.meta.url),'utf8');assert(!/localStorage|sessionStorage|indexedDB|innerHTML|outerHTML|insertAdjacentHTML/.test(source));
 const html=await readFile(new URL('../web/index.html',import.meta.url),'utf8');assert(html.includes('id="page-services"'));assert(html.includes('src="/services.js"'));
});

test('service history displays receipt instants in company time rather than slicing a UTC date',async()=>{
 const f=await fixture();f.state.me={company:{timezone:'Asia/Manila'}};const t=ticket();t.messages[0].created_at='2026-10-28T23:30:00Z';f.response.requests=[t];await f.c.refreshServices();vm.runInContext('serviceState.selected="'+t.id+'"',f.c);f.c.renderServices();
 const all=nodes(f.$('services-detail'));assert(all.some(n=>/Oct 29, 2026/.test(n.textContent)&&n.textContent.includes('7:30')));assert(!all.some(n=>/Oct 28, 2026/.test(n.textContent)));
});
