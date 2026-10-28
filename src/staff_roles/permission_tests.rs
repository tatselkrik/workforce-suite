use super::*;
use crate::{domain::DemoStore, payroll::PayrollAction};
use serde_json::json;

pub(super) fn scoped(roles: StaffRoles) -> crate::domain::Snapshot {
    let mut data = DemoStore::new().data;
    data.access.user_id = Uuid::from_u128(90001);
    data.access.role = roles.compatible_base_role();
    data.access.staff_contract_version = SCHEMA_VERSION;
    data.access
        .bind_staff_context(Context {
            schema_version: SCHEMA_VERSION,
            company_id: data.access.company_id,
            user_id: data.access.user_id,
            employee_id: data.access.employee_id.unwrap(),
            base_role: data.access.role,
            revision: 2,
            runtime_ready: true,
            roles,
        })
        .unwrap();
    data
}

#[test]
fn actual_payroll_commands_and_report_guard_follow_specialties_not_legacy_base() {
    #[derive(Deserialize)]
    struct Case {
        roles: StaffRoles,
        capabilities: Vec<String>,
    }
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../tests/fixtures/staff_role_capabilities.json"
    ))
    .unwrap();
    for case in cases {
        let data = scoped(case.roles.clone());
        let create = PayrollAction::new(
            "run_create",
            json!({"id":Uuid::new_v4(),"month":"2026-09-01","title":"Fictional reviewed payroll","reason":"Prepare authorized payroll"}),
        );
        assert_eq!(
            create.payload(&data).is_ok(),
            case.capabilities.iter().any(|c| c == "payroll_preparation"),
            "{:?}",
            case.roles
        );
        assert_eq!(
            crate::payroll_reporting::authorize(&data).is_ok(),
            case.capabilities.iter().any(|c| c == "payroll_reports"),
            "{:?}",
            case.roles
        );
        let payment = PayrollAction::new(
            "run_paid",
            json!({"id":Uuid::new_v4(),"reason":"Record actual payment"}),
        );
        assert_eq!(
            payment.payload(&data).is_ok(),
            case.roles.contains(StaffRole::Administrator),
            "Only Administrators may record payment: {:?}",
            case.roles
        );
    }
}

#[test]
fn foreign_or_incompatible_role_context_cannot_open_payroll() {
    let data = scoped(StaffRoles::new(vec![StaffRole::PayrollOfficer]).unwrap());
    assert_eq!(data.access.role, crate::access::Role::Employee);
    assert!(data.access.uses_desktop());
    assert!(crate::payroll_reporting::authorize(&data).is_ok());
    let mut changed = data.clone();
    changed.access.staff_context.as_mut().unwrap().company_id = Uuid::new_v4();
    assert!(crate::payroll_reporting::authorize(&changed).is_err());
    let mut changed = data;
    changed.access.staff_context.as_mut().unwrap().base_role = crate::access::Role::Administrator;
    assert!(crate::payroll_reporting::authorize(&changed).is_err());
}

