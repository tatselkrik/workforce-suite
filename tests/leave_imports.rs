use chrono::{Datelike, Duration, NaiveDate};
use hris_desktop::{
    access::Role,
    domain::*,
    leave::{
        self, DayPart,
        request_import::{self, Demo, Import, Post},
    },
};
use uuid::Uuid;
fn seed() -> Snapshot {
    let mut d = DemoStore::new().data;
    d.leave.clear();
    d.leave_setup.events.clear();
    d.access.user_id = Uuid::new_v4();
    for e in &mut d.employees {
        e.joined_on = Some(NaiveDate::from_ymd_opt(d.company.today().year() - 1, 1, 1).unwrap());
        e.annual_leave_days = 2;
        e.status = EmployeeStatus::Active;
    }
    d.leave_setup.policy.tracking_starts_year = d.company.today().year();
    d.leave_setup.policy.holidays.clear();
    d
}
fn days(d: &Snapshot) -> Vec<NaiveDate> {
    let mut dates = Vec::new();
    let mut date = d.company.today() + Duration::days(1);
    while dates.len() < 4 {
        if leave::dated::policy_on(d, date).working(date) {
            dates.push(date);
        }
        date += Duration::days(1);
    }
    dates
}
fn source(d: &Snapshot, parts: &[(usize, usize, &str)]) -> Import {
    let dates = days(d);
    let mut text = request_import::HEADERS.join(",") + "\n";
    for (person, day, part) in parts {
        let code = &d.employees[*person].employee_code;
        let date = dates[*day];
        text += &format!(
            "{code},annual,{date},{date},{part},Fictional reviewed source,+PRIVATE_REFERENCE\n"
        );
    }
    let mut s = Import::from_csv(d, "Fictional leave.csv", &text).unwrap();
    s.reason = "PRIVATE_IMPORT_NOTE".into();
    s.reference = "PRIVATE_BATCH_REFERENCE".into();
    s
}
fn post(d: &Snapshot, s: &Import) -> Post {
    let b = s.batch(d).unwrap();
    let p = Demo::preview(d, &b).unwrap();
    p.validate(d, &b).unwrap();
    Post {
        scope: d.access.clone(),
        batch: b,
        preview: p,
    }
}
#[test]
fn halves_use_running_reservations_and_preview_has_no_side_effects() {
    let mut d = seed();
    let s = source(&d, &[(1, 0, "am"), (1, 0, "pm")]);
    assert!(s.valid_rows());
    let p = post(&d, &s);
    assert_eq!(
        p.preview
            .rows
            .iter()
            .map(|r| r.balance_before.as_ref().unwrap().available_units)
            .collect::<Vec<_>>(),
        vec![4, 3]
    );
    assert!(d.leave.is_empty() && d.leave_setup.events.is_empty());
    assert_eq!(
        p.preview.fingerprint,
        Demo::preview(&d, &p.batch).unwrap().fingerprint
    );
    let mut demo = Demo::default();
    let r = demo.post(&mut d, &p).unwrap();
    r.validate(&d, &p).unwrap();
    assert_eq!(r.items.len(), 2);
    assert!(d.leave.iter().all(|r| r.status == LeaveStatus::Pending));
    assert_eq!(d.leave_setup.events.len(), 2);
    assert_eq!(demo.post(&mut d, &p).unwrap().items, r.items);
    assert_eq!(d.leave.len(), 2);
}
#[test]
fn overbooking_and_duplicate_halves_block_the_entire_local_batch() {
    let d = seed();
    for parts in [
        vec![(1, 0, "full"), (1, 1, "full"), (1, 2, "full")],
        vec![(1, 0, "am"), (1, 0, "am")],
    ] {
        let s = source(&d, &parts);
        assert!(!s.valid_rows());
        assert!(!s.rows.last().unwrap().errors.is_empty());
        assert!(s.batch(&d).is_err());
        assert!(d.leave.is_empty());
    }
}
#[test]
fn stale_sources_actor_changes_and_forged_receipts_are_refused() {
    let mut d = seed();
    let s = source(&d, &[(1, 0, "full"), (2, 0, "full")]);
    let p = post(&d, &s);
    let mut demo = Demo::default();
    d.employees[1].department.push_str(" changed");
    assert!(demo.post(&mut d, &p).is_err());
    assert!(demo.history(&d).unwrap().batches.is_empty());
    let p = post(&d, &source(&d, &[(1, 0, "full"), (2, 0, "full")]));
    let mut receipt = demo.post(&mut d, &p).unwrap();
    receipt.items[1] = receipt.items[0].clone();
    receipt.items[1].request_id = Uuid::new_v4();
    assert!(receipt.validate(&d, &p).is_err());
    d.access.user_id = Uuid::new_v4();
    assert!(demo.post(&mut d, &p).is_err());
    for role in [Role::Manager, Role::Employee] {
        d.access.role = role;
        assert!(Import::from_csv(&d, "Denied.csv", &request_import::HEADERS.join(",")).is_err());
        assert!(demo.history(&d).is_err());
    }
}
#[test]
fn normal_review_and_cancellation_preserve_import_history_and_exact_retry() {
    let mut d = seed();
    let p = post(&d, &source(&d, &[(1, 0, "am"), (1, 0, "pm")]));
    let mut demo = Demo::default();
    let receipt = demo.post(&mut d, &p).unwrap();
    let original = d.access.clone();
    d.access.role = Role::Manager;
    d.access.user_id = Uuid::new_v4();
    d.access.employee_id = Some(d.employees[2].id);
    d.access.direct_reports = vec![d.employees[1].id];
    leave::apply_demo(
        &mut d,
        Action::ReviewLeave(
            receipt.items[0].request_id,
            0,
            true,
            "Fictional manager review".into(),
        ),
    )
    .unwrap();
    d.access = original;
    leave::apply_demo(
        &mut d,
        Action::CancelLeave(
            receipt.items[1].request_id,
            0,
            "Fictional source cancellation".into(),
        ),
    )
    .unwrap();
    let h = demo.history(&d).unwrap();
    assert_eq!(h.items[0].status, LeaveStatus::Approved);
    assert_eq!(h.items[1].status, LeaveStatus::Cancelled);
    assert_eq!(demo.post(&mut d, &p).unwrap().items, receipt.items);
}
#[test]
fn row_errors_export_as_literal_text_and_preserve_physical_row_numbers() {
    let d = seed();
    let date = days(&d)[0];
    let text = format!(
        "{}\n\n=UNKNOWN,annual,{date},{date},full,Fictional invalid source,+REFERENCE\n",
        request_import::HEADERS.join(",")
    );
    let s = Import::from_csv(&d, "Errors.csv", &text).unwrap();
    assert_eq!(s.rows[0].number, 3);
    assert!(!s.valid_rows());
    let csv = String::from_utf8(
        s.error_bytes(std::path::Path::new("errors.csv"), None)
            .unwrap(),
    )
    .unwrap();
    assert!(csv.contains("'=UNKNOWN") && csv.contains("'+REFERENCE"));
    use calamine::Reader;
    let mut x = calamine::Xlsx::new(std::io::Cursor::new(
        s.error_bytes(std::path::Path::new("errors.xlsx"), None)
            .unwrap(),
    ))
    .unwrap();
    assert_eq!(
        x.worksheet_range("HRIS data").unwrap().get_value((1, 1)),
        Some(&calamine::Data::String("=UNKNOWN".into()))
    );
}
#[test]
fn excel_native_start_and_end_dates_reuse_the_same_validation() {
    use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};
    let d = seed();
    let date = days(&d)[0];
    let folder = std::env::current_dir()
        .unwrap()
        .join("work/imports052-native")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("Fictional leave.xlsx");
    let mut b = Workbook::new();
    let sheet = b.add_worksheet();
    for (i, h) in request_import::HEADERS.iter().enumerate() {
        sheet.write_string(0, i as u16, *h).unwrap();
    }
    sheet
        .write_string(1, 0, &d.employees[1].employee_code)
        .unwrap();
    sheet.write_string(1, 1, "annual").unwrap();
    let value = ExcelDateTime::parse_from_str(&date.to_string()).unwrap();
    let style = Format::new().set_num_format("yyyy-mm-dd");
    for col in [2, 3] {
        sheet
            .write_datetime_with_format(1, col, &value, &style)
            .unwrap();
    }
    sheet.write_string(1, 4, "am").unwrap();
    sheet
        .write_string(1, 5, "Fictional reviewed source")
        .unwrap();
    b.save(&path).unwrap();
    let s = Import::load(&d, &path).unwrap();
    assert!(s.valid_rows(), "{:?}", s.rows[0].errors);
    assert_eq!(s.rows[0].record.as_ref().unwrap().day_part, DayPart::Am);
    assert_eq!(s.rows[0].record.as_ref().unwrap().starts_on, date);
}
#[test]
fn captured_opening_balances_use_the_normal_dated_engine() {
    use leave::dated::*;
    let mut d = seed();
    d.leave_setup.credits = Some(leave::credits::Context::empty(&d));
    d.leave_setup.dated = Some(Context::empty(&d));
    let record = OpeningRecord {
        id: Uuid::new_v4(),
        employee_id: d.employees[1].id,
        type_code: "annual".into(),
        effective_on: d.company.today(),
        target_available_units: 3,
        remaining_carry_units: 0,
        source_requests_reviewed: true,
        expected_revision: 0,
        reason: "Fictional reviewed opening".into(),
        reference: String::new(),
    };
    let p = preview_demo(&d, &record).unwrap();
    apply_demo(&mut d, &Change::Opening(Box::new(p))).unwrap();
    let s = source(&d, &[(1, 0, "am"), (1, 0, "pm"), (1, 1, "am")]);
    let p = post(&d, &s);
    assert_eq!(
        p.preview
            .rows
            .iter()
            .map(|r| r.balance_before.as_ref().unwrap().available_units)
            .collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    let mut demo = Demo::default();
    demo.post(&mut d, &p).unwrap();
    assert_eq!(demo.history(&d).unwrap().items.len(), 3);
}
