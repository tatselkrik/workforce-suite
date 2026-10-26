use super::*;
fn rows() -> (Vec<String>, Vec<String>) {
    let mut d = vec![String::new(); 59];
    for (i, v) in [
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
        d[i] = v.into();
    }
    // Published monetary ranges: previous13–26, present29–32 and34–53, PERA58/basic59.
    for i in (12..=25).chain(28..=31).chain(33..=52).chain([57, 58]) {
        d[i] = "0.00".into();
    }
    let mut c = vec!["0.00".to_owned(); 45];
    for (i, v) in [
        (0, "C2"),
        (1, "1604C"),
        (2, "012345678"),
        (3, "0000"),
        (4, "12/31/2026"),
    ] {
        c[i] = v.into();
    }
    (d, c)
}
fn text(d: &[Vec<String>], c: &[String]) -> String {
    "H1604C,012345678,0000,12/31/2026\r\n".to_owned()
        + &d.iter().map(|r| r.join(",") + "\r\n").collect::<String>()
        + &c.join(",")
        + "\r\n"
}
pub(crate) fn fictional_source() -> Vec<u8> {
    let (d, c) = rows();
    text(&[d], &c).into_bytes()
}
fn check(d: &[Vec<String>], c: &[String]) -> Report {
    inspect(
        text(d, c).as_bytes(),
        "0123456780000123120261604C.DAT",
        "2026-10-26".parse().unwrap(),
        Some(&Employer {
            tin: "012345678".into(),
            branch: "0000".into(),
        }),
        Uuid::from_u128(1),
        Uuid::from_u128(2),
    )
    .unwrap()
}
#[test]
fn bir_mwe_published_boundary_columns_pera_and_net_basic_map_to_45_column_control_without_factor() {
    let (mut d, mut c) = rows();
    for (di, ci, value) in [
        (12, 5, "1.01"),
        (25, 18, "2.02"),
        (28, 19, "3.03"),
        (31, 22, "4.04"),
        (33, 23, "5.05"),
        (52, 42, "6.06"),
        (57, 43, "7.07"),
        (58, 44, "8.08"),
    ] {
        d[di] = value.into();
        c[ci] = value.into();
    }
    let r = check(&[d], &c);
    assert_eq!(
        (r.schedule, r.errors, r.reviews, r.details),
        (Schedule::Two, 0, 1, 1)
    );
    assert_eq!(r.totals.len(), 40);
    assert_eq!(r.totals[38].detail_cents, Some(707));
    assert_eq!(r.totals[39].detail_cents, Some(808));
    assert!(r.totals.iter().all(|t| t.detail_cents == t.control_cents));
    assert!(!r.totals.iter().any(|t| t.field.contains("factor")));
}
#[test]
fn bir_mwe_reported_factor_is_shape_checked_and_never_certified_or_added_as_money() {
    let (mut d, c) = rows();
    for bad in ["", "0", "1000", "365.25", "AUTO", "-1"] {
        d[32] = bad.into();
        let r = check(&[d.clone()], &c);
        assert!(
            r.errors > 0
                && r.issues
                    .iter()
                    .any(|i| i.field == "Reported days/year factor")
        );
    }
    for shape in ["1", "313", "365", "999"] {
        d[32] = shape.into();
        let r = check(&[d.clone()], &c);
        assert_eq!(r.errors, 0);
        assert!(r.reviews > 0);
        assert!(
            r.issues
                .iter()
                .any(|i| i.message.contains("basis are unverified"))
        );
    }
}
#[test]
fn bir_mwe_missing_values_and_one_cent_pera_net_basic_differences_never_get_zero_or_tolerance() {
    let (mut d, mut c) = rows();
    d[14] = String::new();
    let r = check(&[d], &c);
    assert!(r.errors > 0);
    assert!(r.totals[2].detail_cents.is_none());
    let (d, _) = rows();
    c[43] = "0.01".into();
    c[44] = "0.01".into();
    let r = check(&[d], &c);
    assert_eq!(r.errors, 2);
    assert!(
        r.issues
            .iter()
            .any(|i| i.line == 3 && i.field.starts_with("C2 column 44"))
    );
    assert!(r.issues.iter().any(|i| i.field.starts_with("C2 column 45")));
}
#[test]
fn bir_mwe_uses_its_actual_date_and_code_columns_and_does_not_shift_them_into_amounts() {
    let (d, c) = rows();
    for (column, bad) in [
        (26, "02/30/2026"),
        (27, "01/01/2025"),
        (53, "NAME THAT EXCEEDS THE THIRTY CHARACTER WIDTH"),
        (54, "ADMIN"),
        (55, "ARCHIVED"),
        (56, "AUTO"),
    ] {
        let mut row = d.clone();
        row[column] = bad.into();
        assert!(check(&[row], &c).errors > 0);
    }
    let mut malformed = d;
    malformed.pop();
    assert!(check(&[malformed], &c).errors > 0);
}
#[test]
fn bir_mwe_mixed_schedules_fail_whole_file_and_findings_remain_private_and_schedule_labelled() {
    let (d, c) = rows();
    let mut other = vec!["0".to_owned(); 49];
    other[0] = "D1".into();
    let mixed = text(&[other, d.clone()], &c);
    assert!(
        inspect(
            mixed.as_bytes(),
            "source.dat",
            "2026-10-26".parse().unwrap(),
            None,
            Uuid::from_u128(1),
            Uuid::from_u128(2)
        )
        .unwrap_err()
        .to_string()
        .contains("Mixed Schedule")
    );
    let r = check(&[d], &c);
    let out = String::from_utf8(r.findings_csv().unwrap()).unwrap();
    assert!(out.contains("Schedule 2") && out.contains("MWE declarations"));
    for private in [
        "234567890",
        "012345678",
        "FICTIONAL MWE FAMILY",
        "FICTIONAL GIVEN",
    ] {
        assert!(!out.contains(private));
    }
}

