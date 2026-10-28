use super::*;
use crate::domain::{Mode, Snapshot};
use anyhow::Context;
use uuid::Uuid;
pub fn totals_rows(c: &Calculation) -> Vec<(&'static str, i64)> {
    vec![
        ("Included payslip net balances", c.source_net_cents),
        ("Vacation leave conversion", c.vacation_gross_cents),
        ("Vacation de minimis exclusion", c.vacation_exempt_cents),
        ("Separation pay still owed", c.separation_cents),
        ("Retirement pay still owed", c.retirement_cents),
        ("Additional taxable earnings", c.taxable_extras_cents),
        ("Additional shared benefits", c.shared_extras_cents),
        ("Additional exempt earnings", c.exempt_extras_cents),
        ("Deposit returns", c.deposit_returns_cents),
        ("Additional gross / returns", c.extras_gross_cents),
        (
            "Additional mandatory contributions",
            c.additional_contributions_cents,
        ),
        ("Other lawful deductions", c.lawful_deductions_cents),
        ("Cash before annual adjustment", c.cash_before_tax_cents),
        ("Annual taxable compensation", c.tax.taxable_cents),
        ("Annual income tax", c.tax.annual_tax_cents),
        (
            "Original current/prior withholding",
            c.tax.current_withheld_cents + c.tax.prior_withheld_cents,
        ),
        (
            "Earlier settled tax adjustments",
            c.tax.prior_adjustment_cents,
        ),
        (
            "Final tax collected from this cash",
            c.tax.available_deduction_cents,
        ),
        ("Final tax refund in this cash", c.tax.refund_cents),
        (
            "Separate employer tax handling",
            c.tax.employer_advance_cents,
        ),
        ("Combined final cash", c.net_cents),
    ]
}
pub fn status<'a>(data: &'a Snapshot, id: Uuid) -> Option<&'a Status> {
    data.payroll
        .finals
        .statuses
        .iter()
        .find(|s| s.id == id && s.company_id == data.company.id)
}
pub fn export_csv(data: &Snapshot, kind: &str, id: Option<Uuid>) -> Result<Vec<u8>> {
    ensure!(
        data.mode != Mode::SignedOut && data.access.company_id == data.company.id,
        "Sign in to export final pay."
    );
    let hr = data.access.allows(crate::staff_roles::Capability::PayrollPreparation);
    let mut rows = vec![vec![
        "Employee code".into(),
        "Employee name".into(),
        "Department".into(),
        "Employment start".into(),
        "Last working day".into(),
        "Planned cash date".into(),
        "Status".into(),
        "Recorded settlement / withdrawal".into(),
    ]];
    rows[0].extend(
        totals_rows(&Calculation::default())
            .iter()
            .map(|(l, _)| format!("{l} (centavos)")),
    );
    let mut add = |code: &str,
                   name: &str,
                   dept: &str,
                   start: NaiveDate,
                   end: NaiveDate,
                   cash: NaiveDate,
                   state: &str,
                   date: Option<NaiveDate>,
                   c: &Calculation| {
        let mut row = vec![
            code.into(),
            name.into(),
            dept.into(),
            start.to_string(),
            end.to_string(),
            cash.to_string(),
            state.into(),
            date.map(|d| d.to_string()).unwrap_or_default(),
        ];
        row.extend(totals_rows(c).iter().map(|(_, n)| n.to_string()));
        rows.push(row);
    };
    match kind {
        "final_statement" => {
            let s = data
                .payroll
                .finals
                .slips
                .iter()
                .find(|s| Some(s.id) == id && s.company_id == data.company.id)
                .context("Issued final statement unavailable.")?;
            ensure!(
                hr || data.access.employee_id == Some(s.employee_id),
                "Only your own final statement is available."
            );
            let st = status(data, s.id);
            add(
                &s.employee_code,
                &s.employee_name,
                &s.department,
                s.employment_start,
                s.service_end,
                s.cash_date,
                st.map(|s| s.kind.as_str())
                    .unwrap_or("approved; cash not recorded"),
                st.map(|s| s.recorded_on),
                &s.totals,
            );
        }
        "final_register" => {
            ensure!(hr, "Only HR/Admin export final-pay registers.");
            for r in data
                .payroll
                .finals
                .records
                .iter()
                .filter(|r| r.company_id == data.company.id && id.is_none_or(|id| id == r.id))
            {
                add(
                    &r.employee_code,
                    &r.employee_name,
                    &r.department,
                    r.employment_start,
                    r.service_end,
                    r.cash_date,
                    &r.status,
                    r.settled_on,
                    &r.totals,
                );
            }
        }
        _ => anyhow::bail!("Unknown final-pay export."),
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
