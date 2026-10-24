//! Validate a protected reconciliation response from stdin without retaining it.
//! This diagnostic never signs in, writes remotely, files or sends a payment.
use anyhow::{Result, ensure};
use hris_desktop::payroll_government::{Filter, Reconciliation, ReviewWire};
use std::io::{self, Read};
fn main() -> Result<()> {
    let mut input = Vec::new();
    io::stdin()
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut input)?;
    ensure!(
        input.len() <= 32 * 1024 * 1024,
        "Response exceeds diagnostic limit."
    );
    let values: Vec<ReviewWire> = serde_json::from_slice(&input)?;
    ensure!(
        (1..=32).contains(&values.len()),
        "Provide 1–32 protected responses."
    );
    let mut sources = 0;
    let mut bytes = 0;
    for wire in &values {
        let filter = Filter {
            month: wire.source.month,
            agency: wire.source.agency,
        };
        let r = Reconciliation::from_wire(wire.clone(), wire.source.company_id, filter)?;
        sources += r.wire.source.facts.len();
        bytes += r.csv()?.len();
    }
    println!(
        "{}",
        serde_json::json!({"validated_snapshots":values.len(),"source_rows":sources,"worksheet_bytes_checked":bytes,"private_response_saved":false})
    );
    Ok(())
}