#[test]
fn development_actions_and_private_event_projections_have_different_permissions() {
    #[derive(Deserialize)]
    struct Case {
        roles: StaffRoles,
        capabilities: Vec<String>,
    }
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../tests/fixtures/staff_role_capabilities.json"
    ))
    .unwrap();
    for case in cases {
        let mut data = scoped(case.roles.clone());
        let mut cycle = crate::performance::CycleDraft::new(&data);
        cycle.name = "Fictional capability review".into();
        cycle.reason = "Reviewed scope".into();
        let development = case
            .capabilities
            .iter()
            .any(|c| c == "development_administration");
        assert_eq!(
            crate::performance::PerformanceAction::CycleSave(cycle)
                .payload(&data)
                .is_ok(),
            development,
            "{:?}",
            case.roles
        );
        let employee = data
            .employees
            .iter()
            .find(|e| Some(e.id) != data.access.employee_id)
            .unwrap()
            .id;
        let mut training = crate::learning::TrainingDraft::new(Some(employee));
        training.title = "Fictional specialist training".into();
        training.reason = "Reviewed scope".into();
        assert_eq!(
            crate::learning::LearningAction::TrainingSave(training)
                .payload(&data)
                .is_ok(),
            development,
            "{:?}",
            case.roles
        );
        let note_id = Uuid::new_v4();
        data.performance.events.push(crate::performance::Event {
            id: Uuid::new_v4(),
            company_id: data.company.id,
            goal_id: None,
            cycle_id: None,
            review_id: None,
            note_id: Some(note_id),
            kind: "note_created".into(),
            reason: "Fictional private note".into(),
            actor_id: data.access.user_id,
            actor_name: "Fictional actor".into(),
            actor_role: crate::access::Role::Administrator,
            before_record: None,
            after_record: json!({"note":"private"}),
            recorded_at: chrono::Utc::now(),
        });
        assert_eq!(
            !crate::performance::visible_events(&data).is_empty(),
            case.capabilities.iter().any(|c| c == "private_hr_notes"),
            "Private notes: {:?}",
            case.roles
        );
        data.learning.events.push(crate::learning::Event {
            id: Uuid::new_v4(),
            company_id: data.company.id,
            training_id: None,
            certificate_id: None,
            evidence_id: Some(Uuid::new_v4()),
            kind: "evidence_linked".into(),
            reason: "Fictional private binding".into(),
            actor_id: data.access.user_id,
            actor_name: "Fictional actor".into(),
            actor_role: crate::access::Role::Administrator,
            before_record: None,
            after_record: json!({"document_id":Uuid::new_v4()}),
            recorded_at: chrono::Utc::now(),
        });
        assert_eq!(
            !crate::learning::visible_events(&data, None, None).is_empty(),
            case.capabilities.iter().any(|c| c == "personnel_documents"),
            "Private document IDs: {:?}",
            case.roles
        );
    }
}

#[test]
fn protected_workflow_directory_supports_specialist_reviewers_without_broad_hr_access() {
    let mut data = scoped(StaffRoles::new(vec![StaffRole::Administrator]).unwrap());
    let employee = data
        .employees
        .iter()
        .find(|e| Some(e.id) != data.access.employee_id)
        .unwrap()
        .id;
    let roles = StaffRoles::new(vec![StaffRole::LearningPerformanceOfficer]).unwrap();
    let person = crate::recruiting::Interviewer {
        employee_id: employee,
        full_name: "Fictional development reviewer".into(),
        role: roles.compatible_base_role(),
        roles: Some(roles),
    };
    let directory = crate::staff_roles::workflow::Directory {
        schema_version: 1,
        company_id: data.company.id,
        user_id: data.access.user_id,
        runtime_ready: true,
        person_count: 1,
        people: vec![person.clone()],
    };
    directory.validate(&data.access).unwrap();
    assert!(person.allows(Capability::DevelopmentAdministration));
    assert!(!person.allows(Capability::PrivateHrNotes));
    assert_eq!(person.role, crate::access::Role::Employee);
    data.recruiting.interviewers = directory.people.clone();
    let subject = data.access.employee_id.unwrap();
    assert!(crate::performance::eligible_reviewer(
        &data, subject, employee
    ));
    let mut bad = directory.clone();
    bad.user_id = Uuid::new_v4();
    assert!(bad.validate(&data.access).is_err());
    let mut bad = directory.clone();
    bad.people[0].roles = None;
    assert!(bad.validate(&data.access).is_err());
    let mut bad = directory.clone();
    bad.people[0].role = crate::access::Role::Administrator;
    assert!(bad.validate(&data.access).is_err());
    let mut bad = directory.clone();
    bad.people.push(person);
    bad.person_count += 1;
    assert!(bad.validate(&data.access).is_err());
    let mut bad = directory;
    bad.person_count = 2;
    assert!(bad.validate(&data.access).is_err());
}

#[test]
fn directory_visibility_does_not_create_operational_attendance_or_contact_access() {
    for role in [
        StaffRole::Recruiter,
        StaffRole::LearningPerformanceOfficer,
        StaffRole::PayrollOfficer,
    ] {
        let data = scoped(StaffRoles::new(vec![role]).unwrap());
        assert!(data.access.has_full_employee_directory());
        let own = data.access.employee_id.unwrap();
        let rows = crate::attendance::daily_roster(&data, data.company.today());
        assert!(rows.iter().all(|r| r.employee.id == own));
        for person in &data.employees {
            assert_eq!(data.access.can_read_contacts(person.id), person.id == own);
            assert_eq!(
                data.access.can_read_operational_employee(person),
                person.id == own
            );
        }
    }
    let mut data = scoped(StaffRoles::new(vec![StaffRole::Manager]).unwrap());
    let own = data.access.employee_id.unwrap();
    let target = data.employees.iter_mut().find(|e| e.id != own).unwrap();
    target.reports_to = Some(own);
    data.access.direct_reports = vec![target.id];
    assert!(data.access.can_read_operational_employee(target));
    target.status = crate::domain::EmployeeStatus::Archived;
    assert!(!data.access.can_read_operational_employee(target));
}


