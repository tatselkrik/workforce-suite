// Code-level state tests, not browser/layout evidence.
import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
import test from 'node:test';
import assert from 'node:assert/strict';
async function fixture(){
 const elements=new Map(),revoked=[];const $=id=>{if(!elements.has(id))elements.set(id,{hidden:false,value:'',textContent:'',parentElement:{classList:{toggle(){}}},removeAttribute(k){delete this[k];}});return elements.get(id);};
 const state={generation:0,me:{employment:{full_name:'Fictional Staff'}},pictureDirty:false,dirty:false};
 const c=vm.createContext({$,state,DOMException,URL:{revokeObjectURL(u){revoked.push(u);},createObjectURL(){return'blob:new';}},blocked(e,v){e.blocked=v;},async api(){return{has_logo:false};},async fetch(){return{status:404,ok:false};}});
 vm.runInContext(await readFile(new URL('../web/pictures.js',import.meta.url),'utf8'),c);return{c,$,revoked,state};
}
test('discarding a photo selection preserves saved photo and contact draft',async()=>{
 const f=await fixture();f.state.dirty=true;f.state.pictureDirty=true;
 vm.runInContext("pictureState.photo='blob:saved';pictureState.preview='blob:preview';pictureState.file={};renderPictures()",f.c);
 assert.equal(f.$('profile-photo').src,'blob:preview');assert.equal(f.$('sidebar-photo').src,'blob:saved');assert.equal(f.$('photo-remove').blocked,true);
 f.c.discardPicture();assert.equal(f.$('profile-photo').src,'blob:saved');assert.deepEqual(f.revoked,['blob:preview']);assert.equal(f.state.dirty,true);assert.equal(f.state.pictureDirty,false);
});
test('sign-out revokes all private picture URLs and removes employee initials',async()=>{
 const f=await fixture();vm.runInContext("pictureState.photo='blob:photo';pictureState.logo='blob:logo';pictureState.preview='blob:preview';pictureState.file={};renderPictures()",f.c);
 f.c.resetPictures();assert.deepEqual(f.revoked,['blob:photo','blob:logo','blob:preview']);assert.equal(f.$('profile-initials').textContent,'');assert.equal(f.$('sidebar-initials').textContent,'');assert.equal(f.$('profile-photo').hidden,true);assert.equal(vm.runInContext('pictureState.file',f.c),null);
});
test('optional picture errors do not block records but expired sessions propagate',async()=>{
 const f=await fixture();f.c.fetch=async()=>{throw Error('Image storage unavailable');};let data=await f.c.loadPictures();assert.equal(data.warning,true);assert.equal(data.photo,null);
 f.c.api=async()=>{const e=Error('Expired');e.status=401;throw e;};await assert.rejects(f.c.loadPictures(),/Expired/);
});
test('an image response from an earlier session never becomes a private URL',async()=>{
 const f=await fixture();f.c.fetch=async()=>({status:200,ok:true,async blob(){f.state.generation++;return{};}});await assert.rejects(f.c.imageBlob('/api/photo'),{name:'AbortError'});
});
