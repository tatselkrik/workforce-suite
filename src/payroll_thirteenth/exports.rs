use super::*;
use crate::domain::{Mode, Snapshot};
use anyhow::Context;
pub fn totals_rows(c: &Calculation) -> Vec<(&'static str, i64)> {
    vec![
        ("Earned basic incl. reviewed adjustments", c.basic_cents),
        ("Cumulative annual entitlement", c.entitlement_cents),
        ("Prior current-employer benefit pay", c.prior_company_cents),
        ("Unpaid entitlement before this payout", c.remaining_cents),
        ("Earlier overpayment requiring review", c.overpayment_cents),
        ("This 13th-month gross payout", c.gross_cents),
        ("Exempt portion under shared annual cap", c.exempt_cents),
        ("Taxable portion", c.taxable_cents),
        ("Reviewed withholding", c.withholding_cents),
        ("SSS contribution adjustment", c.sss_adjustment_cents),
        (
            "PhilHealth contribution adjustment",
            c.philhealth_adjustment_cents,
        ),
        (
            "Pag-IBIG contribution adjustment",
            c.pagibig_adjustment_cents,
        ),
        ("Net 13th-month pay", c.net_cents),
    ]
}
pub fn export_csv(data: &Snapshot, kind: &str, id: Option<Uuid>) -> Result<Vec<u8>> {
    ensure!(
        data.mode != Mode::SignedOut && data.access.company_id == data.company.id,
        "Sign in to export benefit pay."
    );
    let hr = data
        .access
        .allows(crate::staff_roles::Capability::PayrollPreparation);
    let mut rows = vec![vec![
        "Year".into(),
        "Earned through".into(),
        "Planned pay date".into(),
        "Status".into(),
        "Employee code".into(),
        "Employee name".into(),
        "Department".into(),
    ]];
    let row = |year: i32,
               date: NaiveDate,
               pay: NaiveDate,
               status: &str,
               i: &Item|
     -> Result<Vec<String>> {
        ensure!(
            i.company_id == data.company.id,
            "Benefit employee belongs to another company."
        );
        let mut r = vec![
            year.to_string(),
            date.to_string(),
            pay.to_string(),
            status.into(),
            i.employee_code.clone(),
            i.employee_name.clone(),
            i.department.clone(),
        ];
        r.extend(totals_rows(&i.totals).iter().map(|(_, n)| n.to_string()));
        r.push(
            i.totals
                .coverage_gaps
                .iter()
                .map(|g| format!("{} to {}", g.starts_on, g.ends_on))
                .collect::<Vec<_>>()
                .join("; "),
        );
        Ok(r)
    };
    if kind == "thirteenth_payslip" {
        let s = data
            .payroll
            .thirteenth
            .slips
            .iter()
            .find(|s| Some(s.id) == id && s.company_id == data.company.id)
            .context("Issued benefit slip unavailable.")?;
        ensure!(
            hr || data.access.employee_id == Some(s.employee_id),
            "Only your own benefit slip is available."
        );
        rows.push(row(
            s.year,
            s.as_of,
            s.pay_date,
            "Issued calculation; payment recorded separately",
            &s.item,
        )?);
    } else if kind == "thirteenth_register" {
        ensure!(hr, "Only HR/Admin export benefit registers.");
        let r = data
            .payroll
            .thirteenth
            .runs
            .iter()
            .find(|r| Some(r.id) == id && r.company_id == data.company.id)
            .context("Benefit run unavailable.")?;
        for i in data
            .payroll
            .thirteenth
            .items
            .iter()
            .filter(|i| i.run_id == r.id)
        {
            rows.push(row(r.year, r.as_of, r.pay_date, &r.status, i)?);
        }
    } else {
        anyhow::bail!("Unknown annual benefit export.");
    }
    rows[0].extend(
        totals_rows(&Calculation::default())
            .iter()
            .map(|(k, _)| format!("{k} (centavos)")),
    );
    rows[0].push("Missing reviewed coverage".into());
    let mut out = String::from("\u{feff}");
    for r in rows {
        out.push_str(
            &r.iter()
                .map(|v| crate::reports::csv_cell(v))
                .collect::<Result<Vec<_>>>()?
                .join(","),
        );
        out.push_str("\r\n");
    }
    Ok(out.into_bytes())
}