#[test]
fn changed_staff_responsibilities_invalidate_report_scope_without_changing_base_role() {
    let before=scoped(StaffRoles::new(vec![StaffRole::HrGeneralist]).unwrap());
    let after=scoped(StaffRoles::new(vec![StaffRole::PayrollOfficer]).unwrap());
    assert_eq!(before.access.role,after.access.role);
    assert!(!crate::reports::same_scope(&before.access,&after.access));
    assert!(before.access.can_manage_reports());
    assert!(!after.access.can_manage_reports());
}

#[test]
fn specialized_payroll_creates_and_source_selection_work_without_broad_hr() {
    #[derive(Deserialize)] struct Case {roles:StaffRoles,capabilities:Vec<String>}
    let cases:Vec<Case>=serde_json::from_str(include_str!("../../tests/fixtures/staff_role_capabilities.json")).unwrap();
    for case in cases {
        let data=scoped(case.roles.clone());let employee=data.employees[1].id;
        let allowed=case.capabilities.iter().any(|c|c=="payroll_preparation");
        let actions=[
            PayrollAction::new("final_create",json!({"id":Uuid::new_v4(),"employee_id":employee,"service_end":"2026-10-01","cash_date":"2026-10-28","title":"Fictional final review","reason":"Fictional documented preparation"})),
            PayrollAction::new("tax_create",json!({"id":Uuid::new_v4(),"employee_id":employee,"year":2026,"kind":"termination","as_of":"2026-10-28","service_end":"2026-10-01","title":"Fictional annual review","reason":"Fictional documented preparation"})),
            PayrollAction::new("thirteenth_run_create",json!({"id":Uuid::new_v4(),"year":2026,"as_of":"2026-10-01","pay_date":"2026-10-28","title":"Fictional benefit review","reason":"Fictional documented preparation"})),
            PayrollAction::new("cutoff_run_create",json!({"id":Uuid::new_v4(),"month":"2026-10-01","part":2,"pay_date":"2026-10-31","title":"Fictional cutoff","reason":"Fictional documented preparation"})),
        ];
        for action in actions {let result=action.payload(&data);assert_eq!(result.is_ok(),allowed,"{:?}: {} => {:?}",case.roles,action.operation,result.err());}
        assert_eq!(crate::payroll_amendments::targets(&data).is_ok(),allowed,"{:?}",case.roles);
    }
}
#[test]
fn final_statement_export_respects_payroll_or_own_scope_for_all_roles(){
    #[derive(Deserialize)]struct Case {roles:StaffRoles,capabilities:Vec<String>}
    let cases:Vec<Case>=serde_json::from_str(include_str!("../../tests/fixtures/staff_role_capabilities.json")).unwrap();
    for case in cases {
        let mut data=scoped(case.roles.clone());let employee=data.employees[1].id;let id=Uuid::new_v4();let day=chrono::NaiveDate::from_ymd_opt(2026,10,28).unwrap();
        data.payroll.finals.slips.push(crate::payroll_final::Slip {id,company_id:data.company.id,employee_id:employee,employment_start:day,service_end:day,cash_date:day,
            company_name:"Fictional company".into(),employee_name:"Fictional former staff".into(),employee_code:"FICTIONAL".into(),department:"Fictional department".into(),
            totals:Default::default(),sources:json!([]),components:json!([]),approved_by:Uuid::new_v4(),approved_at:chrono::Utc::now(),rule_version:"Fictional projection".into()});
        let allowed=case.capabilities.iter().any(|c|c=="payroll_preparation");
        let result=crate::payroll_final::export_csv(&data,"final_statement",Some(id));
        assert_eq!(result.is_ok(),allowed,"{:?} => {:?}",case.roles,result.err());
        data.payroll.finals.slips[0].employee_id=data.access.employee_id.unwrap();
        assert!(crate::payroll_final::export_csv(&data,"final_statement",Some(id)).is_ok(),"Own immutable advice remains exportable {:?}",case.roles);
    }
}
