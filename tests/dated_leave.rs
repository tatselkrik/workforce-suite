use chrono::{Datelike, Duration, NaiveDate};
use hris_desktop::leave::dated::apply_demo;
use hris_desktop::{
    access::{Access, Role},
    domain::*,
    leave::{
        self,
        dated::*,
        opening_import::{BatchPreview, Import, Post},
        versions::Scheduled,
        *,
    },
};
use uuid::Uuid;
fn seed() -> Snapshot {
    let mut d = DemoStore::new().data;
    d.leave.clear();
    let year = d.company.today().year();
    d.access = Access {
        company_id: d.company.id,
        user_id: Uuid::from_u128(49001),
        employee_id: Some(d.employees[0].id),
        role: Role::Administrator,
        ..Default::default()
    };
    for e in &mut d.employees {
        e.joined_on = Some(NaiveDate::from_ymd_opt(year - 1, 1, 1).unwrap());
        e.annual_leave_days = 15;
        e.status = EmployeeStatus::Active;
    }
    d.leave_setup.policy.tracking_starts_year = year - 1;
    d.leave_setup.policy.types[0].carryover_units = 12;
    d.leave_setup.policy.types[0].carryover_expiry_month = 12;
    d.leave_setup.credits = Some(leave::credits::Context::empty(&d));
    d.leave_setup.dated = Some(Context::empty(&d));
    d
}
fn opening(d: &Snapshot, person: Uuid) -> OpeningRecord {
    OpeningRecord {
        id: Uuid::new_v4(),
        employee_id: person,
        type_code: "annual".into(),
        effective_on: d.company.today(),
        target_available_units: 12,
        remaining_carry_units: 4,
        source_requests_reviewed: true,
        expected_revision: d.leave_setup.dated.as_ref().unwrap().revision,
        reason: "Fictional reviewed opening source".into(),
        reference: "PRIVATE_FIXTURE_REFERENCE".into(),
    }
}
fn future_policy(d: &Snapshot) -> PolicyRecord {
    let c = d.leave_setup.dated.as_ref().unwrap();
    let mut p = c.timeline.policy_on(d.company.today()).clone();
    p.types[0].use_employee_allowance = false;
    p.types[0].annual_units = 20;
    p.types[0].carryover_units = 6;
    PolicyRecord {
        id: Uuid::new_v4(),
        effective_on: NaiveDate::from_ymd_opt(d.company.today().year() + 1, 1, 1).unwrap(),
        expected_revision: c.revision,
        policy: p,
        reason: "Fictional future entitlement version".into(),
        reference: "".into(),
    }
}
fn workday(mut date: NaiveDate) -> NaiveDate {
    while date.weekday().number_from_monday() > 5 {
        date += Duration::days(1);
    }
    date
}
#[test]
fn opening_takeover_is_not_a_second_grant_and_does_not_carry_imported_carry_twice() {
    let mut d = seed();
    let e = d.employees[0].clone();
    let r = opening(&d, e.id);
    let p = preview_demo(&d, &r).unwrap();
    assert_eq!(p.before.available_units, 42);
    assert_eq!(p.after.available_units, 12);
    apply_demo(&mut d, &Change::Opening(Box::new(p.clone()))).unwrap();
    let t = d.leave_setup.policy.leave_type("annual").unwrap();
    let year = d.company.today().year();
    assert_eq!(
        balance_checked(&d, &e, t, year, d.company.today(), None)
            .unwrap()
            .available_units,
        12
    );
    let next = NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap();
    assert_eq!(
        balance_checked(&d, &e, t, year + 1, next, None)
            .unwrap()
            .carryover_units,
        8
    );
    apply_demo(&mut d, &Change::Opening(Box::new(p))).unwrap();
    assert_eq!(d.leave_setup.dated.as_ref().unwrap().revision, 1);
}
#[test]
fn frozen_rate_and_incoming_carry_survive_employee_default_changes() {
    let mut d = seed();
    let person = d.employees[1].id;
    let r = future_policy(&d);
    apply_demo(&mut d, &Change::Schedule(r)).unwrap();
    d.employees[1].annual_leave_days = 1;
    d.employees[1].joined_on = Some(d.company.today());
    let t = d.leave_setup.policy.leave_type("annual").unwrap();
    let b = balance_checked(
        &d,
        &d.employees[1],
        t,
        d.company.today().year(),
        d.company.today(),
        None,
    )
    .unwrap();
    assert_eq!(
        (b.earned_units, b.carryover_units, b.available_units),
        (30, 12, 42)
    );
    let a = d
        .leave_setup
        .dated
        .as_ref()
        .unwrap()
        .anchors
        .iter()
        .find(|a| a.employee_id == person)
        .unwrap();
    assert_eq!(a.incoming_carry_units, 12);
}
#[test]
fn current_year_capture_cannot_be_used_as_invented_earlier_entitlement() {
    let mut d = seed();
    let p = future_policy(&d);
    apply_demo(&mut d, &Change::Schedule(p)).unwrap();
    let e = &d.employees[0];
    let t = d.leave_setup.policy.leave_type("annual").unwrap();
    let before = d.company.today().pred_opt().unwrap();
    assert!(balance_checked(&d, e, t, d.company.today().year(), before, None).is_err());
}
#[test]
fn later_same_day_credit_changes_the_imported_base_once_and_retry_remains_safe() {
    let mut d = seed();
    let person = d.employees[0].id;
    let p = preview_demo(&d, &opening(&d, person)).unwrap();
    let change = Change::Opening(Box::new(p));
    apply_demo(&mut d, &change).unwrap();
    let credit = leave::credits::Record {
        id: Uuid::new_v4(),
        employee_id: person,
        type_code: "annual".into(),
        effective_on: d.company.today(),
        units: 3,
        reverses_id: None,
        expected_revision: 0,
        reason: "Later same-day reviewed credit".into(),
        reference: "".into(),
    };
    leave::credits::apply_demo(&mut d, &credit).unwrap();
    apply_demo(&mut d, &change).unwrap();
    let e = d.employees.iter().find(|e| e.id == person).unwrap();
    let t = d.leave_setup.policy.leave_type("annual").unwrap();
    assert_eq!(
        balance_checked(&d, e, t, d.company.today().year(), d.company.today(), None)
            .unwrap()
            .available_units,
        15
    );
    assert_eq!(d.leave_setup.dated.as_ref().unwrap().openings.len(), 1);
}
#[test]
fn stale_opening_preview_cannot_post_and_invalid_draft_does_not_change_data() {
    let mut d = seed();
    let person = d.employees[0].id;
    let p = preview_demo(&d, &opening(&d, person)).unwrap();
    d.employees[0].annual_leave_days = 14;
    assert!(apply_demo(&mut d, &Change::Opening(Box::new(p))).is_err());
    assert!(d.leave_setup.dated.as_ref().unwrap().openings.is_empty());
    let mut r = opening(&d, person);
    r.remaining_carry_units = 13;
    assert!(r.validate(&d).is_err());
    r.remaining_carry_units = 0;
    r.source_requests_reviewed = false;
    assert!(r.validate(&d).is_err());
}
#[test]
fn policy_versions_use_future_grants_and_withdrawals_keep_the_original_snapshot() {
    let mut d = seed();
    let r = future_policy(&d);
    let id = r.id;
    apply_demo(&mut d, &Change::Schedule(r.clone())).unwrap();
    let e = &d.employees[0];
    let future = balance_checked(
        &d,
        e,
        &r.policy.types[0],
        r.effective_on.year(),
        r.effective_on,
        None,
    )
    .unwrap();
    assert_eq!(future.earned_units, 20);
    assert_eq!(future.carryover_units, 6);
    let undo = Change::Withdraw(WithdrawRecord {
        id: Uuid::new_v4(),
        schedule_id: id,
        expected_revision: 1,
        reason: "Fictional withdrawal before activation".into(),
        reference: "".into(),
    });
    apply_demo(&mut d, &undo).unwrap();
    apply_demo(&mut d, &undo).unwrap();
    let c = d.leave_setup.dated.as_ref().unwrap();
    assert!(c.timeline.versions[0].withdrawn);
    assert_eq!(c.withdrawals.len(), 1);
    assert_eq!(c.revision, 2);
    let mut invalid = r;
    invalid.id = Uuid::new_v4();
    invalid.expected_revision = 2;
    invalid.effective_on = d.company.today();
    assert!(Change::Schedule(invalid).payload(&d).is_err());
}
#[test]
fn active_filed_policy_reference_blocks_withdrawal_without_rewriting_dates() {
    let mut d = seed();
    let r = future_policy(&d);
    let id = r.id;
    apply_demo(&mut d, &Change::Schedule(r.clone())).unwrap();
    let date = workday(r.effective_on);
    let draft = LeaveDraft {
        employee_id: Some(d.employees[1].id),
        starts_on: date.to_string(),
        ends_on: date.to_string(),
        options: LeaveOptions {
            type_code: "annual".into(),
            day_part: DayPart::Full,
        },
        ..Default::default()
    };
    let row = prepare(&d, &draft).unwrap();
    let dates = row.policy.charged_dates.clone();
    d.leave.push(row);
    assert!(
        Change::Withdraw(WithdrawRecord {
            id: Uuid::new_v4(),
            schedule_id: id,
            expected_revision: 1,
            reason: "This still has an active reference".into(),
            reference: "".into()
        })
        .payload(&d)
        .is_err()
    );
    assert_eq!(d.leave[0].policy.charged_dates, dates);
}
#[test]
fn scoped_context_rejects_private_manager_audit_foreign_people_and_invalid_carry() {
    let mut d = seed();
    let person = d.employees[0].id;
    let p = preview_demo(&d, &opening(&d, person)).unwrap();
    apply_demo(&mut d, &Change::Opening(Box::new(p))).unwrap();
    let mut access = d.access.clone();
    access.role = Role::Manager;
    access.employee_id = Some(d.employees[1].id);
    access.direct_reports = vec![person];
    let mut c = d.leave_setup.dated.clone().unwrap();
    c.role = access.role;
    c.employee_id = access.employee_id;
    assert!(c.validate(&access, d.company.today()).is_err());
    c.audit.clear();
    c.validate(&access, d.company.today()).unwrap();
    c.anchors[0].incoming_carry_units = 733;
    assert!(c.validate(&access, d.company.today()).is_err());
    c.anchors[0].incoming_carry_units = 12;
    c.anchors[0].employee_id = d.employees[2].id;
    assert!(c.validate(&access, d.company.today()).is_err());
    assert!(
        Change::Schedule(future_policy(&d))
            .payload(&Snapshot {
                access,
                ..d.clone()
            })
            .is_err()
    );
}
#[test]
fn csv_collects_each_bad_row_and_flags_every_duplicate_employee_type() {
    for (text, units) in [("0", 0), ("2.00", 4), ("2.50", 5), ("366.0", 732)] {
        assert_eq!(leave::opening_import::quantity(text).unwrap(), units);
    }
    for text in ["-1", "1.25", "366.5", "NaN", "1e2"] {
        assert!(leave::opening_import::quantity(text).is_err());
    }
    let d = seed();
    let code = &d.employees[0].employee_code;
    let header = leave::opening_import::HEADERS.join(",");
    let source = format!(
        "{header}\r\n{code},annual,12,4,Reviewed opening,ref\r\nMISSING,annual,1.25,0,Short,ref\r\n{code},annual,6,0,Another opening,ref\r\n"
    );
    let p = Import::parse(&d, "fictional.csv", &source).unwrap();
    assert_eq!(p.rows.len(), 3);
    assert!(p.rows[0].errors.iter().any(|e| e.contains("Duplicate")));
    assert!(p.rows[2].errors.iter().any(|e| e.contains("Duplicate")));
    assert!(p.rows[1].errors.len() >= 2);
    assert!(p.records(&d, true).is_err());
    assert!(
        String::from_utf8(p.error_csv().unwrap())
            .unwrap()
            .contains("source_line")
    );
    assert!(Import::parse(&d, "bad.csv", "employee_code\nX").is_err());
}
#[test]
fn csv_import_is_atomic_and_idempotent_and_rechecks_employee_code_bindings() {
    let mut d = seed();
    let header = leave::opening_import::HEADERS.join(",");
    let text = format!(
        "{header}\n{},annual,12,4,Reviewed first opening,ref\n{},annual,8,0,Reviewed second opening,ref\n",
        d.employees[0].employee_code, d.employees[1].employee_code
    );
    let source = Import::parse(&d, "fictional.csv", &text).unwrap();
    assert!(source.records(&d, false).is_err());
    let records = source.records(&d, true).unwrap();
    let preview = BatchPreview::demo(&d, &records);
    preview.validate(&d, &records).unwrap();
    assert!(preview.ready());
    let change = Change::OpeningBatch(Box::new(Post {
        id: source.id,
        preview,
    }));
    apply_demo(&mut d, &change).unwrap();
    apply_demo(&mut d, &change).unwrap();
    assert_eq!(d.leave_setup.dated.as_ref().unwrap().openings.len(), 2);
    assert_eq!(d.leave_setup.dated.as_ref().unwrap().revision, 2);
    d.employees[0].employee_code = "CHANGED".into();
    assert!(source.records(&d, true).is_err());
}
#[test]
fn batch_with_a_late_invalid_opening_never_posts_its_valid_first_row() {
    let mut d = seed();
    let first = opening(&d, d.employees[0].id);
    let mut second = opening(&d, d.employees[1].id);
    second.remaining_carry_units = 15;
    let preview = BatchPreview::demo(&d, &[first, second]);
    assert!(!preview.ready());
    let change = Change::OpeningBatch(Box::new(Post {
        id: Uuid::new_v4(),
        preview,
    }));
    assert!(apply_demo(&mut d, &change).is_err());
    assert!(d.leave_setup.dated.as_ref().unwrap().openings.is_empty());
}
#[test]
fn dated_calendar_selects_each_charge_day_and_retains_request_start_plan() {
    let mut d = seed();
    let today = d.company.today();
    let effective = workday(today + Duration::days(3));
    let mut policy = d.leave_setup.policy.clone();
    policy.revision += 1;
    policy.holidays.push(Holiday {
        date: effective,
        name: "Fictional closure".into(),
    });
    policy.types[0].name = "Later caption".into();
    d.leave_setup
        .dated
        .as_mut()
        .unwrap()
        .timeline
        .versions
        .push(Scheduled {
            id: Uuid::new_v4(),
            effective_on: effective,
            policy,
            withdrawn: false,
        });
    let draft = LeaveDraft {
        employee_id: Some(d.employees[1].id),
        starts_on: today.to_string(),
        ends_on: effective.to_string(),
        options: LeaveOptions {
            type_code: "annual".into(),
            day_part: DayPart::Full,
        },
        ..Default::default()
    };
    let row = prepare(&d, &draft).unwrap();
    assert!(!row.policy.charged_dates.iter().any(|d| d.date == effective));
    assert_eq!(row.policy.type_name, "Annual leave");
    assert_eq!(row.policy.policy_revision, d.leave_setup.policy.revision);
    let roster = hris_desktop::attendance::daily_roster(&d, effective);
    assert!(roster.iter().all(|r| !r.scheduled_workday));
    let plan = hris_desktop::shifts::ShiftPlan {
        template_id: Uuid::new_v4(),
        template_revision: 0,
        name: "Fictional week".into(),
        days: vec![1, 2, 3, 4, 5, 6, 7],
        start_minute: 480,
        end_minute: 1020,
        works_on_holidays: false,
        timezone: d.company.timezone.clone(),
        break_rules: vec![],
    };
    assert!(!plan.working(&d, effective));
}

