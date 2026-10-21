import {randomUUID} from 'node:crypto';
export async function runDocumentTests({db,eq,deny,session,employee,company,owner,unassigned}) {
  const write=async(operation,data)=>(await db.query('select public.save_employee_document($1,$2::jsonb) as record',[operation,JSON.stringify(data)])).rows[0].record;
  const payload=person=>({id:randomUUID(),employee_id:person,title:'Synthetic contract',category:'contract',notes:'Fictional test file',issued_on:'2026-01-01',expires_on:'2027-01-01',contract_start:'2026-01-02',contract_end:'2027-01-01',original_filename:'Synthetic contract.pdf',mime_type:'application/pdf',byte_size:123,sha256:'a'.repeat(64)});
  const events=async id=>(await db.query('select * from public.employee_document_events where document_id=$1 order by sequence',[id])).rows;
  await session(owner);const person=await employee(company.id,'DOC-QA');
  await db.exec('reset role');
  const bucket=(await db.query("select * from storage.buckets where id='employee-documents'")).rows[0];
  const docxMime='application/vnd.openxmlformats-officedocument.wordprocessingml.document';
  eq([bucket.public,Number(bucket.file_size_limit),bucket.allowed_mime_types],[false,10485760,['application/pdf','image/png','image/jpeg',docxMime]],'Document bucket is private and bounded');
  await session(owner);
  const data=payload(person.id);data.created_by=unassigned;data.created_by_name='Forged actor';data.state='ready';data.archived=true;data.company_id=randomUUID();
  const pending=await write('reserve',data);
  eq([pending.company_id,pending.employee_id,pending.state,pending.archived,pending.revision],[company.id,person.id,'pending',false,0],'Reservation derives company and ignores forged state');
  eq(pending.created_by,owner,'Uploader derives from protected Auth membership');
  eq(pending.created_by_name==='Forged actor',false,'Uploader label is server derived');
  eq(pending.object_name,`${company.id}/${person.id}/${pending.id}/file.pdf`,'Unique immutable path contains only IDs and a fixed extension');
  eq((await events(pending.id)).length,0,'Incomplete upload is not recorded as filed');
  eq(await write('reserve',data),pending,'Identical reservation retry is idempotent');
  await deny('select public.save_employee_document($1,$2)', ['reserve',JSON.stringify({...data,title:'Different file'})],['PT409']);
  await deny('select public.save_employee_document($1,$2)', ['finalize',JSON.stringify({id:pending.id})],['23514']);
  await db.query('insert into storage.objects(bucket_id,name,owner_id,metadata) values($1,$2,$3,$4)',['employee-documents',pending.object_name,owner,{size:124,mimetype:'application/pdf'}]);
  await deny('select public.save_employee_document($1,$2)', ['finalize',JSON.stringify({id:pending.id})],['23514']);
  await db.exec('reset role');await db.query('update storage.objects set metadata=$1 where name=$2',[{size:123,mimetype:'text/plain'},pending.object_name]);
  await session(owner);await deny('select public.save_employee_document($1,$2)',['finalize',JSON.stringify({id:pending.id})],['23514']);
  await db.exec('reset role');await db.query('update storage.objects set metadata=$1 where name=$2',[{size:123,mimetype:'application/pdf'},pending.object_name]);
  await session(owner);const filed=await write('finalize',{id:pending.id});
  eq([filed.state,filed.revision],['ready',1],'Only validated Storage metadata allows filing');
  let history=await events(filed.id);eq(history.length,1,'Filing records exactly one event');
  eq([history[0].kind,history[0].actor_id,history[0].actor_role,history[0].before_record],['uploaded',owner,'administrator',null],'Upload event has protected actor and no invented before state');
  eq(history[0].after_record.id,filed.id,'Upload event captures filed metadata');
  eq(await write('finalize',{id:filed.id}),filed,'Ambiguous finalize retry returns the existing record');
  eq((await events(filed.id)).length,1,'Finalize retry creates no duplicate event');
  // Exercise DOCX in a rolled-back scope so all established retention checks remain meaningful.
  await db.exec('begin');
  const word=await write('reserve',{...payload(person.id),original_filename:'Synthetic contract.DOCX',mime_type:docxMime});
  eq(word.object_name,`${company.id}/${person.id}/${word.id}/file.docx`,'DOCX gets its own immutable extension');
  await db.query('insert into storage.objects(bucket_id,name,owner_id,metadata) values($1,$2,$3,$4)',['employee-documents',word.object_name,owner,{size:123,mimetype:docxMime}]);
  eq((await write('finalize',{id:word.id})).state,'ready','DOCX metadata can finalize after a matching private upload');
  await deny('select public.save_employee_document($1,$2)',['reserve',JSON.stringify({...payload(person.id),original_filename:'renamed.docm',mime_type:docxMime})],['23514']);
  await db.exec('rollback');
  eq((await db.query('select * from storage.objects where bucket_id=$1 and name=$2',['employee-documents',filed.object_name])).rows.length,1,'HR can read private filed object');
  eq((await db.query("update storage.objects set metadata='{}' where bucket_id=$1 and name=$2 returning id",['employee-documents',filed.object_name])).rows.length,0,'Filed bytes cannot be overwritten');
  eq((await db.query('delete from storage.objects where bucket_id=$1 and name=$2 returning id',['employee-documents',filed.object_name])).rows.length,0,'Filed bytes cannot be deleted');
  for(const sql of ["update public.employee_documents set title='Tampered' where id=$1","delete from public.employee_documents where id=$1"])await deny(sql,[filed.id],['42501']);
  await deny("insert into public.employee_documents(id,company_id,employee_id,title,category,original_filename,mime_type,byte_size,sha256,created_by,created_by_name) values($1,$2,$3,'Forged','other','a.pdf','application/pdf',1,$4,$5,'Forged')",[randomUUID(),company.id,person.id,'f'.repeat(64),owner],['42501']);
  const edited=await write('edit',{...filed,title:'Corrected label',revision:1,sha256:'b'.repeat(64),object_name:'malicious/path'});
  eq([edited.title,edited.sha256,edited.object_name,edited.revision],['Corrected label',filed.sha256,filed.object_name,2],'Metadata edit cannot alter the file');
  history=await events(filed.id);eq(history.length,2,'Metadata edit creates one event');eq(history[1].before_record.title,'Synthetic contract','Before metadata retained');
  eq(history[1].after_record.title,'Corrected label','After metadata saved');
  await deny('select public.save_employee_document($1,$2)',['edit',JSON.stringify({...filed,title:'Stale edit',revision:1})],['PT409']);
  await deny('select public.save_employee_document($1,$2)',['archive',JSON.stringify({id:filed.id,revision:1})],['PT409']);
  eq(await write('edit',edited),edited,'Unchanged details do not increment revision');eq((await events(filed.id)).length,2,'No-op produces no event');
  for(const invalid of [{title:''},{notes:'x'.repeat(1001)},{issued_on:'2027-01-02'},{contract_start:null},{category:'other'},{contract_end:'2025-01-01'}]) {
    await deny('select public.save_employee_document($1,$2)',['edit',JSON.stringify({...edited,...invalid})],['23514']);
  }
  eq((await events(filed.id)).length,2,'Invalid details roll back revision and event');
  const archived=await write('archive',{id:filed.id,revision:2});eq([archived.archived,archived.revision],[true,3],'Archive retains filed document');
  eq((await events(filed.id)).at(-1).kind,'archived','Archive is audited');
  eq((await db.query('select * from storage.objects where name=$1',[filed.object_name])).rows.length,1,'Archived file remains downloadable to HR');
  const restored=await write('restore',{id:filed.id,revision:3});eq([restored.archived,restored.revision],[false,4],'Restore reactivates same file');
  await deny('select public.save_employee_document($1,$2)',['discard',JSON.stringify({id:filed.id})],['42501']);
  for(const sql of ["update public.employee_document_events set actor_name='Forged' where document_id=$1","delete from public.employee_document_events where document_id=$1"])await deny(sql,[filed.id],['42501']);
  const discarded=await write('reserve',payload(person.id));
  await db.query('insert into storage.objects(bucket_id,name,owner_id,metadata) values($1,$2,$3,$4)',['employee-documents',discarded.object_name,owner,{size:123,mimetype:'application/pdf'}]);
  eq((await write('discard',{id:discarded.id})).state,'discarded','Discard creates permanent tombstone before cleanup');
  await deny('select public.save_employee_document($1,$2)',['finalize',JSON.stringify({id:discarded.id})],['42501']);
  eq((await db.query('delete from storage.objects where bucket_id=$1 and name=$2 returning id',['employee-documents',discarded.object_name])).rows.length,1,'Only discarded bytes may be cleaned up');
  eq((await write('discard',{id:discarded.id})).state,'discarded','Cleanup can be retried safely');
  eq((await db.query('select * from public.employee_documents where id=$1',[discarded.id])).rows.length,1,'Discard does not erase the reserved metadata');
  for(const invalid of [{byte_size:0},{byte_size:10485761},{original_filename:'../private.pdf'},{original_filename:'a.exe'},{mime_type:'text/html'},{sha256:'BAD'},{category:'unknown'}]) {
    await deny('select public.save_employee_document($1,$2)',['reserve',JSON.stringify({...payload(person.id),...invalid})],['23514']);
  }
  const hr=randomUUID();await db.exec('reset role');await db.query("insert into auth.users(id,email,email_confirmed_at) values($1,'documents-hr@example.invalid',now())",[hr]);
  await session(owner);await db.query("select public.save_company_account($1,'documents-hr@example.invalid','hr_generalist',true,null,null)",[hr]);
  await session(hr);eq((await db.query('select id from public.employee_documents where id=$1',[filed.id])).rows.length,1,'HR can read documents');
  const hrEdit=await write('edit',{...restored,notes:'HR verified'});eq((await events(filed.id)).at(-1).actor_id,hr,'HR metadata changes record actual actor');
  eq((await events(filed.id)).at(-1).actor_role,'hr_generalist','HR audit role is protected');
  for(const actor of ['00000000-0000-0000-0000-000000000102','00000000-0000-0000-0000-000000000103',unassigned]) {
    await session(actor);eq((await db.query('select * from public.employee_documents')).rows.length,0,'Manager/Employee/unassigned cannot read document metadata');
    eq((await db.query('select * from public.employee_document_events')).rows.length,0,'Manager/Employee/unassigned cannot read document history');
    eq((await db.query("select * from storage.objects where bucket_id='employee-documents'")).rows.length,0,'Manager/Employee/unassigned cannot download private document bytes');
    await deny('select public.save_employee_document($1,$2)',['edit',JSON.stringify({...hrEdit,title:'Denied'})],['42501']);
    await deny('insert into storage.objects(bucket_id,name,owner_id) values($1,$2,$3)',['employee-documents',`${company.id}/${person.id}/${randomUUID()}/file.pdf`,actor],['42501']);
  }
  await session(owner);await db.query("select public.save_company_account($1,'documents-hr@example.invalid','hr_generalist',false,null,null)",[hr]);
  await session(hr);eq((await db.query('select * from public.employee_documents')).rows.length,0,'Disabled HR loses metadata access immediately');
  eq((await db.query("select * from storage.objects where bucket_id='employee-documents'")).rows.length,0,'Disabled HR loses Storage access immediately');
  await deny('select public.save_employee_document($1,$2)',['reserve',JSON.stringify(payload(person.id))],['42501']);
  await session('','anon');for(const table of ['employee_documents','employee_document_events'])await deny('select * from public.'+table,[],['42501']);
  await deny('select public.save_employee_document($1,$2)',['reserve',JSON.stringify(payload(person.id))],['42501']);
  await session(owner);await db.query("update public.employees set status='archived' where id=$1",[person.id]);
  eq((await db.query('select * from public.employee_documents where employee_id=$1',[person.id])).rows.length,2,'Employee archive retains filed and discarded document metadata');
  eq((await db.query('select * from storage.objects where name=$1',[filed.object_name])).rows.length,1,'Employee archive retains private file');
  // Even a trusted bypass-RLS writer cannot silently erase audit history or replace a filed file.
  await db.exec('reset role');await deny('delete from public.employee_documents where id=$1',[filed.id],['42501']);
  await deny("update public.employee_documents set sha256=$1,revision=revision+1 where id=$2",['c'.repeat(64),filed.id],['42501']);
  await deny('delete from public.employee_document_events where document_id=$1',[filed.id],['42501']);
  await session(owner);
}
