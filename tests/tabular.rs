use calamine::{Data, Reader, Xlsx};
use hris_desktop::{
    domain::DemoStore,
    imports, reports,
    tabular::{self, ColumnKind::*},
};
use rust_xlsxwriter::{ExcelDateTime, Format, Formula, Workbook};
use std::{
    io::{Cursor, Read, Write},
    path::PathBuf,
};

fn base() -> Vec<u8> {
    tabular::text_xlsx(
        &["employee_code", "full_name"],
        &[vec!["EX-001".into(), "Synthetic Excel Employee".into()]],
    )
    .unwrap()
}
fn edit(bytes: &[u8], path: &str, change: impl FnOnce(String) -> String) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut change = Some(change);
    for i in 0..archive.len() {
        let mut part = archive.by_index(i).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut data = Vec::new();
        part.read_to_end(&mut data).unwrap();
        if name == path {
            data = change.take().unwrap()(String::from_utf8(data).unwrap()).into_bytes();
        }
        out.start_file(name, options).unwrap();
        out.write_all(&data).unwrap();
    }
    assert!(change.is_none());
    out.finish().unwrap().into_inner()
}
fn load(bytes: &[u8]) -> anyhow::Result<Vec<tabular::Row>> {
    tabular::read_xlsx(bytes, 500, 16)
}
fn temp(name: &str) -> PathBuf {
    let root = std::env::current_dir()
        .unwrap()
        .join("work/tabular-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).unwrap();
    root.join(name)
}

#[test]
fn excel_roundtrip_retains_identifiers_unicode_multiline_and_literal_formula_like_text() {
    let headers = ["employee_code", "full_name", "phone", "address"];
    let values = vec![vec![
        "00000012".into(),
        "=Synthetic Ñame".into(),
        "+639000000001".into(),
        "Fictional\nSecond line & <value>".into(),
    ]];
    let bytes = tabular::text_xlsx(&headers, &values).unwrap();
    let rows = load(&bytes).unwrap();
    assert_eq!(rows[0].number, 1);
    assert_eq!(rows[1].number, 2);
    assert_eq!(rows[1].cells, values[0]);
    let mut other = Xlsx::new(Cursor::new(&bytes)).unwrap();
    let formulas = other.worksheet_formula("HRIS data").unwrap();
    assert!(formulas.used_cells().all(|(_, _, f)| f.is_empty()));
}
#[test]
fn numbers_are_typed_only_for_explicit_columns_and_half_days_are_exact() {
    let bytes = tabular::xlsx(
        &["employee_code", "available_days", "count"],
        &[vec!["0001".into(), "2.5".into(), "42".into()]],
        &[Text, HalfDays, Integer],
    )
    .unwrap();
    let mut workbook = Xlsx::new(Cursor::new(bytes)).unwrap();
    let data = workbook.worksheet_range("HRIS data").unwrap();
    assert!(matches!(data.get_value((1,0)),Some(Data::String(s))if s=="0001"));
    assert!(matches!(data.get_value((1,1)),Some(Data::Float(n))if *n==2.5));
    for number in ["NaN", "inf", "2.25", "10000000000000000000"] {
        assert!(
            tabular::xlsx(&["days"], &[vec![number.into()]], &[HalfDays]).is_err(),
            "{number}"
        );
    }
    assert!(tabular::xlsx(&["count"], &[vec!["1.5".into()]], &[Integer]).is_err());
}
#[test]
fn exported_dates_are_native_calendar_cells_with_an_unambiguous_iso_display() {
    let bytes = tabular::xlsx(&["Work date"], &[vec!["2026-10-26".into()]], &[Date]).unwrap();
    let mut workbook = Xlsx::new(Cursor::new(bytes)).unwrap();
    let sheet = workbook.worksheet_range("HRIS data").unwrap();
    assert!(
        matches!(sheet.get_value((1,0)),Some(Data::DateTime(d))if d.as_datetime().unwrap().date().to_string()=="2026-10-26")
    );
    assert!(tabular::xlsx(&["Work date"], &[vec!["2026-02-30".into()]], &[Date]).is_err());
}
#[test]
fn real_worksheet_row_numbers_survive_blank_rows_and_multiline_cells() {
    let mut book = Workbook::new();
    let s = book.add_worksheet();
    s.write_string(0, 0, "employee_code").unwrap();
    s.write_string(0, 1, "full_name").unwrap();
    s.write_string(4, 0, "NEW-EX-4").unwrap();
    s.write_string(4, 1, "Synthetic\nMultiline").unwrap();
    let bytes = book.save_to_buffer().unwrap();
    let rows = load(&bytes).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].number, 5);
    let file = temp("source.xlsx");
    std::fs::write(&file, bytes).unwrap();
    let p = imports::Preview::load(&DemoStore::new().data, &file).unwrap();
    assert_eq!(p.rows[0].source_row, 5);
    std::fs::remove_file(file).unwrap();
}
#[test]
fn formulas_are_rejected_even_with_cached_results_or_a_foreign_namespace() {
    let source = base();
    for formula in ["<f>1+1</f>", "<f/>", "<f xmlns=\"urn:untrusted\">1+1</f>"] {
        let bad = edit(&source, "xl/worksheets/sheet1.xml", |s| {
            s.replace("<c r=\"A2\"", &format!("<c r=\"A2\""))
                .replace("<v>2</v>", &format!("{formula}<v>2</v>"))
        });
        // Shared string indexes vary by library version; inject into the cell itself.
        let bad = edit(&bad, "xl/worksheets/sheet1.xml", |s| {
            s.replace("<row r=\"2\"", &format!("{formula}<row r=\"2\""))
        });
        assert!(load(&bad).is_err());
    }
    let mut book = Workbook::new();
    let s = book.add_worksheet();
    s.write_string(0, 0, "employee_code").unwrap();
    s.write_formula(1, 0, Formula::new("=1+1").set_result("2"))
        .unwrap();
    assert!(load(&book.save_to_buffer().unwrap()).is_err());
}
#[test]
fn extreme_dimensions_hidden_rows_merges_and_untrusted_cell_namespace_are_rejected() {
    let source = base();
    for (old, new) in [
        ("ref=\"A1:B2\"", "ref=\"A1:XFD1048576\""),
        ("<row r=\"2\"", "<row hidden=\"1\" r=\"2\""),
        (
            "</sheetData>",
            "</sheetData><mergeCells count=\"1\"><mergeCell ref=\"A1:B1\"/></mergeCells>",
        ),
        ("<c r=\"A2\"", "<c xmlns=\"urn:untrusted\" r=\"A2\""),
        ("<row r=\"2\"", "<row r=\"501000\""),
    ] {
        let bad = edit(&source, "xl/worksheets/sheet1.xml", |s| {
            assert!(s.contains(old));
            s.replace(old, new)
        });
        assert!(load(&bad).is_err(), "{new}");
    }
}
#[test]
fn multiple_or_hidden_sheets_and_external_links_are_rejected() {
    let mut workbook = Workbook::new();
    workbook
        .add_worksheet()
        .write_string(0, 0, "employee_code")
        .unwrap();
    workbook
        .add_worksheet()
        .write_string(0, 0, "other")
        .unwrap();
    assert!(load(&workbook.save_to_buffer().unwrap()).is_err());
    let hidden = edit(&base(), "xl/workbook.xml", |s| {
        s.replace("sheetId=\"1\"", "state=\"hidden\" sheetId=\"1\"")
    });
    assert!(load(&hidden).is_err());
    let external = edit(&base(), "xl/_rels/workbook.xml.rels", |s| {
        s.replace(
            "<Relationship Id=",
            "<Relationship TargetMode=\"External\" Id=",
        )
    });
    assert!(load(&external).is_err());
}
#[test]
fn manager_excel_report_contains_only_the_current_own_and_team_cohort() {
    let mut data = DemoStore::new().data;
    data.access.role = hris_desktop::access::Role::Manager;
    data.access.employee_id = Some(data.employees[0].id);
    data.access.direct_reports = vec![data.employees[1].id];
    let expected = vec![
        data.employees[0].employee_code.clone(),
        data.employees[1].employee_code.clone(),
    ];
    let mut filter = reports::Filter::new(data.company.today());
    filter.kind = reports::Kind::Summary;
    filter.columns = Some(vec!["Employee code".into()]);
    let report = reports::build(&data, &filter, chrono::Utc::now()).unwrap();
    let mut workbook = Xlsx::new(Cursor::new(report.xlsx().unwrap())).unwrap();
    let sheet = workbook.worksheet_range("HRIS data").unwrap();
    assert_eq!(sheet.height(), 3);
    for row in sheet.rows().skip(1) {
        assert!(matches!(&row[6],Data::String(code)if expected.contains(code)));
    }
    filter.employee_id = Some(data.employees[2].id);
    assert!(reports::build(&data, &filter, chrono::Utc::now()).is_err());
}
#[test]
fn dtds_custom_entities_macro_parts_and_zip_expansion_are_rejected() {
    let source = base();
    let bad = edit(&source, "xl/sharedStrings.xml", |s| {
        s.replace(
            "<sst ",
            "<!DOCTYPE sst [<!ENTITY private 'unexpected'>]><sst ",
        )
    });
    assert!(load(&bad).is_err());
    let bad = edit(&source, "[Content_Types].xml", |s| {
        s.replace(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
            "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
        )
    });
    assert!(load(&bad).is_err());
    let legacy_macro = edit(&source, "[Content_Types].xml", |s| {
        s.replace(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
            "application/vnd.ms-excel.macrosheet+xml",
        )
    });
    assert!(load(&legacy_macro).is_err());
    let expanded = edit(&source, "xl/sharedStrings.xml", |_| {
        " ".repeat(8 * 1024 * 1024 + 1)
    });
    assert!(expanded.len() < tabular::MAX_IMPORT_BYTES);
    assert!(load(&expanded).is_err());
    assert!(load(&vec![0; tabular::MAX_IMPORT_BYTES + 1]).is_err());
}
#[test]
fn excessive_declared_string_or_style_counts_are_refused_before_library_allocation() {
    let source = base();
    for (path, old, new) in [
        (
            "xl/sharedStrings.xml",
            "uniqueCount=\"4\"",
            "uniqueCount=\"1000000000\"",
        ),
        (
            "xl/styles.xml",
            "<cellXfs count=",
            "<cellXfs count=\"1000000000\" originalCount=",
        ),
    ] {
        let bad = edit(&source, path, |s| {
            assert!(s.contains(old));
            s.replace(old, new)
        });
        assert!(load(&bad).is_err());
    }
    let bad = edit(&source, "xl/sharedStrings.xml", |s| {
        s.replace(
            "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
            "urn:untrusted",
        )
        .replace("uniqueCount=\"4\"", "uniqueCount=\"1000000000\"")
    });
    assert!(load(&bad).is_err());
}
#[test]
fn numeric_identifiers_and_boolean_or_error_cells_cannot_silently_change_fields() {
    for invalid in [0, 1, 2] {
        let mut book = Workbook::new();
        let s = book.add_worksheet();
        s.write_string(0, 0, "employee_code").unwrap();
        s.write_string(0, 1, "full_name").unwrap();
        s.write_string(1, 1, "Synthetic").unwrap();
        match invalid {
            0 => {
                s.write_number(1, 0, 1234567890123456.0).unwrap();
            }
            1 => {
                s.write_boolean(1, 0, true).unwrap();
            }
            _ => {
                s.write_formula(1, 0, "=1/0").unwrap();
            }
        }
        assert!(load(&book.save_to_buffer().unwrap()).is_err());
    }
}
#[test]
fn ordinary_excel_employment_dates_are_normalized_without_times_or_leap_day_coercion() {
    for (serial, expected) in [(46023.0, Some("2026-01-01")), (46023.5, None), (60.0, None)] {
        let mut book = Workbook::new();
        let s = book.add_worksheet();
        s.write_string(0, 0, "joined_on").unwrap();
        s.write_number_with_format(1, 0, serial, &Format::new().set_num_format("yyyy-mm-dd"))
            .unwrap();
        let bytes = book.save_to_buffer().unwrap();
        let result = load(&bytes);
        if let Some(date) = expected {
            assert_eq!(result.unwrap()[1].cells[0], date);
        } else {
            assert!(result.is_err());
        }
    }
    let mut book = Workbook::new();
    let s = book.add_worksheet();
    s.write_string(0, 0, "joined_on").unwrap();
    s.write_datetime_with_format(
        1,
        0,
        &ExcelDateTime::from_ymd(2026, 10, 26).unwrap(),
        &Format::new().set_num_format("yyyy-mm-dd"),
    )
    .unwrap();
    assert_eq!(
        load(&book.save_to_buffer().unwrap()).unwrap()[1].cells[0],
        "2026-10-26"
    );
}
#[test]
fn employee_excel_load_uses_existing_validation_and_denies_non_hr() {
    let bytes = tabular::text_xlsx(
        &["employee_code", "full_name", "phone"],
        &[vec![
            "EX-901".into(),
            "Synthetic Excel Employee".into(),
            "000 000 0000".into(),
        ]],
    )
    .unwrap();
    let file = temp("source.XLSX");
    std::fs::write(&file, bytes).unwrap();
    let mut data = DemoStore::new().data;
    assert!(imports::Preview::load(&data, &file).unwrap().valid());
    data.access.role = hris_desktop::access::Role::Manager;
    assert!(imports::Preview::load(&data, &file).is_err());
    std::fs::remove_file(file).unwrap();
}
#[test]
fn excel_openings_reuse_protected_preview_posting_and_all_or_none_validation() {
    use hris_desktop::leave::{
        self,
        dated::{Change, Context},
        opening_import::{BatchPreview, Import, Post},
    };
    let mut data = DemoStore::new().data;
    data.leave.clear();
    data.leave_setup.credits = Some(leave::credits::Context::empty(&data));
    data.leave_setup.dated = Some(Context::empty(&data));
    let code = data.employees[0].employee_code.clone();
    let rows = vec![vec![
        code,
        "annual".into(),
        "2.50".into(),
        "0".into(),
        "Reviewed fictional Excel opening".into(),
        "Fictional register".into(),
    ]];
    let file = temp("openings.xlsx");
    std::fs::write(
        &file,
        tabular::text_xlsx(&leave::opening_import::HEADERS, &rows).unwrap(),
    )
    .unwrap();
    let source = Import::load(&data, &file).unwrap();
    assert_eq!(source.rows[0].line, 2);
    assert!(source.records(&data, false).is_err());
    let records = source.records(&data, true).unwrap();
    let preview = BatchPreview::demo(&data, &records);
    assert!(preview.ready());
    preview.validate(&data, &records).unwrap();
    let change = Change::OpeningBatch(Box::new(Post {
        id: source.id,
        preview,
    }));
    leave::dated::apply_demo(&mut data, &change).unwrap();
    leave::dated::apply_demo(&mut data, &change).unwrap();
    assert_eq!(data.leave_setup.dated.as_ref().unwrap().openings.len(), 1);
    let mut invalid = rows.clone();
    invalid.push(vec![
        "MISSING".into(),
        "annual".into(),
        "1.25".into(),
        "0".into(),
        "Reviewed fictional row".into(),
        "Fictional register".into(),
    ]);
    std::fs::write(
        &file,
        tabular::text_xlsx(&leave::opening_import::HEADERS, &invalid).unwrap(),
    )
    .unwrap();
    let rejected = Import::load(&data, &file).unwrap();
    assert!(rejected.records(&data, true).is_err());
    assert_eq!(data.leave_setup.dated.as_ref().unwrap().openings.len(), 1);
    let error = rejected.error_xlsx().unwrap();
    let mut workbook = Xlsx::new(Cursor::new(error)).unwrap();
    let sheet = workbook.worksheet_range("HRIS data").unwrap();
    assert_eq!(sheet.height(), 3);
    assert_eq!(sheet.get_value((2, 0)), Some(&Data::String("3".into())));
    std::fs::remove_file(file).unwrap();
}
#[test]
fn templates_are_real_empty_workbooks_and_saving_preserves_existing_files() {
    for bytes in [
        imports::template_xlsx().unwrap(),
        hris_desktop::leave::opening_import::template_xlsx().unwrap(),
    ] {
        assert_eq!(load(&bytes).unwrap().len(), 1);
    }
    let data = DemoStore::new().data;
    let filter = reports::Filter::new(data.company.today());
    let report = reports::build(&data, &filter, chrono::Utc::now()).unwrap();
    let file = temp("report.xlsx");
    report.save(&file).unwrap();
    let before = std::fs::read(&file).unwrap();
    assert!(report.save(&file).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), before);
    let mut workbook = Xlsx::new(Cursor::new(&before)).unwrap();
    let rows = workbook.worksheet_range("HRIS data").unwrap();
    assert_eq!(rows.height(), report.rows.len() + 1);
    assert_eq!(rows.width(), report.headers.len() + 6);
    std::fs::remove_file(file).unwrap();
}
#[test]
fn every_report_and_custom_column_selection_exports_only_its_scoped_rows() {
    let data = DemoStore::new().data;
    for kind in reports::Kind::ALL {
        let mut filter = reports::Filter::new(data.company.today());
        filter.kind = kind;
        filter.columns = Some(
            reports::available_columns(kind)
                .into_iter()
                .rev()
                .take(3)
                .map(str::to_owned)
                .collect(),
        );
        let report = reports::build(&data, &filter, chrono::Utc::now()).unwrap();
        let bytes = report.xlsx().unwrap();
        let mut workbook = Xlsx::new(Cursor::new(bytes)).unwrap();
        let data = workbook.worksheet_range("HRIS data").unwrap();
        assert_eq!(data.width(), 9);
        assert_eq!(data.height(), report.rows.len() + 1);
        let headers = data.rows().next().unwrap();
        for (i, header) in report.headers.iter().enumerate() {
            assert_eq!(headers[i + 6], Data::String((*header).into()));
        }
    }
}
