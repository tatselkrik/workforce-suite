// Real browser/HTML/CSS checks with fictional local API responses only.
// This does not establish hosted authorization, payroll calculation or filing.
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const http=require('node:http');
const crypto=require('node:crypto');
const root=path.resolve(__dirname,'..'),web=path.join(root,'employee-portal/web');
const version=fs.readFileSync(path.join(root,'employee-portal/Cargo.toml'),'utf8').match(/^version = "([^"]+)"/m)[1];
const run=process.argv[2]||'run1';assert(/^[a-z0-9-]{1,40}$/.test(run),'Use a simple run label');
const out=path.join(root,'work','portal'+version.replaceAll('.','')+'-fixture-'+run);
fs.mkdirSync(out); // Preserve earlier evidence; never silently overwrite a run.
const id=n=>'00000000-0000-4000-8000-'+String(n).padStart(12,'0');
const today=new Intl.DateTimeFormat('en-CA',{timeZone:'Asia/Manila'}).format(new Date());
const stamp=(date,t)=>date+'T'+t+'Z';
const earlier=new Date(Date.parse(today+'T00:00Z')-86400000).toISOString().slice(0,10);
const attendance={id:id(1),revision:4,work_date:today,checked_in_at:stamp(today,'00:00:00'),checked_out_at:null};
const state={logged:false,attendance,breaks:[{id:id(2),rule_id:id(3),name:'Lunch',paid:false,required:true,
 expected_minutes:60,revision:1,started_at:stamp(today,'04:00:00'),ended_at:null}],leave:[],requests:[],notices:[]};
const me={csrf:id(4),company:{name:'Fictional Northstar QA',timezone:'Asia/Manila'},email:'visual@example.invalid',
 employment:{employee_code:'000045',full_name:'Fictional Mar\u00eda Dela Cruz',department:'People Operations',job_title:'Support Analyst',
 joined_on:'2026-01-15',annual_leave_days:15,employment_type:'Regular',work_location:'Fictional local office',reports_to:'Fictional Manager'},
 contacts:{phone:'',personal_email:'',address:'',emergency_name:'',emergency_relationship:'',emergency_phone:'',emergency_alternate_phone:''}};
state.leave=[{id:id(10),revision:1,type_name:'Annual leave',starts_on:earlier,ends_on:earlier,day_part:'full',charged_units:2,
 status:'approved',reason:'Fictional approved leave',approvals:[{kind:'manager',approved_at:stamp(earlier,'01:00:00'),approved_by:'Fictional Manager'}]},
 {id:id(11),revision:1,type_name:'Sick leave',starts_on:'2026-11-02',ends_on:'2026-11-02',day_part:'am',charged_units:1,
 status:'pending',reason:'Fictional pending request',approvals:[{kind:'hr',approved_at:null,approved_by:null}]}];
state.requests=[{id:id(20),revision:1,kind:'correction',status:'approved',work_date:earlier,proposed_in:stamp(earlier,'00:05:00'),
 proposed_out:stamp(earlier,'09:00:00'),reason:'Fictional corrected punch',approved_minutes:null},
 {id:id(21),revision:1,kind:'overtime',status:'approved',work_date:earlier,requested_minutes:45,approved_minutes:30,reason:'Fictional partially approved overtime'}];
const totals={basic:2000000,paid_leave:100000,unworked_holiday:100000,premium:20000,overtime:12345,night:6789,allowances:150000,
 gross:2389134,reimbursement:50000,sss_employee:100000,philhealth_employee:50000,pagibig_employee:20000,withholding:50000,
 voluntary_deductions:10000,advances:0,net:2209134};
const slip={id:id(30),month:'2026-09-01',net:totals.net,company_name:me.company.name,rule_version:'FICTIONAL-QA-2026',
 employee:{employee_code:'000045',employee_name:me.employment.full_name,department:me.employment.department},totals,
 components:[{label:'Fictional transport allowance',kind:'allowance',amount_cents:150000}],adjustments:[]};
state.notices=[{key:'payroll-monthly-issued:'+id(30)+':1',kind:'payroll-monthly-issued',title:'Your September payslip is ready',detail:'Fictional own issued statement.',date:earlier,target_page:'pay',target_id:id(30),read:false,urgent:false},
 {key:'leave-decision:'+id(10)+':1',kind:'leave-decision',title:'Your leave was approved',detail:'Fictional saved decision and charge.',date:earlier,target_page:'leave',target_id:id(10),read:false,urgent:false},
 {key:'training-due:'+id(31)+':1',kind:'training-due',title:'Fictional training reminder',detail:'Review the schedule with HR; this notification is informational.',date:today,target_page:'notifications',target_id:id(31),read:true,urgent:true}];
