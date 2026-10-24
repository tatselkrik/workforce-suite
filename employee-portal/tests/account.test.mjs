// Account form state tests, not a browser or visual inspection.
import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
import test from 'node:test';
import assert from 'node:assert/strict';
async function fixture(){
 const elements=new Map(),tasks=[],calls=[],notices=[];
 const fields={password:['password-current','password-new','password-confirm'],email:['email-current','email-new']};
 const $=id=>{if(!elements.has(id))elements.set(id,{value:'',textContent:'',listeners:{},addEventListener(k,fn){this.listeners[k]=fn;},querySelectorAll(){return(fields[id.replace('-form','')]||[]).map($);},reset(){for(const x of this.querySelectorAll())x.value='';}});return elements.get(id);};
 const state={busy:false,securityDirty:false,dirty:true,pictureDirty:true,me:{email:'old@example.invalid'}};
 const c=vm.createContext({$,state,blocked(e,v){e.blocked=v;},notify(m){notices.push(m);},busy(fn){const t=fn();t.catch(()=>{});tasks.push(t);},async api(path,body){calls.push({path,body});return path==='/api/account'?{email:'old@example.invalid',pending_email:'new@example.invalid',confirmation_pending:true}:{confirmation_pending:true};}});
 vm.runInContext(await readFile(new URL('../web/account.js',import.meta.url),'utf8'),c);c.initSecurity();return{c,$,state,tasks,calls,notices};
}
test('account drafts disable the other credential form and preserve contact/photo drafts',async()=>{
 const f=await fixture();f.$('password-new').value='fictional';f.c.securityDraft('password');
 assert.equal(f.state.securityDirty,true);assert.equal(f.$('email-fields').disabled,true);assert.equal(f.$('security-refresh').blocked,true);
 f.c.clearSecurityDraft();assert.equal(f.state.securityDirty,false);assert.equal(f.$('password-new').value,'');assert.equal(f.$('email-fields').disabled,false);assert.equal(f.state.dirty,true);assert.equal(f.state.pictureDirty,true);
});
test('failed password update erases all password fields without clearing other profile drafts',async()=>{
 const f=await fixture();for(const id of['password-current','password-new','password-confirm'])f.$(id).value='private-test-value';f.c.securityDraft('password');
 f.c.api=async()=>{throw Error('Unavailable');};f.$('password-form').listeners.submit({preventDefault(){}});await assert.rejects(f.tasks.pop(),/Unavailable/);
 for(const id of['password-current','password-new','password-confirm','email-current'])assert.equal(f.$(id).value,'');
 assert.equal(f.state.securityDirty,false);assert.equal(f.state.dirty,true);assert.equal(f.state.pictureDirty,true);
});
test('failed email update clears the password and retains only the editable new email draft',async()=>{
 const f=await fixture();f.$('email-current').value='private-test-value';f.$('email-new').value='new@example.invalid';f.c.securityDraft('email');
 f.c.api=async()=>{throw Error('Unavailable');};f.$('email-form').listeners.submit({preventDefault(){}});await assert.rejects(f.tasks.pop(),/Unavailable/);
 assert.equal(f.$('email-current').value,'');assert.equal(f.$('email-new').value,'new@example.invalid');assert.equal(f.state.securityDirty,true);assert.equal(f.$('password-fields').disabled,true);
});
test('successful request shows pending confirmation; sign-out clears all account form state',async()=>{
 const f=await fixture();f.$('email-current').value='private-test-value';f.$('email-new').value='new@example.invalid';f.c.securityDraft('email');
 f.$('email-form').listeners.submit({preventDefault(){}});await f.tasks.pop();
 assert.equal(f.state.securityDirty,false);assert.equal(f.$('email-new').value,'');assert.match(f.$('pending-email').textContent,/Awaiting confirmation/);assert.match(f.notices.at(-1),/Confirm/);
 f.$('password-current').value='private-test-value';f.c.resetSecurity();assert.equal(f.$('password-current').value,'');assert.equal(vm.runInContext('accountState.data',f.c),null);assert.equal(vm.runInContext('accountState.kind',f.c),null);
});
