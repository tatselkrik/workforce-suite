import{writeFile}from'node:fs/promises';
export async function runGovernmentReconciliationTests({db,eq,deny,session,company,owner,unassigned}){
 const id=n=>`00000000-0000-4000-8000-${String(810000+n).padStart(12,'0')}`,u={};
 await db.exec('reset role');const payrollBefore={};const tables=(await db.query("select tablename from pg_tables where schemaname='public'and tablename like'payroll_%'order by tablename")).rows;
 for(const{tablename:t}of tables)payrollBefore[t]=(await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows;
 for(const[k,role]of[['prep','hr_generalist'],['editor','hr_generalist'],['admin_editor','administrator'],['reviewer','administrator'],['staff','employee'],['manager','manager']]){
  u[k]=id(Object.keys(u).length+1);await db.exec('reset role');await db.query('insert into auth.users(id,email,email_confirmed_at)values($1,$2,now())',[u[k],`government-${k}@example.invalid`]);await session(owner);await db.query('select public.save_company_account($1,$2,$3,true,null,null)',[u[k],`government-${k}@example.invalid`,role]);
 }
 await db.exec('reset role');
 const pick=(await db.query("select hris_private.government_allocation_source(s.company_id,'thirteenth',s.id,'sss')basis,m.user_id from public.payroll_thirteenth_slips s join public.payroll_thirteenth_runs r on r.company_id=s.company_id and r.id=s.run_id join public.company_members m on m.company_id=s.company_id and m.employee_id=s.employee_id where s.company_id=$1 and m.user_id<>$2 and m.active and r.status in('approved','paid')and s.as_of>=date'2026-02-01'order by s.id limit 1",[company.id,owner])).rows[0];
 eq(!!pick?.basis,true,'Published contribution fixture with its own employee account');
 const src=pick.basis,month=src.contribution_month||src.ends_on.slice(0,7)+'-01';
 const plan=[{month,employee_cents:src.parts.employee_cents,employer_cents:200,ec_cents:30}];
 const call=async(op,record)=>(await db.query('select public.manage_government_allocation($1,$2)r',[op,record])).rows[0].r;
 const no=(op,record,codes=['23514','42501','PT409','23505'])=>deny('select public.manage_government_allocation($1,$2)',[op,record],codes);
 const snap=async(agency='sss')=>(await db.query('select public.government_reconciliation($1,$2)r',[month,agency])).rows[0].r;
 const read=async rid=>(await db.query('select to_jsonb(q)r from public.government_allocation_reviews q where id=$1',[rid])).rows[0].r;
 const create=(n,previous=null,allocations=plan)=>({id:id(n),expected_revision:null,source_kind:src.kind,source_id:src.id,agency:src.agency,previous_id:previous,source_basis:src,allocations,reference:'PRIVATE_GOV_EVIDENCE',note:'PRIVATE_GOV_REVIEW',reason:'Review exact synthetic contribution coverage'});
 let initial;
 await session(u.prep);initial=await snap();eq(initial.version,'government-reconciliation-v1','Versioned government workflow snapshot');
 let r=await call('create',create(20));eq(r.status,'draft','HR prepares an evidenced draft');eq((await call('create',create(20))).id,r.id,'Create retry retains one review identity');
 await no('create',{...create(20),note:'Different retry'},['PT409']);
 await no('create',create(21),['23505']);
 await no('create',{...create(22),source_basis:{...src,employee_id:id(99)}},['PT409']);
 for(const field of['source_basis','expected_revision']){const bad=create(22);delete bad[field];await no('create',bad);}
 await no('save',{id:r.id,expected_revision:r.revision+0.1,allocations:plan,reference:'Evidence',note:'Review',reason:'Fractional revision is invalid'});
 await no('save',{id:r.id,expected_revision:r.revision,allocations:[{...plan[0],employee_cents:src.parts.employee_cents+1}],reference:'Evidence',note:'Changed charge',reason:'Cannot rewrite an issued deduction'});
 for(const badPlan of[[],[plan[0],plan[0]],[{...plan[0],employee_cents:1.5}],[{...plan[0],month:'2026-09-02'}],[{...plan[0],month:'2024-12-01'}],[{...plan[0],employee_cents:String(plan[0].employee_cents)}],[{...plan[0],unexpected:1}]]){
  await no('save',{id:r.id,expected_revision:r.revision,allocations:badPlan,reference:'Evidence',note:'Review',reason:'Invalid plan denied'});
 }
 await no('save',{id:r.id,expected_revision:r.revision,allocations:plan,reference:'',note:'Review',reason:'Evidence required'});
 await session(u.admin_editor);r=await call('save',{id:r.id,expected_revision:r.revision,allocations:plan,reference:'PRIVATE_GOV_EVIDENCE',note:'PRIVATE_GOV_REVIEW',reason:'Second preparer'});
 await session(u.editor);r=await call('save',{id:r.id,expected_revision:r.revision,allocations:plan,reference:'PRIVATE_GOV_EVIDENCE',note:'PRIVATE_GOV_REVIEW',reason:'Third preparer'});
 await session(u.prep);for(const flag of[{}, {reviewed:null},{reviewed:'true'},{reviewed:false}])await no('submit',{id:r.id,expected_revision:r.revision,reason:'Explicit attestation required',...flag});r=await call('submit',{id:r.id,expected_revision:r.revision,reason:'Explicit shares and coverage reviewed',reviewed:true});
 await no('save',{id:r.id,expected_revision:r.revision,allocations:plan,reference:'Evidence',note:'Review',reason:'Submitted cannot change'});
 await session(u.admin_editor);await no('approve',{id:r.id,expected_revision:r.revision,reason:'Earlier editor cannot approve',reviewed:true},['42501']);
 // A different account role cannot allow approval of one's own employee source.
 await db.exec('reset role');await db.exec('begin');
 try{await db.query("update public.company_members set role='administrator'where user_id=$1",[pick.user_id]);await session(pick.user_id);await no('approve',{id:r.id,expected_revision:r.revision,reason:'Own employee source',reviewed:true},['42501']);}
 finally{await db.exec('rollback');}
 await session(owner);for(const flag of[{}, {reviewed:null},{reviewed:'true'}])await no('approve',{id:r.id,expected_revision:r.revision,reason:'Explicit approval attestation required',...flag});r=await call('approve',{id:r.id,expected_revision:r.revision,reason:'Independent evidenced approval',reviewed:true});eq(r.status,'approved','Independent Administrator approves');
 const approved=structuredClone(r);let s=await snap();
 const effective=s.effective_facts.find(f=>f.id===src.id&&f.kind===src.kind);
 eq(effective.parts,{employee_cents:src.parts.employee_cents,employer_cents:200,ec_cents:30},'Explicit review resolves employer and EC without changing employee amount');
 eq(effective.contribution_month,month,'Reviewed original coverage is explicit');eq(s.source.facts.find(f=>f.id===src.id&&f.kind===src.kind).parts,src.parts,'Raw source-v1 remains unchanged');
 await no('save',{id:r.id,expected_revision:r.revision,allocations:plan,reference:'Rewrite',note:'Rewrite',reason:'Approved immutable'});
 await db.exec('reset role');await deny('delete from public.government_allocation_reviews where id=$1',[r.id],['42501']);
 await session(u.prep);await no('create',create(22),['PT409']);
 let replacement=await call('create',create(23,r.id,[{...plan[0],employer_cents:201}]));replacement=await call('submit',{id:replacement.id,expected_revision:replacement.revision,reason:'Replacement review',reviewed:true});
 await session(u.reviewer);replacement=await call('return',{id:replacement.id,expected_revision:replacement.revision,reason:'Return for evidence'});eq(replacement.status,'draft','Returned submission is editable');
 await session(u.prep);replacement=await call('submit',{id:replacement.id,expected_revision:replacement.revision,reason:'Resubmission',reviewed:true});
 await session(u.reviewer);replacement=await call('approve',{id:replacement.id,expected_revision:replacement.revision,reason:'New independent version',reviewed:true});
 eq((await snap()).effective_facts.find(f=>f.id===src.id&&f.kind===src.kind).parts.employer_cents,201,'Only latest approved version applies');
 await db.exec('reset role');eq(JSON.stringify(await read(approved.id)),JSON.stringify(approved),'Original approved review remains immutable');
 await session(u.prep);let negative=await call('create',create(24,replacement.id,[{...plan[0],employer_cents:-100000000000000}]));negative=await call('submit',{id:negative.id,expected_revision:negative.revision,reason:'No invented credit',reviewed:true});
 await session(owner);await no('approve',{id:negative.id,expected_revision:negative.revision,reason:'Unbacked negative charge',reviewed:true},['23514']);negative=await call('cancel',{id:negative.id,expected_revision:negative.revision,reason:'Retain cancelled proposal'});eq(negative.status,'cancelled','Invalid candidate can be cancelled with history');
 const remit=async(op,record)=>(await db.query('select public.manage_government_remittance($1,$2)r',[op,record])).rows[0].r;
 const noRemit=(op,record,codes=['23514','42501','PT409','23505'])=>deny('select public.manage_government_remittance($1,$2)',[op,record],codes);
 const payment={id:id(40),agency:'sss',paid_on:'2026-10-01',amount_cents:333,allocations:[{month,amount_cents:333}],reference:'PRIVATE_GOV_RECEIPT',evidence:'PRIVATE_GOV_PAYMENT_EVIDENCE',note:'PRIVATE_GOV_PAYMENT_NOTE',replaces_id:null,received_confirmed:true,reason:'Record synthetic evidenced remittance'};
 await session(u.prep);await noRemit('record',payment,['42501']);
 await session(owner);
 const missingConfirmation={...payment};delete missingConfirmation.received_confirmed;await noRemit('record',missingConfirmation);
 for(const change of[{received_confirmed:false},{amount_cents:0},{amount_cents:1.5},{allocations:[{month,amount_cents:332}]},{paid_on:'2200-01-01'},{reference:''},{reference:'bad\nreference'},{evidence:''},{user_id:owner}])await noRemit('record',{...payment,...change});
 let receipt=await remit('record',payment);eq(receipt.amount_cents,333,'Administrator records evidence without transferring money');eq((await remit('record',payment)).id,receipt.id,'Exact receipt retry is idempotent');
 await noRemit('record',{...payment,id:id(41)},['PT409']);await noRemit('record',{...payment,id:id(41),reference:' private_gov_receipt '},['PT409']);
 s=await snap();eq(s.remittances.find(x=>x.id===receipt.id).void_id,null,'New recorded remittance is active');
 const originalReceipt=structuredClone(receipt);const voidPayload={id:id(42),remittance_id:receipt.id,reason:'Synthetic correction; preserve original'};let voided=await remit('void',voidPayload);eq((await remit('void',voidPayload)).id,voided.id,'Void retry is idempotent');
 await noRemit('void',{...voidPayload,id:id(43)},['PT409']);await noRemit('record',{...payment,id:id(44)},['PT409']);
 const corrected={...payment,id:id(45),amount_cents:334,allocations:[{month,amount_cents:334}],replaces_id:receipt.id};let fixed=await remit('record',corrected);
 eq((await snap()).remittances.find(x=>x.id===receipt.id).replacement_id,fixed.id,'Corrected receipt retains its predecessor link');
 await db.exec('reset role');eq((await db.query('select to_jsonb(r)-\'reference_key\'r from public.government_remittances r where id=$1',[receipt.id])).rows[0].r,originalReceipt,'Original remittance content stays immutable');
 await session(owner);await noRemit('record',{...corrected,id:id(46)},['PT409']);
 await remit('void',{id:id(47),remittance_id:fixed.id,reason:'Correct reference typo with linked evidence'});
 let renamed=await remit('record',{...corrected,id:id(48),reference:'CORRECTED_PRIVATE_GOV_RECEIPT',replaces_id:fixed.id});
 eq(renamed.replaces_id,fixed.id,'A reference typo can be corrected through the voided leaf');
 const vectors=[];for(const agency of['sss','bir','philhealth','pagibig'])vectors.push(await snap(agency));
 for(const key of['staff','manager']){await session(u[key]);for(const table of['government_allocation_reviews','government_remittances','government_remittance_voids','government_events'])eq((await db.query('select count(*)::int n from public.'+table)).rows[0].n,0,'No '+key+' visibility into '+table);await deny('select public.government_reconciliation($1,$2)',[month,'sss'],['42501']);await no('create',create(70),['42501']);await noRemit('record',{...payment,id:id(71)},['42501']);}
 await session(unassigned);await deny('select public.government_reconciliation($1,$2)',[month,'sss'],['42501']);
 await session('','anon');await deny('select public.government_reconciliation($1,$2)',[month,'sss'],['42501']);
 await session(u.prep);for(const table of['government_allocation_reviews','government_remittances','government_remittance_voids','government_events'])await deny('delete from public.'+table,[],['42501']);
 await db.exec('reset role');for(const t of Object.keys(payrollBefore))eq((await db.query('select to_jsonb(t)r from public.'+t+' t order by to_jsonb(t)::text')).rows,payrollBefore[t],'Government workflows never rewrite '+t);
 await writeFile(new URL('../../work/government-reconciliation-vectors.json',import.meta.url),JSON.stringify(vectors,null,2)+'\n');
}
