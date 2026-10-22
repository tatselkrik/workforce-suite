import assert from 'node:assert/strict';
export async function runNotificationTests({db,eq,deny,session,company,owner,unassigned}) {
 const key=(i,revision=0)=>`checkout:00000000-0000-0000-0000-${String(i).padStart(12,'0')}:${revision}`;
 const mark=async(keys,read=true,reset=false,c=company.id)=>(await db.query('select public.set_notification_reads($1,$2::text[],$3,$4) result',[c,keys,read,reset])).rows[0].result;
 const no=(keys,read=true,reset=false,c=company.id,codes=['23514'])=>deny('select public.set_notification_reads($1,$2::text[],$3,$4)',[c,keys,read,reset],codes);
 await session(owner);eq((await db.query('select * from public.notification_read_state')).rows,[],'No read history manufactured at migration');
 let r=await mark([key(1)]);eq(r.user_id,owner,'Server derives read-history owner');eq(r.company_id,company.id,'Server derives membership scope');eq(r.read_keys,[key(1)],'Read key retained');
 r=await mark([key(2)]);eq(r.read_keys,[key(1),key(2)],'Independent operations merge existing keys');r=await mark([key(1)]);eq(r.read_keys,[key(1),key(2)],'Read is idempotent');
 r=await mark([key(1)],false);eq(r.read_keys,[key(2)],'Unread removes only requested key');r=await mark([key(2,1)]);eq(r.read_keys,[key(2),key(2,1)],'New source revision has its own read key');
 for(const k of [null,[],[key(1),key(1)],['private text'],['checkout:00000000-0000-0000-0000-000000000000:0'],[key(1).replace(':0',':-1')],[key(1).replace(':0',':9223372036854775808')],[key(1)+':private'],[null],Array.from({length:101},(_,i)=>key(100+i))]) await no(k);
 await no([],true,true);await no([key(1)],false,true);await no([key(1)],null);await no([key(1)],true,null);
 await deny('select public.set_notification_reads($1,ARRAY[ARRAY[$2]]::text[],true,false)',[company.id,key(1)],['23514']);
 eq((await db.query('select read_keys from public.notification_read_state')).rows[0].read_keys,[key(2),key(2,1)],'Invalid payloads never alter prior markers');
 await deny('insert into public.notification_read_state(user_id,company_id) values($1,$2)',[owner,company.id],['42501']);
 await deny('update public.notification_read_state set read_keys=ARRAY[$1]',[key(99)],['42501']);
 await deny('delete from public.notification_read_state where user_id=$1',[owner],['42501']);
 await no([key(1)],true,false,'00000000-0000-0000-0000-000000009999',['42501']);
 const users={hr:'00000000-0000-0000-0000-000000095001',manager:'00000000-0000-0000-0000-000000095002',employee:'00000000-0000-0000-0000-000000095003'};
 await db.exec('reset role');
 for(const [role,id] of Object.entries(users)) await db.query('insert into auth.users(id,email,email_confirmed_at) values($1,$2,now())',[id,`notification-${role}@example.invalid`]);
 await session(owner);
 for(const [role,id] of Object.entries(users)) await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[id,`notification-${role}@example.invalid`,role==='hr'?'hr_generalist':role]);
 for(const role of ['hr','manager']) {
  await session(users[role]);eq((await db.query('select * from public.notification_read_state')).rows,[],`${role} cannot read Admin history`);
  r=await mark([key(20)]);eq(r.user_id,users[role],`${role} stores only own history`);
  eq((await db.query('select user_id from public.notification_read_state')).rows,[{user_id:users[role]}],`${role} unfiltered RLS is private`);
 }
 await session(owner);eq((await db.query('select user_id from public.notification_read_state')).rows,[{user_id:owner}],'Admin cannot inspect other private histories');
 await session(users.employee);eq((await db.query('select * from public.notification_read_state')).rows,[],'Portal-only employee cannot read desktop notification history');await no([key(1)],true,false,company.id,['42501']);
 await db.exec('reset role');await db.query("update auth.users set raw_user_meta_data='{"+'"role":"administrator"'+"}'::jsonb where id=$1",[users.employee]);await session(users.employee);await no([key(1)],true,false,company.id,['42501']);
 await session(owner);await db.query('select public.save_company_account($1,$2,$3,false,null,null)',[users.manager,'notification-manager@example.invalid','manager']);
 await session(users.manager);eq((await db.query('select * from public.notification_read_state')).rows,[],'Disabled staff loses read history immediately');await no([key(1)],true,false,company.id,['42501']);
 await session(unassigned);await no([key(1)],true,false,company.id,['42501']);await session('','anon');await deny('select * from public.notification_read_state',[],['42501']);await no([key(1)],true,false,company.id,['42501']);
 await db.exec('reset role');
 const full=Array.from({length:5000},(_,i)=>key(i+10000));await db.query('update public.notification_read_state set read_keys=$1::text[] where user_id=$2 and company_id=$3',[full,owner,company.id]);
 await session(owner);await no([key(99000)]);eq((await db.query('select cardinality(read_keys) n from public.notification_read_state')).rows[0].n,5000,'Full history rejection is atomic');
 r=await mark([],false,true);eq(r.read_keys,[],'Reset affects only own markers');
 await db.exec('reset role');eq((await db.query('select count(*)::int n from public.notification_read_state where user_id<>$1 and cardinality(read_keys)>0',[owner])).rows[0].n,2,'Reset preserves other accounts');
 eq((await db.query("select relrowsecurity from pg_class where oid='public.notification_read_state'::regclass")).rows[0].relrowsecurity,true,'Notification state has RLS');
 eq((await db.query("select count(*)::int n from pg_proc p join pg_namespace n on n.oid=p.pronamespace where n.nspname='public' and p.prosecdef")).rows[0].n,0,'No exposed definer added');
 assert.equal((await db.query('select count(*)::int n from public.notification_read_state')).rows[0].n,3);
 await session(owner);
}
