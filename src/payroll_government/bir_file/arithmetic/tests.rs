use super::*;
use crate::payroll_government::bir_file::{self as bir, Employer};
use chrono::NaiveDate;
use uuid::Uuid;

fn detail(schedule: Schedule) -> Vec<String> {
    let l = schedule.layout();
    let mut d = vec![String::new(); l.fields.len()];
    for (i, value) in [
        (0, l.detail),
        (1, "1604C"),
        (2, "012345678"),
        (3, "0000"),
        (4, "12/31/2026"),
        (5, "1"),
        (6, "234567890"),
        (7, "0000"),
        (8, "FICTIONAL FAMILY"),
        (9, "FICTIONAL PERSON"),
        (11, "NCR"),
        (l.dates[0], "01/01/2026"),
        (l.dates[1], "12/31/2026"),
        (l.nationality, "FILIPINO"),
        (l.status, "R"),
        (l.separation, "NA"),
        (l.filing, "N"),
    ] {
        d[i] = value.into();
    }
    for &i in l.money {
        d[i] = "0.00".into();
    }
    match schedule {
        Schedule::One => {
            for (i, value) in [
                (12, "190.10"),
                (18, "40.04"),
                (19, "100.01"),
                (20, "20.02"),
                (21, "30.03"),
                (22, "150.06"),
                (25, "380.26"),
                (31, "80.08"),
                (32, "200.05"),
                (33, "40.06"),
                (34, "60.07"),
                (35, "300.18"),
                (36, "999999.99"),
                (37, "450.24"),
            ] {
                d[i] = value.into();
            }
        }
        Schedule::Two => {
            for (i, value) in [
                (12, "570.75"),
                (13, "100.01"),
                (14, "20.02"),
                (15, "30.03"),
                (16, "40.04"),
                (17, "50.05"),
                (18, "60.06"),
                (19, "70.07"),
                (20, "80.08"),
                (21, "90.09"),
                (22, "540.45"),
                (23, "10.10"),
                (24, "20.20"),
                (25, "30.30"),
                (28, "681.75"),
                (29, "123.45"),
                (30, "9876.54"),
                (31, "987654.32"),
                (32, "313"),
                (33, "21.02"),
                (34, "31.03"),
                (35, "41.04"),
                (36, "51.05"),
                (37, "61.06"),
                (38, "71.07"),
                (39, "81.08"),
                (40, "91.09"),
                (41, "648.45"),
                (42, "11.10"),
                (43, "22.20"),
                (44, "33.30"),
                (45, "98765.43"),
                (46, "63.60"),
                (58, "200.01"),
            ] {
                d[i] = value.into();
            }
        }
    }
    d
}

fn source(schedule: Schedule, details: &[Vec<String>]) -> Vec<u8> {
    let l = schedule.layout();
    let mut control = vec![
        l.control.into(),
        "1604C".into(),
        "012345678".into(),
        "0000".into(),
        "12/31/2026".into(),
    ];
    for &i in l.money {
        control.push(crate::payroll_government::decimal(
            details.iter().map(|d| money(&d[i]).unwrap_or(0)).sum(),
        ));
    }
    ("H1604C,012345678,0000,12/31/2026\r\n".to_owned()
        + &details
            .iter()
            .map(|r| r.join(",") + "\r\n")
            .collect::<String>()
        + &control.join(",")
        + "\r\n")
        .into_bytes()
}
fn check(raw: &[u8]) -> bir::Report {
    bir::inspect(
        raw,
        "0123456780000123120261604C.DAT",
        NaiveDate::from_ymd_opt(2026, 10, 27).unwrap(),
        Some(&Employer {
            tin: "012345678".into(),
            branch: "0000".into(),
        }),
        Uuid::from_u128(1),
        Uuid::from_u128(2),
    )
    .unwrap()
}

pub(super) fn sample_source(schedule: Schedule, different: bool) -> Vec<u8> {
    let mut d = detail(schedule);
    if different {
        d[12] = crate::payroll_government::decimal(money(&d[12]).unwrap() + 1);
    }
    source(schedule, &[d])
}

