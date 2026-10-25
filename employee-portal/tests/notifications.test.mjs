// Code-level UI tests; this small DOM fixture does not establish visual layout.
import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import test from 'node:test';

async function fixture(render=false){
 const elements=new Map(),calls=[],timers=[],messages=[];
 const make=(tag='',text='')=>({tag,checked:false,textContent:text,attrs:{},children:[],append(...children){this.children.push(...children);},replaceChildren(...children){this.children=children;},setAttribute(k,v){this.attrs[k]=v;}});
 const $=id=>{if(!elements.has(id))elements.set(id,make());return elements.get(id);};
 const state={me:{},busy:false,dirty:false,page:'home'},timeState={dirtyKind:null},document={visibilityState:'visible',getElementById(){return null;}};
 const c=vm.createContext({$,state,timeState,document,node:(tag,cls,text)=>make(tag,text),day:String,when:String,blocked:(e,disabled)=>{e.disabled=disabled;},notify:message=>messages.push(message),async load(){},navigate(page){assert(['home','profile','attendance','leave','pay','notifications'].includes(page));state.page=page;},setInterval(fn,ms){assert.equal(ms,60000);timers.push(fn);},async busy(fn){state.busy=true;try{await fn();}finally{state.busy=false;}},async api(path,body){calls.push({path,body});return {offset:Number(new URL('http://local'+path).searchParams.get('offset')||0),unread_only:false,total_items:0,unread_count:0,items:[]};}});
 vm.runInContext(await readFile(new URL('../web/notifications.js',import.meta.url),'utf8'),c);
 // Rendering is exercised separately by real browsers; this fixture tests state,
 // request shape and polling/draft guards without inventing browser evidence.
 if(!render)vm.runInContext('renderNotifications=()=>notificationBadge()',c);
 return {c,$,calls,timers,state,timeState,document,messages};
}
test('inbox pagination recovers when its last item disappears',async()=>{
 const f=await fixture();await f.c.refreshInbox(25,true);
 assert.deepEqual(f.calls.map(c=>c.path),['/api/notifications?offset=25&unread=true','/api/notifications?offset=0&unread=true']);
 assert.equal(vm.runInContext('inboxState.offset',f.c),0);
});
test('read changes contain no identity and clearing a session removes the badge',async()=>{
 const f=await fixture();await f.c.markNotifications(['correction-status:00000000-0000-0000-0000-00000000006e:1'],true);
 assert.equal(f.calls[0].path,'/api/notifications/read');assert.deepEqual(Object.keys(f.calls[0].body).sort(),['keys','read','reset']);
 vm.runInContext('inboxState.data={unread_count:17};notificationBadge()',f.c);assert.match(f.$('notifications-button').attrs['aria-label'],/17 unread/);
 f.c.resetNotifications();assert.equal(f.$('notifications-button').attrs['aria-label'],'Notifications');assert.equal(vm.runInContext('inboxState.data',f.c),null);
});
test('background polling respects drafts, busy state, visibility and sign-out',async()=>{
 const f=await fixture();f.c.initNotifications();const poll=f.timers[0];
 f.state.dirty=true;await poll();f.state.dirty=false;f.timeState.dirtyKind='correction';await poll();f.timeState.dirtyKind=null;
 f.state.pictureDirty=true;await poll();f.state.pictureDirty=false;
 f.state.busy=true;await poll();f.state.busy=false;f.document.visibilityState='hidden';await poll();f.document.visibilityState='visible';
 assert.equal(f.calls.length,0);await poll();assert.equal(f.calls.length,1);assert.equal(f.state.busy,false);
 f.state.me=null;await poll();assert.equal(f.calls.length,1);
});
const allNodes=n=>[n,...(n.children||[]).flatMap(allNodes)];
const notice=(page,kind)=>({offset:0,limit:25,unread_only:false,total_items:1,unread_count:0,generated_at:'2026-10-25T00:00:00Z',items:[{key:'sample',kind,title:'Your personal update',detail:'Fictional own information',date:'2026-10-25',read:true,urgent:false,target_id:'sample',target_page:page}]});
test('informational lifecycle notices do not promise an unavailable record view',async()=>{
 const f=await fixture(true);f.c.acceptNotifications(notice('notifications','training-due'));f.c.renderNotifications();
 const nodes=allNodes(f.$('inbox-list'));assert.equal(nodes.filter(n=>n.tag==='button'&&n.textContent==='Open record').length,0);
 assert(nodes.some(n=>n.textContent.includes('Contact HR')));assert(nodes.some(n=>n.tag==='button'&&n.textContent==='Mark unread'));
});
test('monthly payslip notice opens the real pay page without an attendance fallback',async()=>{
 const f=await fixture(true);f.c.acceptNotifications(notice('pay','payroll-monthly-issued'));f.c.renderNotifications();
 const button=allNodes(f.$('inbox-list')).find(n=>n.tag==='button'&&n.textContent==='Open record');assert(button);await button.onclick();
 assert.equal(f.state.page,'pay');assert(f.messages.some(s=>s.includes('monthly payslips')));assert.equal(f.calls.length,0);
 const html=await readFile(new URL('../web/index.html',import.meta.url),'utf8');assert(html.includes('id="page-pay"'));
});
