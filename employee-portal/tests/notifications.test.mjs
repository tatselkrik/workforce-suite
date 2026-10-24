// Code-level UI tests; this small DOM fixture does not establish visual layout.
import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import test from 'node:test';

async function fixture(){
 const elements=new Map(),calls=[],timers=[];
 const $=id=>{if(!elements.has(id))elements.set(id,{checked:false,textContent:'',attrs:{},setAttribute(k,v){this.attrs[k]=v;}});return elements.get(id);};
 const state={me:{},busy:false,dirty:false,page:'home'},timeState={dirtyKind:null},document={visibilityState:'visible'};
 const c=vm.createContext({$,state,timeState,document,setInterval(fn,ms){assert.equal(ms,60000);timers.push(fn);},async busy(fn){state.busy=true;try{await fn();}finally{state.busy=false;}},async api(path,body){calls.push({path,body});return {offset:Number(new URL('http://local'+path).searchParams.get('offset')||0),unread_only:false,total_items:0,unread_count:0,items:[]};}});
 vm.runInContext(await readFile(new URL('../web/notifications.js',import.meta.url),'utf8'),c);
 // Rendering is exercised separately by real browsers; this fixture tests state,
 // request shape and polling/draft guards without inventing browser evidence.
 vm.runInContext('renderNotifications=()=>notificationBadge()',c);
 return {c,$,calls,timers,state,timeState,document};
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
 f.state.busy=true;await poll();f.state.busy=false;f.document.visibilityState='hidden';await poll();f.document.visibilityState='visible';
 assert.equal(f.calls.length,0);await poll();assert.equal(f.calls.length,1);assert.equal(f.state.busy,false);
 f.state.me=null;await poll();assert.equal(f.calls.length,1);
});
