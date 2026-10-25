//! Decode protected notification responses from stdin without saving records.
use anyhow::{Result, ensure};
use hris_desktop::{access::Access, notifications::lifecycle::Inbox};
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    access: Access,
    inbox: Inbox,
}
fn main() -> Result<()> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "Diagnostic input is too large."
    );
    let cases: Vec<Case> = serde_json::from_slice(&bytes)?;
    ensure!(
        (1..=20).contains(&cases.len()),
        "Provide 1–20 protected cases."
    );
    let mut items = 0;
    for case in &cases {
        case.inbox.validate(&case.access, case.inbox.as_of)?;
        items += case.inbox.items.len();
    }
    println!(
        "{}",
        serde_json::json!({"validated_snapshots":cases.len(),"validated_items":items,"private_response_saved":false})
    );
    Ok(())
}