#[test]
fn future_type_credits_use_their_dated_policy_and_cannot_be_orphaned_by_withdrawal() {
    let mut d = seed();
    let mut record = future_policy(&d);
    record.policy.types.push(LeaveType {
        code: "future_finite".into(),
        name: "Fictional future finite".into(),
        annual_units: 10,
        ..Default::default()
    });
    let schedule = record.id;
    let effective = record.effective_on;
    apply_demo(&mut d, &Change::Schedule(record)).unwrap();
    let credit = leave::credits::Record {
        id: Uuid::new_v4(),
        employee_id: d.employees[0].id,
        type_code: "future_finite".into(),
        effective_on: effective,
        units: 2,
        reverses_id: None,
        expected_revision: 0,
        reason: "Fictional future dated credit".into(),
        reference: "".into(),
    };
    leave::credits::apply_demo(&mut d, &credit).unwrap();
    let t = leave::dated::policy_on(&d, effective)
        .leave_type("future_finite")
        .unwrap();
    assert_eq!(
        balance_checked(&d, &d.employees[0], t, effective.year(), effective, None)
            .unwrap()
            .available_units,
        12
    );
    let undo = Change::Withdraw(WithdrawRecord {
        id: Uuid::new_v4(),
        schedule_id: schedule,
        expected_revision: 1,
        reason: "Keep the dated credit obligation".into(),
        reference: "".into(),
    });
    assert!(undo.payload(&d).is_err());
    let reversal = leave::credits::Record {
        id: Uuid::new_v4(),
        units: -2,
        reverses_id: Some(credit.id),
        expected_revision: 1,
        reason: "Reverse future effect before withdrawal".into(),
        ..credit
    };
    leave::credits::apply_demo(&mut d, &reversal).unwrap();
    apply_demo(&mut d, &undo).unwrap();
    assert_eq!(d.leave_setup.credits.as_ref().unwrap().entries.len(), 2);
}
