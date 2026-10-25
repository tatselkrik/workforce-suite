//! Explicit opt-in for guarded, synthetic hosted DTO captures. No credential is read.
use hris_desktop::{
    access::Access,
    attendance_imports::{Batch, History, Post, Preview, Receipt},
    domain::{Attendance, Company, Employee, Mode, Snapshot},
};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    access: Access,
    company: Company,
    employees: Vec<Employee>,
    attendance: Vec<Attendance>,
    history: History,
    batch: Option<Batch>,
    preview: Option<Preview>,
    receipt: Option<Receipt>,
    preview_is_current: bool,
}
#[test]
#[ignore = "requires guarded hosted synthetic fixtures"]
fn actual_hosted_attendance_imports_match_native_contracts() {
    let path = std::env::var_os("HRIS_ATTENDANCE_IMPORT_LIVE_FIXTURES")
        .expect("Set the private synthetic fixture path");
    let bytes = std::fs::read(path).unwrap();
    assert!(bytes.len() <= 8_000_000);
    let cases: Vec<Case> = serde_json::from_slice(&bytes).expect("Hosted DTOs must decode");
    assert!(!cases.is_empty());
    let mut count = 0;
    for c in cases {
        let mut d = Snapshot::signed_out();
        d.mode = Mode::Supabase;
        d.company = c.company;
        d.access = c.access;
        d.employees = c.employees;
        d.attendance = c.attendance;
        c.history
            .validate(&d)
            .expect("Protected hosted history binding failed");
        if let (Some(batch), Some(preview)) = (c.batch, c.preview) {
            if c.preview_is_current {
                preview
                    .validate(&d, &batch)
                    .expect("Current protected preview binding failed");
            }
            if let Some(receipt) = c.receipt {
                let post = Post {
                    scope: d.access.clone(),
                    batch,
                    preview,
                };
                receipt
                    .validate(&d, &post)
                    .expect("Exact hosted receipt binding failed");
            }
        }
        assert!(!c.name.is_empty());
        count += 1;
    }
    println!(
        "Verified {count} protected hosted attendance-import projections through actual native contracts."
    );
}
