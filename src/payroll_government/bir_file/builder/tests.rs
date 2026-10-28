use super::*;

fn employer() -> Employer {
    Employer {
        tin: "012345678".into(),
        branch: "0000".into(),
    }
}
fn basis(employer: &Employer, schedule: Schedule) -> Basis<'_> {
    Basis {
        selection: Selection {
            schedule,
            year: 2026,
        },
        today: "2026-10-27".parse().unwrap(),
        employer,
        company_id: Uuid::from_u128(1),
        user_id: Uuid::from_u128(2),
        registration_id: Uuid::from_u128(3),
        registration_revision: 0,
    }
}
fn declaration(schedule: Schedule) -> Vec<String> {
    let layout = schedule.layout();
    let mut full = vec![String::new(); layout.fields.len()];
    for (index, value) in [
        (6, "234567890"),
        (7, "0000"),
        (8, "FICTIONAL FAMILY"),
        (9, "FICTIONAL GIVEN"),
        (11, "NCR"),
    ] {
        full[index] = value.into();
    }
    full[layout.dates[0]] = "01/01/2026".into();
    full[layout.dates[1]] = "12/31/2026".into();
    full[layout.nationality] = "FILIPINO".into();
    full[layout.status] = "R".into();
    full[layout.separation] = "NA".into();
    full[layout.filing] = "N".into();
    for &column in layout.money {
        full[column] = "0.00".into();
    }
    match schedule {
        Schedule::One => {
            for column in [25, 32, 35, 36, 37] {
                full[column] = "100.01".into();
            }
            // Literal declared tax is preserved, never recalculated from income.
            full[38] = "37.99".into();
            full[48] = "1.23".into();
        }
        Schedule::Two => {
            for column in [28, 41, 45, 58] {
                full[column] = "100.01".into();
            }
            full[29] = "12.34".into();
            full[30] = "123.45".into();
            full[31] = "1234.56".into();
            full[32] = "313".into();
            full[57] = "1.23".into();
        }
    }
    full[6..].to_vec()
}
fn csv(schedule: Schedule, rows: &[Vec<String>]) -> Vec<u8> {
    crate::imports::csv(headers(schedule), rows.iter().cloned()).unwrap()
}
fn build(schedule: Schedule, rows: &[Vec<String>]) -> Result<Draft> {
    assemble(&csv(schedule, rows), false, &basis(&employer(), schedule))
}

#[test]
fn bir_builder_both_published_layouts_exact_controls_and_explicit_tax_preserved() {
    for schedule in [Schedule::One, Schedule::Two] {
        let draft = build(schedule, &[declaration(schedule)]).unwrap();
        assert!(draft.bytes.is_ascii() && !draft.bytes.starts_with(&[239, 187, 191]));
        assert!(draft.bytes.ends_with(b"\r\n"));
        let text = std::str::from_utf8(&draft.bytes).unwrap();
        let rows = crate::imports::parse_bounded(text, 1000, 75).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].cells, ["H1604C", "012345678", "0000", "12/31/2026"]);
        assert_eq!(rows[1].cells.len(), schedule.layout().fields.len());
        assert_eq!(rows[2].cells.len(), schedule.layout().money.len() + 5);
        assert_eq!(draft.preview.file_name, "0123456780000123120261604C.DAT");
        assert_eq!(
            draft.preview.output_sha256,
            format!("{:x}", Sha256::digest(&draft.bytes))
        );
        assert_eq!(
            draft.preview.controls,
            if schedule == Schedule::One { 31 } else { 40 }
        );
        if schedule == Schedule::One {
            assert_eq!(rows[1].cells[38], "37.99");
            assert_eq!(rows[2].cells[35], "1.23");
        } else {
            assert_eq!(rows[1].cells[32], "313");
            assert_eq!(rows[2].cells[43], "1.23");
            assert_eq!(rows[2].cells[44], "100.01");
            assert!(draft.preview.review_items > 0);
        }
    }
}