const history=[{status:'pending',actor_name:me.employment.full_name,actor_role:'employee',created_at:stamp(earlier,'01:00:00'),reason:'Fictional submission',approved_minutes:null},
 {status:'approved',actor_name:'Fictional Manager',actor_role:'manager',created_at:stamp(earlier,'02:00:00'),reason:'Fictional independent review',approved_minutes:30}];
const checks=[],screenshots=[],errors=[],requests=[],external=[];
const check=name=>checks.push(name);
const photo=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAJ0lEQVR4nGNgGPKAEZnDUVf3nxhNP5qa4PqYKHUB06gBDKNhMCwAANcZBBQ3Au3RAAAAAElFTkSuQmCC','base64');
function json(res,status,value){res.writeHead(status,{'Content-Type':'application/json; charset=utf-8','Cache-Control':'no-store'});res.end(JSON.stringify(value));}
async function body(req){let n=0,chunks=[];for await(const c of req){n+=c.length;assert(n<1048576);chunks.push(c);}return JSON.parse(Buffer.concat(chunks).toString()||'{}');}
function leaveContext(asof){return{asof,policy_revision:7,workweek:[1,2,3,4,5],holidays:[{date:'2026-11-30',name:'Fictional holiday'}],types:[
 {code:'annual',name:'Annual leave',paid:true,unlimited:false,balance:{available_units:20,used_units:2,reserved_units:1}},
 {code:'sick',name:'Sick leave',paid:true,unlimited:false,balance:{available_units:10,used_units:0,reserved_units:1}}]};}
