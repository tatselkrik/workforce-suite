use chrono::NaiveDate;
use hris_desktop::leave::openings::*;
use uuid::Uuid;
fn day(s: &str) -> NaiveDate {
    s.parse().unwrap()
}
fn booking(id: u128, units: i32, early: i32, approved: bool) -> Booking {
    Booking {
        request_id: Uuid::from_u128(id),
        units,
        early_units: early,
        approved,
    }
}
fn seed<'a>(rows: &'a [Booking]) -> Capture<'a> {
    Capture {
        id: Uuid::from_u128(4901),
        employee_id: Uuid::from_u128(4902),
        type_code: "annual",
        effective_on: day("2027-02-01"),
        carry_expires_on: day("2027-03-31"),
        joined_on: Some(day("2025-01-01")),
        base_earned_units: 30,
        legacy_gross_carry_units: 4,
        carry_cap_units: 12,
        target_available_units: 16,
        remaining_carry_units: 4,
        source_outstanding_requests_reviewed: true,
        bookings: rows,
    }
}
#[test]
fn opening_is_a_takeover_not_another_annual_grant() {
    let rows = [booking(1, 6, 6, false), booking(2, 2, 0, true)];
    let o = Opening::capture(seed(&rows)).unwrap();
    let p = o.project(o.effective_on, 30, &rows).unwrap();
    assert_eq!(p.balance.available_units, 16);
    assert_eq!(p.balance.used_units, 2);
    assert_eq!(p.balance.reserved_units, 6);
    assert_eq!(p.unused_base_units, 12);
    assert_eq!(p.unspent_carry_units, 4);
    assert_eq!(
        o.project(day("2027-04-01"), 30, &rows)
            .unwrap()
            .balance
            .available_units,
        12
    );
}
#[test]
fn imported_carry_never_refunds_old_base_as_permanent_entitlement() {
    let rows = [booking(1, 6, 6, false)];
    let mut s = seed(&rows);
    s.legacy_gross_carry_units = 2;
    let o = Opening::capture(s).unwrap();
    let p = o.project(day("2027-04-01"), 30, &rows).unwrap();
    assert_eq!(p.unused_base_units, 12);
    assert_eq!(
        p.balance.available_units, 12,
        "unspent imported carry expires instead of funding an old base charge"
    );
    let cancelled = o.project(day("2027-04-01"), 30, &[]).unwrap();
    assert_eq!(
        cancelled.balance.available_units, 16,
        "only the4original base units return after expiry;2carry units stay expired"
    );
    assert_eq!(cancelled.unspent_carry_units, 0);
}
#[test]
fn cancellation_restores_the_reviewed_carry_component_before_expiry() {
    let rows = [booking(1, 6, 6, false)];
    let mut s = seed(&rows);
    s.legacy_gross_carry_units = 2;
    let o = Opening::capture(s).unwrap();
    let p = o.project(day("2027-03-01"), 30, &[]).unwrap();
    assert_eq!(p.balance.available_units, 22);
    assert_eq!(p.unused_base_units, 16);
    assert_eq!(p.unspent_carry_units, 6);
    let new = [booking(2, 5, 5, false)];
    let p = o.project(day("2027-03-01"), 30, &new).unwrap();
    assert_eq!(p.balance.available_units, 17);
    assert_eq!(p.unused_base_units, 16);
    assert_eq!(p.unspent_carry_units, 1);
}
#[test]
fn new_bookings_use_remaining_carry_and_accrual_changes_base_once() {
    let old = [booking(1, 6, 6, false)];
    let o = Opening::capture(seed(&old)).unwrap();
    let now = [
        old[0].clone(),
        booking(2, 3, 3, false),
        booking(3, 2, 0, true),
    ];
    let p = o.project(day("2027-03-01"), 34, &now).unwrap();
    assert_eq!(p.balance.available_units, 15);
    assert_eq!(p.unspent_carry_units, 1);
    assert_eq!(p.unused_base_units, 14);
    assert_eq!(p.balance.used_units, 2);
    assert_eq!(p.balance.reserved_units, 9);
    let end = o.project(day("2027-12-31"), 34, &now).unwrap();
    assert_eq!(end.unused_base_units, 14);
    assert_eq!(
        end.unspent_carry_units, 0,
        "next-year carry must use unused_base, not expiring carry"
    );
}
#[test]
fn approval_moves_reserved_to_used_without_changing_available() {
    let old = [booking(1, 3, 3, false)];
    let o = Opening::capture(seed(&old)).unwrap();
    let mut now = old.clone();
    now[0].approved = true;
    let before = o.project(o.effective_on, 30, &old).unwrap();
    let after = o.project(o.effective_on, 30, &now).unwrap();
    assert_eq!(
        before.balance.available_units,
        after.balance.available_units
    );
    assert_eq!(after.balance.reserved_units, 0);
    assert_eq!(after.balance.used_units, 3);
}
#[test]
fn incomplete_or_ambiguous_source_basis_is_rejected() {
    let rows = [booking(1, 4, 4, false)];
    let mut s = seed(&rows);
    s.source_outstanding_requests_reviewed = false;
    assert!(Opening::capture(s).is_err());
    let mut s = seed(&rows);
    s.joined_on = None;
    assert!(Opening::capture(s).is_err());
    let mut s = seed(&rows);
    s.joined_on = Some(day("2027-01-01"));
    assert!(Opening::capture(s).is_err());
    let mut s = seed(&rows);
    s.remaining_carry_units = 20;
    assert!(Opening::capture(s).is_err());
    let mut s = seed(&rows);
    s.carry_cap_units = 5;
    assert!(Opening::capture(s).is_err());
    let mut s = seed(&rows);
    s.effective_on = day("2027-04-01");
    assert!(Opening::capture(s).is_err());
}
#[test]
fn zero_carry_opening_supports_unknown_hire_and_postexpiry_takeover() {
    let rows = [booking(1, 4, 4, false)];
    let mut s = seed(&rows);
    s.remaining_carry_units = 0;
    s.legacy_gross_carry_units = 0;
    s.joined_on = None;
    s.effective_on = day("2027-04-01");
    let o = Opening::capture(s).unwrap();
    assert_eq!(
        o.project(o.effective_on, 30, &rows)
            .unwrap()
            .balance
            .available_units,
        16
    );
}
#[test]
fn changed_or_duplicate_saved_charges_and_wrong_dates_are_rejected() {
    let rows = [booking(1, 4, 4, false)];
    let o = Opening::capture(seed(&rows)).unwrap();
    assert!(o.project(day("2027-01-31"), 30, &rows).is_err());
    assert!(o.project(day("2028-01-01"), 30, &rows).is_err());
    assert!(
        o.project(o.effective_on, 30, &[booking(1, 5, 4, false)])
            .is_err()
    );
    assert!(
        o.project(
            o.effective_on,
            30,
            &[booking(1, 4, 4, false), booking(1, 4, 4, true)]
        )
        .is_err()
    );
    assert!(o.project(o.effective_on, i32::MAX, &rows).is_err());
}
#[test]
fn later_overdraw_is_exposed_instead_of_clamped_or_double_granted() {
    let rows = [booking(1, 4, 4, false)];
    let o = Opening::capture(seed(&rows)).unwrap();
    let p = o.project(day("2027-04-01"), 0, &rows).unwrap();
    assert!(p.balance.available_units < 0);
    assert!(p.unused_base_units < 0);
}

