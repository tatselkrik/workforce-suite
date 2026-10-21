use hris_desktop::{access::Role, domain::*, leave::*};
use uuid::Uuid;
fn date(s: &str) -> chrono::NaiveDate {
    parse_date(s).unwrap()
}
fn store() -> DemoStore {
    let mut s = DemoStore::new();
    s.data.leave.clear();
    for e in &mut s.data.employees {
        e.joined_on = None;
    }
    s
}
fn request(person: Uuid, code: &str, start: &str, end: &str, part: DayPart) -> LeaveDraft {
    LeaveDraft {
        employee_id: Some(person),
        starts_on: start.into(),
        ends_on: end.into(),
        options: LeaveOptions {
            type_code: code.into(),
            day_part: part,
        },
        ..Default::default()
    }
}
#[test]
fn calendar_and_half_days_use_exact_saved_charge_dates() {
    let mut s = store();
    let mut p = s.data.leave_setup.policy.clone();
    p.workweek = vec![2, 3, 4, 5, 6];
    p.holidays.push(Holiday {
        date: date("2028-01-05"),
        name: "Synthetic holiday".into(),
    });
    s.execute(Action::SaveLeavePolicy(p)).unwrap();
    let id = s.data.employees[3].id;
    let draft = request(id, "annual", "2028-01-04", "2028-01-08", DayPart::Full);
    let row = prepare(&s.data, &draft).unwrap();
    assert_eq!(row.units(), 8);
    assert_eq!(row.policy.charged_dates.len(), 4);
    assert!(!row.charges().iter().any(|d| d.date == date("2028-01-05")));
    assert!(
        prepare(
            &s.data,
            &request(id, "annual", "2028-01-05", "2028-01-05", DayPart::Am)
        )
        .is_err()
    );
    assert!(
        prepare(
            &s.data,
            &request(id, "annual", "2028-01-04", "2028-01-06", DayPart::Am)
        )
        .is_err()
    );
    s.execute(Action::RequestLeave(request(
        id,
        "annual",
        "2028-01-04",
        "2028-01-04",
        DayPart::Am,
    )))
    .unwrap();
    assert!(
        s.execute(Action::RequestLeave(request(
            id,
            "annual",
            "2028-01-04",
            "2028-01-04",
            DayPart::Am
        )))
        .is_err()
    );
    s.execute(Action::RequestLeave(request(
        id,
        "annual",
        "2028-01-04",
        "2028-01-04",
        DayPart::Pm,
    )))
    .unwrap();
    let saved = s.data.leave.clone();
    let mut p = s.data.leave_setup.policy.clone();
    p.holidays.clear();
    p.workweek = vec![1, 2, 3, 4, 5];
    s.execute(Action::SaveLeavePolicy(p)).unwrap();
    assert_eq!(s.data.leave, saved);
}
#[test]
fn monthly_accrual_reservations_and_cancellation_do_not_double_charge() {
    let mut s = store();
    let person = s.data.employees[3].id;
    s.data.employees[3].joined_on = Some(date("2027-01-01"));
    let t = LeaveType {
        code: "wellness".into(),
        name: "Wellness".into(),
        annual_units: 24,
        accrual: Accrual::Monthly,
        ..Default::default()
    };
    s.data.leave_setup.policy.types.push(t.clone());
    s.execute(Action::RequestLeave(request(
        person,
        "wellness",
        "2028-03-01",
        "2028-03-02",
        DayPart::Full,
    )))
    .unwrap();
    let row = s.data.leave[0].clone();
    let e = s.data.employees.iter().find(|e| e.id == person).unwrap();
    let b = balance(&s.data, e, &t, 2028, date("2028-03-01"), None);
    assert_eq!(
        (b.earned_units, b.reserved_units, b.available_units),
        (4, 4, 0)
    );
    assert!(
        prepare(
            &s.data,
            &request(person, "wellness", "2028-03-03", "2028-03-03", DayPart::Am)
        )
        .is_err()
    );
    s.execute(Action::CancelLeave(
        row.id,
        0,
        "Synthetic cancellation".into(),
    ))
    .unwrap();
    assert!(
        s.execute(Action::CancelLeave(row.id, 0, "Repeat".into()))
            .is_err()
    );
    let e = s.data.employees.iter().find(|e| e.id == person).unwrap();
    assert_eq!(
        balance(&s.data, e, &t, 2028, date("2028-03-01"), None).available_units,
        4
    );
    assert_eq!(
        s.data
            .leave_setup
            .events
            .iter()
            .filter(|e| e.action == "cancelled")
            .count(),
        1
    );
}