#[test]
#[ignore = "Explicit fictional native MWE source/findings evidence to a new private folder."]
fn write_bir_mwe_findings_vectors() {
    let dir = std::path::PathBuf::from(std::env::var("HRIS_BIR_MWE_VECTORS_OUT").unwrap());
    std::fs::create_dir(&dir).unwrap();
    let mut reports = Vec::new();
    for case in [
        "exact",
        "all-columns",
        "bad-money",
        "one-cent",
        "bad-factor",
        "namesakes",
    ] {
        let (mut d, mut c) = rows();
        if case == "all-columns" {
            for (j, i) in (12..=25)
                .chain(28..=31)
                .chain(33..=52)
                .chain([57, 58])
                .enumerate()
            {
                let amount = super::super::decimal((j as i64 + 1) * 137);
                d[i] = amount.clone();
                c[j + 5] = amount;
            }
        }
        if case == "bad-money" {
            d[14] = String::new();
        }
        if case == "one-cent" {
            c[44] = "0.01".into();
        }
        if case == "bad-factor" {
            d[32] = "365.25".into();
        }
        let mut data = vec![d.clone()];
        if case == "namesakes" {
            d[5] = "2".into();
            d[6] = "345678901".into();
            data.push(d);
        }
        let r = check(&data, &c);
        let folder = dir.join(case);
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(
            folder.join("0123456780000123120261604C.DAT"),
            text(&data, &c),
        )
        .unwrap();
        crate::reports::save_export(&folder.join("findings.csv"), &r.findings_csv().unwrap())
            .unwrap();
        crate::reports::save_export(
            &folder.join("findings.xlsx"),
            &r.findings_export(Path::new("findings.xlsx")).unwrap(),
        )
        .unwrap();
        reports.push(serde_json::json!({"case":case,"report":r}));
    }
    std::fs::write(
        dir.join("reports.json"),
        serde_json::to_vec_pretty(&reports).unwrap(),
    )
    .unwrap();
}