#[test]
fn bir_builder_csv_and_literal_excel_preserve_identical_declared_bytes() {
    for schedule in [Schedule::One, Schedule::Two] {
        let rows = vec![declaration(schedule)];
        let expected = build(schedule, &rows).unwrap();
        let xlsx = crate::tabular::text_xlsx(headers(schedule), &rows).unwrap();
        let actual = assemble(&xlsx, true, &basis(&employer(), schedule)).unwrap();
        assert_eq!(actual.bytes, expected.bytes);
        assert_ne!(actual.preview.source_sha256, expected.preview.source_sha256);
        assert_ne!(actual.preview.basis_sha256, expected.preview.basis_sha256);
    }
}

#[test]
fn bir_builder_blank_explicit_amounts_never_default_to_zero_or_accept_bad_decimals() {
    for schedule in [Schedule::One, Schedule::Two] {
        for &column in schedule.layout().money {
            for bad in ["", "-1.00", "1.001", "1e3", "1,000.00", "NaN"] {
                let mut row = declaration(schedule);
                row[column - 6] = bad.into();
                let error = build(schedule, &[row]).err().unwrap().to_string();
                assert!(!error.contains("234567890") && !error.contains("FICTIONAL FAMILY"));
            }
        }
    }
}

#[test]
fn bir_builder_subtotal_disagreement_is_rejected_without_repair() {
    for schedule in [Schedule::One, Schedule::Two] {
        let mut row = declaration(schedule);
        let column = if schedule == Schedule::One { 25 } else { 28 };
        row[column - 6] = "100.02".into();
        assert!(
            build(schedule, &[row])
                .err()
                .unwrap()
                .to_string()
                .contains("subtotals disagree")
        );
    }
}

#[test]
fn bir_builder_control_width_overflow_fails_even_when_individual_values_fit() {
    let mut first = declaration(Schedule::One);
    for column in [25, 32, 35, 36, 37] {
        first[column - 6] = "99999999999.99".into();
    }
    let mut second = first.clone();
    second[0] = "234567891".into();
    assert!(
        build(Schedule::One, &[first, second])
            .err()
            .unwrap()
            .to_string()
            .contains("monetary width")
    );
}

#[test]
fn bir_builder_rejects_ambiguous_names_delimiters_whitespace_codes_and_encoding() {
    for bad in [
        "Family, Jr",
        "Family\"Name",
        " Name",
        "Name ",
        "NIÑO",
        "Line\nName",
        "A\tB",
    ] {
        let mut row = declaration(Schedule::One);
        row[2] = bad.into();
        assert!(build(Schedule::One, &[row]).is_err());
    }
    for (column, bad) in [(39, "r"), (40, "na"), (41, "n"), (38, "")] {
        let mut row = declaration(Schedule::One);
        row[column] = bad.into();
        assert!(build(Schedule::One, &[row]).is_err());
    }
}

#[test]
fn bir_builder_does_not_guess_identity_codes_dates_or_mwe_factor() {
    for (column, bad) in [
        (0, "23456789"),
        (1, "0"),
        (5, ""),
        (17, "not a date"),
        (40, "UNKNOWN"),
        (41, ""),
    ] {
        let mut row = declaration(Schedule::One);
        row[column] = bad.into();
        assert!(build(Schedule::One, &[row]).is_err());
    }
    let mut row = declaration(Schedule::Two);
    row[26] = "".into();
    assert!(build(Schedule::Two, &[row]).is_err());
}

#[test]
fn bir_builder_duplicate_identities_refused_but_namesakes_preserved() {
    let first = declaration(Schedule::One);
    assert!(build(Schedule::One, &[first.clone(), first.clone()]).is_err());
    let mut second = first.clone();
    second[0] = "234567891".into();
    let draft = build(Schedule::One, &[first, second]).unwrap();
    assert_eq!(draft.preview.details, 2);
    let text = std::str::from_utf8(&draft.bytes).unwrap();
    assert!(text.contains(",1,234567890,") && text.contains(",2,234567891,"));
}

