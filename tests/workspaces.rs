use hris_desktop::{
    access::Access,
    domain::{DemoStore, Snapshot},
    staff_roles::{self, StaffRoles},
    workspaces::*,
};
use serde_json::json;
use uuid::Uuid;
fn data(roles: StaffRoles) -> Snapshot {
    let mut d = DemoStore::new().data;
    d.access = Access {
        company_id: d.company.id,
        user_id: Uuid::from_u128(640),
        employee_id: Some(d.employees[0].id),
        role: roles.compatible_base_role(),
        staff_contract_version: 1,
        ..Default::default()
    };
    d.access
        .bind_staff_context(staff_roles::Context {
            schema_version: 1,
            company_id: d.company.id,
            user_id: d.access.user_id,
            employee_id: d.access.employee_id.unwrap(),
            base_role: d.access.role,
            roles,
            revision: 1,
            runtime_ready: true,
        })
        .unwrap();
    d.workspaces = Context {
        schema_version: 1,
        company_id: d.company.id,
        user_id: d.access.user_id,
        ..Default::default()
    };
    d
}
fn cases() -> Vec<serde_json::Value> {
    serde_json::from_str(include_str!("fixtures/staff_role_capabilities.json")).unwrap()
}
#[test]
fn role_focused_navigation_obeys_all_module_combinations_without_changing_authority() {
    for case in cases() {
        let roles: StaffRoles = serde_json::from_value(case["roles"].clone()).unwrap();
        let mut d = data(roles.clone());
        let before = d.access.clone();
        for mask in 0u8..64 {
            d.workspaces.enabled = Module::ALL
                .into_iter()
                .enumerate()
                .filter_map(|(n, m)| (mask & (1 << n) != 0).then_some(m))
                .collect();
            let nav = entries(&d);
            let mut pages = Vec::new();
            let mut groups = Vec::new();
            for e in &nav {
                assert!(visible(e.page, &d));
                assert!(!pages.contains(&e.page));
                pages.push(e.page);
                groups.push(match e.group {
                    None => 0,
                    Some(Group::Work) => 1,
                    Some(Group::Personal) => 2,
                    Some(Group::Administration) => 3,
                });
            }
            assert!(groups.windows(2).all(|w| w[0] <= w[1]));
            if roles.uses_desktop() {
                for page in [
                    Page::Overview,
                    Page::Attendance,
                    Page::Leave,
                    Page::Settings,
                ] {
                    assert!(pages.contains(&page))
                }
                assert_eq!(
                    pages.contains(&Page::Accounts),
                    roles.allows(staff_roles::Capability::AccountAdministration)
                );
                assert_eq!(
                    pages.contains(&Page::Payroll),
                    d.workspaces.available(Module::Payroll)
                );
                assert!(visible(landing(&d), &d));
            } else {
                assert!(nav.is_empty())
            }
            assert_eq!(d.access, before);
        }
    }
}
#[test]
fn specialists_land_in_their_work_and_keep_one_combined_development_entry() {
    for (roles, expected, label) in [
        (json!(["recruiter"]), Page::Hiring, "Hiring"),
        (
            json!(["learning_performance_officer"]),
            Page::Employees,
            "Development",
        ),
        (json!(["payroll_officer"]), Page::Payroll, "Payroll"),
    ] {
        let mut d = data(serde_json::from_value(roles).unwrap());
        assert_eq!(landing(&d), expected);
        assert_eq!(
            entries(&d)
                .iter()
                .find(|e| e.page == expected)
                .unwrap()
                .label,
            label
        );
        if expected == Page::Employees {
            assert!(prefers_development(&d));
            assert!(!organization(&d));
            assert!(!people_directory(&d.access))
        }
        d.workspaces.enabled.clear();
        assert_eq!(landing(&d), Page::Overview);
    }
    let d = data(serde_json::from_value(json!(["hr_generalist", "payroll_officer"])).unwrap());
    assert_eq!(landing(&d), Page::Overview);
    assert!(people_directory(&d.access));
    assert_eq!(label(Page::Payroll, &d), "Payroll");
}
#[test]
fn context_and_receipts_are_strict_scope_bound_and_unknown_data_never_becomes_defaults() {
    let d = data(serde_json::from_value(json!(["administrator"])).unwrap());
    d.workspaces.validate(&d.access).unwrap();
    for field in ["company_id", "user_id"] {
        let mut v = serde_json::to_value(&d.workspaces).unwrap();
        v[field] = json!(Uuid::new_v4());
        assert!(
            serde_json::from_value::<Context>(v)
                .unwrap()
                .validate(&d.access)
                .is_err()
        );
    }
    let mut bad = d.workspaces.clone();
    bad.enabled.push(Module::Payroll);
    assert!(bad.validate(&d.access).is_err());
    bad = d.workspaces.clone();
    bad.schema_version = 99;
    assert!(bad.validate(&d.access).is_err());
    bad = d.workspaces.clone();
    bad.enabled.clear();
    assert!(bad.validate(&d.access).is_err());
    let mut raw = serde_json::to_value(&d.workspaces).unwrap();
    raw["authority"] = json!("administrator");
    assert!(serde_json::from_value::<Context>(raw).is_err());
    let legacy = Context::legacy(&d.access);
    legacy.validate(&d.access).unwrap();
    assert!(Save::new(&legacy, &d.access, &[]).is_err());
    let request = Save::new(
        &d.workspaces,
        &d.access,
        &[Module::Reports, Module::Development],
    )
    .unwrap();
    assert_eq!(request.enabled, vec![Module::Development, Module::Reports]);
    let receipt = Receipt {
        schema_version: 1,
        id: request.id,
        company_id: request.company_id,
        actor_id: request.actor_id,
        revision: 1,
        enabled: request.enabled.clone(),
        applied_at: chrono::Utc::now(),
    };
    receipt.validate(&request).unwrap();
    let mut wrong = receipt.clone();
    wrong.id = Uuid::new_v4();
    assert!(wrong.validate(&request).is_err());
    wrong = receipt;
    wrong.enabled.clear();
    assert!(wrong.validate(&request).is_err());
}
#[test]
fn uncertain_saves_keep_the_exact_payload_and_stale_choices_require_review() {
    let d = data(serde_json::from_value(json!(["administrator"])).unwrap());
    let mut draft = Draft::from(&d.workspaces);
    draft.enabled.clear();
    let first = draft.request(&d.workspaces, &d.access).unwrap();
    let mut newer = d.workspaces.clone();
    newer.revision = 2;
    newer.updated_at = Some(chrono::Utc::now());
    assert_eq!(draft.request(&newer, &d.access).unwrap(), first);
    assert!(draft.changed());
    draft.pending = None;
    assert!(draft.request(&newer, &d.access).is_err());
    assert!(draft.enabled.is_empty());
    let mut other = d.access.clone();
    other.user_id = Uuid::new_v4();
    assert!(first.validate(&newer, &other).is_err());
}
#[test]
fn only_protected_administrators_prepare_visibility_changes() {
    for case in cases() {
        let roles: StaffRoles = serde_json::from_value(case["roles"].clone()).unwrap();
        let d = data(roles.clone());
        assert_eq!(
            Save::new(&d.workspaces, &d.access, &[]).is_ok(),
            roles.allows(staff_roles::Capability::CompanyConfiguration)
        );
    }
}

