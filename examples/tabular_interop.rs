//! Explicit fictional Excel interoperability evidence; never connects to a company.
use anyhow::{Context, Result, ensure};
use hris_desktop::{domain::DemoStore, reports, tabular};
use std::{io::Write, path::PathBuf};
fn save(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn main() -> Result<()> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("Choose a NEW fictional output folder.")?,
    );
    ensure!(
        !root.exists(),
        "Preserve earlier evidence; choose a new folder."
    );
    std::fs::create_dir(&root)?;
    let mut data = DemoStore::new().data;
    data.company.name = "Fictional Company".into();
    let mut inventory = Vec::new();
    for kind in reports::Kind::ALL {
        let mut filter = reports::Filter::new(data.company.today());
        filter.kind = kind;
        let report = reports::build(&data, &filter, chrono::Utc::now())?;
        let filename = format!("{}.xlsx", kind.slug());
        report.save(&root.join(&filename))?;
        inventory.push(serde_json::json!({"file":filename,"rows":report.rows.len(),"columns":report.headers.len()+6,"headers":report.headers}));
    }
    for (name, bytes) in [
        (
            "employee-template.xlsx",
            hris_desktop::imports::template_xlsx()?,
        ),
        (
            "opening-template.xlsx",
            hris_desktop::leave::opening_import::template_xlsx()?,
        ),
        (
            "literal-values.xlsx",
            tabular::text_xlsx(
                &["employee_code", "full_name", "phone", "notes"],
                &[vec![
                    "000001".into(),
                    "=Synthetic Ñame".into(),
                    "+639000000001".into(),
                    "Fictional\nMultiline & <text>".into(),
                ]],
            )?,
        ),
    ] {
        save(&root.join(name), &bytes)?;
    }
    save(
        &root.join("inventory.json"),
        &serde_json::to_vec_pretty(&inventory)?,
    )?;
    println!("Generated eight fictional native XLSX files for independent read-only verification.");
    Ok(())
}
