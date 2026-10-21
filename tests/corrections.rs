use chrono::{Duration, NaiveDate, TimeZone, Utc};
use hris_desktop::{access::Role, corrections::CorrectionDraft, domain::*};

fn fixture() -> (DemoStore, CorrectionDraft, chrono::DateTime<Utc>) {
    let mut store = DemoStore::new();
    store.data.attendance.clear();
    let now = Utc.with_ymd_and_hms(2026, 10, 21, 12, 0, 0).unwrap();
    let person = &mut store.data.employees[0];
    person.joined_on = None;
    let mut draft = CorrectionDraft::new(
        person.id,
        NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(),
        None,
        &store.data,
    );
    draft.checked_in = "08:30".into();
    draft.checked_out = "17:30".into();
    draft.reason = "Missed check-in confirmed from shift log".into();
    (store, draft, now)
}

#[test]
fn past_entry_and_edit_keep_reason_actor_and_both_versions() {
    let (mut store, draft, now) = fixture();
    let change = draft.change(&store.data, now).unwrap();
    let id = change.record.id;
    store.execute(Action::CorrectAttendance(change)).unwrap();
    let original = store.data.attendance[0].clone();
    let mut edit = CorrectionDraft::new(
        original.employee_id,
        original.work_date,
        Some(original.clone()),
        &store.data,
    );
    edit.checked_in = "08:45".into();
    edit.reason = "Arrival verified by HR".into();
    store
        .execute(Action::CorrectAttendance(
            edit.change(&store.data, now).unwrap(),
        ))
        .unwrap();
    assert_eq!(store.data.attendance.len(), 1);
    assert_eq!(store.data.attendance[0].id, id);
    assert_eq!(store.data.attendance[0].revision, 1);
    assert_eq!(store.data.corrections.len(), 2);
    assert!(store.data.corrections[0].before_record.is_none());
    assert_eq!(
        store.data.corrections[1].before_record.as_ref(),
        Some(&original)
    );
    assert_eq!(
        store.data.corrections[1].after_record,
        store.data.attendance[0]
    );
    assert_eq!(store.data.corrections[1].reason, "Arrival verified by HR");
    assert_eq!(
        store.data.corrections[1].actor_id,
        store.data.access.user_id
    );
}

#[test]
fn missing_long_or_control_character_reason_never_changes_attendance() {
    let (store, mut draft, now) = fixture();
    for reason in [
        String::new(),
        " ".into(),
        "a".repeat(501),
        "line\nbreak".into(),
    ] {
        draft.reason = reason;
        assert!(draft.change(&store.data, now).is_err());
    }
    assert!(store.data.attendance.is_empty() && store.data.corrections.is_empty());
}

#[test]
fn manager_and_employee_cannot_correct_even_their_own_attendance() {
    let (store, draft, now) = fixture();
    for role in [
        Role::Administrator,
        Role::HrGeneralist,
        Role::Manager,
        Role::Employee,
    ] {
        let mut data = store.data.clone();
        data.access.role = role;
        data.access.employee_id = Some(draft.employee_id);
        assert_eq!(draft.change(&data, now).is_ok(), role.manages_hr());
    }
}

#[test]
fn overnight_times_follow_company_timezone_and_reject_future_or_reversed_times() {
    let (store, mut draft, now) = fixture();
    draft.checked_in = "22:00".into();
    draft.checked_out = "06:00".into();
    draft.checked_out_date = (draft.work_date + Duration::days(1)).to_string();
    let row = draft.change(&store.data, now).unwrap().record;
    assert_eq!(
        (row.checked_out_at.unwrap() - row.checked_in_at).num_hours(),
        8
    );
    assert_eq!(row.checked_in_at.hour(), 14);
    draft.checked_out_date = draft.work_date.to_string();
    assert!(draft.change(&store.data, now).is_err());
    draft.checked_out.clear();
    draft.work_date += Duration::days(4);
    assert!(draft.change(&store.data, now).is_err());
}

use chrono::Timelike;

#[test]
fn stale_draft_after_live_checkout_cannot_overwrite_or_append_history() {
    let (mut store, mut draft, now) = fixture();
    draft.checked_out.clear();
    store
        .execute(Action::CorrectAttendance(
            draft.change(&store.data, now).unwrap(),
        ))
        .unwrap();
    let row = store.data.attendance[0].clone();
    let mut stale = CorrectionDraft::new(
        row.employee_id,
        row.work_date,
        Some(row.clone()),
        &store.data,
    );
    stale.checked_out = "17:00".into();
    stale.reason = "Old draft".into();
    let pending = stale.change(&store.data, now).unwrap();
    store.execute(Action::CheckOut(row.id)).unwrap();
    assert!(
        store
            .execute(Action::CorrectAttendance(pending))
            .unwrap_err()
            .to_string()
            .contains("Someone changed")
    );
    assert_eq!(store.data.corrections.len(), 1);
    assert!(store.data.attendance[0].checked_out_at.is_some());
}

#[test]
fn duplicates_overlaps_and_pre_hire_dates_are_denied() {
    let (mut store, draft, now) = fixture();
    let mut original = draft.change(&store.data, now).unwrap().record;
    original.checked_out_at = Some(original.checked_in_at + Duration::hours(26));
    store.data.attendance.push(original);
    assert!(draft.change(&store.data, now).is_err());
    let mut next = draft.clone();
    next.work_date += Duration::days(1);
    next.checked_out_date = next.work_date.to_string();
    assert!(
        next.change(&store.data, now)
            .unwrap_err()
            .to_string()
            .contains("overlap")
    );
    store.data.attendance.clear();
    store.data.employees[0].joined_on = Some(draft.work_date + Duration::days(1));
    assert!(
        draft
            .change(&store.data, now)
            .unwrap_err()
            .to_string()
            .contains("hire date")
    );
}

#[test]
fn archived_employee_history_is_retained_and_correctable_only_before_today() {
    let (mut store, mut draft, now) = fixture();
    store.data.employees[0].status = EmployeeStatus::Archived;
    assert!(draft.change(&store.data, now).is_ok());
    draft.work_date = now.with_timezone(&store.data.company.tz()).date_naive();
    draft.checked_out_date = draft.work_date.to_string();
    assert!(
        draft
            .change(&store.data, now)
            .unwrap_err()
            .to_string()
            .contains("historical")
    );
    assert!(store.execute(Action::CheckIn(draft.employee_id)).is_err());
}

#[test]
fn fractional_seconds_round_trip_and_dst_ambiguities_are_not_silently_changed() {
    let (mut store, mut draft, now) = fixture();
    draft.checked_in = "08:30:15.123456".into();
    let original = draft.change(&store.data, now).unwrap().record;
    store.data.attendance.push(original.clone());
    let mut edit = CorrectionDraft::new(
        original.employee_id,
        original.work_date,
        Some(original.clone()),
        &store.data,
    );
    edit.checked_out = "18:00".into();
    edit.reason = "Only departure corrected".into();
    assert_eq!(
        edit.change(&store.data, now).unwrap().record.checked_in_at,
        original.checked_in_at
    );
    store.data.company.timezone = "America/New_York".into();
    store.data.attendance.clear();
    draft.checked_out.clear();
    for (day, time) in [("2026-03-08", "02:30"), ("2025-11-02", "01:30")] {
        draft.work_date = parse_date(day).unwrap();
        draft.checked_in = time.into();
        assert!(draft.change(&store.data, now).is_err());
    }
}
