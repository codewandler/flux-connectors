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
#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    /// Whether the dev sign-in door exists on this process (C-234).
    dev: bool,
    /// Print what this binary accepts and exit, rather than binding a port.
    help: bool,
}

/// What `--help` prints.
///
/// It names the arguments that do **not** exist as well as the one that does, because the absence
/// of `--bind` is the design rather than an omission, and the person reading `--help` is exactly the
/// person about to look for it.
const USAGE: &str = "\
connectors-api — the reference host for the connector catalogue.

USAGE:
    connectors-api [--dev]

OPTIONS:
    --dev     Add a developer sign-in that needs no Google registration, and say so loudly at
              startup. Authentication is off: anyone who can reach the port can use the host.
              The developer account owns tenant `dev-local` and can see no real account's data.
    --help    Print this and exit.

There is deliberately no --bind and no --port. This process holds plaintext credentials in
memory and listens on 127.0.0.1:8787 by construction; that property is what makes --dev safe
enough to exist. See docs/designs/connectors-api.md.
";

/// Read the process's command line.
fn options() -> anyhow::Result<Options> {
    options_from(std::env::args().skip(1))
}

/// Read a command line, or say what went wrong.
///
/// Split from [`options`] so the refusal below is reachable from a test without a subprocess.
/// `main.rs` is a binary and `options` reads the real `argv`, so without this seam the one rule
/// worth asserting here could only be asserted by spawning the binary — and a rule that is
/// expensive to test is a rule that stops being tested.
///
/// **Unknown arguments are refused rather than ignored**, and that is the interesting half.
/// `docs/designs/connectors-app.md` sets the standard — *"a flag on a live client is something a
/// caller forgets"* — and the comment on [`PORT`] records that the first PR adding a `--bind` flag
/// is the one to refuse. Silently ignoring `--bind 0.0.0.0` would let somebody believe they had
/// changed the bind address and hand the port to their network; refusing it says the word out loud.
/// Loopback-only is the property that makes `--dev` defensible at all, so it is enforced here as
/// well as documented — and, since C-234, pinned by `tests::an_unknown_argument_is_refused` so that
/// deleting the refusal is a red test rather than a silent widening.
fn options_from(arguments: impl IntoIterator<Item = String>) -> anyhow::Result<Options> {
    let mut options = Options::default();
    for argument in arguments {
        match argument.as_str() {
            "--dev" => options.dev = true,
            "--help" | "-h" => options.help = true,
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

    // Asking what a binary accepts is not an error. `--help` exiting non-zero with "unknown
    // argument" is the first thing a person types and the first thing that tells them this binary
    // is unfinished.
    if options.help {
        print!("{USAGE}");
        return Ok(());
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> anyhow::Result<Options> {
        options_from(arguments.iter().map(|argument| (*argument).to_owned()))
    }

    /// **An argument this binary does not know is refused, not ignored** (C-234).
    ///
    /// This is the guard the dev sign-in rests on. `docs/designs/connectors-api.md` §"The dev
    /// sign-in" argues that `POST /auth/dev` is defensible *only* while the listen address is
    /// loopback and unconfigurable, so a binary that silently swallowed `--bind 0.0.0.0` would let
    /// somebody believe they had widened it — and the belief is the whole danger, because they
    /// would then treat the host as though it were reachable and paste credentials accordingly.
    ///
    /// Asserted here rather than left to the doc comment because deleting the `other =>` arm is a
    /// one-line change that no other test in this repository notices. That is the pattern this
    /// wave has been bitten by repeatedly: a property the code has, that nothing would defend if
    /// somebody changed it.
    #[test]
    fn an_unknown_argument_is_refused() {
        for hostile in [
            "--bind",
            "--bind=0.0.0.0",
            "0.0.0.0:8787",
            "--port",
            "--port=80",
            "--host",
            "--listen",
            "--address",
            "-b",
            // Near-misses on the one flag that does exist. `--dev` is matched exactly, so none of
            // these may be read as it.
            "--dev=true",
            "--devel",
            "--DEV",
            "dev",
            "-d",
            "",
        ] {
            let refusal = parse(&[hostile]);
            assert!(
                refusal.is_err(),
                "the binary accepted {hostile:?}, so an argument it does not implement was \
                 silently ignored"
            );
            let message = refusal.expect_err("refused").to_string();
            assert!(
                message.contains("only `--dev`"),
                "the refusal does not say what is accepted: {message}"
            );
        }

        // And the refusal for a bind-shaped argument says *why*, because "unknown argument" alone
        // reads like an oversight to somebody who thinks the flag ought to exist.
        let message = parse(&["--bind"]).expect_err("refused").to_string();
        assert!(
            message.contains("127.0.0.1") && message.contains("no `--bind`"),
            "the refusal must name the loopback bind as the reason: {message}"
        );
    }

    /// An unknown argument is refused even when it arrives beside a good one, so `--dev --bind x`
    /// cannot start a host on the strength of its first word.
    #[test]
    fn one_bad_argument_refuses_the_whole_command_line() {
        assert!(parse(&["--dev", "--bind", "0.0.0.0"]).is_err());
        assert!(parse(&["--bind", "0.0.0.0", "--dev"]).is_err());
    }

    /// The accepted forms, so the refusal above cannot be satisfied by refusing everything.
    #[test]
    fn the_accepted_arguments_are_dev_and_help() {
        assert_eq!(
            parse(&[]).expect("no arguments is valid"),
            Options {
                dev: false,
                help: false
            },
            "dev mode must be off unless it was asked for"
        );
        assert_eq!(
            parse(&["--dev"]).expect("--dev is valid"),
            Options {
                dev: true,
                help: false
            }
        );
        assert_eq!(
            parse(&["--dev", "--dev"]).expect("idempotent"),
            Options {
                dev: true,
                help: false
            }
        );
        for help in [&["--help"][..], &["-h"][..]] {
            assert!(
                parse(help).expect("--help is not an error").help,
                "asking what the binary accepts must not be an error"
            );
        }
    }

    /// `--help` says the two arguments that do not exist, because the reader is looking for them.
    #[test]
    fn usage_names_the_arguments_that_deliberately_do_not_exist() {
        assert!(USAGE.contains("--dev"));
        assert!(
            USAGE.contains("no --bind") && USAGE.contains("no --port"),
            "usage does not record that the bind address is deliberately not configurable"
        );
        assert!(USAGE.contains("127.0.0.1"));
    }
}
