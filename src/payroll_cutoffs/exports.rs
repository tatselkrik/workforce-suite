use super::*;
use std::collections::BTreeMap;
pub fn export_csv(data: &Snapshot, kind: &str, id: Option<Uuid>) -> Result<Vec<u8>> {
    ensure!(
        data.mode != Mode::SignedOut && data.access.company_id == data.company.id,
        "Sign in to export cutoff payroll."
    );
    let hr = data.access.role.manages_hr();
    let mut rows = vec![];
    let mut header = vec![
        "Month".to_string(),
        "Cutoff".into(),
        "From".into(),
        "To".into(),
        "Planned pay date".into(),
        "Record status".into(),
        "Employee code".into(),
        "Employee name".into(),
        "Department".into(),
    ];
    let keys = serde_json::to_value(Totals::default())?
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    header.extend(keys.iter().map(|k| format!("{} (centavos)", k)));
    rows.push(header);
    let row = |month: NaiveDate,
               part: String,
               lo: NaiveDate,
               hi: NaiveDate,
               pay: String,
               status: String,
               i: &payroll::Item|
     -> Result<Vec<String>> {
        ensure!(
            i.company_id == data.company.id,
            "Payroll record belongs to another company."
        );
        let mut r = vec![
            month.to_string(),
            part,
            lo.to_string(),
            hi.to_string(),
            pay,
            status,
            i.employee_code.clone(),
            i.employee_name.clone(),
            i.department.clone(),
        ];
        let t = serde_json::to_value(&i.totals)?;
        r.extend(keys.iter().map(|k| t[k].as_i64().unwrap().to_string()));
        Ok(r)
    };
    match kind {
        "cutoff_payslip" => {
            let s = data
                .payroll
                .cutoffs
                .slips
                .iter()
                .find(|s| Some(s.base.id) == id)
                .context("Issued cutoff payslip unavailable.")?;
            ensure!(
                hr || data.access.employee_id == Some(s.base.employee_id),
                "Only your own payslip is available."
            );
            rows.push(row(
                s.base.month,
                s.part.to_string(),
                s.starts_on,
                s.ends_on,
                s.pay_date.to_string(),
                "Issued calculation; payment recorded separately".into(),
                &s.base.item,
            )?);
        }
        "cutoff_register" | "cutoff_monthly" => {
            ensure!(hr, "Only HR and Administrators export cutoff registers.");
            let run = data
                .payroll
                .cutoffs
                .runs
                .iter()
                .find(|r| Some(r.base.id) == id && r.base.company_id == data.company.id)
                .context("Cutoff run unavailable.")?;
            if kind == "cutoff_register" {
                for i in data
                    .payroll
                    .cutoffs
                    .items
                    .iter()
                    .filter(|i| i.base.run_id == run.base.id)
                {
                    rows.push(row(
                        run.base.month,
                        run.part.to_string(),
                        run.starts_on,
                        run.ends_on,
                        run.pay_date.to_string(),
                        run.base.status.clone(),
                        &i.base,
                    )?);
                }
            } else {
                let mut groups: BTreeMap<Uuid, (Vec<u8>, payroll::Item)> = BTreeMap::new();
                let mut slips: Vec<_> = data
                    .payroll
                    .cutoffs
                    .slips
                    .iter()
                    .filter(|s| s.base.month == run.base.month)
                    .collect();
                slips.sort_by_key(|s| s.part);
                for s in slips {
                    let g = groups.entry(s.base.employee_id).or_insert_with(|| {
                        let mut i = s.base.item.clone();
                        i.totals = Totals::default();
                        (vec![], i)
                    });
                    let mut t = serde_json::to_value(&g.1.totals)?;
                    let add = serde_json::to_value(&s.base.item.totals)?;
                    for k in &keys {
                        t[k] = json!(t[k].as_i64().unwrap() + add[k].as_i64().unwrap());
                    }
                    g.1.totals = serde_json::from_value(t)?;
                    g.0.push(s.part);
                }
                for (parts, i) in groups.values() {
                    rows.push(row(run.base.month,parts.iter().map(u8::to_string).collect::<Vec<_>>().join("+"),run.base.month,month_end(run.base.month),String::new(),format!("{} issued cutoff(s); review incomplete months; not a government return",parts.len()),i)?);
                }
            }
        }
        _ => anyhow::bail!("Choose a cutoff report."),
    }
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
