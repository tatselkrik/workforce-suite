//! Verify an existing signed updater locally; never install or publish it.
use anyhow::{Result, ensure};
use hris_desktop::updater::{SignedRelease, UpdateConfig, verify_package};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 4,
        "Usage: verify-update manifest package public-key-profile"
    );
    let signed: SignedRelease = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let config: UpdateConfig = serde_json::from_slice(&std::fs::read(&args[3])?)?;
    let verified = signed.verify(&config)?;
    verify_package(&verified.release, &std::fs::read(&args[2])?)?;
    println!(
        "{}",
        serde_json::json!({"signature_and_package_verified":true,"version":verified.release.version})
    );
    Ok(())
}
