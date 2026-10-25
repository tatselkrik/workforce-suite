//! Opt-in decoding of protected hosted responses. The harness supplies only
//! synthetic employee rows and deletes its temporary payload after verification.
use chrono::{Datelike, NaiveDate};
use hris_desktop::{
    access::Access,
    domain::*,
    leave::{self, Balance, dated::Context},
};
use serde::Deserialize;
use uuid::Uuid;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    employee_id: Uuid,
    type_code: String,
    on: NaiveDate,
    balance: Balance,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    access: Access,
    company: Company,
    employees: Vec<Employee>,
    leave: Vec<LeaveRequest>,
    credits: leave::credits::Context,
    context: Context,
    expected: Vec<Expected>,
}
#[test]
#[ignore = "requires guarded hosted synthetic fixtures"]
fn actual_hosted_dated_leave_contracts_decode_and_match_native_balances() {
    let path = std::env::var_os("HRIS_DATED_LIVE_FIXTURES")
        .expect("Set the private guarded fixture path.");
    let bytes = std::fs::read(path).unwrap();
    assert!(bytes.len() <= 8_000_000);
    let cases: Vec<Case> =
        serde_json::from_slice(&bytes).expect("Hosted fixture contract did not decode.");
    assert!(!cases.is_empty());
    let mut count = 0;
    for c in cases {
        let today = c.company.today();
        c.context
            .validate(&c.access, today)
            .expect("Hosted dated leave access/date projection failed validation.");
        c.credits
            .validate(&c.access, today)
            .expect("Hosted credit projection failed validation.");
        assert!(c.employees.iter().all(|e| e.company_id == c.company.id));
        assert!(
            c.context
                .anchors
                .iter()
                .all(|a| c.employees.iter().any(|e| e.id == a.employee_id))
        );
        let mut data = Snapshot::signed_out();
        data.mode = Mode::Supabase;
        data.access = c.access;
        data.company = c.company;
        data.employees = c.employees;
        data.leave = c.leave;
        data.leave_setup.policy = c.context.timeline.policy_on(today).clone();
        data.leave_setup.dated = Some(c.context);
        data.leave_setup.credits = Some(c.credits);
        for expected in c.expected {
            let e = data
                .employees
                .iter()
                .find(|e| e.id == expected.employee_id)
                .expect("Expected employee not present in the synthetic scope.");
            let t = leave::dated::policy_on(&data, expected.on)
                .leave_type(&expected.type_code)
                .expect("Expected dated type missing.");
            let b = leave::balance_checked(&data, e, t, expected.on.year(), expected.on, None)
                .expect("Native balance could not use the hosted captured basis.");
            assert!(
                b == expected.balance,
                "Native and hosted balances differed in a named synthetic case."
            );
        }
        assert!(!c.name.is_empty());
        count += 1;
    }
    println!(
        "Verified {count} protected hosted dated-leave projections through the actual native contracts."
    );
}
