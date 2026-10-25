//! Bounded native Excel exchange. Workbooks are data, never executable input.
use anyhow::{Context, Result, bail, ensure};
use calamine::{Data, HeaderRow, Reader, SheetVisible, Xlsx};
use chrono::{Datelike, Timelike};
use quick_xml::{XmlVersion, events::Event, name::ResolveResult, reader::NsReader};
use rust_xlsxwriter::{Color, ExcelDateTime, Format, FormatAlign, Workbook};
use std::{
    collections::BTreeSet,
    io::{Cursor, Read},
    path::Path,
};

pub const MAX_IMPORT_BYTES: usize = 2 * 1024 * 1024;
const MAX_INFLATED_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PART_BYTES: u64 = 8 * 1024 * 1024;
const NS: &[&[u8]] = &[
    b"http://schemas.openxmlformats.org/spreadsheetml/2006/main",
    b"http://purl.oclc.org/ooxml/spreadsheetml/main",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColumnKind {
    #[default]
    Text,
    Integer,
    HalfDays,
    Date,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub number: usize,
    pub cells: Vec<String>,
}

fn valid_xml(text: &str) -> bool {
    text.chars().all(|c| {
        matches!(c, '\t' | '\r' | '\n') || (c >= ' ' && c != '\u{fffe}' && c != '\u{ffff}')
    })
}
fn coordinate(text: &str, max_row: usize, max_col: usize) -> Result<(usize, usize)> {
    let split = text
        .bytes()
        .position(|b| b.is_ascii_digit())
        .context("Invalid Excel cell address.")?;
    let (col, row) = text.split_at(split);
    ensure!(
        !col.is_empty()
            && col.len() <= 3
            && col.bytes().all(|b| b.is_ascii_uppercase())
            && !row.starts_with('0')
            && row.bytes().all(|b| b.is_ascii_digit()),
        "Invalid Excel cell address."
    );
    let col = col
        .bytes()
        .fold(0usize, |n, b| n * 26 + usize::from(b - b'A' + 1));
    let row = row.parse::<usize>().context("Invalid Excel row number.")?;
    ensure!(
        row > 0 && row <= max_row && col > 0 && col <= max_col,
        "Excel data or formatting exceeds the import row/column limit. Use the template and split large files."
    );
    Ok((row, col))
}

fn inspect_xml(bytes: &[u8], max_row: usize, max_col: usize) -> Result<(usize, usize)> {
    let text = std::str::from_utf8(bytes).context("Use XLSX with UTF-8 XML parts.")?;
    ensure!(
        valid_xml(text),
        "The workbook contains invalid XML characters."
    );
    let mut reader = NsReader::from_str(text);
    let (mut depth, mut roots, mut sheets, mut worksheets) = (0usize, 0, 0, 0);
    let mut rows = BTreeSet::new();
    let mut cells = BTreeSet::new();
    let mut current_row = None;
    let mut worksheet = false;
    let mut parents: Vec<Vec<u8>> = Vec::new();
    let mut sheet_data_count = 0usize;
    loop {
        let (ns, event) = reader.read_resolved_event()?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                let spreadsheet =
                    matches!(ns, ResolveResult::Bound(ref n) if NS.contains(&n.as_ref()));
                let local = element.local_name();
                ensure!(
                    local.as_ref() != b"f",
                    "Excel formulas and cached formula results cannot be imported. Paste reviewed values first."
                );
                if worksheet
                    && matches!(
                        local.as_ref(),
                        b"row" | b"c" | b"dimension" | b"col" | b"mergeCell" | b"mergeCells"
                    )
                {
                    ensure!(spreadsheet, "Unsupported worksheet element namespace.");
                }
                if depth == 0 {
                    roots += 1;
                    ensure!(roots == 1, "The workbook XML has multiple roots.");
                    worksheet = spreadsheet && local.as_ref() == b"worksheet";
                    worksheets += usize::from(worksheet);
                }
                let attributes = element
                    .attributes()
                    .map(|a| {
                        let a = a?;
                        Ok((
                            a.key.as_ref().to_vec(),
                            a.normalized_value(XmlVersion::Implicit1_0)?.into_owned(),
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let attr = |key: &[u8]| {
                    attributes
                        .iter()
                        .find(|(k, _)| k == key)
                        .map(|(_, v)| v.as_str())
                };
                // The reader accepts some metadata by local name. Bound allocation
                // hints even when an attacker substitutes a foreign namespace.
                for key in [
                    b"count".as_slice(),
                    b"uniqueCount",
                    b"numFmtId",
                    b"xfId",
                    b"s",
                    b"style",
                ] {
                    if let Some(value) = attr(key) {
                        ensure!(
                            value.parse::<u32>().is_ok_and(|n| n <= 100_000),
                            "An Excel string/style declaration exceeds import limits."
                        );
                    }
                }
                ensure!(
                    !attributes.iter().any(|(_, v)| v.contains("macroEnabled")
                        || v.to_ascii_lowercase().contains("macrosheet")
                        || v.contains("vbaProject")
                        || v.contains("oleObject")),
                    "Macro-enabled or embedded executable workbooks are unsupported."
                );
                ensure!(
                    !attr(b"TargetMode").is_some_and(|s| s.eq_ignore_ascii_case("External")),
                    "External workbook links/connections are unsupported. Paste reviewed values into the template."
                );
                if spreadsheet {
                    ensure!(
                        local.as_ref() != b"f",
                        "Excel formulas and cached formula results cannot be imported. Paste reviewed values first."
                    );
                    if local.as_ref() == b"sheet" {
                        sheets += 1;
                        ensure!(
                            attr(b"state").is_none_or(|s| s == "visible"),
                            "Hidden worksheets cannot be imported."
                        );
                    }
                    if worksheet {
                        match local.as_ref() {
                            b"dimension" => {
                                for end in attr(b"ref")
                                    .context("Invalid worksheet dimension.")?
                                    .split(':')
                                {
                                    coordinate(end, max_row, max_col)?;
                                }
                            }
                            b"row" => {
                                ensure!(
                                    parents.last().is_some_and(|p| p == b"sheetData")
                                        && current_row.is_none(),
                                    "An Excel row is outside the data table."
                                );
                                ensure!(!attr(b"hidden").is_some_and(|s| s == "1" || s.eq_ignore_ascii_case("true")), "Hidden data rows cannot be imported.");
                                let r = attr(b"r")
                                    .context("Use a worksheet with explicit row addresses.")?
                                    .parse::<usize>()?;
                                ensure!(
                                    r > 0 && r <= max_row && rows.insert(r),
                                    "Duplicate or excessive Excel row addresses."
                                );
                                current_row = Some(r);
                            }
                            b"c" => {
                                ensure!(
                                    parents.last().is_some_and(|p| p == b"row"),
                                    "An Excel cell is outside its data row."
                                );
                                let address = attr(b"r")
                                    .context("Use a worksheet with explicit cell addresses.")?;
                                let (r, _) = coordinate(address, max_row, max_col)?;
                                ensure!(
                                    current_row == Some(r) && cells.insert(address.to_owned()),
                                    "Duplicate or mismatched Excel cell address."
                                );
                            }
                            b"col" => {
                                ensure!(!attr(b"hidden").is_some_and(|s| s == "1" || s.eq_ignore_ascii_case("true")), "Hidden columns cannot be imported.");
                                for key in [b"min".as_slice(), b"max"] {
                                    if let Some(value) = attr(key) {
                                        ensure!(
                                            value
                                                .parse::<usize>()
                                                .is_ok_and(|n| n > 0 && n <= max_col),
                                            "Excel column formatting exceeds import limits."
                                        );
                                    }
                                }
                            }
                            b"mergeCell" | b"mergeCells" => bail!(
                                "Merged cells cannot be imported. Use one value per template cell."
                            ),
                            b"sheetData" => {
                                sheet_data_count += 1;
                                ensure!(
                                    sheet_data_count == 1
                                        && parents.last().is_some_and(|p| p == b"worksheet"),
                                    "Use a single worksheet data table."
                                );
                            }
                            _ => {}
                        }
                    }
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                    parents.push(local.as_ref().to_vec());
                } else if worksheet && local.as_ref() == b"row" {
                    current_row = None;
                }
                ensure!(depth <= 64, "Workbook XML nesting is excessive.");
            }
            Event::End(ref end) => {
                if worksheet && end.local_name().as_ref() == b"row" {
                    current_row = None;
                }
                depth = depth
                    .checked_sub(1)
                    .context("Unexpected Excel XML end tag.")?;
                parents.pop().context("Unexpected Excel XML nesting.")?;
            }
            Event::DocType(_) => bail!("Workbook document type declarations are unsupported."),
            Event::GeneralRef(ref entity) => ensure!(
                matches!(&entity[..], b"amp" | b"lt" | b"gt" | b"quot" | b"apos")
                    || entity
                        .resolve_char_ref()?
                        .is_some_and(|c| valid_xml(&c.to_string())),
                "Unsupported workbook XML entity."
            ),
            Event::Decl(ref decl) => ensure!(
                decl.encoding().transpose()?.is_none_or(
                    |e| e.eq_ignore_ascii_case(b"utf-8") || e.eq_ignore_ascii_case(b"us-ascii")
                ),
                "Use UTF-8 workbook XML parts."
            ),
            Event::PI(_) => bail!("Workbook XML processing instructions are unsupported."),
            Event::Text(ref text) => ensure!(
                depth > 0 || text.iter().all(u8::is_ascii_whitespace),
                "Text outside the workbook XML root."
            ),
            Event::CData(_) => ensure!(depth > 0, "Content outside the workbook XML root."),
            Event::Eof => {
                ensure!(roots == 1 && depth == 0, "Incomplete workbook XML.");
                break;
            }
            _ => {}
        }
    }
    Ok((sheets, worksheets))
}

fn preflight(bytes: &[u8], max_rows: usize, max_columns: usize) -> Result<()> {
    ensure!(
        bytes.len() <= MAX_IMPORT_BYTES,
        "Excel import exceeds 2 MiB. Split the file."
    );
    ensure!(
        (1..=500).contains(&max_rows) && (1..=64).contains(&max_columns),
        "Invalid workbook import bounds."
    );
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .context("Choose an ordinary .xlsx workbook, not an encrypted, legacy or renamed file.")?;
    ensure!(
        (1..=128).contains(&archive.len()),
        "The workbook has too many parts."
    );
    let (mut total, mut sheets, mut worksheets) = (0u64, 0usize, 0usize);
    let mut names = BTreeSet::new();
    for i in 0..archive.len() {
        let mut part = archive.by_index(i)?;
        let name = part.name()?.into_owned();
        ensure!(
            !name.contains('\\')
                && !name.starts_with('/')
                && name.split('/').all(|s| s != ".." && s != ".")
                && names.insert(name.clone()),
            "Unsafe or duplicate workbook part name."
        );
        let lower = name.to_ascii_lowercase();
        ensure!(
            !lower.ends_with(".bin")
                && !lower.contains("vbaproject")
                && !lower.starts_with("xl/externallinks/")
                && !lower.starts_with("xl/embeddings/")
                && !lower.starts_with("xl/activex/")
                && !lower.starts_with("xl/macrosheets/")
                && !lower.starts_with("xl/querytables/")
                && lower != "xl/connections.xml",
            "Macros, embedded files and external data connections cannot be imported."
        );
        ensure!(
            matches!(
                part.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            ),
            "Unsupported workbook compression."
        );
        total = total
            .checked_add(part.size())
            .context("Workbook size overflow.")?;
        ensure!(
            part.size() <= MAX_PART_BYTES && total <= MAX_INFLATED_BYTES,
            "The expanded workbook exceeds import limits. Use the template and split large files."
        );
        let mut data = Vec::new();
        (&mut part)
            .take(MAX_PART_BYTES + 1)
            .read_to_end(&mut data)?;
        ensure!(
            data.len() as u64 == part.size() && data.len() as u64 <= MAX_PART_BYTES,
            "Workbook part size mismatch."
        );
        if lower.ends_with(".xml") || lower.ends_with(".rels") {
            let (s, w) = inspect_xml(&data, max_rows + 1, max_columns)
                .with_context(|| format!("Invalid workbook part: {name}"))?;
            sheets += s;
            worksheets += w;
        }
    }
    ensure!(
        names.contains("[Content_Types].xml")
            && names.contains("xl/workbook.xml")
            && sheets == 1
            && worksheets == 1,
        "Use one visible data worksheet with template headers in row 1."
    );
    Ok(())
}

fn cell_text(cell: &Data, header: &str, row: usize, column: usize) -> Result<String> {
    let numeric = matches!(
        header,
        "annual_leave_days" | "available_days" | "remaining_carry_days"
    );
    let text = match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Int(n) if numeric => n.to_string(),
        Data::Float(n) if numeric && n.is_finite() && n.abs() <= 1_000_000.0 => n.to_string(),
        Data::DateTime(d) if header == "joined_on" && !d.is_duration() => {
            ensure!(
                d.as_f64().fract() == 0.0,
                "Excel row {row}, column {column}: joined_on must be a date without a time."
            );
            let serial = d.as_f64();
            let d = d.as_datetime().context("Invalid Excel date.")?;
            ensure!(
                serial.floor() != 60.0 || d.date().format("%Y").to_string() != "1900",
                "Excel's fictitious 1900 leap day cannot be imported."
            );
            ensure!(
                d.time().num_seconds_from_midnight() == 0 && d.time().nanosecond() == 0,
                "Excel row {row}, column {column}: joined_on must be a date without a time."
            );
            d.date().to_string()
        }
        Data::DateTimeIso(s) if header == "joined_on" => {
            if let Ok(date) = s.parse::<chrono::NaiveDate>() {
                date.to_string()
            } else {
                let date = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
                    .context("Use an ISO employment date without a timezone or time.")?;
                ensure!(
                    date.time().num_seconds_from_midnight() == 0 && date.time().nanosecond() == 0,
                    "joined_on must be a date without a time."
                );
                date.date().to_string()
            }
        }
        _ => bail!(
            "Excel row {row}, column {column}: use template text cells for identifiers/contact details and reviewed numbers only in quantity columns. Formulas, errors, booleans and ambiguous dates/times are unsupported."
        ),
    };
    ensure!(
        text.len() <= 4096 && valid_xml(&text),
        "Excel row {row}, column {column}: a field exceeds 4 KiB or contains an invalid character."
    );
    Ok(text)
}

pub fn read_xlsx(bytes: &[u8], max_rows: usize, max_columns: usize) -> Result<Vec<Row>> {
    preflight(bytes, max_rows, max_columns)?;
    let mut workbook =
        Xlsx::new(Cursor::new(bytes)).context("Could not read the Excel workbook.")?;
    workbook.with_header_row(HeaderRow::Row(0));
    let metadata = workbook.sheets_metadata();
    ensure!(
        metadata.len() == 1 && metadata[0].visible == SheetVisible::Visible,
        "Use one visible worksheet."
    );
    let name = workbook.sheet_names()[0].clone();
    let range = workbook.worksheet_range(&name)?;
    ensure!(
        range.start() == Some((0, 0)),
        "Put template headers in row 1, starting in column A."
    );
    let (height, width) = range.get_size();
    ensure!(
        height <= max_rows + 1 && width > 0 && width <= max_columns,
        "The worksheet exceeds import row/column limits."
    );
    let headers = range
        .rows()
        .next()
        .context("Include template headers.")?
        .iter()
        .enumerate()
        .map(|(i, c)| match c {
            Data::String(s) => Ok(s.trim().to_lowercase()),
            _ => bail!("Excel header column {} must contain template text.", i + 1),
        })
        .collect::<Result<Vec<_>>>()?;
    let mut rows = Vec::new();
    for (index, row) in range.rows().enumerate() {
        if row.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }
        let cells = row
            .iter()
            .enumerate()
            .map(|(col, c)| cell_text(c, &headers[col], index + 1, col + 1))
            .collect::<Result<Vec<_>>>()?;
        rows.push(Row {
            number: index + 1,
            cells,
        });
    }
    ensure!(!rows.is_empty(), "Include template headers.");
    Ok(rows)
}

pub fn load_xlsx(path: &Path, max_rows: usize, max_columns: usize) -> Result<Vec<Row>> {
    ensure!(
        path.extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("xlsx")),
        "Choose an .xlsx workbook."
    );
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((MAX_IMPORT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    read_xlsx(&bytes, max_rows, max_columns)
}

pub fn xlsx(headers: &[&str], rows: &[Vec<String>], kinds: &[ColumnKind]) -> Result<Vec<u8>> {
    ensure!(
        !headers.is_empty()
            && headers.len() <= 64
            && kinds.len() == headers.len()
            && rows.len() <= 50_000
            && (rows.len() + 1) * headers.len() <= 2_000_000,
        "Excel export exceeds the supported row/column limits. Narrow the report."
    );
    ensure!(
        rows.iter().all(|r| r.len() == headers.len()),
        "Excel columns do not match. Export was blocked."
    );
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("HRIS data")?;
    sheet.set_freeze_panes(1, 0)?;
    let header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x213C50))
        .set_font_color(Color::White)
        .set_text_wrap();
    let text_format = Format::new()
        .set_num_format("@")
        .set_align(FormatAlign::Top);
    let integer_format = Format::new().set_num_format("0");
    let half_format = Format::new().set_num_format("0.0");
    let date_format = Format::new().set_num_format("yyyy-mm-dd");
    for (i, header) in headers.iter().enumerate() {
        ensure!(
            valid_xml(header) && header.chars().count() <= 32767,
            "An Excel header is invalid."
        );
        sheet.write_string_with_format(0, i as u16, *header, &header_format)?;
        let width = rows
            .iter()
            .take(200)
            .map(|r| r[i].chars().count())
            .chain(std::iter::once(header.chars().count()))
            .max()
            .unwrap_or(12)
            .clamp(12, 48);
        sheet.set_column_width(i as u16, width as f64 + 2.0)?;
        sheet.set_column_format(
            i as u16,
            match kinds[i] {
                ColumnKind::Text => &text_format,
                ColumnKind::Integer => &integer_format,
                ColumnKind::HalfDays => &half_format,
                ColumnKind::Date => &date_format,
            },
        )?;
    }
    sheet.set_row_height(0, 32.0)?;
    sheet.autofilter(0, 0, rows.len() as u32, (headers.len() - 1) as u16)?;
    for (row, values) in rows.iter().enumerate() {
        for (col, text) in values.iter().enumerate() {
            ensure!(
                valid_xml(text) && text.chars().count() <= 32767,
                "Excel row {}, column {} contains invalid or excessive text. Export was blocked.",
                row + 2,
                col + 1
            );
            if kinds[col] == ColumnKind::Text || text.is_empty() {
                sheet.write_string_with_format((row + 1) as u32, col as u16, text, &text_format)?;
            } else if kinds[col] == ColumnKind::Date {
                let date = text
                    .parse::<chrono::NaiveDate>()
                    .context("An Excel source date is invalid.")?;
                let date = ExcelDateTime::from_ymd(
                    u16::try_from(date.year())?,
                    date.month() as u8,
                    date.day() as u8,
                )?;
                sheet.write_datetime_with_format(
                    (row + 1) as u32,
                    col as u16,
                    &date,
                    &date_format,
                )?;
            } else {
                let n = text
                    .parse::<f64>()
                    .context("An Excel numeric source field is invalid.")?;
                ensure!(
                    n.is_finite()
                        && n.abs() <= 1_000_000_000_000.0
                        && match kinds[col] {
                            ColumnKind::Integer => n.fract() == 0.0,
                            ColumnKind::HalfDays => (n * 2.0).fract() == 0.0,
                            ColumnKind::Text => false,
                            ColumnKind::Date => false,
                        },
                    "An Excel quantity cannot be represented safely. Export was blocked."
                );
                sheet.write_number_with_format(
                    (row + 1) as u32,
                    col as u16,
                    n,
                    if kinds[col] == ColumnKind::Integer {
                        &integer_format
                    } else {
                        &half_format
                    },
                )?;
            }
        }
    }
    Ok(workbook.save_to_buffer()?)
}

pub fn text_xlsx(headers: &[&str], rows: &[Vec<String>]) -> Result<Vec<u8>> {
    xlsx(headers, rows, &vec![ColumnKind::Text; headers.len()])
}

pub fn is_xlsx(path: &Path) -> bool {
    path.extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("xlsx"))
}
