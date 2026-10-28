//! Real loopback HTTP proves product-specific reads and fresh protected admission.
use super::government_export_tests::{access_json, client};
use super::*;
use crate::{
    desktop::Product,
    payroll::{Item, Pay, Profile, Slip, Totals, Worksheet},
    staff_roles::{Context as StaffContext, StaffRoles},
};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
fn data(roles: Value) -> Snapshot {
    let roles: StaffRoles = serde_json::from_value(roles).unwrap();
    let mut d = super::staff_roles_tests::data(roles.compatible_base_role());
    d.access.staff_contract_version = 1;
    d.access
        .bind_staff_context(StaffContext {
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
fn profile(d: &Snapshot, employee: Uuid, n: u128) -> Profile {
    Profile {
        id: Uuid::from_u128(n),
        company_id: d.company.id,
        employee_id: employee,
        effective_on: d.company.today(),
        pay: Pay {
            rate_cents: 3_000_000,
            ..Default::default()
        },
        revision: 0,
        created_by: Uuid::from_u128(1),
    }
}
fn slip(d: &Snapshot, employee: Uuid, n: u128) -> Slip {
    let id = Uuid::from_u128(n);
    let month = chrono::NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
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
            profile: profile(d, employee, n + 20),
            source_basis: json!({}),
            worksheet: Worksheet::default(),
            totals: Totals::default(),
            revision: 0,
        },
        approved_by: Uuid::from_u128(2),
        approved_at: Utc::now(),
        rule_version: crate::payroll::RULE_VERSION.into(),
    }
}
const PAYROLL_TABLES: &[&str] = &[
    "payroll_slips",
    "payroll_cutoff_slips",
    "payroll_thirteenth_slips",
    "payroll_final_slips",
    "payroll_final_status",
    "payroll_amendment_slips",
    "payroll_amendment_status",
    "payroll_tax_slips",
    "payroll_amendment_cases",
    "payroll_wage_classifications",
    "payroll_wage_settings",
    "payroll_wage_check_index",
    "payroll_profiles",
    "payroll_holidays",
    "payroll_runs",
    "payroll_items",
    "payroll_events",
    "payroll_cutoff_runs",
    "payroll_cutoff_items",
    "payroll_thirteenth_runs",
    "payroll_thirteenth_items",
    "payroll_final_cases",
    "payroll_tax_reconciliations",
    "payroll_payment_receipts",
];
struct Http {
    url: String,
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<Vec<String>>,
}
impl Http {
    fn finish(self) -> Vec<String> {
        self.stop.store(true, Ordering::Release);
        self.handle.join().unwrap()
    }
}
fn server(d: &Snapshot, product: Product, malicious: bool, revoke: bool) -> Http {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let done = stop.clone();
    let d = d.clone();
    let handle = thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut calls = Vec::new();
        let mut access_reads = 0;
        while !done.load(Ordering::Acquire) {
            assert!(
                std::time::Instant::now() < deadline,
                "Payroll HTTP fixture timed out"
            );
            let (mut socket, _) = match listener.accept() {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let uri = line.split_whitespace().nth(1).unwrap().to_owned();
            let url = reqwest::Url::parse(&format!("http://fixture{uri}")).unwrap();
            let path = url.path();
            let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
            let mut length = 0;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((k, v)) = line.split_once(':') {
                    if k.eq_ignore_ascii_case("content-length") {
                        length = v.trim().parse::<usize>().unwrap();
                    }
                }
            }
            assert!(length <= 4096);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let response = if path == "/rest/v1/rpc/my_hris_access" {
                access_reads += 1;
                let mut v = access_json(&d.access);
                v["staff_contract_version"] = json!(1);
                v
            } else if path == "/rest/v1/rpc/staff_role_context" {
                let mut context = d.access.staff_context.clone().unwrap();
                if revoke && access_reads > 1 {
                    context.roles = serde_json::from_value(json!(["recruiter"])).unwrap();
                    context.revision += 1;
                }
                serde_json::to_value(context).unwrap()
            } else if path == "/auth/v1/logout" {
                json!({})
            } else if path == "/rest/v1/rpc/lifecycle_notifications" {
                json!({"version":"lifecycle-notifications-v1","company_id":d.company.id,"user_id":d.access.user_id,"employee_id":d.access.employee_id,"role":d.access.role,"surface":"desktop","as_of":d.company.today(),"generated_at":Utc::now(),"read_keys":[],"total_items":0,"truncated":false,"items":[]})
            } else {
                let table = path.strip_prefix("/rest/v1/").unwrap();
                let own = d.access.employee_id.unwrap();
                if product == Product::Hris && PAYROLL_TABLES.contains(&table) {
                    assert_eq!(query.get("employee_id"), Some(&format!("eq.{own}")));
                }
                match table {
                    "companies" => json!([d.company]),
                    "employees" => json!(d.employees),
                    "employment_types" | "work_locations" => json!([]),
                    "employee_contacts" => {
                        assert_eq!(query.get("employee_id"), Some(&format!("eq.{own}")));
                        json!([crate::contacts::EmployeeContacts::empty(
                            d.company.id,
                            if malicious { d.employees[1].id } else { own }
                        )])
                    }
                    "payroll_profiles" => {
                        json!([profile(&d, own, 1001), profile(&d, d.employees[1].id, 2001)])
                    }
                    "payroll_slips" => {
                        let own_slip = slip(&d, own, 3001);
                        let peer_slip = slip(&d, d.employees[1].id, 4001);
                        if product == Product::Payroll {
                            json!([own_slip, peer_slip])
                        } else if malicious {
                            json!([peer_slip])
                        } else {
                            json!([own_slip])
                        }
                    }
                    t if PAYROLL_TABLES.contains(&t) => json!([]),
                    _ => panic!("Payroll attempted unrelated endpoint {table}"),
                }
            };
            calls.push(uri);
            let bytes = serde_json::to_vec(&response).unwrap();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",bytes.len()).unwrap();
            socket.write_all(&bytes).unwrap();
        }
        calls
    });
    Http { url, stop, handle }
}
#[test]
fn payroll_snapshot_reads_financial_roster_and_own_profile_without_loading_hr_workflows() {
    let d = data(json!(["payroll_officer"]));
    let http = server(&d, Product::Payroll, false, false);
    let mut c = client(http.url.clone(), &d);
    c.product = Some(Product::Payroll);
    let result = c.snapshot();
    let calls = http.finish();
    let actual = result.unwrap();
    assert_eq!(actual.payroll.profiles.len(), 2);
    assert_eq!(actual.payroll.slips.len(), 2);
    assert_eq!(actual.contacts.len(), 1);
    assert_eq!(actual.access, d.access);
    assert_eq!(actual.mode, Mode::Supabase);
    assert!(
        actual.approvals.is_none()
            && actual.attendance.is_empty()
            && actual.leave.is_empty()
            && actual.documents.is_empty()
            && actual.accounts.is_empty()
    );
    assert!(
        calls
            .iter()
            .any(|p| p.starts_with("/rest/v1/payroll_profiles?"))
    );
    assert_eq!(
        calls
            .iter()
            .filter(|p| p.as_str() == "/rest/v1/rpc/my_hris_access")
            .count(),
        2
    );
}
#[test]
fn hris_admin_payslips_are_employee_filtered_and_unexpected_foreign_rows_fail_closed() {
    let d = data(json!(["administrator"]));
    for malicious in [false, true] {
        let http = server(&d, Product::Hris, malicious, false);
        let mut c = client(http.url.clone(), &d);
        c.product = Some(Product::Hris);
        let result = c.payroll_snapshot(&d.access);
        let calls = http.finish();
        assert_eq!(result.is_ok(), !malicious);
        if let Ok(p) = result {
            assert_eq!(p.slips.len(), 1);
            assert_eq!(p.slips[0].employee_id, d.access.employee_id.unwrap());
            assert!(p.profiles.is_empty() && p.runs.is_empty() && p.receipts.is_empty());
        }
        assert_eq!(calls.len(), 8);
        assert!(
            !calls
                .iter()
                .any(|p| p.starts_with("/rest/v1/payroll_profiles?"))
        );
    }
}
#[test]
fn payroll_denies_every_unassigned_role_before_loading_records_and_ignores_auth_metadata() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../tests/fixtures/staff_role_capabilities.json"
    ))
    .unwrap();
    for case in cases {
        let d = data(case["roles"].clone());
        let expected = Product::Payroll.admits(&d.access);
        let http = server(&d, Product::Payroll, false, false);
        let mut c = client(http.url.clone(), &d);
        c.product = Some(Product::Payroll);
        c.session.user.user_metadata = json!({"role":"administrator","roles":["payroll_officer"]});
        let result = c.current_access();
        let calls = http.finish();
        assert_eq!(result.is_ok(), expected, "{}", case["roles"]);
        assert!(calls.iter().all(|p| p == "/rest/v1/rpc/my_hris_access"
            || p == "/rest/v1/rpc/staff_role_context"
            || p.starts_with("/auth/v1/logout?")));
    }
}
#[test]
fn payroll_rechecks_responsibility_after_loading_and_refuses_foreign_contact_data() {
    let d = data(json!(["payroll_officer"]));
    for (malicious, revoke) in [(true, false), (false, true)] {
        let http = server(&d, Product::Payroll, malicious, revoke);
        let mut c = client(http.url.clone(), &d);
        c.product = Some(Product::Payroll);
        let result = c.snapshot();
        let calls = http.finish();
        assert!(result.is_err());
        if revoke {
            assert!(result.err().unwrap().is::<AccessRevoked>());
            assert!(calls.iter().any(|p| p.starts_with("/auth/v1/logout?")));
        }
    }
}

