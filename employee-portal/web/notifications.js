'use strict';
const inboxState={data:null,offset:0,unread:false};
function resetNotifications(){inboxState.data=null;inboxState.offset=0;inboxState.unread=false;$('inbox-unread').checked=false;$('notifications-button').textContent='Notifications';$('notifications-button').setAttribute('aria-label','Notifications');}
async function loadNotifications(offset=inboxState.offset,unread=inboxState.unread){let data=await api('/api/notifications?offset='+offset+'&unread='+unread);if(offset&&offset>=data.total_items){offset=Math.max(0,Math.floor((data.total_items-1)/25)*25);data=await api('/api/notifications?offset='+offset+'&unread='+unread);}return data;}
function acceptNotifications(data){inboxState.data=data;inboxState.offset=data.offset;inboxState.unread=data.unread_only;}
function notificationBadge(){const count=inboxState.data?.unread_count||0;$('notifications-button').textContent=`Notifications (${count})`;$('notifications-button').setAttribute('aria-label',`Notifications, ${count} unread`);}
async function refreshInbox(offset=inboxState.offset,unread=inboxState.unread){acceptNotifications(await loadNotifications(offset,unread));renderNotifications();}
async function markNotifications(keys,read){await api('/api/notifications/read',{keys,read,reset:false});await refreshInbox();}
function renderNotifications(){notificationBadge();const data=inboxState.data,list=$('inbox-list');list.replaceChildren();if(!data){list.append(node('p','empty','Refresh to load your personal updates.'));return;}
 $('inbox-unread').checked=data.unread_only;$('inbox-summary').textContent=`${data.unread_count} unread · ${data.total_items} ${data.unread_only?'unread ':''}items · Updated ${when(data.generated_at)}`;
 if(!data.items.length)list.append(node('p','empty',data.unread_only?'You have no unread updates.':'You have no current personal updates.'));
 for(const r of data.items){const card=node('article','card notification-card'+(r.read?' notification-read':'')),row=node('div','row');row.append(node('h2','',r.title),node('span','badge',r.read?'Read':'Unread'));card.append(row,node('p','',r.detail),node('p','fine',(r.urgent?'Reminder · ':'')+day(r.date)));
  const buttons=node('div','actions'),open=node('button','primary','Open record'),toggle=node('button','secondary',r.read?'Mark unread':'Mark read');open.type=toggle.type='button';toggle.onclick=()=>busy(()=>markNotifications([r.key],!r.read));
  open.onclick=()=>busy(async()=>{if(!r.read)await markNotifications([r.key],true);await load();navigate(r.target_page);if(state.page!==r.target_page)return;if(r.kind==='checkout'){notify('Review your open shift and contact HR if its punches need correction.');return;}const id=r.target_page==='leave'?'leave-request-'+r.target_id:'time-request-'+(r.kind==='correction-status'?'correction':'overtime')+'-'+r.target_id;const record=document.getElementById(id);if(record)record.scrollIntoView({behavior:'smooth',block:'center'});else notify('This record is outside the recent list. Refresh your records or ask HR for older details.');});buttons.append(open,toggle);card.append(buttons);list.append(card);
 }
 $('inbox-page').textContent=data.total_items?`Showing ${data.offset+1}–${data.offset+data.items.length} of ${data.total_items}`:'No items';
 blocked($('inbox-previous'),data.offset===0);blocked($('inbox-next'),data.offset+data.limit>=data.total_items||data.offset+data.limit>50000);blocked($('inbox-mark-shown'),!data.items.some(r=>!r.read));
}
function initNotifications(){
 $('notifications-button').onclick=()=>{navigate('notifications');if(state.page==='notifications')busy(()=>refreshInbox());};
 $('inbox-unread').onchange=()=>busy(()=>refreshInbox(0,$('inbox-unread').checked));
 $('inbox-previous').onclick=()=>busy(()=>refreshInbox(Math.max(0,inboxState.offset-25)));
 $('inbox-next').onclick=()=>busy(()=>refreshInbox(inboxState.offset+25));
 $('inbox-mark-shown').onclick=()=>busy(()=>markNotifications(inboxState.data.items.filter(r=>!r.read).map(r=>r.key),true));
 $('inbox-reset').onclick=()=>busy(async()=>{await api('/api/notifications/read',{keys:[],read:false,reset:true});await refreshInbox(0);notify('Your read history was reset. Requests and decisions stay saved.');});
 setInterval(async()=>{if(!state.me||state.busy||state.dirty||state.pictureDirty||state.securityDirty||timeState.dirtyKind||document.visibilityState!=='visible')return;await busy(async()=>{acceptNotifications(await loadNotifications());notificationBadge();if(state.page==='notifications')renderNotifications();});},60000);
}