#[test]
fn bir_builder_strict_schedule_headers_no_extra_or_partial_rows() {
    let mut bytes = csv(Schedule::One, &[declaration(Schedule::One)]);
    bytes.extend(b"unexpected,row\r\n");
    assert!(assemble(&bytes, false, &basis(&employer(), Schedule::One)).is_err());
    assert!(
        assemble(
            &csv(Schedule::One, &[declaration(Schedule::One)]),
            false,
            &basis(&employer(), Schedule::Two)
        )
        .is_err()
    );
    let mut row = declaration(Schedule::One);
    row.pop();
    let mut bytes = template(Schedule::One, false).unwrap();
    bytes.extend((row.join(",") + "\r\n").as_bytes());
    assert!(assemble(&bytes, false, &basis(&employer(), Schedule::One)).is_err());
    assert!(
        assemble(
            &template(Schedule::One, false).unwrap(),
            false,
            &basis(&employer(), Schedule::One)
        )
        .is_err()
    );
}

#[test]
fn bir_builder_preview_binds_full_source_scope_selection_and_registration_revision() {
    let employer = employer();
    let mut basis = basis(&employer, Schedule::One);
    let bytes = csv(Schedule::One, &[declaration(Schedule::One)]);
    let first = assemble(&bytes, false, &basis).unwrap().preview;
    basis.registration_revision += 1;
    let next = assemble(&bytes, false, &basis).unwrap().preview;
    assert_ne!(first.basis_sha256, next.basis_sha256);
    assert_eq!(first.output_sha256, next.output_sha256);
    basis.user_id = Uuid::from_u128(9);
    assert_ne!(
        next.basis_sha256,
        assemble(&bytes, false, &basis)
            .unwrap()
            .preview
            .basis_sha256
    );
    basis.selection.year = 2025;
    let prior = assemble(&bytes, false, &basis).unwrap().preview;
    assert_ne!(next.basis_sha256, prior.basis_sha256);
    assert_ne!(next.output_sha256, prior.output_sha256);
}

#[test]
fn bir_builder_rejects_unsupported_year_or_incomplete_protected_employer() {
    let mut employer = employer();
    let bytes = csv(Schedule::One, &[declaration(Schedule::One)]);
    for year in [2022, 2027] {
        let mut basis = basis(&employer, Schedule::One);
        basis.selection.year = year;
        assert!(assemble(&bytes, false, &basis).is_err());
    }
    employer.branch.clear();
    assert!(assemble(&bytes, false, &basis(&employer, Schedule::One)).is_err());
}

#[test]
fn bir_builder_full_500_rows_no_truncation_and_501_refused() {
    let rows: Vec<_> = (0..500)
        .map(|i| {
            let mut row = declaration(Schedule::One);
            row[0] = format!("{:09}", 234567890 + i);
            row
        })
        .collect();
    assert_eq!(build(Schedule::One, &rows).unwrap().preview.details, 500);
    let mut excessive = rows;
    excessive.push(declaration(Schedule::One));
    assert!(build(Schedule::One, &excessive).is_err());
}

