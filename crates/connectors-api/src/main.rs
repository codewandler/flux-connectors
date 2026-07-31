//! The host, as a binary.

use std::net::{Ipv4Addr, SocketAddr};

/// The port the host listens on.
///
/// Loopback is not configurable and deliberately so. `docs/designs/connectors-app.md` sets the
/// standard: *"It is structurally incapable of sending… a flag on a live client is something a
/// caller forgets."* The same reasoning applies to a bind address — this process holds plaintext
/// credentials, and the first PR that adds a `--bind` flag is the one to refuse.
const PORT: u16 = 8787;

/// What this binary was asked to do.
///
/// One field, and there is no argument-parsing dependency behind it. `--dev` is a single boolean
/// and a hand-written match over `std::env::args` is both shorter than a derive and — more to the
/// point — leaves the set of accepted arguments visible in one place, which is the property that
/// matters for a binary whose whole safety argument is about what it will *not* accept.
struct Options {
    /// Whether the dev sign-in door exists on this process (C-234).
    dev: bool,
}

/// Read the command line, or say what went wrong.
///
/// **Unknown arguments are refused rather than ignored**, and that is the interesting half.
/// `docs/designs/connectors-app.md` sets the standard — *"a flag on a live client is something a
/// caller forgets"* — and the comment on [`PORT`] records that the first PR adding a `--bind` flag
/// is the one to refuse. Silently ignoring `--bind 0.0.0.0` would let somebody believe they had
/// changed the bind address and hand the port to their network; refusing it says the word out loud.
/// Loopback-only is the property that makes `--dev` defensible at all, so it is enforced here as
/// well as documented.
fn options() -> anyhow::Result<Options> {
    let mut options = Options { dev: false };
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--dev" => options.dev = true,
            other => anyhow::bail!(
                "unknown argument {other:?}. This binary takes only `--dev`.\n\
                 There is deliberately no `--bind` and no `--port`: this process holds plaintext \
                 credentials in memory, it listens on 127.0.0.1:{PORT} by construction, and that \
                 is what makes the `--dev` sign-in safe enough to exist."
            ),
        }
    }
    Ok(options)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let options = options()?;

    // The workspace root a dispatch happens under. Nothing here reaches the filesystem through it;
    // `System` requires a root that exists, and the current directory is the honest answer.
    let root = std::env::current_dir()?;
    let app = connectors_api::App::new(&root)?;
    // The flag, and the only thing that reads it. `--dev` is not refused when a Google registration
    // is also configured: the two doors mint disjoint tenants (`google-{sub}` and `dev-local`), so
    // they cannot contaminate each other, and refusing would make trying the real flow and the dev
    // flow on one machine a matter of unsetting environment variables. Recorded in
    // `docs/designs/connectors-api.md`.
    let app = if options.dev {
        app.with_dev_signin()
    } else {
        app
    };

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

    // **Sign-in state, said out loud at startup.**
    //
    // A missing Google registration is a first-run state, not a crash: the process binds, serves
    // its page, and prints exactly which variables to set. Panicking here would turn a first
    // `cargo run` into a stack trace, and starting silently would turn it into a sign-in button
    // that leads nowhere — the two failure modes C-204 refuses by name.
    println!();
    match app.setup_message() {
        Some(message) if !options.dev => {
            println!("  ⚠ {}", message.replace('\n', "\n  "));
        }
        Some(_) => {
            println!("  Google sign-in is not configured, and with --dev it does not need to be.");
        }
        None => {
            println!("  Google sign-in is configured. Every connector and credential belongs to");
            println!("  the signed-in account's tenant, never to a tenant a request names.");
        }
    }

    // **Dev mode, said out loud in the same place.**
    //
    // The banner above already warns that this host makes real calls with real credentials. If
    // authentication is off, that belongs in the same breath rather than in a log line somebody
    // scrolls past: an operator who does not know which of the two modes they started is the
    // operator who pastes a real API key into a host that will let anything through.
    if options.dev {
        println!();
        println!("  ⚠ DEV SIGN-IN IS ON (--dev). AUTHENTICATION IS DISABLED.");
        println!("    Anyone who can reach this port can press \"Sign in as DEVELOPER\" and use");
        println!("    this host with no credentials of their own. Only 127.0.0.1 can reach it —");
        println!("    that is the whole of what stands between this door and the network.");
        println!("    The dev account owns tenant `dev-local` and can see no real account's data.");
        println!("    Credentials still live in memory only and still die with the process.");
        println!("    Do not start a host this way to hold a credential you care about.");
    }

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
