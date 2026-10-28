//! Local assembly of explicitly reviewed declarations, never a tax calculation.
//! This published-layout draft still requires official BIR validation.
use super::*;
use std::io::Write;

pub enum Action {
    Template {
        schedule: Schedule,
        destination: std::path::PathBuf,
    },
    Prepare {
        selection: Selection,
        source: std::path::PathBuf,
        expected_basis: Option<String>,
        destination: Option<std::path::PathBuf>,
        reviewed: bool,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Selection {
    pub schedule: Schedule,
    pub year: i32,
}

pub struct Basis<'a> {
    pub selection: Selection,
    pub today: NaiveDate,
    pub employer: &'a Employer,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub registration_id: Uuid,
    pub registration_revision: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preview {
    pub selection: Selection,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub source_sha256: String,
    pub output_sha256: String,
    pub basis_sha256: String,
    pub file_name: String,
    pub details: usize,
    pub controls: usize,
    pub review_items: usize,
    pub totals: Vec<Total>,
}

pub struct Draft {
    pub preview: Preview,
    bytes: Vec<u8>,
}

pub fn headers(schedule: Schedule) -> &'static [&'static str] {
    // Employer/form/period and sequence are assembled, not imported declarations.
    &schedule.layout().fields[6..]
}

pub fn template(schedule: Schedule, excel: bool) -> Result<Vec<u8>> {
    let headers = headers(schedule);
    if excel {
        crate::tabular::text_xlsx(headers, &[])
    } else {
        crate::imports::csv(headers, std::iter::empty())
    }
}

pub fn read_template(path: &Path) -> Result<(Vec<u8>, bool)> {
    let excel = crate::tabular::is_xlsx(path);
    ensure!(
        excel
            || path
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("csv")),
        "Choose the reviewed BIR declaration .csv or .xlsx template."
    );
    let file = std::fs::File::open(path).context("Could not open the declaration template.")?;
    let before = file.metadata()?;
    ensure!(
        before.is_file() && before.len() <= MAX_BYTES as u64,
        "Use a regular declaration file no larger than 2 MiB."
    );
    let mut bytes = Vec::new();
    let mut reader = file.take(MAX_BYTES as u64 + 1);
    reader.read_to_end(&mut bytes)?;
    let after = reader.get_ref().metadata()?;
    ensure!(
        bytes.len() <= MAX_BYTES
            && bytes.len() == before.len() as usize
            && before.len() == after.len()
            && before.modified().ok() == after.modified().ok(),
        "The declaration template changed while being read. Preview it again."
    );
    Ok((bytes, excel))
}

fn rows(bytes: &[u8], excel: bool, schedule: Schedule) -> Result<Vec<crate::tabular::Row>> {
    ensure!(
        bytes.len() <= MAX_BYTES,
        "Declaration template exceeds 2 MiB."
    );
    let width = headers(schedule).len();
    let mut rows = if excel {
        crate::tabular::read_xlsx(bytes, MAX_DETAILS, width)?
    } else {
        crate::imports::parse_bounded(
            std::str::from_utf8(bytes).context("Use UTF-8 CSV or the native Excel template.")?,
            MAX_DETAILS,
            width,
        )?
        .into_iter()
        .map(|r| crate::tabular::Row {
            number: r.line,
            cells: r.cells,
        })
        .collect()
    };
    ensure!(
        rows.len() >= 2 && rows.len() <= MAX_DETAILS + 1,
        "Include template headers and 1–500 explicitly reviewed declaration rows."
    );
    ensure!(
        rows[0].number == 1
            && rows[0]
                .cells
                .iter()
                .map(String::as_str)
                .eq(headers(schedule).iter().copied()),
        "Use the unchanged headers/order for the selected BIR schedule template."
    );
    rows.remove(0);
    for row in &rows {
        ensure!(
            row.cells.len() == width,
            "Template row {}: supply every declared column; missing values are not zero.",
            row.number
        );
        for (index, cell) in row.cells.iter().enumerate() {
            ensure!(
                cell.is_ascii()
                    && !cell.bytes().any(|b| b.is_ascii_control())
                    && !cell.contains([',', '"'])
                    && cell == cell.trim_matches(' '),
                "Template row {}, column {}: use unpadded printable ASCII without commas/quotes. No name is shortened, transliterated or silently changed.",
                row.number,
                index + 1
            );
        }
    }
    Ok(rows)
}

