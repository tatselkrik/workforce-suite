use anyhow::{Context, Result};
use hris_employee_portal::{Config, Portal};
#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .context("Operator: supply the private company connection profile path.")?;
    let origin = args
        .next()
        .unwrap_or_else(|| "http://127.0.0.1:3030".into());
    let listen = args.next().unwrap_or_else(|| "127.0.0.1:3030".into());
    let config = Config::load(std::path::Path::new(&path), &origin, &listen)?;
    let portal = Portal::new(config)?;
    let listener = tokio::net::TcpListener::bind(&listen).await?;
    println!("HRIS employee portal listening at {origin}. Sessions are memory-only.");
    axum::serve(listener, portal.router())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    portal.close().await;
    Ok(())
}
