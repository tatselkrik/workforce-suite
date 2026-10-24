'use strict';
const accountState={data:null,kind:null};
function resetSecurity(){accountState.data=null;accountState.kind=null;state.securityDirty=false;$('password-form').reset();$('email-form').reset();}
async function loadSecurity(){return api('/api/account');}
function acceptSecurity(data){accountState.data=data;}
function securityGuards(){for(const kind of['password','email'])$(kind+'-fields').disabled=state.busy||!!accountState.kind&&accountState.kind!==kind;blocked($('security-discard'),!state.securityDirty);blocked($('security-refresh'),state.securityDirty);blocked($('sign-out-others'),state.securityDirty);}
function renderSecurity(){const data=accountState.data;if(!data)return;$('account-email').textContent='Sign-in email: '+data.email;$('pending-email').textContent=data.confirmation_pending?'Awaiting confirmation: '+data.pending_email+'. Confirm the email links, then return here and refresh account status.':'No email change is awaiting confirmation.';state.me.email=data.email;securityGuards();}
function securityDraft(kind){if([...$(kind+'-form').querySelectorAll('input')].some(e=>e.value)){accountState.kind=kind;state.securityDirty=true;}else{accountState.kind=null;state.securityDirty=false;}securityGuards();}
function clearSecurityDraft(){accountState.kind=null;state.securityDirty=false;$('password-form').reset();$('email-form').reset();securityGuards();}
async function refreshSecurity(){acceptSecurity(await loadSecurity());renderSecurity();}
function initSecurity(){
 for(const kind of['password','email'])$(kind+'-form').addEventListener('input',()=>securityDraft(kind));
 $('security-discard').onclick=()=>{clearSecurityDraft();notify('Unsaved account changes discarded.');};
 $('security-refresh').onclick=()=>busy(refreshSecurity);
 $('password-form').addEventListener('submit',e=>{e.preventDefault();busy(async()=>{const body={current_password:$('password-current').value,new_password:$('password-new').value,confirmation:$('password-confirm').value};try{await api('/api/account/password',body);clearSecurityDraft();await refreshSecurity();notify('Password changed. Use your new password the next time you sign in.');}finally{$('password-form').reset();$('email-current').value='';if(state.me)securityDraft('email');}});});
 $('email-form').addEventListener('submit',e=>{e.preventDefault();busy(async()=>{const body={current_password:$('email-current').value,new_email:$('email-new').value.trim()};try{const result=await api('/api/account/email',body);clearSecurityDraft();await refreshSecurity();notify(result.confirmation_pending?'Email change requested. Confirm the email links, then refresh account status.':'Your sign-in email was updated.');}finally{$('email-current').value='';if(state.me)securityDraft('email');}});});
 $('sign-out-others').onclick=()=>busy(async()=>{await api('/api/account/sign-out-others',{});notify('Other portal sessions were signed out. Existing access tokens on other clients follow the company service expiry policy.');});
 securityGuards();
}
