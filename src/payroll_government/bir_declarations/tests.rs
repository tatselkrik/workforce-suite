use super::*;
use crate::payroll_government::bir_identity::tests as fixture;

fn run(wire: identity::Wire, raw: &[u8]) -> Result<Report> {
    let company = wire.ledger.company_id;
    review(
        identity::Basis::from_wire(wire, company, 2026)?,
        raw,
        fixture::FILE_NAME,
        "2026-10-26".parse().unwrap(),
        Uuid::from_u128(4),
    )
}
fn s1() -> Vec<u8> {
    fixture::source(&["234567890"])
}
fn s2() -> Vec<u8> {
    let mut d = vec![String::new(); 59];
    for (i, s) in [
        (0, "D2"),
        (1, "1604C"),
        (2, "012345678"),
        (3, "0000"),
        (4, "12/31/2026"),
        (5, "1"),
        (6, "234567890"),
        (7, "0000"),
        (8, "FICTIONAL MWE FAMILY"),
        (9, "FICTIONAL GIVEN"),
        (11, "NCR"),
        (26, "01/01/2026"),
        (27, "12/31/2026"),
        (32, "365"),
        (53, "FILIPINO"),
        (54, "R"),
        (55, "NA"),
        (56, "N"),
    ] {
        d[i] = s.into();
    }
    let columns = (12..=25).chain(28..=31).chain(33..=52).chain([57, 58]);
    let mut c = vec![
        "C2".into(),
        "1604C".into(),
        "012345678".into(),
        "0000".into(),
        "12/31/2026".into(),
    ];
    for (n, column) in columns.enumerate() {
        d[column] = format!("0.{:02}", n + 1);
        c.push(d[column].clone());
    }
    format!(
        "H1604C,012345678,0000,12/31/2026\r\n{}\r\n{}\r\n",
        d.join(","),
        c.join(",")
    )
    .into_bytes()
}

