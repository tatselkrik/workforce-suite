use chrono::NaiveDate;
use hris_desktop::leave::{versions::*, *};
use uuid::Uuid;
fn d(s: &str) -> NaiveDate {
    s.parse().unwrap()
}
fn timeline() -> Timeline {
    Timeline {
        baseline: LeavePolicy {
            company_id: Uuid::from_u128(1),
            tracking_starts_year: 2026,
            ..Default::default()
        },
        versions: vec![],
    }
}
fn next(t: &Timeline, on: &str) -> Scheduled {
    let mut policy = t.versions.last().map_or(&t.baseline, |v| &v.policy).clone();
    policy.revision += 1;
    Scheduled {
        id: Uuid::new_v4(),
        effective_on: d(on),
        policy,
        withdrawn: false,
    }
}
#[test]
fn future_calendar_never_rewrites_a_previous_charge_snapshot() {
    let mut t = timeline();
    let old = t
        .charges("annual", d("2027-03-01"), d("2027-03-03"), DayPart::Full)
        .unwrap();
    let mut v = next(&t, "2027-03-02");
    v.policy.holidays.push(Holiday {
        date: d("2027-03-02"),
        name: "Fictional future holiday".into(),
    });
    t.validate_schedule(&v, d("2026-10-25")).unwrap();
    t.versions.push(v);
    assert_eq!(old.len(), 3);
    let fresh = t
        .charges("annual", d("2027-03-01"), d("2027-03-03"), DayPart::Full)
        .unwrap();
    assert_eq!(fresh.len(), 2);
    assert_eq!(old.len(), 3);
    assert_eq!(t.policy_on(d("2027-03-01")).revision, 0);
    assert_eq!(t.policy_on(d("2027-03-02")).revision, 1);
}
#[test]
fn annual_terms_change_only_at_a_future_year_boundary() {
    let t = timeline();
    let mut v = next(&t, "2026-11-01");
    v.policy.types[0].annual_units = 40;
    assert!(t.validate_schedule(&v, d("2026-10-25")).is_err());
    v.effective_on = d("2027-01-01");
    assert!(t.validate_schedule(&v, d("2026-10-25")).is_ok());
    v.effective_on = d("2026-01-01");
    assert!(t.validate_schedule(&v, d("2026-10-25")).is_err());
}
#[test]
fn prospective_name_chain_and_workweek_changes_allow_midyear() {
    let t = timeline();
    let mut v = next(&t, "2026-11-01");
    v.policy.types[0].name = "Planned paid leave".into();
    v.policy.types[0].approval_chain = vec![ApprovalRule {
        kind: ApproverKind::Administrator,
        employee_id: None,
    }];
    v.policy.workweek = vec![1, 2, 3, 4];
    assert!(t.validate_schedule(&v, d("2026-10-25")).is_ok());
}
#[test]
fn only_unactivated_tail_can_be_withdrawn_and_the_old_selection_remains() {
    let mut t = timeline();
    let one = next(&t, "2026-11-01");
    let id = one.id;
    t.versions.push(one);
    let two = next(&t, "2026-12-01");
    let tail = two.id;
    t.versions.push(two);
    assert!(!t.can_withdraw(id, d("2026-10-25"), &[]));
    assert!(t.can_withdraw(tail, d("2026-10-25"), &[]));
    assert!(!t.can_withdraw(tail, d("2026-12-01"), &[]));
    t.versions[1].withdrawn = true;
    t.validate().unwrap();
    assert_eq!(t.policy_on(d("2027-01-01")).revision, 1);
}
#[test]
fn overlapping_or_out_of_order_versions_and_lost_codes_are_rejected() {
    let mut t = timeline();
    let v = next(&t, "2026-12-01");
    t.versions.push(v);
    let mut p = next(&t, "2026-11-01");
    assert!(t.validate_schedule(&p, d("2026-10-25")).is_err());
    p.effective_on = d("2027-01-01");
    p.policy.types.retain(|t| t.code != "sick");
    assert!(t.validate_schedule(&p, d("2026-10-25")).is_err());
}
#[test]
fn activation_boundaries_halfdays_and_cross_year_restrictions_are_explicit() {
    let mut t = timeline();
    let mut v = next(&t, "2027-03-02");
    v.policy.types[0].active = false;
    t.versions.push(v);
    assert_eq!(
        t.charges("annual", d("2027-03-01"), d("2027-03-01"), DayPart::Am)
            .unwrap()[0]
            .units,
        1
    );
    assert!(
        t.charges("annual", d("2027-03-01"), d("2027-03-03"), DayPart::Full)
            .is_err()
    );
    assert!(
        t.charges("annual", d("2026-12-31"), d("2027-01-01"), DayPart::Full)
            .is_err()
    );
}

#[test]
fn live_request_references_prevent_withdrawal_and_revisions_never_reuse() {
    let mut t = timeline();
    let first = next(&t, "2026-11-01");
    let id = first.id;
    t.versions.push(first);
    assert!(t.can_withdraw(id, d("2026-10-25"), &[]));
    assert!(!t.can_withdraw(id, d("2026-10-25"), &[1]));
    t.versions[0].withdrawn = true;
    let mut duplicate = next(&t, "2026-11-02");
    duplicate.policy.revision = 1;
    t.versions.push(duplicate);
    assert!(t.validate().is_err());
}