#[test]
fn unknown_hire_dates_do_not_invent_accrual_or_carryover() {
    let mut s = store();
    let e = s.data.employees[3].clone();
    let t = LeaveType {
        code: "accrued".into(),
        name: "Accrued".into(),
        accrual: Accrual::Monthly,
        carryover_units: 10,
        ..Default::default()
    };
    s.data.leave_setup.policy.types.push(t.clone());
    let b = balance(&s.data, &e, &t, 2029, date("2029-05-01"), None);
    assert_eq!((b.earned_units, b.carryover_units), (0, 0));
    assert!(
        prepare(
            &s.data,
            &request(e.id, "accrued", "2029-05-01", "2029-05-01", DayPart::Am)
        )
        .unwrap_err()
        .to_string()
        .contains("Date joined")
    );
    let mut t = t;
    t.accrual = Accrual::Upfront;
    assert_eq!(
        balance(&s.data, &e, &t, 2029, date("2029-01-01"), None).carryover_units,
        0
    );
}
#[test]
fn carryover_expires_and_monthly_partial_joining_month_is_excluded() {
    let mut s = store();
    let e = &mut s.data.employees[3];
    e.joined_on = Some(date("2027-01-15"));
    let e = e.clone();
    let t = LeaveType {
        code: "annual".into(),
        name: "Annual".into(),
        annual_units: 25,
        accrual: Accrual::Monthly,
        carryover_units: 4,
        ..Default::default()
    };
    assert_eq!(
        balance(&s.data, &e, &t, 2027, date("2027-02-28"), None).earned_units,
        2
    );
    let before = balance(&s.data, &e, &t, 2028, date("2028-03-31"), None);
    let after = balance(&s.data, &e, &t, 2028, date("2028-04-01"), None);
    assert_eq!(before.carryover_units, 4);
    assert_eq!(after.carryover_units, 0);
    assert_eq!(before.available_units - after.available_units, 4);
}
#[test]
fn approval_chains_enforce_order_self_denial_and_revision_and_keep_snapshots() {
    let mut s = store();
    let person = s.data.employees[3].id;
    let manager = s.data.employees[1].id;
    s.data.employees[3].reports_to = Some(manager);
    s.data.leave_setup.policy.types[0].approval_chain = vec![
        ApprovalRule {
            kind: ApproverKind::Manager,
            employee_id: None,
        },
        ApprovalRule {
            kind: ApproverKind::Hr,
            employee_id: None,
        },
    ];
    s.execute(Action::RequestLeave(request(
        person,
        "annual",
        "2028-03-01",
        "2028-03-01",
        DayPart::Am,
    )))
    .unwrap();
    let id = s.data.leave[0].id;
    assert!(!can_review(&s.data, &s.data.leave[0]));
    s.data.access.role = Role::Manager;
    s.data.access.employee_id = Some(person);
    assert!(!can_review(&s.data, &s.data.leave[0]));
    s.data.access.employee_id = Some(manager);
    s.data.access.user_id = Uuid::from_u128(9001);
    s.data.access.direct_reports = vec![person];
    s.execute(Action::ReviewLeave(id, 0, true, "manager".into()))
        .unwrap();
    assert_eq!(s.data.leave[0].status, LeaveStatus::Pending);
    s.data.access.role = Role::HrGeneralist;
    s.data.access.employee_id = Some(s.data.employees[2].id);
    s.data.access.user_id = Uuid::from_u128(9002);
    assert!(
        s.execute(Action::ReviewLeave(id, 0, true, "stale".into()))
            .is_err()
    );
    s.execute(Action::ReviewLeave(id, 1, true, "HR".into()))
        .unwrap();
    assert_eq!(s.data.leave[0].status, LeaveStatus::Approved);
    assert_eq!(s.data.leave[0].policy.approval_steps.len(), 2);
    assert!(
        s.execute(Action::ReviewLeave(id, 2, true, "again".into()))
            .is_err()
    );
}
#[test]
fn policies_retain_type_codes_and_deactivated_types_reject_new_requests() {
    let mut s = store();
    let mut p = s.data.leave_setup.policy.clone();
    p.workweek.clear();
    assert!(p.validate(None).is_err());
    p = s.data.leave_setup.policy.clone();
    p.types.pop();
    assert!(p.validate(Some(&s.data.leave_setup.policy)).is_err());
    let mut p = s.data.leave_setup.policy.clone();
    p.types[0].active = false;
    s.execute(Action::SaveLeavePolicy(p.clone())).unwrap();
    assert!(s.execute(Action::SaveLeavePolicy(p)).is_err());
    assert!(
        prepare(
            &s.data,
            &request(
                s.data.employees[3].id,
                "annual",
                "2028-03-01",
                "2028-03-01",
                DayPart::Full
            )
        )
        .is_err()
    );
}