#[test]
fn bir_builder_output_never_overwrites_source_or_existing_files() {
    let root = std::env::temp_dir().join(format!("hris-bir-builder-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let draft = build(Schedule::One, &[declaration(Schedule::One)]).unwrap();
    let destination = root.join(&draft.preview.file_name);
    draft.save_new(&destination).unwrap();
    let before = std::fs::read(&destination).unwrap();
    assert!(draft.save_new(&destination).is_err());
    assert!(draft.save_new(&root.join("different.DAT")).is_err());
    assert_eq!(before, std::fs::read(&destination).unwrap());
    std::fs::remove_file(destination).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bir_builder_rejects_numeric_excel_identifiers_and_formulas() {
    for formula in [false, true] {
        let schedule = Schedule::One;
        let mut workbook = rust_xlsxwriter::Workbook::new();
        let sheet = workbook.add_worksheet();
        for (i, header) in headers(schedule).iter().enumerate() {
            sheet.write_string(0, i as u16, *header).unwrap();
        }
        for (i, value) in declaration(schedule).iter().enumerate() {
            sheet.write_string(1, i as u16, value).unwrap();
        }
        if formula {
            sheet.write_formula(1, 0, "=234567890").unwrap();
        } else {
            sheet.write_number(1, 0, 234567890.0).unwrap();
        }
        assert!(
            assemble(
                &workbook.save_to_buffer().unwrap(),
                true,
                &basis(&employer(), schedule)
            )
            .is_err()
        );
    }
}

#[test]
#[ignore = "writes fictional declaration templates/DAT to fixed project-local QA evidence"]
fn bir_builder_fictional_native_artifacts() {
    fn save(path: &Path, bytes: &[u8]) {
        if path.exists() {
            assert_eq!(
                std::fs::read(path).unwrap(),
                bytes,
                "Preserve earlier evidence"
            );
        } else {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap();
            file.write_all(bytes).unwrap();
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/bir062-vectors");
    for schedule in [Schedule::One, Schedule::Two] {
        let mut row = declaration(schedule);
        let amounts: Vec<(usize, &str)> = match schedule {
            Schedule::One => vec![
                (12, "335.78"),
                (13, "11.11"),
                (14, "22.22"),
                (15, "33.33"),
                (16, "44.44"),
                (17, "55.55"),
                (18, "166.65"),
                (19, "100.01"),
                (20, "12.34"),
                (21, "56.78"),
                (22, "169.13"),
                (25, "1328.62"),
                (26, "1.01"),
                (27, "2.02"),
                (28, "3.03"),
                (29, "4.04"),
                (30, "5.05"),
                (31, "15.15"),
                (32, "1000.01"),
                (33, "234.56"),
                (34, "78.90"),
                (35, "1313.47"),
                (36, "1664.40"),
                (37, "1482.60"),
                (38, "249.01"),
                (39, "90.01"),
                (40, "95.02"),
                (41, "30.03"),
                (42, "1.04"),
                (43, "214.02"),
                (48, "2.05"),
            ],
            Schedule::Two => vec![
                (12, "66.66"),
                (13, "1.01"),
                (14, "2.02"),
                (15, "3.03"),
                (16, "4.04"),
                (17, "5.05"),
                (18, "6.06"),
                (19, "7.07"),
                (20, "8.08"),
                (21, "9.09"),
                (22, "45.45"),
                (23, "10.10"),
                (24, "11.11"),
                (25, "21.21"),
                (28, "1550.66"),
                (29, "1.23"),
                (30, "12.34"),
                (31, "123.45"),
                (33, "10.02"),
                (34, "20.03"),
                (35, "30.04"),
                (36, "40.05"),
                (37, "50.06"),
                (38, "60.07"),
                (39, "70.08"),
                (40, "80.09"),
                (41, "1360.45"),
                (42, "90.10"),
                (43, "100.11"),
                (44, "190.21"),
                (45, "1617.32"),
                (46, "211.42"),
                (47, "12.12"),
                (48, "13.13"),
                (49, "14.14"),
                (50, "15.15"),
                (51, "16.16"),
                (52, "17.17"),
                (57, "18.18"),
                (58, "1000.01"),
            ],
        };
        for (column, value) in amounts {
            row[column - 6] = value.into();
        }
        let mut namesake = declaration(schedule);
        namesake[0] = "234567891".into();
        let rows = vec![row, namesake];
        for excel in [false, true] {
            let dir = root.join(format!(
                "schedule{}-{}",
                if schedule == Schedule::One { 1 } else { 2 },
                if excel { "xlsx" } else { "csv" }
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let bytes = if excel {
                crate::tabular::text_xlsx(headers(schedule), &rows).unwrap()
            } else {
                csv(schedule, &rows)
            };
            let draft = assemble(&bytes, excel, &basis(&employer(), schedule)).unwrap();
            save(
                &dir.join(if excel {
                    "declarations.xlsx"
                } else {
                    "declarations.csv"
                }),
                &bytes,
            );
            save(&dir.join(&draft.preview.file_name), &draft.bytes);
            save(
                &dir.join("preview.json"),
                &serde_json::to_vec_pretty(&draft.preview).unwrap(),
            );
            save(
                &dir.join("blank-template.csv"),
                &template(schedule, false).unwrap(),
            );
            save(
                &dir.join("blank-template.xlsx"),
                &template(schedule, true).unwrap(),
            );
        }
    }
}
