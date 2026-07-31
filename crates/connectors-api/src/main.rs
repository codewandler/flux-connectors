//! The host, as a binary.

use std::net::{Ipv4Addr, SocketAddr};

/// The port the host listens on.
///
/// Loopback is not configurable and deliberately so. `docs/designs/connectors-app.md` sets the
/// standard: *"It is structurally incapable of sending… a flag on a live client is something a
/// caller forgets."* The same reasoning applies to a bind address — this process holds plaintext
/// credentials, and the first PR that adds a `--bind` flag is the one to refuse.
const PORT: u16 = 8787;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // The workspace root a dispatch happens under. Nothing here reaches the filesystem through it;
    // `System` requires a root that exists, and the current directory is the honest answer.
    let root = std::env::current_dir()?;
    let app = connectors_api::App::new(&root)?;

    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, PORT));
    let listener = tokio::net::TcpListener::bind(address).await?;

    println!("connectors-api listening on http://{address}");
    println!(
        "  {} connectors, {} operations",
        catalog::providers().len(),
        catalog::operations().count()
    );
    println!();
    println!("  This host makes REAL calls to REAL vendors with REAL credentials.");
    println!("  Credentials are held in memory only: stopping the process is the cleanup.");

    axum::serve(listener, connectors_api::router(app))
        .with_graceful_shutdown(shutdown())
        .await?;

    Ok(())
}

/// Stop on Ctrl-C. The credentials go with the process, which is the intended cleanup.
async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
    println!("\nstopping; in-memory credentials discarded");
}