#[test]
fn coherent_compensation_sums_use_exact_cents_in_both_schedules() {
    for (schedule, count) in [(Schedule::One, 5), (Schedule::Two, 7)] {
        let raw = source(schedule, &[detail(schedule)]);
        let before = raw.clone();
        let r = check(&raw);
        assert_eq!(r.errors, 0);
        assert!(r.arithmetic.checked());
        assert_eq!(
            (
                r.arithmetic.rows,
                r.arithmetic.comparisons,
                r.arithmetic.differences
            ),
            (1, count, 0)
        );
        assert!(r.arithmetic.findings.is_empty());
        assert_eq!(raw, before);
        assert!(r.arithmetic.summary().contains("without calculating tax"));
    }
}

#[test]
fn one_cent_subtotal_changes_are_visible_even_when_all_file_controls_match() {
    for (schedule, targets) in [
        (Schedule::One, vec![12, 22, 25, 35, 37]),
        (Schedule::Two, vec![12, 22, 25, 28, 41, 44, 46]),
    ] {
        for i in targets {
            let mut d = detail(schedule);
            d[i] = crate::payroll_government::decimal(money(&d[i]).unwrap() + 1);
            let r = check(&source(schedule, &[d]));
            assert_eq!(r.errors, 0);
            assert!(r.totals.iter().all(|t| t.detail_cents == t.control_cents));
            assert!(r.arithmetic.differences > 0);
            assert!(
                r.arithmetic
                    .findings
                    .iter()
                    .any(|f| f.line == 2 && f.field == schedule.layout().fields[i])
            );
        }
    }
}

