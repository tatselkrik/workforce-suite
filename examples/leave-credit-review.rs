//! Validate a protected credit response in memory, without saving personnel data.
use anyhow::{Result, ensure};
use hris_desktop::{access::Access, leave::credits::Context};
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    access: Access,
    credits: Context,
}
fn main() -> Result<()> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "Credit responses exceed diagnostic limit."
    );
    let cases: Vec<Case> = serde_json::from_slice(&bytes)?;
    ensure!(
        (1..=16).contains(&cases.len()),
        "Provide1–16protected snapshots."
    );
    let mut entries = 0;
    for c in &cases {
        c.credits.validate(&c.access, c.credits.as_of)?;
        entries += c.credits.entries.len();
    }
    println!(
        "{}",
        serde_json::json!({"validated_snapshots":cases.len(),"validated_entries":entries,"private_response_saved":false})
    );
    Ok(())
}
