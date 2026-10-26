use super::*;
use crate::payroll_exchange as x;
use super::payroll_view::field;

#[derive(Default)]
pub(super) struct State{
 scope:Option<(Uuid,Uuid,Role)>,pub(super) state:Option<x::State>,pub(super) detail:Option<x::Detail>,
 preview:Option<x::Preview>,request:Option<x::Request>,pending:Option<x::Operation>,edit:Option<x::ProfileChange>,
 profile:Option<Uuid>,employee:Option<Uuid>,lo:String,hi:String,axis:String,kind:String,search:String,reason:String,
 ack_reason:String,ack:Option<x::Ack>,ack_page:usize,reset_key:String,reset_reason:String,tab:usize,page:usize,
}
#[cfg(test)]
mod tests{
 use super::*;
 fn draft()->x::ProfileChange{x::ProfileChange{request_id:Uuid::from_u128(20),id:Uuid::from_u128(21),expected_revision:None,name:"Fictional exchange".into(),enabled:true,columns:x::default_columns(),reason:"Fictional review".into()}}
 fn state(d:&Snapshot)->x::State{x::State{version:"payroll-exchange-state-v1".into(),company_id:d.company.id,user_id:d.access.user_id,role:d.access.role,profiles:vec![],history:vec![]}}
 #[test]fn a_read_only_refresh_never_clears_an_uncertain_write(){let d=DemoStore::new().data;let mut s=State::default();s.bind(&d);let change=draft();s.edit=Some(change.clone());s.pending=Some(x::Operation::SaveProfile(change.clone()));s.accept(x::Update{state:Some(state(&d)),..Default::default()},&d);assert!(s.pending.is_some());assert!(s.has_draft());s.accept(x::Update{profile_saved:Some(change.request_id),..Default::default()},&d);assert!(s.pending.is_none());assert!(s.edit.is_none());}
 #[test]fn missing_attempt_receipt_renews_identity_without_losing_profile_draft(){let d=DemoStore::new().data;let mut s=State::default();s.bind(&d);let change=draft();s.edit=Some(change.clone());s.pending=Some(x::Operation::SaveProfile(change.clone()));let receipt=x::Attempt{version:"payroll-exchange-attempt-v1".into(),company_id:d.company.id,user_id:d.access.user_id,role:d.access.role,kind:"profile".into(),id:change.request_id,committed:false};s.accept(x::Update{resolved:Some(receipt),..Default::default()},&d);assert!(s.pending.is_none());let retained=s.edit.unwrap();assert_ne!(retained.request_id,change.request_id);assert_eq!(retained.columns,change.columns);assert_eq!(retained.reason,change.reason);}
 #[test]fn changed_user_or_role_clears_private_exchange_state(){let mut d=DemoStore::new().data;let mut s=State::default();s.bind(&d);s.state=Some(state(&d));s.edit=Some(draft());d.access.role=Role::Manager;s.bind(&d);assert!(s.state.is_none());assert!(s.edit.is_none());assert!(!s.has_draft());}
 #[test]fn typed_export_and_acknowledgement_reasons_are_unsaved_work(){let d=DemoStore::new().data;let mut s=State::default();s.bind(&d);assert!(!s.has_draft());s.reason="Unsaved export note".into();assert!(s.has_draft());s.reason.clear();s.ack_reason="Unsaved acknowledgement note".into();assert!(s.has_draft());}
 #[test]fn completing_one_workflow_preserves_other_draft_notes(){let d=DemoStore::new().data;let mut s=State::default();s.bind(&d);s.reason="Other unsaved export note".into();s.ack_reason="Acknowledgement note".into();s.finish_attempt("ack",true);assert_eq!(s.reason,"Other unsaved export note");assert!(s.ack_reason.is_empty());s.ack_reason="Another acknowledgement draft".into();s.finish_attempt("prepare",true);assert!(s.reason.is_empty());assert_eq!(s.ack_reason,"Another acknowledgement draft");}
}
impl State{
 pub(super) fn has_draft(&self)->bool{self.edit.is_some()||self.pending.is_some()||self.ack.is_some()||self.request.is_some()||!self.reason.is_empty()||!self.ack_reason.is_empty()||!self.reset_reason.is_empty()}
 fn finish_attempt(&mut self,kind:&str,committed:bool){self.pending=None;match kind{
  "profile"=>{if committed{self.edit=None;}else if let Some(edit)=&mut self.edit{edit.request_id=Uuid::new_v4();}},
  "prepare"=>{self.request=None;self.preview=None;if committed{self.reason.clear();}},
  "ack"=>{if committed{self.ack=None;self.ack_reason.clear();self.reset_reason.clear();}else if let Some(ack)=&mut self.ack{ack.id=Uuid::new_v4();}},_=>{}
 }}
 fn bind(&mut self,d:&Snapshot){let scope=Some((d.company.id,d.access.user_id,d.access.role));if self.scope!=scope||!d.access.role.manages_hr(){let year=d.company.today().year();*self=Self{scope,lo:format!("{year}-01-01"),hi:format!("{year}-12-31"),axis:"issued".into(),kind:"all".into(),..Default::default()};}}
 pub(super) fn accept(&mut self,update:x::Update,d:&Snapshot){self.bind(d);if !d.access.role.manages_hr(){return;}
  if let Some(state)=update.state.filter(|s|s.validate(&d.access).is_ok()){self.state=Some(state);}
  if let Some(id)=update.profile_saved{if matches!(&self.pending,Some(x::Operation::SaveProfile(c))if c.request_id==id){self.pending=None;self.edit=None;}}
  if let Some(resolved)=update.resolved{if resolved.validate(&d.access,&resolved.kind,resolved.id).is_ok()&&self.pending.as_ref().and_then(x::Operation::attempt).is_some_and(|(kind,id,_)|kind==resolved.kind&&id==resolved.id){self.finish_attempt(&resolved.kind,resolved.committed);}}
  if let Some(preview)=update.preview{if self.request.as_ref().is_some_and(|r|preview.validate(&d.access,r).is_ok()){self.preview=Some(preview);self.page=0;}}
  if let Some(detail)=update.detail.filter(|v|v.validate(&d.access,v.basis.exchange_id).is_ok()){
   let completed=match &self.pending{Some(x::Operation::Prepare{request,fingerprint})if detail.basis.matches(request).is_ok()&&detail.basis.fingerprint==*fingerprint=>Some("prepare"),Some(x::Operation::Acknowledge(a))if detail.basis.exchange_id==a.exchange_id&&a.rows.iter().all(|r|detail.events.iter().any(|e|e.batch_id==a.id&&e.record_key==r.record_key&&e.revision==r.expected_revision+1&&e.status==r.status&&e.reference==r.reference&&e.note==r.note))=>Some("ack"),_=>None};
   if let Some(kind)=completed{self.finish_attempt(kind,true);}
   self.detail=Some(detail);self.page=0;
  }
 }
 fn filter(&self)->anyhow::Result<crate::payroll_reporting::Filter>{let f=crate::payroll_reporting::Filter{starts_on:self.lo.parse().map_err(|_|anyhow::anyhow!("Enter a valid starting date."))?,ends_on:self.hi.parse().map_err(|_|anyhow::anyhow!("Enter a valid ending date."))?,axis:self.axis.clone(),kind:self.kind.clone(),employee_id:self.employee,search:self.search.clone()};f.validate()?;Ok(f)}
}
fn facts(ui:&mut Ui,b:&x::Basis){
 ui.label(format!("{} · profile version{} · {} rows",b.profile.name,b.profile.revision+1,b.report.rows.len()));
 ui.label(format!("Exchange {}",b.exchange_id));
 ui.label(format!("Snapshot {} UTC",b.report.generated_at.format("%Y-%m-%d %H:%M:%S")));
 ui.collapsing("Integrity controls",|ui|{ui.label(format!("CSV SHA-256: {}",b.sha256));ui.label(format!("Source fingerprint: {}",b.fingerprint));});
 if let Ok(r)=crate::payroll_reporting::Report::from_wire(b.report.clone(),b.company_id,&b.report.filter){ui.horizontal_wrapped(|ui|{ui.label(format!("Recorded cash: {}",crate::payroll::money(r.totals.recorded_cash_cents)));ui.label(format!("Source offsets: {}",crate::payroll::money(r.totals.source_offset_cents)));});}
}
fn source_label(d:&x::Detail,key:&str)->String{d.source(key).map(|r|format!("{} · {} · {} · {} to {}",r.employee_code,r.employee_name,super::payroll_report_view::kind(&r.kind),r.period_start,r.period_end)).unwrap_or_else(||"Choose a recorded outcome".into())}
fn rows(ui:&mut Ui,b:&x::Basis,page:&mut usize,detail:Option<&x::Detail>){
 let pages=b.report.rows.len().div_ceil(15).max(1);*page=(*page).min(pages-1);
 ui.horizontal(|ui|{if ui.add_enabled(*page>0,egui::Button::new("Previous")).clicked(){*page-=1;}ui.label(format!("Page{}of{}",*page+1,pages));if ui.add_enabled(*page+1<pages,egui::Button::new("Next")).clicked(){*page+=1;}});
 egui::ScrollArea::horizontal().id_salt(("exchange_rows_scroll",b.exchange_id)).show(ui,|ui|{egui::Grid::new(("exchange_rows",b.exchange_id)).striped(true).spacing([18.,10.]).show(ui,|ui|{
  for h in["Employee","Source / state","Net (PHP)","Cash (PHP)","Offset (PHP)","Acknowledgement"]{ui.strong(h);}ui.end_row();
  for (r,c)in b.report.rows.iter().zip(&b.controls).skip(*page*15).take(15){ui.label(format!("{}\n{}",r.employee_code,r.employee_name));ui.label(format!("{}\n{}",super::payroll_report_view::kind(&r.kind),r.payment_state.replace('_'," ")));ui.label(r.net_cents.map(crate::payroll::money).unwrap_or_else(||"Unknown / n/a".into()));ui.label(crate::payroll::money(r.cash_cents));ui.label(crate::payroll::money(r.offset_cents));ui.label(detail.and_then(|d|d.current(&c.record_key)).map_or("Pending",|e|e.status.as_str()));ui.end_row();}
 });});
}
impl HrisApp{
 #[cfg(feature="qa-screenshot")]
 pub(super) fn prepare_exchange_preview(&mut self,form:&str){
  self.prepare_consolidated_report_preview("payroll-report-amendment");self.payroll_state.tab=14;
  let s=&mut self.payroll_state.exchange;s.bind(&self.data);
  let profile=x::Profile{id:Uuid::from_u128(53001),revision:0,name:"Fictional accounting worksheet".into(),enabled:true,columns:x::default_columns(),created_at:chrono::Utc::now()};
  s.profile=Some(profile.id);s.reason="Fictional reviewed monthly export".into();let filter=s.filter().unwrap();
  let report=crate::payroll_reporting::demo(&self.data,&filter,chrono::Utc::now()).unwrap();
  let request=x::Request{id:Uuid::from_u128(53002),profile_id:profile.id,expected_revision:0,filter,reason:s.reason.clone()};
  let controls=report.wire.rows.iter().map(|r|x::Control{record_key:format!("{}:{}",r.kind,r.id),row_sha256:x::sha(&serde_json::to_vec(r).unwrap())}).collect();
  let mut basis=x::Basis{company_id:self.data.company.id,exchange_id:request.id,profile:profile.clone(),report:report.wire,controls,csv:String::new(),sha256:String::new(),fingerprint:"a".repeat(64)};
  basis.csv=basis.render().unwrap();basis.sha256=x::sha(basis.csv.as_bytes());basis.validate(&self.data.access).unwrap();
  let mut detail=x::Detail{version:"payroll-exchange-detail-v1".into(),company_id:self.data.company.id,user_id:self.data.access.user_id,role:self.data.access.role,basis:basis.clone(),created_at:chrono::Utc::now(),events:vec![]};
  for(i,c)in basis.controls.iter().take(2).enumerate(){detail.events.push(x::Event{record_key:c.record_key.clone(),revision:1,batch_id:Uuid::from_u128(53003),status:if i==0{"accepted"}else{"rejected"}.into(),reference:"Fictional local review reference".into(),note:if i==0{"Confirmed in a fictional worksheet"}else{"Department mapping needs review"}.into(),created_at:chrono::Utc::now()});}
  s.state=Some(x::State{version:"payroll-exchange-state-v1".into(),company_id:self.data.company.id,user_id:self.data.access.user_id,role:self.data.access.role,profiles:vec![profile.clone()],history:vec![x::History{id:request.id,profile_name:profile.name.clone(),profile_revision:0,row_count:basis.controls.len(),sha256:basis.sha256.clone(),created_at:detail.created_at,accepted:1,rejected:usize::from(basis.controls.len()>1)}]});
  s.request=Some(request.clone());s.preview=Some(x::Preview{version:"payroll-exchange-preview-v1".into(),company_id:self.data.company.id,user_id:self.data.access.user_id,role:self.data.access.role,basis});
  if form.ends_with("profile"){s.tab=2;s.request=None;s.preview=None;s.reason.clear();s.edit=Some(x::ProfileChange{request_id:Uuid::from_u128(53004),id:profile.id,expected_revision:Some(0),name:profile.name,enabled:true,columns:profile.columns,reason:"Fictional column review".into()});}
  if form.ends_with("history")||form.ends_with("reconcile"){s.tab=1;s.preview=None;s.request=None;s.detail=Some(detail);}
  if form.ends_with("reconcile"){s.ack_reason="Reviewed fictional response".into();let d=s.detail.as_ref().unwrap();if let Some(c)=d.basis.controls.iter().find(|c|d.current(&c.record_key).is_none()){s.ack=Some(x::Ack{id:Uuid::from_u128(53005),exchange_id:d.basis.exchange_id,reason:s.ack_reason.clone(),rows:vec![x::AckRow{record_key:c.record_key.clone(),row_sha256:c.row_sha256.clone(),expected_revision:0,status:"accepted".into(),reference:"Fictional reviewed acknowledgement".into(),note:"Local-only fixture".into()}]});}}
  if form.ends_with("uncertain"){s.pending=Some(x::Operation::Prepare{request,fingerprint:"a".repeat(64)});}
  if form.ends_with("manager"){self.data.access.role=Role::Manager;self.payroll_state.tab=3;}
 }
 pub(super) fn payroll_exchange(&mut self,ui:&mut Ui){
  self.payroll_state.exchange.bind(&self.data);if !self.data.access.role.manages_hr(){return;}
  ui.heading("Data exchange");ui.label("Prepare local payroll CSV files, save their original snapshots and record reconciliation.");
  ui.label("CSV amounts use PHP centavos: PHP1.00 = 100. Cash, source net, final-pay offsets and tax advice have separate columns; unknown amounts stay blank.");
  let mut operation=None;let s=&mut self.payroll_state.exchange;let locked=self.busy||s.pending.is_some();
  ui.horizontal_wrapped(|ui|{
   if ui.add_enabled(!self.busy,egui::Button::new("Refresh exchange history")).clicked(){operation=Some(x::Operation::State);}
   for(i,t)in["Prepare export","Export history","Column profiles"].iter().enumerate(){if ui.selectable_label(s.tab==i,*t).clicked(){s.tab=i;}}
   ui.add_enabled(false,egui::Button::new("Connect accounting service")).on_disabled_hover_text("Deferred until company software and an approved no-charge target are selected.");
  });
  if let Some(pending)=s.pending.as_ref(){card(ui,|ui|{ui.label("An exact submission is retained. Retry it after a connection failure, or resolve its saved outcome. Resolving an unused identity closes it against delayed requests before you edit again.");if ui.add_enabled(!self.busy,egui::Button::new("Retry exact submission")).clicked(){operation=Some(pending.clone());}if ui.add_enabled(!self.busy,egui::Button::new("Resolve retained attempt")).clicked(){if let Some((kind,id,payload))=pending.attempt(){operation=Some(x::Operation::Resolve{kind,id,payload});}}});}
  if s.state.is_none(){ui.label("Refresh to load private profiles and export history.");}
  match s.tab{
   2=>{
    ui.label("Choose the columns and headers for your local CSV file. Identity, payment state and money-unit controls are included automatically.");
    if self.data.access.role==Role::Administrator&&ui.add_enabled(!locked&&s.ack.is_none()&&s.request.is_none(),egui::Button::new("New column profile")).clicked(){s.edit=Some(x::ProfileChange{request_id:Uuid::new_v4(),id:Uuid::new_v4(),expected_revision:None,name:"Local payroll exchange".into(),enabled:true,columns:x::default_columns(),reason:String::new()});}
    if let Some(state)=&s.state{for p in &state.profiles{ui.horizontal_wrapped(|ui|{ui.label(format!("{} · v{} · {} columns · {}",p.name,p.revision+1,p.columns.len(),if p.enabled{"Enabled"}else{"Disabled"}));if self.data.access.role==Role::Administrator&&ui.add_enabled(!locked&&p.revision<99&&s.request.is_none()&&s.ack.is_none(),egui::Button::new("Edit new version")).clicked(){s.edit=Some(x::ProfileChange{request_id:Uuid::new_v4(),id:p.id,expected_revision:Some(p.revision),name:p.name.clone(),enabled:p.enabled,columns:p.columns.clone(),reason:String::new()});}});}}
    let mut close_edit=false;
    if let Some(e)=&mut s.edit{
     ui.separator();ui.add_enabled_ui(!locked,|ui|{field(ui,"Profile name",&mut e.name);ui.checkbox(&mut e.enabled,"Enabled for new exports");field(ui,"Private change reason",&mut e.reason);
      ui.label("Selected columns, in export order. Rename headers below.");let mut move_row=None;let mut remove=None;
      for(i,c)in e.columns.iter_mut().enumerate(){ui.horizontal_wrapped(|ui|{ui.label(&c.key);ui.add(egui::TextEdit::singleline(&mut c.header).desired_width(190.).char_limit(64));if ui.add_enabled(i>0,egui::Button::new("Up")).clicked(){move_row=Some((i,i-1));}if ui.button("Remove").clicked(){remove=Some(i);}});}
      if let Some((a,b))=move_row{e.columns.swap(a,b);}if let Some(i)=remove{e.columns.remove(i);}
      egui::ComboBox::from_id_salt("exchange_add_column").selected_text("Add a field").show_ui(ui,|ui|{for k in x::KEYS{if !e.columns.iter().any(|c|c.key==*k)&&ui.button(*k).clicked(){e.columns.push(x::Column{key:(*k).into(),header:(*k).into()});ui.close();}}});
      match e.validate(){Ok(())=>{if ui.button("Save profile version").clicked(){operation=Some(x::Operation::SaveProfile(e.clone()));}},Err(err)=>{ui.label(RichText::new(err.to_string()).color(muted(ui)));}}
      if ui.button("Close profile draft").clicked(){close_edit=true;}
     });
    }
    if close_edit{s.edit=None;}
   },
   1=>{
    if let Some(state)=&s.state{for h in &state.history{card(ui,|ui|{ui.horizontal_wrapped(|ui|{ui.label(format!("{} · {} UTC · {} rows",h.profile_name,h.created_at.format("%Y-%m-%d %H:%M"),h.row_count));ui.label(format!("{} accepted / {} rejected / {} pending",h.accepted,h.rejected,h.row_count-h.accepted-h.rejected));if ui.add_enabled(!self.busy,egui::Button::new("Open frozen export")).clicked(){operation=Some(x::Operation::Detail{id:h.id,destination:None});}});});}}
   },
   _=>{
    let old=(s.profile,s.employee,s.lo.clone(),s.hi.clone(),s.axis.clone(),s.kind.clone(),s.search.clone(),s.reason.clone());
    let export_locked=locked||s.edit.is_some()||s.ack.is_some();
    ui.add_enabled_ui(!export_locked,|ui|{
     egui::ComboBox::from_id_salt("exchange_profile").selected_text(s.profile.and_then(|id|s.state.as_ref()?.profiles.iter().find(|p|p.id==id).map(|p|p.name.clone())).unwrap_or_else(||"Choose a column profile".into())).show_ui(ui,|ui|{if let Some(state)=&s.state{for p in state.profiles.iter().filter(|p|p.enabled){ui.selectable_value(&mut s.profile,Some(p.id),format!("{} · v{}",p.name,p.revision+1));}}});
     ui.columns(2,|c|{field(&mut c[0],"From (YYYY-MM-DD)",&mut s.lo);field(&mut c[1],"Through (YYYY-MM-DD)",&mut s.hi);});
     ui.horizontal_wrapped(|ui|{egui::ComboBox::from_id_salt("exchange_axis").selected_text(if s.axis=="received"{"Payment / settlement date"}else{"Approval date"}).show_ui(ui,|ui|{ui.selectable_value(&mut s.axis,"issued".into(),"Approval date");ui.selectable_value(&mut s.axis,"received".into(),"Payment / settlement date");});egui::ComboBox::from_id_salt("exchange_kind").selected_text(super::payroll_report_view::kind(&s.kind)).show_ui(ui,|ui|{for k in["all","monthly","cutoff","thirteenth","annual_tax","final","amendment"]{ui.selectable_value(&mut s.kind,k.into(),super::payroll_report_view::kind(k));}});egui::ComboBox::from_id_salt("exchange_employee").selected_text(s.employee.map(|id|self.data.employee_name(id)).unwrap_or("All employees")).show_ui(ui,|ui|{ui.selectable_value(&mut s.employee,None,"All employees");for e in self.data.employees.iter().filter(|e|e.company_id==self.data.company.id){ui.selectable_value(&mut s.employee,Some(e.id),format!("{} · {}",e.employee_code,e.full_name));}});});
     field(ui,"Search saved employee code, name or department",&mut s.search);field(ui,"Private export reason",&mut s.reason);
    });
    if old!=(s.profile,s.employee,s.lo.clone(),s.hi.clone(),s.axis.clone(),s.kind.clone(),s.search.clone(),s.reason.clone()){s.preview=None;s.request=None;}
    let selected=s.profile.and_then(|id|s.state.as_ref()?.profiles.iter().find(|p|p.id==id&&p.enabled));let filter=s.filter();
    if ui.add_enabled(!export_locked&&selected.is_some()&&filter.is_ok()&&!s.reason.trim().is_empty(),egui::Button::new("Preview current payroll rows")).clicked(){if let(Some(p),Ok(filter))=(selected,filter){let r=x::Request{id:Uuid::new_v4(),profile_id:p.id,expected_revision:p.revision,filter,reason:s.reason.clone()};s.request=Some(r.clone());operation=Some(x::Operation::Preview(r));}}
    let mut close_preview=false;
    if let Some(preview)=&s.preview{card(ui,|ui|{facts(ui,&preview.basis);rows(ui,&preview.basis,&mut s.page,None);if ui.add_enabled(!export_locked,egui::Button::new("Create export snapshot")).clicked(){if let Some(request)=&s.request{operation=Some(x::Operation::Prepare{request:request.clone(),fingerprint:preview.basis.fingerprint.clone()});}}if ui.add_enabled(!locked,egui::Button::new("Close preview draft")).clicked(){close_preview=true;}});}
    if s.preview.is_none()&&!s.reason.is_empty()&&ui.add_enabled(!locked,egui::Button::new("Clear export draft")).clicked(){close_preview=true;}
    if close_preview{s.preview=None;s.request=None;s.reason.clear();}
   }
  }
  if let Some(d)=&s.detail{
   ui.separator();ui.heading("Frozen export and reconciliation");ui.label("Historical snapshot: later payments or profile changes do not update this file. Reconciliation is a private manual record, not verified vendor acceptance.");facts(ui,&d.basis);
   ui.horizontal_wrapped(|ui|{
    for(template,label)in[(false,"Save original CSV"),(true,"Save acknowledgement template")]{if ui.add_enabled(!self.busy,egui::Button::new(label)).clicked(){let name=format!("HRIS-{}-{}.csv",if template{"acknowledgement"}else{"exchange"},d.basis.exchange_id);match crate::reports::choose_export(&name){Ok(Some(destination))=>operation=Some(if template{x::Operation::Template{id:d.basis.exchange_id,destination}}else{x::Operation::Detail{id:d.basis.exchange_id,destination:Some(destination)}}),Ok(None)=>{},Err(e)=>self.notice=Some((e.to_string(),true))}}}
   });
   rows(ui,&d.basis,&mut s.page,Some(d));
   let mut close_ack=false;
   ui.add_enabled_ui(!locked,|ui|{field(ui,"Private acknowledgement batch reason",&mut s.ack_reason);if ui.add_enabled(!s.ack_reason.trim().is_empty()&&s.edit.is_none()&&s.request.is_none(),egui::Button::new("Load reviewed acknowledgement CSV")).clicked(){match crate::imports::choose_source(){Ok(Some(path))=>match x::Ack::load(&path,d,s.ack_reason.clone(),&self.data.access){Ok(a)=>{s.ack=Some(a);s.ack_page=0;},Err(e)=>self.notice=Some((e.to_string(),true))},Ok(None)=>{},Err(e)=>self.notice=Some((e.to_string(),true))}}
    ui.label("Enter accepted or rejected only for reviewed rows. Blank statuses are skipped; pending is an Administrator reset and needs a row note.");
    if let Some(a)=&s.ack{ui.label(format!("Validated batch: {} reviewed rows, posted together",a.rows.len()));let pages=a.rows.len().div_ceil(10).max(1);s.ack_page=s.ack_page.min(pages-1);
     ui.horizontal(|ui|{if ui.add_enabled(s.ack_page>0,egui::Button::new("Previous reviewed rows")).clicked(){s.ack_page-=1;}ui.label(format!("Page {} of {}",s.ack_page+1,pages));if ui.add_enabled(s.ack_page+1<pages,egui::Button::new("Next reviewed rows")).clicked(){s.ack_page+=1;}});
     for r in a.rows.iter().skip(s.ack_page*10).take(10){ui.group(|ui|{ui.label(source_label(d,&r.record_key));ui.label(format!("Outcome: {}",r.status));if !r.reference.is_empty(){ui.label(format!("Reference: {}",r.reference));}if !r.note.is_empty(){ui.label(&r.note);}});}
     if ui.add_enabled(a.validate(d,&self.data.access).is_ok(),egui::Button::new("Record acknowledgement batch")).clicked(){operation=Some(x::Operation::Acknowledge(a.clone()));}if ui.button("Close acknowledgement draft").clicked(){close_ack=true;}}
    if self.data.access.role==Role::Administrator{ui.collapsing("Reset a recorded outcome",|ui|{
     egui::ComboBox::from_id_salt("exchange_reset_key").width(420.0).selected_text(source_label(d,&s.reset_key)).show_ui(ui,|ui|{for c in &d.basis.controls{if let Some(e)=d.current(&c.record_key).filter(|e|e.status!="pending"){ui.selectable_value(&mut s.reset_key,c.record_key.clone(),format!("{} · {}",source_label(d,&c.record_key),e.status));}}});field(ui,"Reason for resetting to pending",&mut s.reset_reason);
     if ui.add_enabled(!s.reset_reason.trim().is_empty()&&d.current(&s.reset_key).is_some_and(|e|e.status!="pending"),egui::Button::new("Record linked reset to pending")).clicked(){if let Some(c)=d.basis.controls.iter().find(|c|c.record_key==s.reset_key){let a=x::Ack{id:Uuid::new_v4(),exchange_id:d.basis.exchange_id,reason:s.reset_reason.clone(),rows:vec![x::AckRow{record_key:c.record_key.clone(),row_sha256:c.row_sha256.clone(),expected_revision:d.current(&c.record_key).map_or(0,|e|e.revision),status:"pending".into(),reference:String::new(),note:s.reset_reason.clone()}]};match a.validate(d,&self.data.access){Ok(())=>operation=Some(x::Operation::Acknowledge(a)),Err(e)=>self.notice=Some((e.to_string(),true))}}}
    });}
   });
   if close_ack{s.ack=None;s.ack_reason.clear();}
   ui.collapsing(format!("Immutable reconciliation history ({} events)",d.events.len()),|ui|{egui::ScrollArea::vertical().max_height(280.).id_salt("exchange_history_scroll").show(ui,|ui|{for e in d.events.iter().rev(){ui.label(format!("{} · {} · v{} · {} UTC",source_label(d,&e.record_key),e.status,e.revision,e.created_at.format("%Y-%m-%d %H:%M")));if !e.reference.is_empty(){ui.label(format!("Reference: {}",e.reference));}if !e.note.is_empty(){ui.label(&e.note);}ui.separator();}});});
  }
  if let Some(operation)=operation{if matches!(operation,x::Operation::SaveProfile(_)|x::Operation::Prepare{..}|x::Operation::Acknowledge(_)){s.pending=Some(operation.clone());}self.send(Command::PayrollExchange(operation));}
 }
}
