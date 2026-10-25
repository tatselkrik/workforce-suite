use chrono::Duration;
use hris_desktop::{
    access::Role,
    attendance_imports::{self, Demo, Import, Post, Withdraw},
    domain::DemoStore,
    requests::{self, RequestAction, Status},
};
use uuid::Uuid;
fn seed() -> hris_desktop::domain::Snapshot {
    let mut d = DemoStore::new().data;
    d.attendance.clear();
    d.requests.records.clear();
    d.requests.events.clear();
    d.breaks.records.clear();
    d.shifts = Default::default();
    for e in &mut d.employees {
        e.joined_on = Some("2025-01-01".parse().unwrap());
    }
    d
}
fn source(d: &hris_desktop::domain::Snapshot) -> Import {
    let day = d.company.today() - Duration::days(1);
    let code = &d.employees[1].employee_code;
    let text = format!(
        "{}\n{code},{day},{day}T08:00:00+08:00,{day}T17:00:00+08:00,Fictional clock correction,PRIVATE_SOURCE_REFERENCE\n",
        attendance_imports::HEADERS.join(",")
    );
    let mut s = Import::from_csv(d, "Fictional attendance.csv", &text).unwrap();
    s.reason = "PRIVATE_IMPORT_NOTE".into();
    s.reference = "PRIVATE_BATCH_REFERENCE".into();
    s
}
fn prepared(d: &hris_desktop::domain::Snapshot) -> Post {
    let b = source(d).batch(d).unwrap();
    let p = Demo::preview(d, &b).unwrap();
    p.validate(d, &b).unwrap();
    Post {
        scope: d.access.clone(),
        batch: b,
        preview: p,
    }
}
#[test]
fn explicit_offsets_and_microseconds_are_preserved_without_wall_clock_guessing() {
    assert_eq!(
        attendance_imports::timestamp("2026-10-25T08:30:00.123456+08:00")
            .unwrap()
            .to_rfc3339(),
        "2026-10-25T00:30:00.123456+00:00"
    );
    for bad in [
        "2026-10-25 08:30",
        "2026-10-25T08:30:00",
        "2026-10-25T08:30:00-00:00",
        "2026-10-25T08:30:00.1234567Z",
        "2026-02-30T08:30:00Z",
        "2026-10-25T08:30:60Z",
        "2026-10-25T08:30:00+15:00",
    ] {
        assert!(attendance_imports::timestamp(bad).is_err(), "{bad}");
    }
}
#[test]
fn preview_is_stable_and_pure_and_post_creates_only_pending_requests() {
    let mut d = seed();
    let post = prepared(&d);
    assert_eq!(
        post.preview.fingerprint,
        Demo::preview(&d, &post.batch).unwrap().fingerprint
    );
    assert!(d.requests.records.is_empty());
    assert!(d.attendance.is_empty());
    let mut demo = Demo::default();
    let r = demo.post(&mut d, &post).unwrap();
    r.validate(&d, &post).unwrap();
    assert_eq!(r.items.len(), 1);
    assert!(d.attendance.is_empty());
    assert!(d.corrections.is_empty());
    assert_eq!(d.requests.records[0].status, Status::Pending);
    assert_eq!(d.requests.records[0].submitted_by, d.access.user_id);
    assert_eq!(d.requests.records[0].employee_id, d.employees[1].id);
    let retry = demo.post(&mut d, &post).unwrap();
    assert_eq!(r.items, retry.items);
    assert_eq!(d.requests.records.len(), 1);
}
#[test]
fn source_changes_and_different_actor_replays_are_refused_without_partial_writes() {
    let mut d = seed();
    let post = prepared(&d);
    let mut demo = Demo::default();
    d.employees[1].full_name.push_str(" changed");
    assert!(demo.post(&mut d, &post).is_err());
    assert!(d.requests.records.is_empty());
    assert!(demo.history(&d).unwrap().batches.is_empty());
    let post = prepared(&d);
    demo.post(&mut d, &post).unwrap();
    d.access.user_id = Uuid::new_v4();
    assert!(demo.post(&mut d, &post).is_err());
    assert_eq!(d.requests.records.len(), 1);
}
#[test]
fn normal_independent_review_and_calculation_history_apply_the_imported_proposal() {
    let mut d = seed();
    let post = prepared(&d);
    let mut demo = Demo::default();
    let r = demo.post(&mut d, &post).unwrap();
    let rid = r.items[0].request_id;
    let approve = RequestAction::Resolve {
        id: rid,
        expected_revision: 0,
        status: Status::Approved,
        reason: "Independent fictional source review".into(),
    };
    assert!(requests::apply_demo(&mut d, approve.clone()).is_err());
    d.access.role = Role::Manager;
    d.access.user_id = Uuid::new_v4();
    d.access.employee_id = Some(d.employees[2].id);
    d.access.direct_reports = vec![d.employees[1].id];
    requests::apply_demo(&mut d, approve).unwrap();
    assert_eq!(d.attendance.len(), 1);
    assert_eq!(d.corrections.len(), 1);
    assert_eq!(d.requests.records[0].status, Status::Approved);
    assert!(demo.history(&d).is_err());
}
#[test]
fn withdrawal_is_original_actor_only_pending_and_idempotent() {
    let mut d = seed();
    let post = prepared(&d);
    let mut demo = Demo::default();
    let receipt = demo.post(&mut d, &post).unwrap();
    let change = Withdraw {
        id: Uuid::new_v4(),
        item_id: receipt.items[0].id,
        expected_revision: 0,
        reason: "Fictional source corrected before review".into(),
    };
    let actor = d.access.user_id;
    d.access.user_id = Uuid::new_v4();
    assert!(demo.withdraw(&mut d, &change).is_err());
    d.access.user_id = actor;
    demo.withdraw(&mut d, &change).unwrap();
    demo.withdraw(&mut d, &change).unwrap();
    assert_eq!(d.requests.records[0].status, Status::Cancelled);
    assert_eq!(demo.history(&d).unwrap().withdrawals.len(), 1);
    assert!(d.attendance.is_empty());
    d.access.role = Role::Manager;
    assert!(demo.withdraw(&mut d, &change).is_err());
}
#[test]
fn duplicate_days_overlapping_intervals_bad_rows_and_non_hr_are_blocked() {
    let mut d = seed();
    let s = source(&d);
    let good = s.rows[0].cells.join(",");
    let text = format!(
        "{}\n{good}\n{good}\n",
        attendance_imports::HEADERS.join(",")
    );
    let mut invalid = Import::from_csv(&d, "Duplicates.csv", &text).unwrap();
    invalid.reason = "Fictional check".into();
    assert!(invalid.rows.iter().all(|r| !r.errors.is_empty()));
    assert!(invalid.batch(&d).is_err());
    let mut b = s.batch(&d).unwrap();
    let mut other = b.rows[0].clone();
    other.id = Uuid::new_v4();
    other.source_row = 3;
    other.work_date -= Duration::days(1);
    other.checked_in_at -= Duration::days(1);
    other.checked_out_at = None;
    b.rows.push(other);
    assert!(b.validate(&d).is_err());
    for role in [Role::Manager, Role::Employee] {
        d.access.role = role;
        assert!(Import::from_csv(&d, "Denied.csv", &text).is_err());
    }
}
#[test]
fn source_row_numbers_and_error_exports_preserve_literal_source_values() {
    let d = seed();
    let day = d.company.today() - Duration::days(1);
    let text = format!(
        "{}\n\n=UNKNOWN,{day},{day}T08:00:00Z,,Fictional invalid employee,+REFERENCE\n",
        attendance_imports::HEADERS.join(",")
    );
    let s = Import::from_csv(&d, "Errors.csv", &text).unwrap();
    assert_eq!(s.rows[0].number, 3);
    assert!(!s.valid_rows());
    let csv = String::from_utf8(
        s.error_bytes(std::path::Path::new("errors.csv"), None)
            .unwrap(),
    )
    .unwrap();
    assert!(csv.contains("'=UNKNOWN"));
    assert!(csv.contains("'+REFERENCE"));
    let bytes = s
        .error_bytes(std::path::Path::new("errors.xlsx"), None)
        .unwrap();
    let mut x = calamine::Xlsx::new(std::io::Cursor::new(bytes)).unwrap();
    use calamine::Reader;
    let rows = x.worksheet_range("HRIS data").unwrap();
    assert_eq!(
        rows.get_value((1, 1)),
        Some(&calamine::Data::String("=UNKNOWN".into()))
    );
}
#[test]
fn forged_preview_or_receipt_scopes_and_mismatched_rows_are_rejected() {
    let d = seed();
    let post = prepared(&d);
    for kind in 0..5 {
        let mut p = post.preview.clone();
        match kind {
            0 => p.company_id = Uuid::new_v4(),
            1 => p.user_id = Uuid::new_v4(),
            2 => p.rows[0].employee_id = Some(d.employees[0].id),
            3 => p.rows[0].source_row += 1,
            _ => p.ready = false,
        }
        assert!(p.validate(&d, &post.batch).is_err());
    }
    let mut batch = post.batch.clone();
    let mut second = batch.rows[0].clone();
    second.id = Uuid::new_v4();
    second.source_row = 3;
    second.employee_code = d.employees[2].employee_code.clone();
    batch.rows.push(second);
    let preview = Demo::preview(&d, &batch).unwrap();
    let post = Post {
        scope: d.access.clone(),
        batch,
        preview,
    };
    let mut data = d.clone();
    let mut demo = Demo::default();
    let mut receipt = demo.post(&mut data, &post).unwrap();
    receipt.items[1] = receipt.items[0].clone();
    receipt.items[1].request_id = Uuid::new_v4();
    assert!(
        receipt.validate(&data, &post).is_err(),
        "A repeated row cannot replace another protected subject"
    );
}
#[test]
fn explicit_dst_fold_instants_are_not_rejected_as_ambiguous_wall_times() {
    let mut d = seed();
    d.company.timezone = "America/New_York".into();
    let code = &d.employees[1].employee_code;
    let text = format!(
        "{}\n{code},2025-11-02,2025-11-02T01:30:00-04:00,2025-11-02T02:30:00-05:00,Fictional DST source,\n",
        attendance_imports::HEADERS.join(",")
    );
    let s = Import::from_csv(&d, "DST.csv", &text).unwrap();
    assert!(s.valid_rows(), "{:?}", s.rows[0].errors);
    let r = s.rows[0].record.as_ref().unwrap();
    assert_eq!((r.checked_out_at.unwrap() - r.checked_in_at).num_hours(), 2);
}

