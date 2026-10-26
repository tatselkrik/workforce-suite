//! Read-only preflight of the published 1604C Schedule 1 plaintext structure.
//! This is not the official validator, a tax computation or payroll reconciliation.
use anyhow::{Context, Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, io::Read, path::Path};
use uuid::Uuid;

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_DETAILS: usize = 500;
const MAX_ISSUES: usize = 500;
pub const MONEY_COLUMNS: [usize; 31] = [
    12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37,
    38, 39, 40, 41, 42, 43, 48,
];
pub const FIELDS: [&str; 49] = [
    "Schedule",
    "Form",
    "Employer TIN",
    "Employer branch",
    "Return period",
    "Sequence",
    "Employee TIN",
    "Employee branch",
    "Last name",
    "First name",
    "Middle name",
    "Region",
    "Previous gross compensation",
    "Previous exempt basic",
    "Previous exempt 13th-month/benefits",
    "Previous de minimis",
    "Previous exempt contributions/dues",
    "Previous other exempt salaries",
    "Previous exempt total",
    "Previous taxable basic",
    "Previous taxable 13th-month/benefits",
    "Previous other taxable salaries",
    "Previous taxable total",
    "Employment from",
    "Employment to",
    "Present gross compensation",
    "Present exempt basic",
    "Present exempt 13th-month/benefits",
    "Present de minimis",
    "Present exempt contributions/dues",
    "Present other exempt salaries",
    "Present exempt total",
    "Present taxable basic",
    "Present taxable 13th-month/benefits",
    "Present other taxable salaries",
    "Present taxable total",
    "Combined gross compensation",
    "Net taxable compensation",
    "Declared tax due",
    "Previous withholding",
    "Present withholding",
    "December withholding",
    "Overwithheld refund",
    "Actual withholding",
    "Nationality",
    "Employment status",
    "Separation reason",
    "Substituted filing",
    "PERA tax credit",
];

