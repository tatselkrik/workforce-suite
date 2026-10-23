use super::*;
use crate::domain::{Mode, Snapshot};
use anyhow::Context;
use uuid::Uuid;
pub fn totals_rows(c: &Calculation) -> Vec<(&'static str, i64)> {
    vec![
        (
            "Current-employer taxable before cap review",
            c.current_taxable_cents,
        ),
        ("Prior-employer certified taxable", c.prior_taxable_cents),
        ("Annual shared benefits gross", c.benefit_gross_cents),
        ("Shared benefit exclusion", c.benefit_exempt_cents),
        ("Taxable shared benefits", c.benefit_taxable_cents),
        (
            "Benefit basis reconciliation",
            c.benefit_basis_adjustment_cents,
        ),
        ("Reviewed taxable correction", c.taxable_adjustment_cents),
        ("Annual taxable compensation", c.taxable_cents),
        ("Annual income tax due", c.annual_tax_cents),
        (
            "Current-employer original withholding",
            c.current_withheld_cents,
        ),
        (
            "Prior-employer certified withholding",
            c.prior_withheld_cents,
        ),
        ("Earlier settled tax adjustments", c.prior_adjustment_cents),
        (
            "Signed tax adjustment (+withhold / -refund)",
            c.adjustment_cents,
        ),
        (
            "Additional withholding advised",
            c.additional_withholding_cents,
        ),
        ("Refund advised", c.refund_cents),
        (
            "Within reviewed available last-pay cash",
            c.available_deduction_cents,
        ),
        (
            "Employer advance / unfunded deficiency",
            c.employer_advance_cents,
        ),
    ]
}
pub fn export_csv(data: &Snapshot, kind: &str, id: Option<Uuid>) -> Result<Vec<u8>> {
    ensure!(
        data.mode != Mode::SignedOut && data.access.company_id == data.company.id,
        "Sign in to export annual tax."
    );
    let hr = data.access.role.manages_hr();
    let mut rows = vec![vec![
        "Tax year".into(),
        "As of".into(),
        "Service end".into(),
        "Close kind".into(),
        "Status".into(),
        "Employee code".into(),
        "Employee name".into(),
        "Department".into(),
    ]];
    rows[0].extend(
        totals_rows(&Calculation::default())
            .iter()
            .map(|(label, _)| format!("{label} (centavos)")),
    );
    let mut add = |year: i32,
                   asof: NaiveDate,
                   end: NaiveDate,
                   kind: &str,
                   status: &str,
                   code: &str,
                   name: &str,
                   dept: &str,
                   c: &Calculation| {
        let mut row = vec![
            year.to_string(),
            asof.to_string(),
            end.to_string(),
            kind.into(),
            status.into(),
            code.into(),
            name.into(),
            dept.into(),
        ];
        row.extend(totals_rows(c).iter().map(|(_, n)| n.to_string()));
        rows.push(row);
    };
    if kind == "tax_payslip" {
        let s = data
            .payroll
            .tax
            .slips
            .iter()
            .find(|s| Some(s.id) == id && s.company_id == data.company.id)
            .context("Issued tax advice unavailable.")?;
        ensure!(
            hr || data.access.employee_id == Some(s.employee_id),
            "Only your own tax advice is available."
        );
        add(
            s.year,
            s.as_of,
            s.service_end,
            &s.kind,
            "Issued advice; settlement recorded separately",
            &s.employee_code,
            &s.employee_name,
            &s.department,
            &s.totals,
        );
    } else if kind == "tax_register" {
        ensure!(hr, "Only HR/Admin export tax registers.");
        for r in &data.payroll.tax.records {
            if r.company_id == data.company.id && id.is_none_or(|id| id == r.id) {
                add(
                    r.year,
                    r.as_of,
                    r.service_end,
                    &r.kind,
                    &r.status,
                    &r.employee_code,
                    &r.employee_name,
                    &r.department,
                    &r.totals,
                );
            }
        }
    } else {
        anyhow::bail!("Unknown annual tax export.");
    }
    let mut out = String::from("\u{feff}");
    for row in rows {
        out.push_str(
            &row.iter()
                .map(|v| crate::reports::csv_cell(v))
                .collect::<Result<Vec<_>>>()?
                .join(","),
        );
        out.push_str("\r\n");
    }
    Ok(out.into_bytes())
}