/// The caller must obtain a fresh protected employer basis and recheck access
/// after preview/render and immediately before any new-file write.
pub fn assemble(bytes: &[u8], excel: bool, basis: &Basis<'_>) -> Result<Draft> {
    let selection = basis.selection;
    ensure!(
        (2023..=basis.today.year()).contains(&selection.year),
        "Choose a supported BIR declaration year from 2023 through the current year."
    );
    ensure!(
        matchable_identity(&basis.employer.tin, &basis.employer.branch)
            && !basis.company_id.is_nil()
            && !basis.user_id.is_nil()
            && !basis.registration_id.is_nil()
            && basis.registration_revision >= 0,
        "Complete this company's protected BIR registration before building a draft."
    );
    let rows = rows(bytes, excel, selection.schedule)?;
    let layout = selection.schedule.layout();
    let period = format!("12/31/{}", selection.year);
    let file_name = format!(
        "{}{}1231{}1604C.DAT",
        basis.employer.tin, basis.employer.branch, selection.year
    );
    let mut output = format!(
        "H1604C,{},{},{}\r\n",
        basis.employer.tin, basis.employer.branch, period
    );
    let mut sums = vec![0i64; layout.money.len()];
    for (index, row) in rows.iter().enumerate() {
        let mut detail = vec![
            layout.detail.into(),
            "1604C".into(),
            basis.employer.tin.clone(),
            basis.employer.branch.clone(),
            period.clone(),
            (index + 1).to_string(),
        ];
        detail.extend(row.cells.iter().cloned());
        for (control, &column) in layout.money.iter().enumerate() {
            let cents = money(&detail[column]).with_context(|| format!(
                "Template row {}, column {}: declare a plain nonnegative PHP amount with at most two decimals. Blank is not zero.",
                row.number, column - 5))?;
            sums[control] = sums[control]
                .checked_add(cents)
                .context("An exact declaration control exceeds its integer bound.")?;
            // Controls have the same published 14-character monetary width.
            ensure!(
                sums[control] <= 9_999_999_999_999,
                "The aggregate control for {} exceeds the published monetary width. No rows were dropped.",
                layout.fields[column]
            );
            detail[column] = super::super::decimal(cents);
        }
        for column in [layout.status, layout.separation, layout.filing] {
            ensure!(
                detail[column] == detail[column].to_ascii_uppercase(),
                "Template row {}: use the explicit uppercase published {} code.",
                row.number,
                layout.fields[column]
            );
        }
        ensure!(
            !detail[layout.nationality].is_empty(),
            "Template row {}: declare nationality explicitly; none is inferred.",
            row.number
        );
        output.push_str(&detail.join(","));
        output.push_str("\r\n");
    }
    let mut control = vec![
        layout.control.into(),
        "1604C".into(),
        basis.employer.tin.clone(),
        basis.employer.branch.clone(),
        period,
    ];
    control.extend(sums.iter().map(|&c| super::super::decimal(c)));
    output.push_str(&control.join(","));
    output.push_str("\r\n");
    let output = output.into_bytes();
    let checked = inspect(
        &output,
        &file_name,
        basis.today,
        Some(basis.employer),
        basis.company_id,
        basis.user_id,
    )?;
    ensure!(
        checked.errors == 0,
        "The declaration rows fail the published layout checks. First issue: {}",
        checked
            .issues
            .iter()
            .find(|i| i.severity == Severity::Error)
            .map(|i| {
                let location = i
                    .line
                    .checked_sub(2)
                    .and_then(|index| rows.get(index))
                    .map(|r| format!("template row {}", r.number))
                    .unwrap_or_else(|| format!("output line {}", i.line));
                format!("{location} — {}: {}", i.field, i.message)
            })
            .unwrap_or_default()
    );
    ensure!(
        checked.arithmetic.checked() && checked.arithmetic.differences == 0,
        "The declared compensation subtotals disagree. Review the declarations; no amount is automatically corrected."
    );
    let source_sha256 = format!("{:x}", Sha256::digest(bytes));
    let basis_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            "bir-declaration-builder-v1",
            selection,
            basis.company_id,
            basis.user_id,
            basis.registration_id,
            basis.registration_revision,
            &basis.employer.tin,
            &basis.employer.branch,
            excel,
            &source_sha256,
            &checked.sha256,
        ))?)
    );
    Ok(Draft {
        preview: Preview {
            selection,
            company_id: basis.company_id,
            user_id: basis.user_id,
            source_sha256,
            output_sha256: checked.sha256,
            basis_sha256,
            file_name,
            details: checked.details,
            controls: checked.totals.len(),
            review_items: checked.reviews,
            totals: checked.totals,
        },
        bytes: output,
    })
}

impl Draft {
    /// Fresh access, employer/source binding and explicit declaration review must
    /// be checked by the adapter before this new-file-only local write.
    pub(crate) fn save_new(&self, destination: &Path) -> Result<()> {
        ensure!(
            destination.file_name().is_some_and(|n| n
                .to_string_lossy()
                .eq_ignore_ascii_case(&self.preview.file_name)),
            "Keep the BIR filename shown in the preview; choose a new folder if it exists."
        );
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true)
            .open(destination).context("Could not create the BIR draft. Existing files are preserved; choose a new folder.")?;
        if let Err(error) = file.write_all(&self.bytes).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = std::fs::remove_file(destination);
            return Err(error).context("The new BIR draft could not be written completely.");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
