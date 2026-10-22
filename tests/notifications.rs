use chrono::{Duration,TimeZone,Utc};
use hris_desktop::{access::Role,documents::*,domain::*,notifications::*};
use uuid::Uuid;
fn now()->chrono::DateTime<Utc>{Utc.with_ymd_and_hms(2026,10,22,8,0,0).unwrap()}
fn fixture()->Snapshot{
    let mut d=DemoStore::new().data;d.leave.clear();d.attendance.clear();d.documents.clear();d.requests=Default::default();d.overtime=Default::default();d.leave_setup.events.clear();
    d.access.user_id=Uuid::from_u128(900);d.access.employee_id=Some(d.employees[0].id);d
}
fn leave(d:&Snapshot,id:u128,employee:Uuid,status:LeaveStatus,offset:i64)->LeaveRequest{
    let day=now().with_timezone(&d.company.tz()).date_naive()+Duration::days(offset);
    let mut r=LeaveDraft{employee_id:Some(employee),starts_on:day.to_string(),ends_on:day.to_string(),reason:"Private reason must not appear in alerts".into(),..Default::default()}.request(d.company.id).unwrap();
    r.id=Uuid::from_u128(id);r.status=status;r.policy.revision=1;r
}
fn doc(d:&Snapshot,id:u128,employee:Uuid,expiry:i64)->EmployeeDocument{
    EmployeeDocument{id:Uuid::from_u128(id),company_id:d.company.id,employee_id:employee,
    details:DocumentDetails{title:"Synthetic contract".into(),category:DocumentCategory::Contract,notes:"Private notes never copied".into(),issued_on:None,expires_on:Some(now().with_timezone(&d.company.tz()).date_naive()+Duration::days(expiry)),contract_start:None,contract_end:None},
    original_filename:"private-original.docx".into(),mime_type:DOCX_MIME.into(),byte_size:1,sha256:"a".repeat(64),object_name:"private/storage/path".into(),state:DocumentState::Ready,archived:false,revision:2,created_by:d.access.user_id,created_by_name:"HR".into(),created_at:now(),updated_at:now()}
}
#[test]
fn document_windows_phases_and_active_filed_scope_are_exact(){
    let mut d=fixture();let e=d.employees[1].id;
    d.documents=vec![doc(&d,1,e,-1),doc(&d,2,e,0),doc(&d,3,e,30),doc(&d,4,e,31)];
    let mut archived=doc(&d,5,e,1);archived.archived=true;d.documents.push(archived);
    let mut pending=doc(&d,6,e,1);pending.state=DocumentState::Pending;d.documents.push(pending);
    let mut foreign=doc(&d,7,e,1);foreign.company_id=Uuid::from_u128(99999);d.documents.push(foreign);
    d.documents[2].details.contract_end=d.documents[2].details.expires_on;
    let a=alerts(&d,now());assert_eq!(a.len(),4);assert_eq!(a.iter().filter(|a|a.urgent).count(),2);
    assert!(a.iter().any(|a|a.key.ends_with(":expired")));assert!(a.iter().any(|a|a.key.ends_with(":today")));
    assert!(a.iter().all(|a|!format!("{}{}",a.title,a.detail).contains("Private")));
    d.employees[1].status=EmployeeStatus::Archived;assert!(alerts(&d,now()).is_empty());
}
#[test]
fn manager_and_portal_never_receive_hr_document_alerts_even_with_forged_rows(){
    let mut d=fixture();let e=d.employees[1].id;d.documents.push(doc(&d,10,e,1));
    for role in [Role::Manager,Role::Employee]{d.access.role=role;d.access.direct_reports=vec![e];assert!(alerts(&d,now()).is_empty());}
    d.access.role=Role::Administrator;d.access.company_id=Uuid::new_v4();assert!(alerts(&d,now()).is_empty());
    d.access.company_id=d.company.id;d.mode=Mode::SignedOut;assert!(alerts(&d,now()).is_empty());
}
#[test]
fn current_approval_step_and_no_self_review_control_the_leave_inbox(){
    let mut d=fixture();let own=d.access.employee_id.unwrap();let direct=d.employees[1].id;let outside=d.employees[2].id;
    d.access.role=Role::Manager;d.access.direct_reports=vec![direct];
    d.leave=vec![leave(&d,20,own,LeaveStatus::Pending,1),leave(&d,21,direct,LeaveStatus::Pending,1),leave(&d,22,outside,LeaveStatus::Pending,1)];
    assert_eq!(alerts(&d,now()).len(),1);
    d.leave[1].policy.approval_steps=vec![hris_desktop::leave::ApprovalStep{kind:hris_desktop::leave::ApproverKind::Administrator,..Default::default()}];
    assert!(alerts(&d,now()).is_empty());d.access.role=Role::Administrator;
    let a=alerts(&d,now());assert_eq!(a.len(),2);assert!(a.iter().all(|a|a.category==Category::Approvals));
    d.access.role=Role::Manager;d.access.direct_reports.clear();
    d.leave.retain(|r|r.employee_id==outside);d.employees.retain(|e|e.id!=outside);
    d.leave[0].policy.requester_name="Filed requester name".into();
    d.leave[0].policy.approval_steps=vec![hris_desktop::leave::ApprovalStep{kind:hris_desktop::leave::ApproverKind::Person,employee_id:Some(own),..Default::default()}];
    let a=alerts(&d,now());assert_eq!(a.len(),1);assert!(a[0].detail.starts_with("Filed requester name"));
}
#[test]
fn own_updates_use_company_local_event_dates_and_upcoming_leave_changes_phase(){
    let mut d=fixture();let own=d.access.employee_id.unwrap();d.leave=vec![leave(&d,30,own,LeaveStatus::Approved,7),leave(&d,31,own,LeaveStatus::Approved,8)];
    d.leave_setup.events.push(hris_desktop::leave::LeaveEvent{id:Uuid::new_v4(),company_id:d.company.id,request_id:d.leave[0].id,employee_id:own,actor_id:Some(Uuid::new_v4()),actor_name:"HR".into(),action:"approved".into(),note:"Private decision note".into(),created_at:now()-Duration::days(30),revision:1});
    let a=alerts(&d,now());assert_eq!(a.len(),2);assert!(a.iter().any(|a|a.category==Category::Updates));
    let soon=a.iter().find(|a|a.category==Category::Reminders).unwrap().key.clone();assert!(soon.ends_with(":soon"));
    let later=alerts(&d,now()+Duration::days(7));assert!(later.iter().any(|a|a.key.ends_with(":today") && a.key!=soon));
    d.leave_setup.events[0].created_at=now()-Duration::days(31);assert!(!alerts(&d,now()).iter().any(|a|a.category==Category::Updates));
}
#[test]
fn unfinished_checkouts_wait_24_hours_and_never_imply_missing_attendance(){
    let mut d=fixture();let own=d.access.employee_id.unwrap();let day=now().with_timezone(&d.company.tz()).date_naive()-Duration::days(1);
    let a=Attendance{id:Uuid::from_u128(40),company_id:d.company.id,employee_id:own,work_date:day,checked_in_at:now()-Duration::hours(24),checked_out_at:None,revision:0,calculation_snapshot:None,planned_shift:None};
    d.attendance.push(a);assert_eq!(alerts(&d,now()).len(),1);
    d.attendance[0].checked_in_at=now()-Duration::hours(23);assert!(alerts(&d,now()).is_empty());
    d.attendance[0].checked_in_at=now()-Duration::hours(30);d.attendance[0].employee_id=d.employees[1].id;assert!(alerts(&d,now()).is_empty());
    d.attendance.clear();assert!(alerts(&d,now()).is_empty());
}
#[test]
fn read_markers_are_idempotent_bounded_and_new_revisions_stay_unread(){
    let mut d=fixture();let e=d.employees[1].id;d.documents.push(doc(&d,50,e,1));let a=alerts(&d,now());
    let mut state=ReadState{user_id:d.access.user_id,company_id:d.company.id,..Default::default()};
    let mark=ReadAction::Mark{keys:vec![a[0].key.clone()],read:true};state.apply(&mark).unwrap();state.apply(&mark).unwrap();assert_eq!(state.read_keys.len(),1);
    state.validate(d.access.user_id,d.company.id).unwrap();assert!(state.validate(Uuid::new_v4(),d.company.id).is_err());
    d.documents[0].revision+=1;assert!(!state.read_keys.contains(&alerts(&d,now())[0].key));
    state.apply(&ReadAction::Mark{keys:vec![a[0].key.clone()],read:false}).unwrap();assert!(state.read_keys.is_empty());
    state.apply(&mark).unwrap();state.apply(&ReadAction::Reset).unwrap();assert!(state.read_keys.is_empty());
    for keys in [vec![],vec![a[0].key.clone();2],vec![a[0].key.clone();101],vec!["private data".into()]]{assert!(state.apply(&ReadAction::Mark{keys,read:true}).is_err());assert!(state.read_keys.is_empty());}
    state.read_keys=(1..=MAX_READ_KEYS).map(|i|format!("checkout:{}:0",Uuid::from_u128(i as u128))).collect();let old=state.read_keys.clone();
    assert!(state.apply(&ReadAction::Mark{keys:vec![format!("checkout:{}:0",Uuid::new_v4())],read:true}).is_err());assert_eq!(state.read_keys,old);
}
#[test]
fn correction_and_overtime_reviews_exclude_self_outside_and_foreign_company(){
    let mut d=fixture();let own=d.access.employee_id.unwrap();let direct=d.employees[1].id;let outside=d.employees[2].id;let day=now().date_naive()-Duration::days(2);
    d.access.role=Role::Manager;d.access.direct_reports=vec![direct];
    let a=Attendance{id:Uuid::from_u128(60),company_id:d.company.id,employee_id:direct,work_date:day,checked_in_at:now()-Duration::days(2),checked_out_at:Some(now()-Duration::days(2)+Duration::hours(9)),revision:0,calculation_snapshot:None,planned_shift:None};
    let r=hris_desktop::requests::Request{id:Uuid::from_u128(61),company_id:d.company.id,employee_id:direct,employee_name:"Synthetic peer".into(),employee_code:"QA".into(),submitted_by:Uuid::from_u128(901),work_date:day,timezone:d.company.timezone.clone(),base_record:None,proposed_in:a.checked_in_at,proposed_out:a.checked_out_at,planned_shift:None,reason:"Private correction reason".into(),status:hris_desktop::requests::Status::Pending,revision:1,applied_attendance_id:None,correction_id:None,created_at:now()};
    let ot=hris_desktop::overtime::Request{id:Uuid::from_u128(62),company_id:d.company.id,attendance_id:a.id,employee_id:direct,employee_name:"Synthetic peer".into(),employee_code:"QA".into(),submitted_by:Uuid::from_u128(901),work_date:day,requested_minutes:60,basis:hris_desktop::overtime::Basis{attendance:a,breaks:vec![],minutes:60},status:hris_desktop::overtime::Status::Pending,approved_minutes:None,reason:"Private overtime reason".into(),revision:1,created_at:now()};
    d.requests.records.push(r.clone());d.overtime.records.push(ot.clone());assert_eq!(alerts(&d,now()).len(),2);
    d.requests.records[0].submitted_by=d.access.user_id;d.overtime.records[0].employee_id=outside;assert!(alerts(&d,now()).is_empty());
    d.requests.records[0]=r;d.overtime.records[0]=ot;d.requests.records[0].company_id=Uuid::new_v4();d.overtime.records[0].employee_id=own;assert!(alerts(&d,now()).is_empty());
}
#[test]
fn malformed_keys_and_portal_read_writes_are_rejected(){
    for bad in ["checkout:00000000-0000-0000-0000-000000000000:0","checkout:00000000-0000-0000-0000-000000000001:-1","checkout:00000000-0000-0000-0000-000000000001:9223372036854775808","checkout:00000000-0000-0000-0000-000000000001:0:secret","other:00000000-0000-0000-0000-000000000001:0"]{assert!(!valid_key(bad));}
    let mut s=DemoStore::new();s.data.access.role=Role::Employee;
    assert!(s.execute(Action::NotificationReads(ReadAction::Reset)).is_err());
    s.data.access.role=Role::Manager;assert!(s.execute(Action::NotificationReads(ReadAction::Reset)).is_ok());
}