#[test]
fn committed_retry_survives_review_and_source_changes_but_not_scope_changes() {
    let mut d = seed();
    let original_access = d.access.clone();
    let post = prepared(&d);
    let mut demo = Demo::default();
    let receipt = demo.post(&mut d, &post).unwrap();
    d.access.user_id = Uuid::new_v4();
    d.access.employee_id = Some(d.employees[0].id);
    requests::apply_demo(
        &mut d,
        RequestAction::Resolve {
            id: receipt.items[0].request_id,
            expected_revision: 0,
            status: Status::Approved,
            reason: "Fictional independent review".into(),
        },
    )
    .unwrap();
    d.access = original_access;
    d.employees[1].full_name.push_str(" current name");
    post.identity(&d.access).unwrap();
    assert_eq!(demo.post(&mut d, &post).unwrap().items, receipt.items);
    assert_eq!(d.attendance.len(), 1);
    d.company.id = Uuid::new_v4();
    d.access.company_id = d.company.id;
    assert!(demo.post(&mut d, &post).is_err());
}

#[test]
fn excel_date_only_cells_load_and_numeric_clock_cells_are_refused() {
    use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};
    let d = seed();
    let day = d.company.today() - Duration::days(1);
    let folder = std::env::current_dir()
        .unwrap()
        .join("work/imports051-native")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("Fictional attendance.xlsx");
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    for (i, h) in attendance_imports::HEADERS.iter().enumerate() {
        sheet.write_string(0, i as u16, *h).unwrap();
    }
    sheet
        .write_string(1, 0, &d.employees[1].employee_code)
        .unwrap();
    sheet
        .write_datetime_with_format(
            1,
            1,
            &ExcelDateTime::parse_from_str(&day.to_string()).unwrap(),
            &Format::new().set_num_format("yyyy-mm-dd"),
        )
        .unwrap();
    sheet
        .write_string(1, 2, format!("{day}T08:00:00+08:00"))
        .unwrap();
    sheet
        .write_string(1, 3, format!("{day}T17:00:00+08:00"))
        .unwrap();
    sheet.write_string(1, 4, "Fictional source review").unwrap();
    book.save(&path).unwrap();
    let loaded = Import::load(&d, &path).unwrap();
    assert!(loaded.valid_rows(), "{:?}", loaded.rows[0].errors);
    assert_eq!(loaded.rows[0].record.as_ref().unwrap().work_date, day);
    book.worksheet_from_index(0)
        .unwrap()
        .write_number(1, 2, 0.333333)
        .unwrap();
    book.save(&path).unwrap();
    assert!(Import::load(&d, &path).is_err());
}

#[test]
fn history_rejects_foreign_rows_duplicate_bindings_and_forged_withdrawals() {
    let mut d = seed();
    let post = prepared(&d);
    let mut demo = Demo::default();
    let receipt = demo.post(&mut d, &post).unwrap();
    let change = Withdraw {
        id: Uuid::new_v4(),
        item_id: receipt.items[0].id,
        expected_revision: 0,
        reason: "Fictional withdrawal".into(),
    };
    demo.withdraw(&mut d, &change).unwrap();
    let history = demo.history(&d).unwrap();
    for kind in 0..5 {
        let mut h = history.clone();
        match kind {
            0 => h.items[0].company_id = Uuid::new_v4(),
            1 => h.withdrawals[0].actor_id = Uuid::new_v4(),
            2 => h.withdrawals[0].id = Uuid::nil(),
            3 => h.items[0].status = Status::Pending,
            _ => h.items[0].proposed_out = Some(h.items[0].proposed_in - Duration::seconds(1)),
        }
        assert!(h.validate(&d).is_err(), "kind {kind}");
    }
}
