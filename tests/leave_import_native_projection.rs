//! Explicit guarded synthetic wire captures; balance parity has separate checks.
use hris_desktop::{
    access::Access,
    domain::{Company, Employee, Mode, Snapshot},
    leave::request_import::{Batch, History, Post, Preview, Receipt},
};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    access: Access,
    company: Company,
    employees: Vec<Employee>,
    history: History,
    batch: Option<Batch>,
    preview: Option<Preview>,
    receipt: Option<Receipt>,
}
#[test]
#[ignore = "requires guarded hosted synthetic fixtures"]
fn actual_hosted_leave_imports_decode_and_bind_receipts_history() {
    let path = std::env::var_os("HRIS_LEAVE_IMPORT_LIVE_FIXTURES")
        .expect("Set private synthetic fixture path");
    let bytes = std::fs::read(path).unwrap();
    assert!(bytes.len() <= 8_000_000);
    let cases: Vec<Case> =
        serde_json::from_slice(&bytes).expect("Hosted leave import wire contract must decode");
    assert!(!cases.is_empty());
    let mut count = 0;
    for c in cases {
        let mut d = Snapshot::signed_out();
        d.mode = Mode::Supabase;
        d.access = c.access;
        d.company = c.company;
        d.employees = c.employees;
        c.history.validate(&d).unwrap();
        if let (Some(batch), Some(preview)) = (c.batch, c.preview) {
            assert_eq!(preview.rows.len(), batch.rows.len());
            let post = Post {
                scope: d.access.clone(),
                batch,
                preview,
            };
            post.identity(&d.access).unwrap();
            if let Some(r) = c.receipt {
                r.validate(&d, &post).unwrap();
            }
        }
        assert!(!c.name.is_empty());
        count += 1;
    }
    println!(
        "Verified {count} protected hosted leave-import wire projections and receipt/history bindings. Funding parity is tested separately."
    );
}
