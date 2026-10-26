use super::*;

fn decode(wire: Wire) -> Result<Ledger> {
    let cid = wire.company_id;
    let year = wire.year;
    Ledger::from_wire(wire, cid, year)
}

#[test]
fn annual_signed_received_total_excludes_unpaid_missing_postdated_and_review() {
    let ledger = fixture();
    let s = &ledger.summary;
    assert_eq!(s.complete_received_cents, Some(111611));
    assert_eq!(
        (
            s.received,
            s.unpaid,
            s.missing_receipt,
            s.postdated,
            s.excluded
        ),
        (3, 1, 1, 1, 1)
    );
    assert_eq!(ledger.wire.entries[0].fact.starts_on.year(), 2025);
    assert_eq!(
        ledger.wire.entries[0].fact.received_on.unwrap().year(),
        2026
    );
    assert!(
        ledger.wire.entries[4]
            .treatment()
            .contains("year unverified")
    );
}
#[test]
fn observed_months_preserve_identical_source_context_without_extra_money() {
    let mut w = fixture().wire;
    w.entries[5].observed_months =
        vec!["2026-01-01".parse().unwrap(), "2026-10-01".parse().unwrap()];
    let ledger = decode(w).unwrap();
    assert_eq!(ledger.summary.complete_received_cents, Some(111611));
    assert_eq!(ledger.wire.entries.len(), 7);
}
#[test]
fn no_receipts_unknown_receipt_and_recorded_zero_are_distinct() {
    let mut w = fixture().wire;
    w.entries.retain(|e| e.fact.state != State::Received);
    let r = decode(w).unwrap();
    assert_eq!(r.summary.complete_received_cents, None);
    assert_eq!(r.summary.received, 0);
    let mut w = fixture().wire;
    w.entries.retain(|e| e.fact.kind == Kind::AnnualTax);
    w.entries[0].fact.parts.employee_cents = None;
    let r = decode(w).unwrap();
    assert_eq!(r.summary.complete_received_cents, None);
    assert_eq!(r.summary.unknown_received, 1);
    let mut w = r.wire;
    w.entries[0].fact.parts.employee_cents = Some(0);
    assert_eq!(decode(w).unwrap().summary.complete_received_cents, Some(0));
    let mut w = fixture().wire;
    w.entries.clear();
    w.employees.clear();
    assert_eq!(decode(w).unwrap().summary, Summary::default());
}
#[test]
fn unavailable_legacy_regular_withholding_stays_unknown_only_in_annual_review() {
    let mut w = fixture().wire;
    w.entries[0].fact.parts.employee_cents = None;
    assert!(super::super::validate_fact(&w.entries[0].fact, w.company_id, w.as_of).is_err());
    let r = decode(w).unwrap();
    assert_eq!(r.summary.complete_received_cents, None);
    assert_eq!(r.summary.unknown_received, 1);
    assert_eq!(r.summary.known_received_cents, -11845);
}
#[test]
fn final_adjustment_adds_only_its_signed_tax_beside_original_source() {
    let mut w = fixture().wire;
    w.entries.truncate(2);
    w.entries[1].fact.kind = Kind::FinalAdjustment;
    assert_eq!(
        decode(w).unwrap().summary.complete_received_cents,
        Some(111111)
    );
}
#[test]
fn rejects_duplicate_conflicting_or_foreign_source_and_extra_fields() {
    let original = fixture().wire;
    for variant in 0..8 {
        let mut w = original.clone();
        match variant {
            0 => w.entries.push(w.entries[0].clone()),
            1 => {
                let mut e = w.entries[0].clone();
                e.fact.parts.employee_cents = Some(5);
                w.entries.push(e);
            }
            2 => w.entries[0].fact.company_id = Uuid::new_v4(),
            3 => w.entries[0].fact.agency = Agency::Sss,
            4 => w.entries[0].fact.kind = Kind::FinalContribution,
            5 => w.entries[0].fact.id = Uuid::nil(),
            6 => w.entries[0].fact.contribution_month = Some("2026-01-01".parse().unwrap()),
            _ => w.entries[0].fact.parts.employer_cents = Some(1),
        }
        assert!(decode(w).is_err(), "variant {variant}");
    }
    let mut v = serde_json::to_value(original).unwrap();
    v["private_evidence"] = serde_json::json!("PRIVATE_SENTINEL");
    assert!(serde_json::from_value::<Wire>(v).is_err());
}
#[test]
fn rejects_unsupported_year_future_month_and_receipt_year_mismatch() {
    for variant in 0..7 {
        let mut w = fixture().wire;
        match variant {
            0 => w.year = 2024,
            1 => w.year = 2027,
            2 => w.through_month = "2026-11-01".parse().unwrap(),
            3 => w.entries[0].fact.received_on = Some("2025-12-31".parse().unwrap()),
            4 => w.entries[0].observed_months = vec!["2026-02-01".parse().unwrap()],
            5 => w.entries[0].fact.received_on = Some("2026-11-01".parse().unwrap()),
            _ => w.version = "government-payroll-sources-v1".into(),
        }
        assert!(decode(w).is_err(), "variant {variant}");
    }
    let mut w = fixture().wire;
    w.year = 2025;
    w.through_month = "2025-12-01".parse().unwrap();
    w.entries.clear();
    w.employees.clear();
    assert!(decode(w).is_ok());
}
#[test]
fn rejects_invalid_observations_and_missing_or_ambiguous_label_bindings() {
    for variant in 0..8 {
        let mut w = fixture().wire;
        match variant {
            0 => w.entries[0].observed_months.clear(),
            1 => {
                let month = w.entries[0].observed_months[0];
                w.entries[0].observed_months.push(month);
            }
            2 => w.entries[0].observed_months[0] = "2026-01-02".parse().unwrap(),
            3 => w.entries[0].observed_months[0] = "2025-12-01".parse().unwrap(),
            4 => w.employees.clear(),
            5 => w.employees.push(w.employees[0].clone()),
            6 => w.employees[0].employee_id = Uuid::new_v4(),
            _ => w.employees[0].employee_name = "PRIVATE\nCONTROL".into(),
        }
        assert!(decode(w).is_err(), "variant {variant}");
    }
}
#[test]
fn namesakes_remain_separate_and_centavo_overflow_fails_closed() {
    let mut w = fixture().wire;
    let mut e = w.employees[0].clone();
    e.employee_id = Uuid::from_u128(3);
    e.employee_code = "000009".into();
    w.entries[1].fact.employee_id = e.employee_id;
    w.employees.push(e);
    let r = decode(w).unwrap();
    assert_eq!(r.wire.employees.len(), 2);
    assert_ne!(
        r.employee(Uuid::from_u128(2)).employee_code,
        r.employee(Uuid::from_u128(3)).employee_code
    );
    let mut w = r.wire;
    w.entries[0].fact.parts.employee_cents = Some(LIMIT);
    w.entries[1].fact.parts.employee_cents = Some(1);
    assert!(decode(w).is_err());
}
#[test]
fn count_bounds_reject_whole_ledger_without_truncation() {
    let mut w = fixture().wire;
    let e = w.entries[0].clone();
    w.entries = vec![e.clone(); MAX_ENTRIES + 1];
    assert!(decode(w).is_err());
    let mut w = fixture().wire;
    w.entries = (0..4001)
        .map(|i| {
            let mut e = e.clone();
            e.fact.id = Uuid::from_u128(100 + i);
            e.observed_months = (1..=10)
                .map(|m| NaiveDate::from_ymd_opt(2026, m, 1).unwrap())
                .collect();
            e
        })
        .collect();
    assert!(decode(w).is_err());
}
#[test]
fn native_csv_excel_keep_literal_codes_unicode_exact_signed_amounts_and_no_identifiers() {
    let mut w = fixture().wire;
    w.employees[0].employee_name = "=Fictional, Ángela 李".into();
    let r = decode(w).unwrap();
    let raw = r.export(Path::new("annual.xlsx")).unwrap();
    let rows = crate::tabular::read_xlsx(&raw, 30, 24).unwrap();
    assert_eq!(rows.len(), 8);
    assert_eq!(rows[1].cells[6], "000008");
    assert_eq!(rows[1].cells[7], "=Fictional, Ángela 李");
    assert_eq!(rows[2].cells[14], "-123.45");
    assert_eq!(rows[2].cells[15], "-123.45");
    assert!(rows[4].cells[15].is_empty());
    assert_eq!(rows[4].cells[14], "5.00");
    let csv = String::from_utf8(r.export(Path::new("annual.csv")).unwrap()).unwrap();
    assert!(csv.contains("'=Fictional"));
    assert!(csv.contains("-123.45"));
    for field in [
        "bir_tin",
        "bir_branch",
        "sss_number",
        "reference",
        "evidence",
        "private_note",
    ] {
        assert!(!rows[0].cells.iter().any(|c| c == field));
    }
    assert!(r.export(Path::new("annual.dat")).is_err());
}
#[test]
fn empty_export_retains_scope_without_zero_tax_claim() {
    let mut w = fixture().wire;
    w.entries.clear();
    w.employees.clear();
    let r = decode(w).unwrap();
    let raw = r.export(Path::new("empty.xlsx")).unwrap();
    let rows = crate::tabular::read_xlsx(&raw, 10, 24).unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows[1].cells[16].contains("not a zero-tax"));
}
#[test]
#[ignore = "Set HRIS_ANNUAL_VECTORS to fictional local or hosted protected response evidence."]
fn actual_database_annual_snapshots_decode_and_render() {
    let path = std::env::var("HRIS_ANNUAL_VECTORS").unwrap();
    let wires: Vec<Wire> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(wires.len() >= 2);
    let out = std::env::var("HRIS_ANNUAL_WORKBOOKS").ok();
    for (i, w) in wires.into_iter().enumerate() {
        let r = decode(w).unwrap();
        let bytes = r.export(Path::new("annual.xlsx")).unwrap();
        if let Some(out) = &out {
            let p = Path::new(out).join(format!("annual-{i}.xlsx"));
            std::fs::write(&p, &bytes).unwrap();
            std::fs::write(
                p.with_extension("csv"),
                r.export(Path::new("annual.csv")).unwrap(),
            )
            .unwrap();
        }
    }
}