#[test]
fn extreme_amounts_are_errors_without_overflow_or_guessed_carry_dates() {
    let rows = [booking(1, 4, 4, false)];
    let mut s = seed(&rows);
    s.remaining_carry_units = i32::MAX;
    assert!(Opening::capture(s).is_err());
    let mut s = seed(&rows);
    s.target_available_units = i32::MIN;
    assert!(Opening::capture(s).is_err());
    let mut s = seed(&rows);
    s.carry_expires_on = day("2027-03-15");
    assert!(Opening::capture(s).is_err());
}

#[test]
fn takeover_conserves_available_units_through_booking_and_credit_changes() {
    for old_carry in 0..=6 {
        for imported in 0..=6 {
            for early in 0..=4 {
                for keep in [true, false] {
                    let old = [booking(1, 4, early, false)];
                    let mut s = seed(&old);
                    s.legacy_gross_carry_units = old_carry;
                    s.remaining_carry_units = imported;
                    let o = Opening::capture(s).unwrap();
                    let mut current = vec![booking(2, 3, 2, true)];
                    if keep {
                        current.push(old[0].clone());
                    }
                    for change in [-2, 0, 2] {
                        let before = o.project(day("2027-03-01"), 30 + change, &current).unwrap();
                        let expected = 16 + change + 4 - (if keep { 4 } else { 0 }) - 3;
                        assert_eq!(before.balance.available_units, expected);
                        let after = o.project(day("2027-04-01"), 30 + change, &current).unwrap();
                        assert_eq!(after.balance.available_units, after.unused_base_units);
                        assert_eq!(after.unspent_carry_units, 0);
                    }
                }
            }
        }
    }
}