#[derive(Clone, Debug)]
pub struct Employer {
    pub tin: String,
    pub branch: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Error,
    Review,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Issue {
    pub line: usize,
    pub field: String,
    pub severity: Severity,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Total {
    pub field: String,
    pub detail_cents: Option<i64>,
    pub control_cents: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub sha256: String,
    pub file_name: String,
    pub year: Option<i32>,
    pub details: usize,
    pub employer_match: Option<bool>,
    pub errors: usize,
    pub reviews: usize,
    pub issues: Vec<Issue>,
    pub totals: Vec<Total>,
}
impl Report {
    fn issue(&mut self, line: usize, field: &str, severity: Severity, message: &str) {
        if severity == Severity::Error {
            self.errors += 1;
        } else {
            self.reviews += 1;
        }
        if self.issues.len() < MAX_ISSUES {
            self.issues.push(Issue {
                line,
                field: field.into(),
                severity,
                message: message.into(),
            });
        }
    }
    pub fn omitted_issues(&self) -> usize {
        self.errors + self.reviews - self.issues.len()
    }
    pub fn findings_csv(&self) -> Result<Vec<u8>> {
        self.findings_table().csv()
    }
    pub fn findings_export(&self, path: &Path) -> Result<Vec<u8>> {
        self.findings_table().render(path)
    }
    fn findings_table(&self) -> super::export::Table {
        let mut rows = vec![vec![
            "PREFLIGHT_ONLY_NOT_OFFICIAL_VALIDATION".into(),
            self.sha256.clone(),
            self.year.map(|y| y.to_string()).unwrap_or_default(),
            "0".into(),
            "Summary".into(),
            if self.errors == 0 {
                "STRUCTURE_CONSISTENT"
            } else {
                "ERRORS_FOUND"
            }
            .into(),
            format!(
                "{} detail rows; {} errors; {} review items; {} diagnostics omitted. No tax calculation, identity certification or payroll comparison.",
                self.details,
                self.errors,
                self.reviews,
                self.omitted_issues()
            ),
        ]];
        rows.extend(self.issues.iter().map(|i| {
            vec![
                "PREFLIGHT_ONLY_NOT_OFFICIAL_VALIDATION".into(),
                self.sha256.clone(),
                self.year.map(|y| y.to_string()).unwrap_or_default(),
                i.line.to_string(),
                i.field.clone(),
                format!("{:?}", i.severity),
                i.message.clone(),
            ]
        }));
        if self.omitted_issues() > 0 {
            rows.push(vec!["PREFLIGHT_ONLY_NOT_OFFICIAL_VALIDATION".into(),self.sha256.clone(),
            self.year.map(|y|y.to_string()).unwrap_or_default(),"0".into(),"Diagnostics limit".into(),"INCOMPLETE_LIST".into(),
            "More issues exist than can be listed. Fix these and check the source again; no data rows were treated as verified.".into()]);
        }
        super::export::Table {
            headers: [
                "scope",
                "source_sha256",
                "year",
                "line",
                "field",
                "result",
                "message",
            ]
            .map(str::to_owned)
            .to_vec(),
            rows,
        }
    }
}

fn date(v: &str) -> Option<NaiveDate> {
    (v.len() == 10)
        .then(|| NaiveDate::parse_from_str(v, "%m/%d/%Y").ok())
        .flatten()
        .filter(|d| d.format("%m/%d/%Y").to_string() == v)
}
fn digits(v: &str, n: usize) -> bool {
    v.len() == n && v.bytes().all(|b| b.is_ascii_digit())
}
fn tin(v: &str) -> bool {
    digits(v, 9) && !["000000000", "111111111", "999999999"].contains(&v)
}
fn money(v: &str) -> Option<i64> {
    if v.is_empty() || v.len() > 14 {
        return None;
    }
    let mut pieces = v.split('.');
    let whole = pieces.next()?;
    let fraction = pieces.next().unwrap_or("");
    if whole.is_empty()
        || whole.len() > 11
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 2
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || (v.contains('.') && fraction.is_empty())
        || pieces.next().is_some()
    {
        return None;
    }
    let fractional = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().ok()? * 10,
        _ => fraction.parse::<i64>().ok()?,
    };
    whole
        .parse::<i64>()
        .ok()?
        .checked_mul(100)?
        .checked_add(fractional)
}

pub fn read_source(path: &Path) -> Result<Vec<u8>> {
    ensure!(
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dat")),
        "Choose an unvalidated plaintext .dat file."
    );
    let f = std::fs::File::open(path).context("Could not open the BIR source file.")?;
    let before = f.metadata()?;
    ensure!(
        before.is_file() && before.len() <= MAX_BYTES as u64,
        "Choose a regular BIR source file no larger than 2 MiB."
    );
    let mut out = Vec::new();
    let mut reader = f.take(MAX_BYTES as u64 + 1);
    reader.read_to_end(&mut out)?;
    let after = reader.get_ref().metadata()?;
    ensure!(
        out.len() <= MAX_BYTES
            && out.len() == before.len() as usize
            && before.len() == after.len()
            && before.modified().ok() == after.modified().ok(),
        "The BIR source changed while being read, or exceeds 2 MiB. Check it again."
    );
    Ok(out)
}

pub fn inspect(
    bytes: &[u8],
    name: &str,
    today: NaiveDate,
    employer: Option<&Employer>,
    company_id: Uuid,
    user_id: Uuid,
) -> Result<Report> {
    ensure!(bytes.len() <= MAX_BYTES, "BIR source exceeds 2 MiB.");
    let text=std::str::from_utf8(bytes).context("Use a plaintext ASCII/UTF-8 source; UTF-16, legacy encodings and encrypted files are unsupported.")?;
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    ensure!(
        body.is_ascii(),
        "This preflight currently supports ASCII source data. It never transliterates names or certifies an agency encoding."
    );
    let rows = crate::imports::parse_bounded(text, 1000, 75)?;
    ensure!(
        !rows.is_empty() && rows.len() <= MAX_DETAILS + 2,
        "Use one header, at most 500 Schedule 1 details and one control record."
    );
    for r in &rows {
        ensure!(
            matches!(
                r.cells
                    .first()
                    .map(|s| s.trim().to_ascii_uppercase())
                    .as_deref(),
                Some("H1604C" | "D1" | "C1")
            ),
            "Only unvalidated plaintext 1604C Schedule 1 (H1604C/D1/C1) is supported. Other schedules, validation markers and sealed files need the official module."
        );
    }
    let mut report = Report {
        company_id,
        user_id,
        sha256: format!("{:x}", Sha256::digest(bytes)),
        file_name: name.into(),
        year: None,
        details: 0,
        employer_match: None,
        errors: 0,
        reviews: 0,
        issues: Vec::new(),
        totals: Vec::new(),
    };
    if text.starts_with('\u{feff}') {
        report.issue(
            1,
            "Encoding",
            Severity::Review,
            "UTF-8 BOM detected. Agency encoding compatibility is not verified.",
        );
    }
    let mut prefix: Option<(String, String, String)> = None;
    let (mut headers, mut controls) = (0, 0);
    let mut control_line = 0;
    let mut sums = [Some(0i64); 31];
    let mut declared = [None; 31];
    let mut seen = HashSet::new();
    let mut sequences = HashSet::new();
    for (position, r) in rows.iter().enumerate() {
        let cells: Vec<_> = r.cells.iter().map(|s| s.trim_matches(' ')).collect();
        let tag = cells[0].to_ascii_uppercase();
        if tag == "H1604C" {
            headers += 1;
            if position != 0 || headers > 1 {
                report.issue(
                    r.line,
                    "Header",
                    Severity::Error,
                    "Exactly one header must be first.",
                );
            }
            if cells.len() != 4 {
                report.issue(
                    r.line,
                    "Header",
                    Severity::Error,
                    "The published header has exactly four columns.",
                );
                continue;
            }
            if !tin(cells[1]) {
                report.issue(r.line,"Employer TIN",Severity::Error,"Use nine digits; empty/placeholder identifiers are unsupported. Agency assignment is not verified.");
            }
            if !digits(cells[2], 4) {
                report.issue(
                    r.line,
                    "Employer branch",
                    Severity::Error,
                    "Use a four-digit text branch code, retaining zeros.",
                );
            }
            let at = date(cells[3]);
            if at.is_none_or(|d| {
                d.month() != 12 || d.day() != 31 || d.year() < 2023 || d.year() > today.year()
            }) {
                report.issue(
                    r.line,
                    "Return period",
                    Severity::Error,
                    "Use 12/31/YYYY for a supported year from 2023 through the current year.",
                );
            } else {
                report.year = at.map(|d| d.year());
            }
            if headers == 1 {
                prefix = Some((cells[1].into(), cells[2].into(), cells[3].into()));
            }
            if let Some(e) = employer {
                report.employer_match = Some(e.tin == cells[1] && e.branch == cells[2]);
                if report.employer_match == Some(false) {
                    report.issue(r.line,"Employer",Severity::Error,"The header does not match this company's current registered BIR TIN/branch.");
                }
            } else {
                report.issue(r.line,"Employer",Severity::Review,"Company BIR registration is incomplete; employer matching is unverified. No registration is inferred or changed.");
            }
            if let Some(d) = at {
                let expected = format!("{}{}{}1604C.DAT", cells[1], cells[2], d.format("%m%d%Y"));
                if !name.eq_ignore_ascii_case(&expected) {
                    report.issue(r.line,"Filename",Severity::Error,"The annual filename must follow header TIN + branch + MMDDYYYY + 1604C.DAT. Rename only a separate copy if needed.");
                }
            }
            continue;
        }
        let detail = tag == "D1";
        if detail {
            report.details += 1;
        } else {
            controls += 1;
            control_line = r.line;
        }
        if headers != 1 {
            report.issue(
                r.line,
                "Record order",
                Severity::Error,
                "A valid single header must precede detail/control records.",
            );
        }
        if detail && controls > 0 {
            report.issue(
                r.line,
                "Record order",
                Severity::Error,
                "Details must appear before the control record.",
            );
        }
        if !detail && (controls > 1 || position != rows.len() - 1) {
            report.issue(
                r.line,
                "Control",
                Severity::Error,
                "Exactly one Schedule 1 control must be last.",
            );
        }
        let width = if detail { 49 } else { 36 };
        if cells.len() != width {
            report.issue(r.line,&tag,Severity::Error,&format!("The published {tag} record requires exactly {width} columns; no extra or padded columns are ignored."));
            if detail {
                sums = [None; 31];
            }
            continue;
        }
        if !cells[1].eq_ignore_ascii_case("1604C") {
            report.issue(r.line, "Form", Severity::Error, "Use form type 1604C.");
        }
        if prefix
            .as_ref()
            .is_none_or(|p| p.0 != cells[2] || p.1 != cells[3] || p.2 != cells[4])
        {
            report.issue(r.line,"Employer/period",Severity::Error,"Every detail/control employer TIN, branch and return period must match the header.");
        }
        if detail {
            if cells[5].is_empty()
                || cells[5].len() > 6
                || !cells[5].bytes().all(|b| b.is_ascii_digit())
                || cells[5]
                    .parse::<u32>()
                    .ok()
                    .is_none_or(|n| n == 0 || !sequences.insert(n))
            {
                report.issue(
                    r.line,
                    FIELDS[5],
                    Severity::Error,
                    "Use a unique positive sequence number of at most six digits.",
                );
            }
            if !tin(cells[6]) {
                report.issue(r.line,FIELDS[6],Severity::Error,"Use a nine-digit non-placeholder employee TIN. Identity and agency assignment are not certified.");
            }
            if !digits(cells[7], 4) {
                report.issue(
                    r.line,
                    FIELDS[7],
                    Severity::Error,
                    "Use a four-digit employee branch code.",
                );
            }
            if !seen.insert((cells[6].to_owned(), cells[7].to_owned())) {
                report.issue(r.line,FIELDS[6],Severity::Error,"Duplicate employee TIN/branch in this Schedule 1 file; namesakes with different identifiers remain separate.");
            }
            for i in [8, 9, 10, 44] {
                if cells[i].len() > 30
                    || cells[i].bytes().any(|b| b.is_ascii_control())
                    || ([8, 9].contains(&i) && cells[i].is_empty())
                {
                    report.issue(r.line,FIELDS[i],Severity::Error,"Use at most 30 printable ASCII characters; last and first names are required. Names are not shortened or inferred.");
                }
            }
            if cells[11].is_empty()
                || cells[11].len() > 4
                || cells[11].bytes().any(|b| b.is_ascii_control())
            {
                report.issue(r.line,FIELDS[11],Severity::Error,"Supply an explicit printable region value fitting the published four-character field. Current agency region assignment is not verified.");
            }
            let (from, to) = (date(cells[23]), date(cells[24]));
            if from.is_none() || to.is_none() || from > to {
                report.issue(r.line,"Employment dates",Severity::Error,"Use valid MM/DD/YYYY employment dates with From not after To. Dates are not inferred from HRIS employment.");
            }
            if !["R", "C", "CP", "S", "P", "AL"].contains(&cells[45].to_ascii_uppercase().as_str())
            {
                report.issue(
                    r.line,
                    FIELDS[45],
                    Severity::Error,
                    "Use an explicit published employment code: R, C, CP, S, P or AL.",
                );
            }
            if !["NA", "T", "TR", "R", "D"].contains(&cells[46].to_ascii_uppercase().as_str()) {
                report.issue(
                    r.line,
                    FIELDS[46],
                    Severity::Error,
                    "Use an explicit published separation code: NA, T, TR, R or D.",
                );
            }
            if !["Y", "N"].contains(&cells[47].to_ascii_uppercase().as_str()) {
                report.issue(
                    r.line,
                    FIELDS[47],
                    Severity::Error,
                    "Declare substituted filing as Y or N; eligibility is not inferred.",
                );
            }
        }
        for (j, &column) in MONEY_COLUMNS.iter().enumerate() {
            let parsed = money(cells[if detail { column } else { j + 5 }]);
            let field = format!(
                "{tag} column {} — {}",
                if detail { column + 1 } else { j + 6 },
                FIELDS[column]
            );
            if parsed.is_none() {
                report.issue(r.line,&field,Severity::Error,"Use plain nonnegative PHP, at most 11 whole digits/two decimals; blanks, rounding, separators and exponents are unsupported.");
            }
            if detail {
                let was_known = sums[j].is_some();
                sums[j] = match (sums[j], parsed) {
                    (Some(a), Some(b)) => a.checked_add(b),
                    _ => None,
                };
                if was_known && sums[j].is_none() && parsed.is_some() {
                    report.issue(r.line,FIELDS[column],Severity::Error,"An exact detail total is unavailable or exceeds its integer bound; no partial total is certified.");
                }
            } else {
                declared[j] = parsed;
            }
        }
    }
    if headers != 1 {
        report.issue(
            0,
            "Header",
            Severity::Error,
            "Exactly one header is required.",
        );
    }
    if report.details == 0 {
        report.issue(
            0,
            "Details",
            Severity::Error,
            "At least one supported D1 detail is required; an empty schedule is not certified.",
        );
    }
    if controls != 1 {
        report.issue(
            0,
            "Control",
            Severity::Error,
            "Exactly one C1 control is required.",
        );
    }
    for (j, &column) in MONEY_COLUMNS.iter().enumerate() {
        if sums[j].is_some() && declared[j].is_some() && sums[j] != declared[j] {
            report.issue(control_line,&format!("C1 column {} — {}",j+6,FIELDS[column]),Severity::Error,"The exact C1 control amount differs from the sum of D1 details. No tolerance or automatic adjustment was applied.");
        }
        report.totals.push(Total {
            field: FIELDS[column].into(),
            detail_cents: sums[j],
            control_cents: declared[j],
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn detail() -> Vec<String> {
        let mut row = vec![String::new(); 49];
        for (i, v) in [
            (0, "D1"),
            (1, "1604C"),
            (2, "012345678"),
            (3, "0000"),
            (4, "12/31/2026"),
            (5, "000001"),
            (6, "234567890"),
            (7, "0000"),
            (8, "SAMPLE FAMILY"),
            (9, "FICTIONAL PERSON"),
            (11, "NCR"),
            (23, "01/01/2026"),
            (24, "12/31/2026"),
            (44, "FILIPINO"),
            (45, "R"),
            (46, "NA"),
            (47, "N"),
        ] {
            row[i] = v.into();
        }
        for &i in &MONEY_COLUMNS {
            row[i] = "0.00".into();
        }
        for i in [25, 32, 35, 36, 37] {
            row[i] = "100000.00".into();
        }
        row
    }
    fn csv_cell(s: &str) -> String {
        if s.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.into()
        }
    }
    fn file(details: &[Vec<String>]) -> String {
        let mut rows = vec![vec![
            "H1604C".into(),
            "012345678".into(),
            "0000".into(),
            "12/31/2026".into(),
        ]];
        rows.extend(details.iter().cloned());
        let mut c = vec![
            "C1".into(),
            "1604C".into(),
            "012345678".into(),
            "0000".into(),
            "12/31/2026".into(),
        ];
        for &column in &MONEY_COLUMNS {
            c.push(super::super::decimal(
                details.iter().filter_map(|d| money(&d[column])).sum(),
            ));
        }
        rows.push(c);
        rows.into_iter()
            .map(|r| r.iter().map(|v| csv_cell(v)).collect::<Vec<_>>().join(",") + "\r\n")
            .collect()
    }
    fn check(text: &str, expected: bool) -> Report {
        let employer = Employer {
            tin: "012345678".into(),
            branch: "0000".into(),
        };
        inspect(
            text.as_bytes(),
            "0123456780000123120261604C.DAT",
            "2026-10-26".parse().unwrap(),
            expected.then_some(&employer),
            Uuid::from_u128(1),
            Uuid::from_u128(2),
        )
        .unwrap()
    }

    #[test]
    fn published_49_and_36_column_structure_keeps_exact_control_totals_and_private_rows_out_of_findings()
     {
        let text = file(&[detail()]);
        let r = check(&text, true);
        assert_eq!(
            (r.errors, r.reviews, r.details, r.year, r.employer_match),
            (0, 0, 1, Some(2026), Some(true))
        );
        assert_eq!(r.totals.len(), 31);
        assert!(
            r.totals
                .iter()
                .all(|t| t.detail_cents == t.control_cents && t.detail_cents.is_some())
        );
        assert_eq!(
            r.totals
                .iter()
                .find(|t| t.field == "Present gross compensation")
                .unwrap()
                .control_cents,
            Some(10_000_000)
        );
        let findings = String::from_utf8(r.findings_csv().unwrap()).unwrap();
        assert!(findings.contains("PREFLIGHT_ONLY_NOT_OFFICIAL_VALIDATION"));
        for private in [
            "234567890",
            "SAMPLE FAMILY",
            "FICTIONAL PERSON",
            "100000.00",
        ] {
            assert!(!findings.contains(private));
        }
    }
    #[test]
    fn missing_employer_registration_is_review_and_foreign_employer_is_error() {
        let text = file(&[detail()]);
        let r = check(&text, false);
        assert_eq!((r.errors, r.reviews, r.employer_match), (0, 1, None));
        let wrong = Employer {
            tin: "345678901".into(),
            branch: "0000".into(),
        };
        let r = inspect(
            text.as_bytes(),
            "0123456780000123120261604C.DAT",
            "2026-10-26".parse().unwrap(),
            Some(&wrong),
            Uuid::from_u128(1),
            Uuid::from_u128(2),
        )
        .unwrap();
        assert!(r.errors > 0 && r.employer_match == Some(false));
    }
    #[test]
    fn one_cent_control_difference_is_visible_without_reference_tolerance_or_adjustment() {
        let text = file(&[detail()]);
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let mut row: Vec<String> = lines
            .last()
            .unwrap()
            .split(',')
            .map(str::to_owned)
            .collect();
        row[5] = "0.01".into();
        *lines.last_mut().unwrap() = row.join(",");
        let r = check(&lines.join("\r\n"), true);
        assert!(
            r.issues
                .iter()
                .any(|i| i.message.contains("exact C1 control amount differs"))
        );
        assert_eq!(
            (r.totals[0].detail_cents, r.totals[0].control_cents),
            (Some(0), Some(1))
        );
    }
    #[test]
    fn invalid_money_never_becomes_zero_or_a_complete_partial_total() {
        for value in [
            "",
            "-1.00",
            "1e4",
            "NaN",
            "1,000.00",
            "0.001",
            "100000000000.00",
            "1.",
        ] {
            let mut d = detail();
            d[12] = value.into();
            let r = check(&file(&[d]), true);
            assert!(r.errors > 0, "{value}");
            assert_eq!(r.totals[0].detail_cents, None, "{value}");
        }
        assert_eq!(money("99999999999.99"), Some(9_999_999_999_999));
        assert_eq!(money("0.01"), Some(1));
        assert_eq!(money("12.5"), Some(1250));
    }
    #[test]
    fn duplicate_identifiers_fail_but_namesakes_with_distinct_identifiers_are_retained() {
        let a = detail();
        let mut b = a.clone();
        b[5] = "2".into();
        assert!(check(&file(&[a.clone(), b.clone()]), true).errors > 0);
        b[6] = "345678901".into();
        let r = check(&file(&[a, b]), true);
        assert_eq!((r.errors, r.details), (0, 2));
    }
    #[test]
    fn malformed_prefixes_missing_controls_duplicate_headers_and_oversized_rows_fail() {
        let original = file(&[detail()]);
        for text in [
            original.replace("D1,1604C,012345678", "D1,1604C,123456780"),
            original.lines().take(2).collect::<Vec<_>>().join("\n"),
            format!(
                "{}{}",
                original.lines().next().unwrap().to_owned() + "\r\n",
                original
            ),
            original.replace("D1,1604C", "D1,1604C,extra"),
        ] {
            assert!(check(&text, true).errors > 0);
        }
    }
    #[test]
    fn unsupported_schedules_validation_markers_and_non_ascii_do_not_receive_partial_passes() {
        for text in [
            file(&[detail()]).replace("D1,", "D2,"),
            file(&[detail()]) + "V,signature\r\n",
            file(&[detail()]).replace("SAMPLE FAMILY", "PEÑA"),
        ] {
            assert!(
                inspect(
                    text.as_bytes(),
                    "file.dat",
                    "2026-10-26".parse().unwrap(),
                    None,
                    Uuid::from_u128(1),
                    Uuid::from_u128(2)
                )
                .is_err()
            );
        }
    }
    #[test]
    fn dates_codes_widths_and_filenames_are_checked_without_inferred_employment() {
        for (column, value) in [
            (23, "02/30/2026"),
            (24, "01/01/2025"),
            (45, "HR_GENERALIST"),
            (46, "ARCHIVED"),
            (47, "AUTO"),
            (8, "NAME THAT IS LONGER THAN THIRTY ASCII CHARACTERS"),
            (7, "0"),
            (5, "000000"),
            (11, "\t"),
        ] {
            let mut d = detail();
            d[column] = value.into();
            assert!(check(&file(&[d]), true).errors > 0, "{column}");
        }
        let r = inspect(
            file(&[detail()]).as_bytes(),
            "renamed.dat",
            "2026-10-26".parse().unwrap(),
            None,
            Uuid::from_u128(1),
            Uuid::from_u128(2),
        )
        .unwrap();
        assert!(r.issues.iter().any(|i| i.field == "Filename"));
    }
    #[test]
    fn all_money_columns_including_pera_are_in_the_published_control_mapping() {
        let mut a = detail();
        let mut b = detail();
        b[5] = "2".into();
        b[6] = "345678901".into();
        for (i, &column) in MONEY_COLUMNS.iter().enumerate() {
            a[column] = super::super::decimal((i as i64 + 1) * 101);
            b[column] = super::super::decimal((i as i64 + 1) * 37);
        }
        let r = check(&file(&[a, b]), true);
        assert_eq!(r.errors, 0);
        for (i, t) in r.totals.iter().enumerate() {
            assert_eq!(t.detail_cents, Some((i as i64 + 1) * 138));
        }
        assert_eq!(r.totals.last().unwrap().field, "PERA tax credit");
    }
    #[test]
    fn structural_consistency_does_not_recalculate_tax_or_certify_substituted_filing() {
        let mut d = detail();
        d[38] = "999.99".into();
        d[47] = "Y".into();
        let r = check(&file(&[d]), true);
        assert_eq!(r.errors, 0);
        assert!(
            String::from_utf8(r.findings_csv().unwrap())
                .unwrap()
                .contains("No tax calculation, identity certification or payroll comparison")
        );
    }
    #[test]
    fn diagnostic_cap_is_explicit_and_never_turns_later_errors_into_success() {
        let mut rows = Vec::new();
        for i in 0..30 {
            let mut d = detail();
            d[5] = (i + 1).to_string();
            d[6] = format!("2{i:08}");
            for &column in &MONEY_COLUMNS {
                d[column] = String::new();
            }
            rows.push(d);
        }
        let r = check(&file(&rows), true);
        assert!(r.errors > MAX_ISSUES && r.omitted_issues() > 0);
        let csv = String::from_utf8(r.findings_csv().unwrap()).unwrap();
        assert!(csv.contains("INCOMPLETE_LIST"));
        assert!(csv.contains("ERRORS_FOUND"));
    }
    #[test]
    fn byte_hash_and_bom_review_bind_results_to_the_original_file_bytes() {
        let text = file(&[detail()]);
        let a = check(&text, true);
        let b = check(&("\u{feff}".to_owned() + &text), true);
        assert_ne!(a.sha256, b.sha256);
        assert_eq!(b.errors, 0);
        assert_eq!(b.reviews, 1);
        assert!(b.issues.iter().any(|i| i.field == "Encoding"));
    }
    #[test]
    fn bounded_file_read_never_modifies_the_source_and_rejects_other_extensions() {
        let p = std::env::temp_dir().join(format!("hris-bir-{}.DAT", Uuid::new_v4()));
        let bytes = file(&[detail()]).into_bytes();
        std::fs::write(&p, &bytes).unwrap();
        assert_eq!(read_source(&p).unwrap(), bytes);
        assert_eq!(std::fs::read(&p).unwrap(), bytes);
        std::fs::remove_file(p).unwrap();
        assert!(read_source(Path::new("unopened.exe")).is_err());
        let huge = vec![b' '; MAX_BYTES + 1];
        assert!(
            inspect(
                &huge,
                "file.dat",
                "2026-10-26".parse().unwrap(),
                None,
                Uuid::from_u128(1),
                Uuid::from_u128(2)
            )
            .is_err()
        );
    }
    #[test]
    fn detail_limit_and_physical_lines_remain_explicit_without_partial_acceptance() {
        let rows: Vec<_> = (1..=MAX_DETAILS)
            .map(|i| {
                let mut d = detail();
                d[5] = i.to_string();
                d[6] = format!("2{i:08}");
                d
            })
            .collect();
        assert_eq!(check(&file(&rows), true).details, MAX_DETAILS);
        let mut too_many = rows;
        let mut d = detail();
        d[5] = "501".into();
        d[6] = "234567899".into();
        too_many.push(d);
        assert!(
            inspect(
                file(&too_many).as_bytes(),
                "source.dat",
                "2026-10-26".parse().unwrap(),
                None,
                Uuid::from_u128(1),
                Uuid::from_u128(2)
            )
            .is_err()
        );
        let mut d = detail();
        d[8] = "A, \"FICTIONAL\" FAMILY".into();
        d[12] = String::new();
        let text = file(&[d]).replacen("\r\n", "\r\n\r\n", 1);
        let r = check(&text, true);
        assert!(
            r.issues
                .iter()
                .any(|i| i.line == 3 && i.field.starts_with("D1 column 13"))
        );
    }
    #[test]
    #[ignore = "Explicit fictional native source/findings evidence to a new private folder."]
    fn write_bir_fictional_findings_vectors() {
        let dir = std::path::PathBuf::from(std::env::var("HRIS_BIR_VECTORS_OUT").unwrap());
        std::fs::create_dir(&dir).unwrap();
        let mut all = Vec::new();
        for case in [
            "exact",
            "all-columns",
            "bad-money",
            "one-cent",
            "namesakes",
            "unknown-employer",
        ] {
            let mut d = detail();
            let mut rows = Vec::new();
            if case == "all-columns" {
                for (i, &column) in MONEY_COLUMNS.iter().enumerate() {
                    d[column] = super::super::decimal((i as i64 + 1) * 137);
                }
            }
            if case == "bad-money" {
                d[12] = String::new();
            }
            rows.push(d.clone());
            if case == "namesakes" {
                d[5] = "2".into();
                d[6] = "345678901".into();
                rows.push(d);
            }
            let mut text = file(&rows);
            if case == "one-cent" {
                let mut lines: Vec<_> = text.lines().map(str::to_owned).collect();
                let mut c: Vec<_> = lines[2].split(',').map(str::to_owned).collect();
                c[35] = "0.01".into();
                lines[2] = c.join(",");
                text = lines.join("\r\n") + "\r\n";
            }
            let r = check(&text, case != "unknown-employer");
            let folder = dir.join(case);
            std::fs::create_dir(&folder).unwrap();
            std::fs::write(
                &folder.join("0123456780000123120261604C.DAT"),
                text.as_bytes(),
            )
            .unwrap();
            crate::reports::save_export(&folder.join("findings.csv"), &r.findings_csv().unwrap())
                .unwrap();
            crate::reports::save_export(
                &folder.join("findings.xlsx"),
                &r.findings_export(Path::new("findings.xlsx")).unwrap(),
            )
            .unwrap();
            all.push(serde_json::json!({"case":case,"report":r}));
        }
        std::fs::write(
            &dir.join("reports.json"),
            &serde_json::to_vec_pretty(&all).unwrap(),
        )
        .unwrap();
    }
}

#[cfg(feature = "qa-screenshot")]
pub(crate) fn preview_report(errors: bool) -> Report {
    // Fictional preview only. No real file, registration or account data is loaded.
    let mut r = Report {
        company_id: Uuid::from_u128(1),
        user_id: Uuid::nil(),
        sha256: "a".repeat(64),
        file_name: "Fictional Schedule 1 source.dat".into(),
        year: Some(2026),
        details: 2,
        employer_match: if errors { None } else { Some(true) },
        errors: 0,
        reviews: 0,
        issues: Vec::new(),
        totals: Vec::new(),
    };
    for &i in &MONEY_COLUMNS {
        r.totals.push(Total {
            field: FIELDS[i].into(),
            detail_cents: Some(if [25, 32, 35, 36, 37].contains(&i) {
                20_000_000
            } else {
                0
            }),
            control_cents: Some(if [25, 32, 35, 36, 37].contains(&i) {
                20_000_000
            } else {
                0
            }),
        });
    }
    if errors {
        r.issue(
            2,
            "Previous gross compensation",
            Severity::Error,
            "A required amount is blank; its exact detail total is unavailable.",
        );
        r.issue(2,"Employment status",Severity::Error,"Use an explicit published employment code; an account role is not an employment classification.");
        r.issue(
            1,
            "Employer",
            Severity::Review,
            "Company BIR registration is incomplete; employer matching is unverified.",
        );
        r.totals[0].detail_cents = None;
    }
    r
}
