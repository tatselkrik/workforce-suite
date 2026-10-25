use chrono::NaiveDate;
use hris_desktop::{
    domain::*,
    leave::{entitlements::*, *},
};
fn d(s: &str) -> NaiveDate {
    s.parse().unwrap()
}
fn person() -> Employee {
    let mut e = DemoStore::new().data.employees[0].clone();
    e.joined_on = Some(d("2025-01-01"));
    e.annual_leave_days = 15;
    e
}
fn annual() -> LeaveType {
    LeaveType {
        code: "annual".into(),
        name: "Annual".into(),
        use_employee_allowance: true,
        ..Default::default()
    }
}
#[test]
fn later_allowance_policy_and_hire_edits_do_not_rewrite_capture() {
    let mut e = person();
    let mut t = annual();
    let b = FrozenBasis::capture(&e, &t, d("2027-02-01")).unwrap();
    e.annual_leave_days = 50;
    e.joined_on = Some(d("2027-01-01"));
    t.annual_units = 99;
    t.use_employee_allowance = false;
    assert_eq!(b.earned_rate(d("2027-12-31")).unwrap(), 30);
    assert_ne!(
        b.rate_units,
        FrozenBasis::capture(&e, &t, d("2027-02-01"))
            .unwrap()
            .rate_units
    );
}
#[test]
fn monthly_fraction_is_rounded_as_a_total_not_each_month() {
    let e = person();
    let mut t = annual();
    t.accrual = Accrual::Monthly;
    let b = FrozenBasis::capture(&e, &t, d("2027-02-01")).unwrap();
    assert_eq!(b.earned_rate_at_capture, 2);
    assert_eq!(b.earned_rate(d("2027-02-28")).unwrap(), 5);
    assert_eq!(b.earned_rate(d("2027-12-31")).unwrap(), 30);
}
#[test]
fn partial_joining_month_does_not_gain_a_full_month() {
    let mut e = person();
    e.joined_on = Some(d("2027-01-15"));
    let mut t = annual();
    t.accrual = Accrual::Monthly;
    let b = FrozenBasis::capture(&e, &t, d("2027-02-01")).unwrap();
    assert_eq!(b.earned_rate_at_capture, 0);
    assert_eq!(b.earned_rate(d("2027-02-28")).unwrap(), 2);
}
#[test]
fn captured_input_cannot_claim_dates_before_it_or_another_year() {
    let b = FrozenBasis::capture(&person(), &annual(), d("2027-02-01")).unwrap();
    assert!(b.earned_rate(d("2027-01-31")).is_err());
    assert!(b.earned_rate(d("2028-01-01")).is_err());
}
#[test]
fn unsupported_or_missing_facts_and_forged_totals_are_rejected() {
    let mut e = person();
    e.joined_on = None;
    let mut t = annual();
    t.accrual = Accrual::Monthly;
    assert!(FrozenBasis::capture(&e, &t, d("2027-02-01")).is_err());
    t.accrual = Accrual::Unlimited;
    assert!(FrozenBasis::capture(&e, &t, d("2027-02-01")).is_err());
    let mut b = FrozenBasis::capture(&person(), &annual(), d("2027-02-01")).unwrap();
    b.earned_rate_at_capture = 300;
    assert!(b.validate().is_err());
}
#[test]
fn same_day_later_credits_are_counted_once_without_rebasing_capture() {
    let b = FrozenBasis::capture(&person(), &annual(), d("2027-02-01")).unwrap();
    let at_post = b.base_with_credits(b.known_from, 4).unwrap();
    let later = b.base_with_credits(b.known_from, 7).unwrap();
    assert_eq!(later - at_post, 3);
    assert!(b.base_with_credits(b.known_from, i32::MAX).is_err());
}