fn financial_facts(p: &crate::payroll::Setup) -> Value {
    json!({"payroll_slips":p.slips,"payroll_cutoff_slips":p.cutoffs.slips,"payroll_thirteenth_slips":p.thirteenth.slips,"payroll_final_slips":p.finals.slips,"payroll_final_status":p.finals.statuses,"payroll_amendment_slips":p.amendments.slips,"payroll_amendment_status":p.amendments.statuses,"payroll_tax_slips":p.tax.slips,"payroll_amendment_cases":p.amendments.records,"payroll_wage_classifications":p.wages.classifications,"payroll_wage_settings":p.wages.settings,"payroll_wage_check_index":p.wages.checks,"payroll_profiles":p.profiles,"payroll_holidays":p.holidays,"payroll_runs":p.runs,"payroll_items":p.items,"payroll_events":p.events,"payroll_cutoff_runs":p.cutoffs.runs,"payroll_cutoff_items":p.cutoffs.items,"payroll_thirteenth_runs":p.thirteenth.runs,"payroll_thirteenth_items":p.thirteenth.items,"payroll_final_cases":p.finals.records,"payroll_tax_reconciliations":p.tax.records,"payroll_payment_receipts":p.receipts})
}

#[test]
#[ignore = "requires synthetic managed Auth on the exact installed dedicated project"]
fn hosted_native_payroll_product_snapshot() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = std::fs::read_to_string(root.join("supabase/.temp/project-ref")).unwrap();
    let target = target.trim();
    let guard =
        std::fs::read_to_string(root.join("deployment-private/SQL-AUTHORIZATION.md")).unwrap();
    assert!(guard.contains(&format!("{}{}{}", char::from(96), target, char::from(96))));
    let installed = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap())
        .join("Programs/HRIS Desktop/hris-connection.json");
    let config = ConnectionConfig::load_profile(&installed, true).unwrap();
    assert_eq!(
        config.url.trim_end_matches('/'),
        format!("https://{target}.supabase.co")
    );
    let session: AuthSession =
        serde_json::from_str(&std::env::var("HRIS_PAYROLL_AUTH_SESSION").unwrap()).unwrap();
    assert!(
        session.user.email.starts_with("qa-gov042-")
            && session.user.email.ends_with("@example.invalid")
    );
    let roles: StaffRoles =
        serde_json::from_str(&std::env::var("HRIS_PAYROLL_EXPECTED_ROLES").unwrap()).unwrap();
    let case = std::env::var("HRIS_PAYROLL_CASE").unwrap();
    assert!(case.len() < 40 && case.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
    let product = match std::env::var("HRIS_PAYROLL_PRODUCT").unwrap().as_str() {
        "payroll" => Product::Payroll,
        "hris" => Product::Hris,
        _ => panic!("Unsupported test product"),
    };
    let seed = data(json!(["administrator"]));
    let mut c = client(config.url.clone(), &seed);
    c.config = config;
    c.session = session;
    c.product = Some(product);
    c.http = Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let result = c.snapshot();
    let allowed = std::env::var("HRIS_PAYROLL_EXPECT_ACTIVE").as_deref() != Ok("false")
        && roles.uses_desktop()
        && (product == Product::Hris
            || roles.allows(crate::staff_roles::Capability::PayrollPreparation));
    assert_eq!(
        result.is_ok(),
        allowed,
        "Native product admission must match protected roles"
    );
    let proof = if let Ok(mut d) = result {
        assert_eq!(d.access.staff_roles().unwrap(), roles);
        if product == Product::Hris {
            assert!(
                d.payroll.profiles.is_empty()
                    && d.payroll.runs.is_empty()
                    && d.payroll.items.is_empty()
                    && d.payroll.receipts.is_empty()
            );
            assert!(
                d.payroll
                    .slips
                    .iter()
                    .all(|s| Some(s.employee_id) == d.access.employee_id)
            );
        } else {
            assert!(
                d.accounts.is_empty()
                    && d.documents.is_empty()
                    && d.attendance.is_empty()
                    && d.leave.is_empty()
                    && d.approvals.is_none()
            );
            assert!(
                d.contacts
                    .iter()
                    .all(|x| Some(x.employee_id) == d.access.employee_id)
            );
        }
        let mut financial_parity = false;
        let mut read_features = 0;
        let mut synthetic_salary_saved = false;
        if product == Product::Payroll {
            if std::env::var("HRIS_PAYROLL_WRITE_SYNTHETIC").as_deref() == Ok("true") {
                let person = d.own_employee().unwrap();
                assert!(person.employee_code.starts_with("QA-PAY100-"));
                let employee = person.id;
                let action = crate::payroll::PayrollAction::new(
                    "profile_save",
                    json!({"id":Uuid::new_v4(),"employee_id":employee,"effective_on":"2026-02-01","pay":Pay{rate_cents:3_000_000,..Default::default()},"reason":"Fictional Payroll Desktop parity verification"}),
                );
                d = c.execute(Action::Payroll(action), &d).unwrap();
                assert!(d.payroll.profiles.iter().any(|p| p.employee_id == employee));
                synthetic_salary_saved = true;
            }
            let mut legacy = client(c.config.url.clone(), &seed);
            legacy.config = c.config.clone();
            legacy.session =
                serde_json::from_str(&std::env::var("HRIS_PAYROLL_AUTH_SESSION").unwrap()).unwrap();
            legacy.http = c.http.clone();
            assert_eq!(legacy.current_access().unwrap(), d.access);
            let before = legacy.payroll_snapshot(&d.access).unwrap();
            legacy.verify_snapshot_staff_scope(&d.access).unwrap();
            assert!(
                financial_facts(&d.payroll) == financial_facts(&before),
                "Financial source projections differ; raw records are omitted from diagnostics."
            );
            financial_parity = true;
            let start = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
            let end = d.company.today();
            for axis in ["issued", "received"] {
                let filter = crate::payroll_reporting::Filter {
                    starts_on: start,
                    ends_on: end,
                    axis: axis.into(),
                    kind: "all".into(),
                    employee_id: None,
                    search: String::new(),
                };
                c.consolidated_payroll_report(&d, &filter, None).unwrap();
                read_features += 1;
            }
            c.government_registration(&d, None, None).unwrap();
            read_features += 1;
            c.government_annual_pay_details(&d, 2026, None).unwrap();
            read_features += 1;
            c.government_annual_ledger(&d, 2026, None).unwrap();
            read_features += 1;
            use chrono::Datelike;
            let month = chrono::NaiveDate::from_ymd_opt(end.year(), end.month(), 1).unwrap();
            for agency in [
                crate::payroll_government::Agency::Bir,
                crate::payroll_government::Agency::Sss,
                crate::payroll_government::Agency::Philhealth,
                crate::payroll_government::Agency::Pagibig,
            ] {
                c.government_reconciliation(
                    &d,
                    crate::payroll_government::Filter { month, agency },
                    None,
                    None,
                )
                .unwrap();
                read_features += 1;
            }
            c.payroll_exchange(&d, &crate::payroll_exchange::Operation::State)
                .unwrap();
            read_features += 1;
        }
        json!({"financial_source_parity":financial_parity,"financial_table_projections":if financial_parity{24}else{0},"protected_read_features":read_features,"synthetic_salary_saved":synthetic_salary_saved,"allowed":true,"role_count":roles.as_slice().len(),"profiles":d.payroll.profiles.len(),"slips":d.payroll.slips.len(),"employees":d.employees.len(),"no_unrelated_hr_records":product==Product::Payroll,"own_only_financial_projection":product==Product::Hris})
    } else {
        json!({"allowed":false})
    };
    std::fs::write(
        root.join(format!("work/payroll-desktop-native-{case}.json")),
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .unwrap();
}

#[test]
fn installed_payroll_missing_company_profile_or_wrong_product_marker_never_falls_back_to_demo() {
    let folder = std::env::temp_dir().join(format!("payroll-company-profile-{}", Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    let marker = folder.join(Product::Payroll.marker());
    std::fs::write(&marker, Product::Payroll.id()).unwrap();
    assert!(ConnectionConfig::load_folder(&folder, Product::Payroll).is_err());
    std::fs::write(
        folder.join("hris-connection.json"),
        serde_json::to_vec(&ConnectionConfig {
            url: "https://fictional.supabase.co".into(),
            publishable_key: "sb_publishable_fictional_deployment".into(),
        })
        .unwrap(),
    )
    .unwrap();
    assert!(ConnectionConfig::load_folder(&folder, Product::Payroll).is_ok());
    std::fs::write(&marker, Product::Hris.id()).unwrap();
    assert!(ConnectionConfig::load_folder(&folder, Product::Payroll).is_err());
    std::fs::remove_file(marker).unwrap();
    std::fs::remove_file(folder.join("hris-connection.json")).unwrap();
    std::fs::remove_dir(folder).unwrap();
}
