//! Generate a fictional payslip for rendering QA; never connects to a database.
use anyhow::{Context, Result};
use chrono::NaiveDate;
use hris_desktop::{
    payroll::{Item, Pay, Profile, RULE_VERSION, Slip, Worksheet, calculate},
    payroll_pdf,
};
use uuid::Uuid;
fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("Supply a new output .pdf path.")?;
    let id = Uuid::from_u128(88000);
    let month = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    let p = Profile {
        id,
        company_id: id,
        employee_id: id,
        effective_on: month,
        pay: Pay {
            rate_cents: 3_000_000,
            ..Default::default()
        },
        revision: 0,
        created_by: id,
    };
    let w = Worksheet::default();
    let i = Item {
        id,
        company_id: id,
        run_id: id,
        employee_id: id,
        employee_name: "Fictional Peña Test".into(),
        employee_code: "DEMO-001".into(),
        department: "Fictional Operations".into(),
        profile: p.clone(),
        source_basis: serde_json::Value::Null,
        totals: calculate(&p.pay, &w, month)?,
        worksheet: w,
        revision: 0,
    };
    let s = Slip {
        id,
        company_id: id,
        run_id: id,
        employee_id: id,
        month,
        company_name: "Fictional HRIS Company".into(),
        item: i,
        approved_by: id,
        approved_at: "2026-10-01T00:00:00Z".parse()?,
        rule_version: RULE_VERSION.into(),
    };
    payroll_pdf::save(std::path::Path::new(&path), &payroll_pdf::build(&s)?)?;
    println!("Fictional PDF generated.");
    Ok(())
}
