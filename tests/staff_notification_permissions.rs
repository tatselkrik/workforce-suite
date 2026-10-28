use chrono::Utc;
use hris_desktop::{
    access::Role,
    documents::{DOCX_MIME, DocumentCategory, DocumentDetails, DocumentState, EmployeeDocument},
    domain::{DemoStore, Snapshot},
    notifications::{
        self,
        lifecycle::{Inbox, Item},
    },
    staff_roles::{Context, StaffRoles},
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
struct Case {
    roles: StaffRoles,
    capabilities: Vec<String>,
}
fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!("fixtures/staff_role_capabilities.json")).unwrap()
}
fn scoped(roles: StaffRoles) -> Snapshot {
    let mut data = DemoStore::new().data;
    data.leave.clear();
    data.attendance.clear();
    data.documents.clear();
    data.requests = Default::default();
    data.overtime = Default::default();
    data.leave_setup.events.clear();
    data.lifecycle_notifications = None;
    data.access.user_id = Uuid::from_u128(97001);
    data.access.role = roles.compatible_base_role();
    data.access.employee_id = Some(data.employees[0].id);
    data.access.direct_reports = vec![data.employees[1].id];
    data.access.staff_contract_version = 1;
    data.access
        .bind_staff_context(Context {
            schema_version: 1,
            company_id: data.company.id,
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
fn inbox(data: &Snapshot, kind: &str, page: &str, employee: Uuid) -> Inbox {
    let id = Uuid::from_u128(97002);
    let staged = matches!(
        kind,
        "training-due" | "training-start" | "certificate-expiry" | "appraisal-review"
    );
    Inbox {
        version: "lifecycle-notifications-v1".into(),
        company_id: data.company.id,
        user_id: data.access.user_id,
        employee_id: data.access.employee_id,
        role: data.access.role,
        surface: "desktop".into(),
        as_of: data.company.today(),
        generated_at: Utc::now(),
        read_keys: vec![],
        total_items: 1,
        truncated: false,
        items: vec![Item {
            key: format!("{kind}:{id}:0{}", if staged { ":soon" } else { "" }),
            kind: kind.into(),
            title: "Fictional task".into(),
            detail: "Minimal fictional notice".into(),
            date: data.company.today(),
            urgent: false,
            target_id: id,
            target_page: page.into(),
            employee_id: employee,
        }],
    }
}
#[test]
fn all_fourteen_combinations_validate_personal_team_and_development_notices() {
    for case in cases() {
        let desktop = case.capabilities.iter().any(|c| c == "desktop");
        let dev = case
            .capabilities
            .iter()
            .any(|c| c == "development_administration");
        let team = case.capabilities.iter().any(|c| c == "team_review");
        let data = scoped(case.roles.clone());
        let own = data.access.employee_id.unwrap();
        let direct = data.employees[1].id;
        let outside = data.employees[2].id;
        for (kind, page) in [
            ("payroll-monthly-issued", "payroll_monthly"),
            ("payroll-receipt", "payroll_cutoff"),
            ("payroll-final-settled", "payroll_final"),
            ("appraisal-published", "appraisal"),
            ("training-due", "training"),
            ("certificate-expiry", "certificate"),
        ] {
            assert_eq!(
                inbox(&data, kind, page, own)
                    .validate(&data.access, data.company.today())
                    .is_ok(),
                desktop,
                "own {kind} {:?}",
                case.roles
            );
        }
        for (kind, page) in [
            ("training-due", "training"),
            ("certificate-expiry", "certificate"),
            ("appraisal-review", "appraisal"),
        ] {
            assert_eq!(
                inbox(&data, kind, page, direct)
                    .validate(&data.access, data.company.today())
                    .is_ok(),
                desktop && (dev || team),
                "direct {kind} {:?}",
                case.roles
            );
            assert_eq!(
                inbox(&data, kind, page, outside)
                    .validate(&data.access, data.company.today())
                    .is_ok(),
                desktop && dev,
                "outside {kind} {:?}",
                case.roles
            );
        }
        assert_eq!(
            inbox(&data, "appraisal-publish", "appraisal", outside)
                .validate(&data.access, data.company.today())
                .is_ok(),
            desktop && dev
        );
        assert!(
            inbox(&data, "appraisal-publish", "appraisal", own)
                .validate(&data.access, data.company.today())
                .is_err()
        );
        assert!(
            inbox(&data, "payroll-monthly-issued", "payroll_monthly", outside)
                .validate(&data.access, data.company.today())
                .is_err()
        );
        let mut portal = inbox(&data, "training-due", "training", own);
        portal.surface = "portal".into();
        assert!(
            portal.validate(&data.access, data.company.today()).is_err(),
            "Native DTO never accepts a portal surface"
        );
    }
}
#[test]
fn same_base_role_change_removes_development_notices_but_retains_personal_advice() {
    use hris_desktop::staff_roles::StaffRole as S;
    let mut data = scoped(StaffRoles::new(vec![S::LearningPerformanceOfficer]).unwrap());
    let saved = inbox(
        &data,
        "appraisal-publish",
        "appraisal",
        data.employees[2].id,
    );
    let own = inbox(
        &data,
        "payroll-monthly-issued",
        "payroll_monthly",
        data.access.employee_id.unwrap(),
    );
    saved.validate(&data.access, data.company.today()).unwrap();
    data.access.staff_context.as_mut().unwrap().roles =
        StaffRoles::new(vec![S::PayrollOfficer]).unwrap();
    data.access.staff_context.as_mut().unwrap().revision += 1;
    assert_eq!(data.access.role, Role::Employee);
    assert!(saved.validate(&data.access, data.company.today()).is_err());
    own.validate(&data.access, data.company.today()).unwrap();
    data.lifecycle_notifications = Some(saved);
    assert!(
        !notifications::alerts(&data, Utc::now())
            .iter()
            .any(|a| a.key.starts_with("appraisal-publish:"))
    );
}
#[test]
fn forged_hr_document_rows_stay_hidden_without_personnel_document_permission() {
    for case in cases() {
        let mut data = scoped(case.roles);
        let id = Uuid::from_u128(97003);
        data.documents.push(EmployeeDocument {
            id,
            company_id: data.company.id,
            employee_id: data.employees[1].id,
            details: DocumentDetails {
                title: "Fictional contract".into(),
                category: DocumentCategory::Contract,
                notes: "PRIVATE_NOTE".into(),
                issued_on: None,
                expires_on: Some(data.company.today()),
                contract_start: None,
                contract_end: None,
            },
            original_filename: "private.docx".into(),
            mime_type: DOCX_MIME.into(),
            byte_size: 1,
            sha256: "a".repeat(64),
            object_name: "PRIVATE_STORAGE_KEY".into(),
            state: DocumentState::Ready,
            archived: false,
            revision: 1,
            created_by: data.access.user_id,
            created_by_name: "Fictional staff".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });
        let alerts = notifications::alerts(&data, Utc::now());
        assert_eq!(
            alerts.iter().any(|a| a.key.starts_with("document-expiry:")),
            case.capabilities.iter().any(|c| c == "personnel_documents")
        );
        assert!(
            alerts
                .iter()
                .all(|a| !format!("{}{}", a.title, a.detail).contains("PRIVATE_"))
        );
        data.access.role = if data.access.role == Role::Administrator {
            Role::Employee
        } else {
            Role::Administrator
        };
        assert!(
            notifications::alerts(&data, Utc::now()).is_empty(),
            "Header/role mismatch fails closed"
        );
    }
}