#[test]
fn schedule_one_retains_every_literal_money_column_and_physical_line() {
    let raw = s1();
    let r = run(fixture::wire(), &raw).unwrap();
    assert_eq!(r.rows.len(), 1);
    let row = &r.rows[0];
    assert_eq!(row.identity.line, 2);
    assert_eq!(row.declared.len(), 31);
    assert_eq!(
        row.declared.last().unwrap(),
        &bir::DeclaredAmount {
            column: 49,
            field: "PERA tax credit".into(),
            cents: 0
        }
    );
    assert_eq!(
        row.declared
            .iter()
            .find(|d| d.field == "Actual withholding")
            .unwrap()
            .column,
        44
    );
    assert!(row.declared.iter().all(|d| d.cents == 0));
    assert_eq!(raw, s1());
}
#[test]
fn schedule_two_keeps_forty_exact_centavos_and_excludes_factor() {
    let r = run(fixture::wire(), &s2()).unwrap();
    let row = &r.rows[0];
    assert_eq!(r.identity.schedule, bir::Schedule::Two);
    assert!(r.identity.preflight_reviews > 0);
    assert_eq!(row.declared.len(), 40);
    assert_eq!(
        row.declared.iter().map(|d| d.cents).collect::<Vec<_>>(),
        (1..=40).collect::<Vec<_>>()
    );
    assert!(!row.declared.iter().any(|d| d.column == 33));
    assert_eq!(
        (
            row.declared[38].column,
            row.declared[38].field.as_str(),
            row.declared[38].cents
        ),
        (58, "PERA tax credit", 39)
    );
    assert_eq!(
        (
            row.declared[39].column,
            row.declared[39].field.as_str(),
            row.declared[39].cents
        ),
        (59, "Statutory basic net of contributions/dues", 40)
    );
}
#[test]
fn received_signed_and_prior_year_service_remain_distinct_from_declarations() {
    let r = run(fixture::wire(), &s1()).unwrap();
    let row = &r.rows[0];
    assert_eq!(row.sources.len(), 7);
    assert_eq!(
        row.sources
            .iter()
            .filter(|s| s.fact.state == State::Received)
            .count(),
        3
    );
    assert!(
        row.sources
            .iter()
            .any(|s| s.fact.parts.employee_cents == Some(-12345))
    );
    assert!(row.sources.iter().all(|s| s.fact.starts_on.year() == 2025));
    assert!(
        row.sources
            .iter()
            .any(|s| s.fact.state == State::ReceiptMissing)
    );
    assert!(
        row.sources
            .iter()
            .any(|s| s.fact.state == State::DatedAfterAsOf)
    );
    assert!(
        row.sources
            .iter()
            .any(|s| s.fact.state == State::ReviewOnly)
    );
    assert!(row.declared.iter().all(|d| d.cents == 0));
}
#[test]
fn unknown_received_amount_stays_unknown_in_the_explicit_internal_export() {
    let mut wire = fixture::wire();
    wire.ledger.entries[0].fact.parts.employee_cents = None;
    let r = run(wire, &s1()).unwrap();
    assert!(r.rows[0].sources[0].fact.parts.employee_cents.is_none());
    let csv = String::from_utf8(r.export(Path::new("review.csv")).unwrap()).unwrap();
    assert!(csv.contains("Unknown"));
    assert!(csv.contains("total incomplete"));
    assert!(csv.contains("-123.45"));
    assert!(csv.contains("2025-12-01"));
    assert!(csv.contains("NOT EXPECTED TAX OR A BIR RETURN"));
}
#[test]
fn unmatched_and_ambiguous_pairs_never_receive_sources() {
    let r = run(fixture::wire(), &fixture::source(&["456789012"])).unwrap();
    assert_eq!(r.rows[0].identity.status, identity::Status::Unmatched);
    assert!(r.rows[0].sources.is_empty());
    assert_eq!(r.sources_without_unique_file_match, 7);
    let mut wire = fixture::wire();
    wire.people[1].registration.as_mut().unwrap().tin = "234567890".into();
    let r = run(wire, &s1()).unwrap();
    assert_eq!(r.rows[0].identity.status, identity::Status::Ambiguous);
    assert!(r.rows[0].sources.is_empty());
    assert_eq!(r.rows[0].declared.len(), 31);
}
#[test]
fn namesakes_do_not_share_sources_and_no_observations_is_not_zero_tax() {
    let r = run(
        fixture::wire(),
        &fixture::source(&["234567890", "345678901"]),
    )
    .unwrap();
    assert_eq!(
        r.rows[0].identity.employee.as_ref().unwrap().employee_name,
        r.rows[1].identity.employee.as_ref().unwrap().employee_name
    );
    assert_eq!(r.rows[0].sources.len(), 7);
    assert!(r.rows[1].sources.is_empty());
    let csv = String::from_utf8(r.export(Path::new("review.csv")).unwrap()).unwrap();
    assert!(csv.contains("not a zero-tax declaration"));
}
#[test]
fn no_money_review_for_wrong_company_year_control_or_revoked_identity_shape() {
    let mut wire = fixture::wire();
    wire.employer = None;
    assert!(run(wire, &s1()).is_err());
    let mut wire = fixture::wire();
    wire.ledger.year = 2025;
    assert!(run(wire, &s1()).is_err());
    let mut raw = String::from_utf8(s1()).unwrap();
    raw = raw.replace(
        "C1,1604C,012345678,0000,12/31/2026,0.00",
        "C1,1604C,012345678,0000,12/31/2026,0.01",
    );
    assert!(run(fixture::wire(), raw.as_bytes()).is_err());
}
#[test]
fn changed_bytes_cannot_reuse_the_old_preflight_amount_extraction() {
    let raw = s1();
    let checked = bir::inspect(
        &raw,
        fixture::FILE_NAME,
        "2026-10-26".parse().unwrap(),
        None,
        Uuid::from_u128(1),
        Uuid::from_u128(4),
    )
    .unwrap();
    let mut changed = raw.clone();
    changed.push(b'\n');
    assert!(bir::checked_declared_money(&changed, &checked).is_err());
    let mut errored = checked;
    errored.errors = 1;
    assert!(bir::checked_declared_money(&raw, &errored).is_err());
}
#[test]
fn exports_omit_tax_ids_file_names_and_file_declared_names() {
    let r = run(fixture::wire(), &s2()).unwrap();
    let csv = String::from_utf8(r.export(Path::new("review.csv")).unwrap()).unwrap();
    for omitted in [
        "012345678",
        "234567890",
        fixture::FILE_NAME,
        "FICTIONAL MWE FAMILY",
        "FICTIONAL GIVEN",
    ] {
        assert!(
            !csv.contains(omitted),
            "Unexpected raw identity field: {omitted}"
        );
    }
    assert!(csv.contains("000008"));
    assert!(csv.contains("Ángela"));
    assert!(csv.contains("FILE_DECLARATION"));
    assert!(csv.contains("RECORDED_SOURCE"));
    assert!(
        r.export(Path::new("review.xlsx"))
            .unwrap()
            .starts_with(b"PK")
    );
    assert!(r.export(Path::new("review.txt")).is_err());
}

#[test]
#[ignore = "Explicit fictional workbook output for independent reading"]
fn emit_declaration_review_workbooks() {
    use std::io::Write;
    let root = Path::new("work/declarations059-excel");
    std::fs::create_dir_all(root).unwrap();
    let mut manifest = Vec::new();
    for case in [
        "schedule1",
        "schedule2",
        "unknown",
        "unmatched",
        "ambiguous",
        "namesakes",
    ] {
        let mut wire = fixture::wire();
        if case == "unknown" {
            wire.ledger.entries[0].fact.parts.employee_cents = None;
        }
        if case == "ambiguous" {
            wire.people[1].registration.as_mut().unwrap().tin = "234567890".into();
        }
        let raw = match case {
            "schedule2" => s2(),
            "unmatched" => fixture::source(&["456789012"]),
            "namesakes" => fixture::source(&["234567890", "345678901"]),
            _ => s1(),
        };
        let report = run(wire, &raw).unwrap();
        for ext in ["csv", "xlsx"] {
            let file = root.join(format!("{case}.{ext}"));
            let bytes = report.export(&file).unwrap();
            if file.exists() {
                assert_eq!(std::fs::read(&file).unwrap(), bytes);
            } else {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(file)
                    .unwrap()
                    .write_all(&bytes)
                    .unwrap();
            }
        }
        manifest.push(serde_json::json!({"case":case,"report":report}));
    }
    let path = root.join("fictional-native-manifest.json");
    let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    if path.exists() {
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    } else {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
    }
}
