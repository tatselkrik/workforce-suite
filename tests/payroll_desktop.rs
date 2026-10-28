use hris_desktop::{
    access::Access,
    backend::{self, Command},
    desktop::{Application, OWN_EXPORTS, PAYROLL_TABS, Product},
    domain::{Action, DemoStore, Snapshot},
    payroll::{self, Item, Pay, Profile, Slip, Totals, Worksheet},
    staff_roles::{self, Capability, StaffRoles},
    workspaces::{Module, Page},
};
use serde_json::json;
use uuid::Uuid;
fn data(roles: StaffRoles) -> Snapshot {
    let mut d = DemoStore::new().data;
    d.access = Access {
        company_id: d.company.id,
        user_id: Uuid::from_u128(650),
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
    d
}
fn slip(d: &Snapshot, employee: Uuid, n: u128) -> Slip {
    let id = Uuid::from_u128(n);
    let month = chrono::NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    let profile = Profile {
        id: Uuid::from_u128(n + 20),
        company_id: d.company.id,
        employee_id: employee,
        effective_on: month,
        pay: Pay {
            rate_cents: 3_000_000,
            ..Default::default()
        },
        revision: 0,
        created_by: Uuid::from_u128(1),
    };
    Slip {
        id,
        company_id: d.company.id,
        run_id: Uuid::from_u128(n + 1),
        employee_id: employee,
        month,
        company_name: "Fictional company".into(),
        item: Item {
            id,
            company_id: d.company.id,
            run_id: Uuid::from_u128(n + 1),
            employee_id: employee,
            employee_name: d.employee_name(employee).into(),
            employee_code: format!("QA-{n}"),
            department: "QA".into(),
            profile,
            source_basis: json!({}),
            worksheet: Worksheet::default(),
            totals: Totals::default(),
            revision: 0,
        },
        approved_by: Uuid::from_u128(2),
        approved_at: chrono::Utc::now(),
        rule_version: payroll::RULE_VERSION.into(),
    }
}
#[test]
fn two_products_keep_distinct_identities_and_payroll_requires_real_protected_responsibility() {
    assert_ne!(Product::Hris.id(), Product::Payroll.id());
    assert_eq!(Product::Payroll.id(), "io.github.tatselkrik.payrolldesktop");
    assert_ne!(Product::Hris.marker(), Product::Payroll.marker());
    assert_ne!(Product::Hris.executable(), Product::Payroll.executable());
    assert_eq!(Application::payroll("0.1.0").version, "0.1.0");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/staff_role_capabilities.json")).unwrap();
    for c in cases {
        let d = data(serde_json::from_value(c["roles"].clone()).unwrap());
        let before = d.access.clone();
        let expected = d.access.allows(Capability::PayrollPreparation);
        assert_eq!(Product::Payroll.admits(&d.access), expected);
        for mask in 0u8..64 {
            let mut d = d.clone();
            d.workspaces.enabled = Module::ALL
                .into_iter()
                .enumerate()
                .filter_map(|(n, m)| (mask & (1 << n) != 0).then_some(m))
                .collect();
            let nav = Product::Payroll.entries(&d);
            assert_eq!(!nav.is_empty(), expected);
            if expected {
                assert_eq!(Product::Payroll.landing(&d), Page::Payroll);
                assert!(nav.iter().any(|e| e.page == Page::Payroll));
                for p in [
                    Page::Payroll,
                    Page::Profile,
                    Page::Settings,
                    Page::Notifications,
                ] {
                    assert!(Product::Payroll.visible(p, &d));
                }
            }
            for p in [
                Page::Employees,
                Page::Attendance,
                Page::Leave,
                Page::Hiring,
                Page::Exits,
                Page::Accounts,
                Page::Reports,
                Page::Overview,
            ] {
                assert!(!Product::Payroll.visible(p, &d));
            }
            assert_eq!(d.access, before);
        }
    }
}
#[test]
fn worker_product_gate_keeps_all_financial_families_and_own_advice_but_denies_hr_writes() {
    assert_eq!(PAYROLL_TABS.len(), 15);
    for tab in [
        "Salaries & benefits",
        "Cutoffs",
        "13th-month pay",
        "Annual tax",
        "Payments",
        "Final pay",
        "Wage compliance",
        "Amendments",
        "Government",
        "Data exchange",
    ] {
        assert!(PAYROLL_TABS.contains(&tab));
    }
    for op in [
        "profile_save",
        "holiday_save",
        "run_create",
        "cutoff_run_create",
        "thirteenth_run_create",
        "tax_create",
        "receipt_record",
        "final_create",
        "wage_save",
        "amendment_create",
    ] {
        let c = Command::Change(Action::Payroll(payroll::PayrollAction::new(op, json!({}))));
        assert!(Product::Payroll.command_allowed(&c));
        assert!(!Product::Hris.command_allowed(&c));
    }
    for kind in OWN_EXPORTS {
        let c = Command::PayrollExport {
            kind: (*kind).into(),
            record_id: Some(Uuid::new_v4()),
            year: 2026,
            destination: "fictional-new-file.csv".into(),
        };
        assert!(Product::Hris.command_allowed(&c));
        assert!(Product::Payroll.command_allowed(&c));
    }
    for kind in [
        "register",
        "statutory",
        "annual",
        "cutoff_register",
        "thirteenth_register",
        "tax_register",
        "final_register",
        "amendment_register",
    ] {
        let c = Command::PayrollExport {
            kind: kind.into(),
            record_id: None,
            year: 2026,
            destination: "fictional-new-file.csv".into(),
        };
        assert!(!Product::Hris.command_allowed(&c));
        assert!(Product::Payroll.command_allowed(&c));
    }
    for c in [
        Command::Change(Action::SaveCompany(Default::default())),
        Command::SaveWorkspaces(hris_desktop::workspaces::Save {
            id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            actor_id: Uuid::new_v4(),
            expected_revision: 0,
            enabled: vec![],
        }),
        Command::Services(hris_desktop::services::Operation::Refresh),
        Command::CheckUpdates,
    ] {
        assert!(!Product::Payroll.command_allowed(&c));
    }
    assert!(
        !Product::Payroll.command_allowed(&Command::Change(Action::NotificationReads(
            hris_desktop::notifications::ReadAction::Reset
        )))
    );
    let key = format!("payroll-receipt:{}:0", Uuid::new_v4());
    assert!(
        Product::Payroll.command_allowed(&Command::Change(Action::NotificationReads(
            hris_desktop::notifications::ReadAction::Mark {
                keys: vec![key],
                read: true
            }
        )))
    );
}
#[test]
fn separating_application_projections_preserves_original_slips_and_never_mutates_source_store() {
    let mut d = data(serde_json::from_value(json!(["administrator"])).unwrap());
    let own = d.access.employee_id.unwrap();
    let peer = d.employees.iter().find(|e| e.id != own).unwrap().id;
    d.payroll.slips = vec![slip(&d, own, 6501), slip(&d, peer, 6601)];
    d.payroll.profiles = d
        .payroll
        .slips
        .iter()
        .map(|s| s.item.profile.clone())
        .collect();
    let before = serde_json::to_value(&d.payroll.slips).unwrap();
    let profiles = serde_json::to_value(&d.payroll.profiles).unwrap();
    let employees = d.employees.clone();
    let access = d.access.clone();
    let mut hris = d.clone();
    Product::Hris.project(&mut hris);
    assert_eq!(hris.payroll.slips.len(), 1);
    assert_eq!(
        serde_json::to_value(&hris.payroll.slips[0]).unwrap(),
        before[0]
    );
    assert!(hris.payroll.profiles.is_empty());
    assert_eq!(hris.employees, employees);
    assert_eq!(hris.access, access);
    let mut payroll = d.clone();
    Product::Payroll.project(&mut payroll);
    assert_eq!(
        serde_json::to_value(&payroll.payroll.slips).unwrap(),
        before
    );
    assert_eq!(
        serde_json::to_value(&payroll.payroll.profiles).unwrap(),
        profiles
    );
    assert_eq!(payroll.access, access);
    assert!(
        payroll.attendance.is_empty()
            && payroll.leave.is_empty()
            && payroll.accounts.is_empty()
            && payroll.documents.is_empty()
    );
    assert_eq!(payroll.employees, employees);
    assert_eq!(serde_json::to_value(&d.payroll.slips).unwrap(), before);
    assert_eq!(serde_json::to_value(&d.payroll.profiles).unwrap(), profiles);
}
#[test]
fn real_worker_admits_shared_payroll_action_only_in_payroll_and_preserves_demo_source_on_denial() {
    for product in [Product::Hris, Product::Payroll] {
        let s = DemoStore::new();
        let person = s.data.employees[0].id;
        let company = s.data.company.clone();
        let (commands, updates) =
            backend::worker_for(product, Some(s), eframe::egui::Context::default());
        commands.send(Command::Change(Action::Payroll(payroll::PayrollAction::new("profile_save",json!({"id":Uuid::new_v4(),"employee_id":person,"effective_on":"2026-02-01","pay":Pay{rate_cents:3_000_000,..Default::default()},"reason":"Fictional separate app test"}))))).unwrap();
        let u = updates
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(
            u.result.is_ok(),
            product == Product::Payroll,
            "{:?}",
            u.result.as_ref().err()
        );
        if let Ok(d) = u.result {
            assert_eq!(d.payroll.profiles.len(), 1);
            assert!(d.attendance.is_empty());
        }
        commands.send(Command::Refresh).unwrap();
        let d = updates
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .result
            .unwrap();
        assert_eq!(d.company, company);
        assert_eq!(
            d.payroll.profiles.len(),
            usize::from(product == Product::Payroll)
        );
    }
}