const server=http.createServer(async(req,res)=>{try{
 const u=new URL(req.url,'http://127.0.0.1'),p=u.pathname;
 if(p==='/'||/^\/(app\.css|(?:app|self-service|time-requests|notifications|pictures|account)\.js)$/.test(p)){
  const name=p==='/'?'index.html':p.slice(1);res.writeHead(200,{'Content-Type':name.endsWith('.css')?'text/css':name.endsWith('.js')?'text/javascript':'text/html; charset=utf-8'});res.end(fs.readFileSync(path.join(web,name)));return;}
 if(p==='/api/branding'){json(res,200,{company_name:me.company.name,has_logo:true});return;}
 if(p==='/api/branding/logo'||p==='/api/photo'||p==='/api/company-logo'){res.writeHead(200,{'Content-Type':'image/png'});res.end(photo);return;}
 if(p==='/api/sign-in'){await body(req);state.logged=true;res.setHeader('Set-Cookie','qa_fixture=local-only; HttpOnly; SameSite=Strict; Path=/');json(res,200,{csrf:me.csrf});return;}
 if(p==='/favicon.ico'){res.writeHead(204);res.end();return;}
 if(!state.logged){json(res,401,{error:'Your session ended. Sign in again.'});return;}
 requests.push({method:req.method,path:p});
 if(req.method==='POST'){
  const b=await body(req);assert.equal(req.headers['x-csrf-token'],me.csrf,'Fictional write lacks CSRF');
  if(p==='/api/leave/request'){assert.equal(b.policy_revision,7);assert.equal(b.day_part,'am');state.leave.push({id:id(12),revision:1,type_name:'Annual leave',...b,status:'pending',charged_units:1,approvals:[{kind:'manager',approved_at:null}]});}
  else if(p==='/api/leave/cancel'){const r=state.leave.find(r=>r.id===b.id);assert(r&&b.reason&&r.revision===b.revision);r.status='cancelled';r.revision++;}
  else if(p==='/api/time/correction'){assert.equal(b.attendance_id,id(1));assert(b.checked_in_local&&b.reason);state.requests.push({id:id(22),revision:1,kind:'correction',status:'pending',work_date:b.work_date,proposed_in:stamp(today,'00:05:00'),proposed_out:null,reason:b.reason,approved_minutes:null});}
  else if(p==='/api/time/overtime'){assert.equal(b.attendance_id,id(1));assert.equal(b.minutes,30);state.requests.push({id:id(23),revision:1,kind:'overtime',status:'pending',work_date:today,requested_minutes:b.minutes,approved_minutes:null,reason:b.reason});}
  else if(p==='/api/time/cancel'){const r=state.requests.find(r=>r.id===b.id&&r.kind===b.kind);assert(r&&r.revision===b.revision&&b.reason);r.status='cancelled';r.revision++;}
  else if(p==='/api/breaks/end'){assert.equal(b.attendance_id,id(1));state.breaks[0].ended_at=stamp(today,'04:30:00');state.breaks[0].revision++;}
  else if(p==='/api/breaks/start'){assert.equal(b.rule_id,id(3));state.breaks=[{id:id(2),rule_id:id(3),name:'Lunch',paid:false,required:true,expected_minutes:60,revision:1,started_at:stamp(today,'04:00:00'),ended_at:null}];}
  else if(p==='/api/attendance/check-out'){assert.equal(b.id,id(1));state.attendance.checked_out_at=stamp(today,'09:30:00');}
  else if(p==='/api/notifications/read'){if(b.reset)state.notices.forEach(r=>r.read=false);else for(const r of state.notices)if(b.keys.includes(r.key))r.read=b.read;}
  else if(p==='/api/sign-out'){state.logged=false;res.setHeader('Set-Cookie','qa_fixture=; Max-Age=0; Path=/');}
  else throw Error('Unexpected fictional write '+p);
  json(res,200,{saved:true});return;
 }
 if(p==='/api/me')json(res,200,me);
 else if(p==='/api/attendance')json(res,200,[state.attendance,{id:id(5),work_date:earlier,checked_in_at:stamp(earlier,'00:00:00'),checked_out_at:stamp(earlier,'09:00:00'),revision:1}]);
 else if(p==='/api/payslips')json(res,200,[{id:slip.id,month:slip.month,net:slip.net}]);
 else if(p==='/api/payslips/'+slip.id)json(res,200,slip);
 else if(p==='/api/pictures')json(res,200,{has_logo:true});
 else if(p==='/api/account')json(res,200,{email:me.email,pending_email:null,confirmation_pending:false,idle_timeout_minutes:30});
 else if(p==='/api/leave/context')json(res,200,leaveContext(u.searchParams.get('asof')||today));
 else if(p==='/api/leave')json(res,200,state.leave);
 else if(p.startsWith('/api/breaks/'))json(res,200,{attendance:state.attendance,records:state.breaks,rules:[{id:id(3),name:'Lunch',paid:false,required:true,minutes:60}]});
 else if(p==='/api/time/context')json(res,200,{work_date:today,timezone:me.company.timezone,attendance:state.attendance,candidate_minutes:45,current_approved_minutes:0,
  correction_pending:state.requests.some(r=>r.work_date===today&&r.kind==='correction'&&r.status==='pending'),
  overtime_blocked:state.requests.some(r=>r.work_date===today&&r.kind==='overtime'&&r.status==='pending'),break_revisions:[]});
 else if(p==='/api/time/requests')json(res,200,state.requests);
 else if(p.startsWith('/api/time/history/'))json(res,200,history);
 else if(p==='/api/notifications'){
  const unread=u.searchParams.get('unread')==='true',offset=Number(u.searchParams.get('offset')||0),items=state.notices.filter(r=>!unread||!r.read);
  json(res,200,{offset,limit:25,unread_only:unread,total_items:items.length,unread_count:state.notices.filter(r=>!r.read).length,generated_at:new Date().toISOString(),items:items.slice(offset,offset+25)});
 }else throw Error('Unexpected fictional read '+p);
}catch(e){errors.push(e.message);json(res,500,{error:'Fictional fixture rejected a request.'});}});
function rgb(s){return s.match(/[\d.]+/g).slice(0,3).map(Number).map(v=>v/255).map(v=>v<=.04045?v/12.92:((v+.055)/1.055)**2.4);}
function contrast(a,b){const l=x=>{const r=rgb(x);return .2126*r[0]+.7152*r[1]+.0722*r[2];};return(Math.max(l(a),l(b))+.05)/(Math.min(l(a),l(b))+.05);}
(async()=>{await new Promise(r=>server.listen(0,'127.0.0.1',r));const origin='http://127.0.0.1:'+server.address().port;
 let browser,failure;
 try{
  browser=await chromium.launch({headless:true,channel:'msedge'});const context=await browser.newContext({viewport:{width:1440,height:1000},locale:'en-PH',acceptDownloads:true});
  await context.addInitScript(()=>{window.__printClicks=0;window.print=()=>window.__printClicks++;});
  const page=await context.newPage();page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(!r.url().startsWith(origin)&&!r.url().startsWith('blob:')&&!r.url().startsWith('data:'))external.push(r.url().split('/').slice(0,3).join('/'));});
  const idle=()=>page.waitForFunction(()=>!document.querySelector('#refresh').disabled,null,{timeout:10000});
  const nav=async name=>{await page.locator('nav [data-page="'+name+'"]').click();await page.locator('#page-'+name).waitFor({state:'visible'});};
  const shot=async name=>{await page.screenshot({path:path.join(out,name+'.png'),fullPage:true});screenshots.push(name);};
  await page.goto(origin,{waitUntil:'networkidle'});await page.locator('#email').fill(me.email);await page.locator('#password').fill('Fictional fixture only');await page.locator('#password').press('Enter');await page.locator('#workspace').waitFor({state:'visible'});await idle();
  for(const scheme of['light','dark'])for(const width of[320,360,390,640,1024,1440]){
   await page.setViewportSize({width,height:844});await page.emulateMedia({colorScheme:scheme});await nav('home');
   for(const button of await page.locator('nav [data-page]').all()){
    const box=await button.boundingBox();assert(box.x>=0&&box.x+box.width<=width+.5,'Clipped nav at '+width);assert(box.width>=44&&box.height>=44,'Small navigation target');
    await button.click();await button.hover();const c=await button.evaluate(e=>({color:getComputedStyle(e).color,background:getComputedStyle(e).backgroundColor}));
    assert(contrast(c.color,c.background)>=4.5,'Unreadable selected hover at '+width);await page.keyboard.press('Tab');await button.focus();
    assert.notEqual(await button.evaluate(e=>getComputedStyle(e).outlineStyle),'none','Missing keyboard focus');
   }
   assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),'Document overflow');check(scheme+' '+width+'px navigation fit, touch size, hover contrast and keyboard focus');
   if(width===320)await shot(scheme+'-320-profile');
  }
  await page.setViewportSize({width:390,height:844});await page.emulateMedia({colorScheme:'light'});await nav('attendance');
  assert(await page.getByRole('button',{name:'Check out',exact:true}).isDisabled());await shot('mobile-active-break');
  await page.getByRole('button',{name:'End break',exact:true}).click();await idle();assert((await page.locator('#break-controls').innerText()).includes('30 whole minutes'));check('actual break duration and checkout guard rendered');
  state.breaks=[];await page.locator('#refresh').click();await idle();await page.getByRole('button',{name:'Start break',exact:true}).click();await idle();assert(await page.getByRole('button',{name:'Check out',exact:true}).isDisabled());await page.getByRole('button',{name:'End break',exact:true}).click();await idle();await page.getByRole('button',{name:'Check out',exact:true}).click();await idle();check('fixture start/end break and checkout button flows');
  await page.locator('#correction-reason').fill('Fictional missing punch review');await page.locator('#correction-submit').click();await idle();
  const correction=page.locator('#time-request-correction-'+id(22));await correction.getByRole('button',{name:'Submission & decision history',exact:true}).click();await idle();assert((await correction.innerText()).includes('Fictional independent review'));check('populated correction submission and decision history');
  await correction.getByRole('button',{name:'Cancel request',exact:true}).click();await correction.locator('textarea').fill('Fictional cancellation');await correction.getByRole('button',{name:'Save cancellation',exact:true}).click();await idle();
  await page.locator('#overtime-minutes').fill('30');await page.locator('#overtime-reason').fill('Fictional overtime review');await page.locator('#overtime-submit').click();await idle();await shot('mobile-time-history');check('fixture correction cancellation and overtime request');
  await nav('leave');await page.locator('#leave-start').fill('2026-11-02');await page.locator('#leave-start').dispatchEvent('change');await page.locator('#leave-end').fill('2026-11-02');await page.locator('#leave-end').dispatchEvent('change');await page.locator('#leave-part').selectOption('am');await page.waitForFunction(()=>!document.querySelector('#leave-submit').disabled);assert((await page.locator('#leave-estimate').innerText()).includes('0.5 days'));
  await page.locator('#leave-reason').fill('Fictional half-day');await page.locator('#leave-submit').click();await idle();const leave=page.locator('#leave-request-'+id(12));assert((await leave.innerText()).includes('0.5 days'));
  await leave.getByRole('button',{name:'Cancel request',exact:true}).click();await leave.locator('textarea').fill('Fictional changed plans');await leave.getByRole('button',{name:'Save cancellation',exact:true}).click();await idle();await shot('mobile-leave-history');check('half-day preview, filed charge, approval and cancellation history');
  await nav('pay');await page.getByRole('button',{name:'Open payslip',exact:true}).click();await idle();await shot('mobile-payslip');
  const downloadPromise=page.waitForEvent('download');await page.getByRole('button',{name:'Download CSV',exact:true}).click();const download=await downloadPromise;await download.saveAs(path.join(out,'payslip.csv'));assert.equal(download.suggestedFilename(),'payslip-2026-09.csv');const csv=fs.readFileSync(path.join(out,'payslip.csv'),'utf8');assert(csv.startsWith('\ufeff'));assert(csv.includes('"Employee code","000045"'));assert(csv.includes('"Net due","22091.34"'));assert(csv.includes('Mar\u00eda'));check('real browser CSV download retains cents, UTF-8 and leading-zero code');
  await page.setViewportSize({width:1440,height:1000});await page.getByRole('button',{name:'Print / save PDF',exact:true}).click();assert.equal(await page.evaluate(()=>window.__printClicks),1);for(const colorScheme of ['light','dark']){await page.emulateMedia({media:'print',colorScheme});const style=await page.locator('.payslip .intro').evaluate(e=>({month:e.textContent,display:getComputedStyle(e).display,background:getComputedStyle(document.documentElement).backgroundColor}));assert.equal(style.month,'September 2026');assert.notEqual(style.display,'none');assert.equal(style.background,'rgb(255, 255, 255)');await page.pdf({path:path.join(out,colorScheme==='light'?'payslip.pdf':'payslip-dark.pdf'),format:'A4',printBackground:true,margin:{top:'12mm',bottom:'12mm',left:'12mm',right:'12mm'}});}await page.emulateMedia({media:'screen',colorScheme:'light'});await shot('desktop-payslip');check('print button dispatch and light/dark Chromium print PDFs generated');
  await page.locator('#notifications-button').click();await page.locator('#page-notifications').waitFor({state:'visible'});await idle();await shot('desktop-inbox');
  assert.equal(await page.locator('.notification-card').count(),3);const info=page.locator('.notification-card').filter({hasText:'Fictional training reminder'});assert.equal(await info.getByRole('button',{name:'Open record',exact:true}).count(),0);assert((await info.innerText()).includes('Contact HR'));check('informational lifecycle notice does not promise missing record view');
  await page.locator('#inbox-mark-shown').click();await idle();assert((await page.locator('#inbox-summary').innerText()).startsWith('0 unread'));await page.locator('#inbox-unread').check();await idle();assert.equal(await page.locator('.notification-card').count(),0);await page.locator('#inbox-reset').click();await idle();assert.equal(await page.locator('.notification-card').count(),3);check('populated inbox read/unread filter and reset controls');
  const notice=page.locator('.notification-card').filter({hasText:'Your September payslip is ready'});await notice.getByRole('button',{name:'Open record',exact:true}).click();await idle();assert(await page.locator('#page-pay').isVisible());check('payslip notification opens actual pay page');
  assert.equal(errors.length,0,'Fixture/browser errors');assert.equal(external.length,0,'Unexpected external request');
 }catch(e){failure=e.message;try{const p=browser?.contexts()[0]?.pages()[0];if(p){await p.screenshot({path:path.join(out,'failure.png'),fullPage:true});screenshots.push('failure');}}catch{}}
 finally{if(browser)await browser.close();await new Promise(r=>server.close(r));const assets=Object.fromEntries(fs.readdirSync(web).map(n=>[n,crypto.createHash('sha256').update(fs.readFileSync(path.join(web,n))).digest('hex')]));
  const report={all_passed:!failure,failure:failure||null,application_version:version,fictional_local_api:true,hosted_scope_tested:false,real_print_dialog_tested:false,physical_phone_tested:false,checks,screenshots,errors,external,assets,requests};fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({all_passed:report.all_passed,failure:report.failure,checks:checks.length,screenshots:screenshots.length}));if(failure)process.exitCode=1;
 }
})().catch(e=>{console.error(e.message);process.exitCode=1;});