#[test]
fn hidden_routes_preserve_drafts_and_recover_after_save_or_cancel() {
    let mut d = data(serde_json::from_value(json!(["administrator"])).unwrap());
    d.workspaces.enabled.clear();
    assert_eq!(
        reconcile(Page::Payroll, Section::Directory, &d, true),
        (Page::Payroll, Section::Directory)
    );
    assert_eq!(
        reconcile(Page::Payroll, Section::Directory, &d, false),
        (Page::Overview, Section::Directory)
    );
    assert_eq!(
        reconcile(Page::Employees, Section::Development, &d, true),
        (Page::Employees, Section::Development)
    );
    assert_eq!(
        reconcile(Page::Employees, Section::Development, &d, false),
        (Page::Employees, Section::Directory)
    );
    assert_eq!(
        reconcile(Page::Employees, Section::Organization, &d, false),
        (Page::Employees, Section::Directory)
    );
    let mut d = data(serde_json::from_value(json!(["learning_performance_officer"])).unwrap());
    assert_eq!(
        reconcile(Page::Employees, Section::Directory, &d, false),
        (Page::Employees, Section::Development)
    );
    d.workspaces.enabled.clear();
    assert_eq!(
        reconcile(Page::Employees, Section::Development, &d, false).0,
        Page::Overview
    );
    assert_eq!(
        reconcile(Page::Settings, Section::Development, &d, false).0,
        Page::Settings
    );
}
#[test]
fn assigned_clearance_tasks_remain_discoverable_without_hr_permissions() {
    let mut d = data(serde_json::from_value(json!(["recruiter"])).unwrap());
    assert!(!exits(&d));
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    d.exits.tasks.push(hris_desktop::exits::Task {
        id,
        company_id: d.company.id,
        case_id: id,
        employee_id: id,
        employee_name: "Fictional person".into(),
        employee_code: "QA-64".into(),
        last_working_on: now.date_naive(),
        case_status: "open".into(),
        assigned_employee_id: d.access.employee_id,
        title: "Return fictional item".into(),
        instructions: String::new(),
        due_on: None,
        required: false,
        status: "open".into(),
        revision: 1,
        created_by: d.access.user_id,
        created_at: now,
        updated_at: now,
    });
    assert!(exits(&d));
    assert_eq!(label(Page::Exits, &d), "Clearance tasks");
    d.workspaces.enabled.retain(|m| *m != Module::ExitClearance);
    assert!(!exits(&d));
}
