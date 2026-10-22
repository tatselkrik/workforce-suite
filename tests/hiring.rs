use hris_desktop::{access::Role,domain::DemoStore,hiring::*};
use chrono::NaiveDate;
fn today()->NaiveDate{NaiveDate::from_ymd_opt(2026,10,22).unwrap()}
fn draft()->ApplicantDraft{ApplicantDraft{full_name:"Synthetic Applicant".into(),email:"applicant@example.invalid".into(),position:"HR Associate".into(),applied_on:"2026-10-22".into(),..Default::default()}}
#[test]
fn applicant_contacts_and_dates_are_bounded_without_creating_accounts(){
 let good=draft();good.validate(today()).unwrap();
 for edit in 0..6 {let mut d=good.clone();match edit {0=>d.full_name.clear(),1=>d.email="bad address".into(),2=>d.phone="1".repeat(41),3=>d.position=" ".into(),4=>d.applied_on="2026-10-23".into(),_=>d.notes="hidden\0text".into()};assert!(d.validate(today()).is_err());}
 let mut d=good;d.email.clear();d.phone.clear();d.notes="Interview notes\nSecond line".into();d.validate(today()).unwrap();
}
#[test]
fn pipeline_changes_require_reasons_and_closed_applications_reopen_at_applied(){
 Stage::Applied.change(Stage::Screening,"Review application").unwrap();Stage::Screening.change(Stage::Offer,"Experience reviewed").unwrap();
 assert!(Stage::Applied.change(Stage::Offer,"Skip review").is_err());assert!(Stage::Interview.change(Stage::Offer,"").is_err());assert!(Stage::Interview.change(Stage::Interview,"Unchanged").is_err());
 for end in [Stage::Rejected,Stage::Withdrawn]{Stage::Offer.change(end,"Documented outcome").unwrap();end.change(Stage::Applied,"Applicant reapplied").unwrap();assert!(end.change(Stage::Offer,"Reopen directly").is_err());}
 assert!(serde_json::from_str::<Stage>("\"hired\"").is_err());
}
#[test]
fn normalized_draft_fields_keep_one_retry_identity_and_exclude_protected_fields(){
 let mut d=ApplicantDraft::new(today());d.full_name="  Synthetic Applicant  ".into();d.position=" HR Associate ".into();let first=d.fields(today()).unwrap();let second=d.fields(today()).unwrap();assert_eq!(first,second);assert_eq!(first["full_name"],"Synthetic Applicant");
 for protected in ["company_id","stage","revision","user_id","employee_id","role","password"]{assert!(first.get(protected).is_none());}
 assert!(draft().fields(today()).is_err());
}
#[test]
fn stage_payload_requires_current_revision_and_an_unarchived_application(){
 let mut r=Applicant{id:uuid::Uuid::new_v4(),company_id:uuid::Uuid::new_v4(),full_name:"Synthetic Applicant".into(),email:String::new(),phone:String::new(),position:"HR Associate".into(),source:String::new(),applied_on:today(),notes:String::new(),stage:Stage::Applied,archived:false,revision:3};
 let p=r.stage_change(3,Stage::Screening," Review started ").unwrap();assert_eq!(p["reason"],"Review started");assert!(p.get("actor_id").is_none());assert!(r.stage_change(2,Stage::Screening,"Review").is_err());
 r.archived=true;assert!(r.stage_change(3,Stage::Screening,"Review").is_err());
}
#[test]
fn applicants_are_hr_private_and_never_inherit_manager_directory_access(){
 let mut data=DemoStore::new().data;
 for role in Role::ALL {data.access.role=role;assert_eq!(authorize(&data.access,&data).is_ok(),role.manages_hr());}
 data.access.role=Role::Administrator;data.access.company_id=uuid::Uuid::new_v4();assert!(authorize(&data.access,&data).is_err());
}
