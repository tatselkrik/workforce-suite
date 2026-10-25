use chrono::{Datelike, Duration, Utc};
use hris_desktop::{
    access::Role,
    domain::*,
    leave::{self, credits::*},
};
use uuid::Uuid;
fn context(data: &Snapshot, entries: Vec<Entry>) -> Context {
    let audit = entries
        .iter()
        .enumerate()
        .map(|(i, e)| Audit {
            id: e.id,
            reason: "Fictional reviewed credit".into(),
            reference: String::new(),
            created_by_name: "Fictional Administrator".into(),
            posted_at: Utc::now(),
            company_revision: i as i64 + 1,
        })
        .collect();
    Context {
        version: "leave-credits-v1".into(),
        company_id: data.company.id,
        user_id: data.access.user_id,
        employee_id: data.access.employee_id,
        role: data.access.role,
        as_of: data.company.today(),
        revision: entries.len() as i64,
        entries,
        audit,
    }
}
fn entry(data: &Snapshot, units: i32, days: i64) -> Entry {
    Entry {
        id: Uuid::from_u128(470001),
        employee_id: data.employees[0].id,
        type_code: "annual".into(),
        effective_on: data.company.today() + Duration::days(days),
        units,
        reverses_id: None,
    }
}
#[test]
fn signed_half_day_parser_never_rounds_an_inexact_adjustment() {
    for (text, units) in [
        ("2", 4),
        ("-1", -2),
        ("+0.5", 1),
        ("-0.5", -1),
        ("366", 732),
        ("-366", -732),
        (" 1.5 ", 3),
    ] {
        assert_eq!(parse_delta(text).unwrap(), units);
    }
    for text in [
        "",
        "0",
        "-0.0",
        "1.25",
        "1e2",
        "NaN",
        "--1",
        "366.5",
        "9999999999999",
        ".5",
        "1.5x",
    ] {
        assert!(parse_delta(text).is_err(), "{text}");
    }
}
#[test]
fn dated_credits_and_reversals_preserve_prior_dates_and_capped_carry() {
    let mut data = DemoStore::new().data;
    data.leave.clear();
    let today = data.company.today();
    let year = today.year();
    data.leave_setup.policy.tracking_starts_year = year;
    data.employees[0].joined_on = Some(chrono::NaiveDate::from_ymd_opt(year, 1, 1).unwrap());
    let kind = data
        .leave_setup
        .policy
        .types
        .iter_mut()
        .find(|t| t.code == "annual")
        .unwrap();
    kind.carryover_units = 8;
    let kind = kind.clone();
    let original = entry(&data, 3, 2);
    let mut reverse = entry(&data, -3, 4);
    reverse.id = Uuid::from_u128(470002);
    reverse.reverses_id = Some(original.id);
    let c = context(&data, vec![original, reverse]);
    c.validate(&data.access, today).unwrap();
    data.leave_setup.credits = Some(c);
    let balance =
        |day| leave::balance(&data, &data.employees[0], &kind, year, day, None).available_units;
    assert_eq!(balance(today), 30);
    assert_eq!(balance(today + Duration::days(2)), 33);
    assert_eq!(balance(today + Duration::days(4)), 30);
    let next = chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap();
    assert_eq!(
        leave::balance(&data, &data.employees[0], &kind, year + 1, next, None).carryover_units,
        8
    );
}
#[test]
fn bound_credit_context_rejects_foreign_scope_private_history_and_bad_reversals() {
    let mut data = DemoStore::new().data;
    let row = entry(&data, 1, 0);
    let good = context(&data, vec![row.clone()]);
    good.validate(&data.access, data.company.today()).unwrap();
    let mut bad = good.clone();
    bad.user_id = Uuid::new_v4();
    assert!(bad.validate(&data.access, data.company.today()).is_err());
    let mut bad = good.clone();
    bad.entries.push(row.clone());
    assert!(bad.validate(&data.access, data.company.today()).is_err());
    let mut bad = good.clone();
    bad.audit.clear();
    assert!(bad.validate(&data.access, data.company.today()).is_err());
    let mut bad = good.clone();
    bad.entries[0].reverses_id = Some(row.id);
    assert!(bad.validate(&data.access, data.company.today()).is_err());
    let mut bad = good.clone();
    bad.audit[0].reason = "line\nfeed".into();
    assert!(bad.validate(&data.access, data.company.today()).is_err());
    let mut raw = serde_json::to_value(&good).unwrap();
    raw["private_token"] = serde_json::json!("FORBIDDEN");
    assert!(serde_json::from_value::<Context>(raw).is_err());
    data.access.role = Role::Manager;
    let mut limited = good;
    limited.role = Role::Manager;
    limited.audit.clear();
    limited
        .validate(&data.access, data.company.today())
        .unwrap();
    limited.entries[0].employee_id = data.employees[2].id;
    assert!(
        limited
            .validate(&data.access, data.company.today())
            .is_err()
    );
    data.access.direct_reports.push(data.employees[2].id);
    limited
        .validate(&data.access, data.company.today())
        .unwrap();
}
#[test]
fn administrator_records_require_current_scope_finite_type_reason_and_revision() {
    let mut data = DemoStore::new().data;
    let today = data.company.today();
    data.leave_setup.credits = Some(context(&data, vec![]));
    let r = Record {
        id: Uuid::from_u128(470010),
        employee_id: data.employees[0].id,
        type_code: "annual".into(),
        effective_on: today,
        units: 1,
        reverses_id: None,
        expected_revision: 0,
        reason: "Reviewed correction".into(),
        reference: String::new(),
    };
    r.validate(&data).unwrap();
    let mut bad = r.clone();
    bad.expected_revision = 1;
    assert!(bad.validate(&data).is_err());
    let mut bad = r.clone();
    bad.effective_on = today - Duration::days(1);
    assert!(bad.validate(&data).is_err());
    let mut bad = r.clone();
    bad.type_code = "sick".into();
    assert!(bad.validate(&data).is_err());
    let mut bad = r.clone();
    bad.reverses_id = Some(Uuid::new_v4());
    assert!(bad.validate(&data).is_err());
    data.access.role = Role::HrGeneralist;
    assert!(r.validate(&data).is_err());
}
#[test]
fn demo_post_and_retry_preserve_original_and_overdraw_is_atomic() {
    let mut store = DemoStore::new();
    store.data.leave.clear();
    let today = store.data.company.today();
    let r = Record {
        id: Uuid::from_u128(470011),
        employee_id: store.data.employees[0].id,
        type_code: "annual".into(),
        effective_on: today,
        units: 3,
        reverses_id: None,
        expected_revision: 0,
        reason: "Reviewed synthetic credit".into(),
        reference: String::new(),
    };
    store.execute(Action::LeaveCredit(r.clone())).unwrap();
    store.execute(Action::LeaveCredit(r.clone())).unwrap();
    assert_eq!(
        store
            .data
            .leave_setup
            .credits
            .as_ref()
            .unwrap()
            .entries
            .len(),
        1
    );
    let mut bad = r.clone();
    bad.id = Uuid::from_u128(470012);
    bad.expected_revision = 1;
    bad.units = -100;
    assert!(store.execute(Action::LeaveCredit(bad)).is_err());
    assert_eq!(store.data.leave_setup.credits.as_ref().unwrap().revision, 1);
    let mut reverse = r.clone();
    reverse.id = Uuid::from_u128(470013);
    reverse.units = -3;
    reverse.expected_revision = 1;
    reverse.reverses_id = Some(r.id);
    store.execute(Action::LeaveCredit(reverse.clone())).unwrap();
    store.execute(Action::LeaveCredit(reverse)).unwrap();
    let c = store.data.leave_setup.credits.as_ref().unwrap();
    assert_eq!(c.entries.len(), 2);
    assert_eq!(c.units(r.employee_id, "annual", today.year(), today), 0);
}