#[test]
fn opposite_person_discrepancies_cannot_cancel_in_an_aggregate() {
    let mut a = detail(Schedule::One);
    let mut b = a.clone();
    a[12] = "190.11".into();
    b[12] = "190.09".into();
    b[5] = "2".into();
    b[6] = "345678901".into();
    let r = check(&source(Schedule::One, &[a, b]));
    assert_eq!(r.errors, 0);
    assert_eq!(r.arithmetic.differences, 2);
    assert_eq!(
        r.arithmetic
            .findings
            .iter()
            .map(|f| f.line)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(r.totals[0].detail_cents, Some(38_020));
}

#[test]
fn mwe_uses_net_basic_not_projection_rates_factor_or_combined_compensation() {
    let mut d = detail(Schedule::Two);
    for i in [29, 30, 31, 45] {
        d[i] = "99999999999.99".into();
    }
    d[32] = "999".into();
    let r = check(&source(Schedule::Two, &[d.clone()]));
    assert_eq!(r.errors, 0);
    assert_eq!(r.arithmetic.differences, 0);
    assert!(r.reviews > 0); // MWE eligibility remains unverified.
    d[58] = "200.02".into();
    let r = check(&source(Schedule::Two, &[d]));
    assert_eq!(r.arithmetic.differences, 1);
    assert_eq!(r.arithmetic.findings[0].field, "Present exempt total");
    assert!(r.arithmetic.findings[0].equation.contains("column 59"));
    assert!(!r.arithmetic.findings[0].equation.contains("column 32"));
}

#[test]
fn arithmetic_never_certifies_tax_credit_withholding_or_ambiguous_combined_basis() {
    for (schedule, columns) in [
        (
            Schedule::One,
            vec![
                13, 14, 15, 16, 17, 26, 27, 28, 29, 30, 36, 38, 39, 40, 41, 42, 43, 48,
            ],
        ),
        (Schedule::Two, vec![45, 47, 48, 49, 50, 51, 52, 57]),
    ] {
        let mut d = detail(schedule);
        for i in columns {
            d[i] = "87654321.09".into();
        }
        let r = check(&source(schedule, &[d]));
        assert_eq!(r.errors, 0);
        assert_eq!(r.arithmetic.differences, 0);
    }
}

#[test]
fn any_missing_amount_or_control_error_withholds_whole_file_arithmetic() {
    for schedule in [Schedule::One, Schedule::Two] {
        let mut d = detail(schedule);
        d[12] = String::new();
        let r = check(&source(schedule, &[d]));
        assert!(r.errors > 0);
        assert!(!r.arithmetic.checked());
        assert_eq!(r.arithmetic.comparisons, 0);
        let raw = source(schedule, &[detail(schedule)]);
        let mut text = String::from_utf8(raw).unwrap();
        let at = text.rfind(',').unwrap();
        text.replace_range(at + 1..text.len() - 2, "1.01");
        let r = check(text.as_bytes());
        assert!(r.errors > 0);
        assert_eq!(r.arithmetic, Review::default());
        assert!(r.arithmetic.summary().contains("no partial"));
    }
}

#[test]
fn physical_lines_and_private_minimal_findings_survive_csv_and_excel() {
    let mut d = detail(Schedule::One);
    d[12] = "190.11".into();
    d[38] = "87654321.09".into();
    let raw = source(Schedule::One, &[d]);
    let text = String::from_utf8(raw)
        .unwrap()
        .replacen("\r\n", "\r\n\r\n", 1);
    let r = check(text.as_bytes());
    assert_eq!(r.arithmetic.findings[0].line, 3);
    let csv = String::from_utf8(r.findings_csv().unwrap()).unwrap();
    assert!(csv.contains("DECLARED_SUM_DIFFERENCE"));
    for forbidden in [
        "012345678",
        "234567890",
        "FICTIONAL FAMILY",
        "FICTIONAL PERSON",
        "190.11",
        "87654321.09",
    ] {
        assert!(!csv.contains(forbidden));
    }
    let bytes = r
        .findings_export(std::path::Path::new("findings.xlsx"))
        .unwrap();
    use calamine::Reader;
    let mut book = calamine::Xlsx::new(std::io::Cursor::new(bytes)).unwrap();
    let sheet = book.worksheet_range_at(0).unwrap().unwrap();
    let cells = sheet
        .rows()
        .flatten()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("|");
    assert!(cells.contains("DECLARED_SUM_DIFFERENCE"));
    assert!(!cells.contains("FICTIONAL FAMILY") && !cells.contains("87654321.09"));
}

#[test]
fn capped_diagnostics_retain_all_row_comparison_and_difference_counts() {
    let mut rows = Vec::new();
    for n in 0..MAX_DETAILS {
        let mut d = detail(Schedule::Two);
        d[5] = (n + 1).to_string();
        d[6] = (234_567_890 + n).to_string();
        for &i in &[12, 22, 25, 28, 41, 44, 46] {
            d[i] = "0.01".into();
        }
        rows.push(d);
    }
    let r = check(&source(Schedule::Two, &rows));
    assert_eq!(r.errors, 0);
    assert_eq!(
        (
            r.arithmetic.rows,
            r.arithmetic.comparisons,
            r.arithmetic.differences
        ),
        (500, 3500, 3500)
    );
    assert_eq!(r.arithmetic.findings.len(), 500);
    assert_eq!(r.arithmetic.omitted(), 3000);
    assert!(
        r.arithmetic
            .summary()
            .contains("3000 further findings omitted")
    );
}

#[test]
fn legacy_serialized_reports_never_gain_an_invented_arithmetic_pass() {
    let r = check(&source(Schedule::One, &[detail(Schedule::One)]));
    let mut v = serde_json::to_value(r).unwrap();
    v.as_object_mut().unwrap().remove("arithmetic");
    let old: bir::Report = serde_json::from_value(v).unwrap();
    assert!(!old.arithmetic.checked());
    assert!(
        String::from_utf8(old.findings_csv().unwrap())
            .unwrap()
            .contains("NOT_CHECKED")
    );
}

#[test]
#[ignore = "Explicit fictional source/findings artifacts in a new private evidence directory."]
fn write_bir_arithmetic_vectors() {
    let out = std::path::PathBuf::from(std::env::var("HRIS_BIR_ARITHMETIC_VECTORS_OUT").unwrap());
    std::fs::create_dir(&out).unwrap();
    let mut reports = Vec::new();
    for (case, schedule, changed) in [
        ("s1-match", Schedule::One, false),
        ("s1-cent", Schedule::One, true),
        ("s2-match", Schedule::Two, false),
        ("s2-cent", Schedule::Two, true),
    ] {
        let mut d = detail(schedule);
        if changed {
            d[12] = crate::payroll_government::decimal(money(&d[12]).unwrap() + 1);
        }
        let raw = source(schedule, &[d]);
        let r = check(&raw);
        let dir = out.join(case);
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("0123456780000123120261604C.DAT"), &raw).unwrap();
        crate::reports::save_export(&dir.join("findings.csv"), &r.findings_csv().unwrap()).unwrap();
        crate::reports::save_export(
            &dir.join("findings.xlsx"),
            &r.findings_export(std::path::Path::new("findings.xlsx"))
                .unwrap(),
        )
        .unwrap();
        reports.push(serde_json::json!({"case":case,"report":r}));
    }
    std::fs::write(
        out.join("reports.json"),
        serde_json::to_vec_pretty(&reports).unwrap(),
    )
    .unwrap();
}
