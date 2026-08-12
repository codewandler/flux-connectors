//! The build is hermetic and offline. This file is the proof.
//!
//! `connector-spec` and `connector-flux` are pure by construction, so the network can only enter
//! through `connector-cli`. The invariant is therefore local to this crate, and it is proven three
//! ways, weakest to strongest:
//!
//! 1. [`build_records_no_network_attempt`] — runs a real build with the [`connector_cli::net`]
//!    seam armed to refuse, and asserts the seam was never reached.
//! 2. [`the_network_seam_is_the_only_door`] — a source audit, so that (1) cannot be defeated by
//!    code that opens a socket without going through the seam.
//! 3. [`build_succeeds_with_networking_unavailable`] — runs the real binary inside a network
//!    namespace with no interfaces, which is the Acceptance item read literally.

use crate::common::Fixture;

fn build(root: &str) -> anyhow::Result<String> {
    let invocation =
        connector_cli::cli::parse(["build", "--root", root].iter().map(|a| a.to_string()))?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("CLI output is UTF-8"))
}

/// Acceptance: "The build performs **no network IO**."
#[test]
fn build_records_no_network_attempt() {
    let fixture = Fixture::with_provider("no-network", "zendesk");

    let denial = connector_cli::net::deny();
    build(fixture.root().to_str().unwrap()).expect("build succeeds offline");

    assert_eq!(
        denial.attempts(),
        0,
        "`build` reached the network seam; only `fetch` (C-14) may"
    );
    assert!(fixture.exists("connectors/zendesk.flux"));
}

/// Every network primitive in this crate must live behind `src/net.rs`, or the counter above is
/// measuring nothing.
#[test]
fn the_network_seam_is_the_only_door() {
    const FORBIDDEN: &[&str] = &[
        "std::net",
        "TcpStream",
        "TcpListener",
        "UdpSocket",
        "reqwest",
        "ureq",
        "hyper",
        "curl",
    ];

    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offences = Vec::new();
    for entry in std::fs::read_dir(&src).expect("read src/") {
        let path = entry.expect("src entry").path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        if path.file_name().is_some_and(|name| name == "net.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read source file");
        for (number, line) in text.lines().enumerate() {
            // Doc comments and ordinary comments name these primitives on purpose.
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            for needle in FORBIDDEN {
                if code.contains(needle) {
                    offences.push(format!("{}:{}: {needle}", path.display(), number + 1));
                }
            }
        }
    }

    assert!(
        offences.is_empty(),
        "network primitives outside src/net.rs:\n{}",
        offences.join("\n")
    );
}

/// The Acceptance item read literally: run the shipped binary with networking unavailable.
///
/// Skipped, loudly, where unprivileged network namespaces are not available.
#[test]
fn build_succeeds_with_networking_unavailable() {
    let Some(sandbox) = network_namespace_sandbox() else {
        eprintln!(
            "skipping: `unshare --user --map-root-user --net` is unavailable on this host, so \
             networking cannot be removed from the child; the seam-counter and source-audit tests \
             still cover the invariant"
        );
        return;
    };

    let fixture = Fixture::with_provider("netns", "zendesk");
    let mut command = std::process::Command::new(sandbox.0);
    command
        .args(sandbox.1)
        .arg(env!("CARGO_BIN_EXE_flux-connectors"))
        .arg("build")
        .arg("--root")
        .arg(fixture.root());

    let output = command.output().expect("run the binary under a netns");
    assert!(
        output.status.success(),
        "build failed with networking unavailable:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(fixture.exists("connectors/zendesk.flux"));
}

/// `Some((program, args))` if this host can drop the child into an empty network namespace.
fn network_namespace_sandbox() -> Option<(&'static str, &'static [&'static str])> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let program = "unshare";
    let args: &[&str] = &["--user", "--map-root-user", "--net", "--"];
    let probe = std::process::Command::new(program)
        .args(args)
        .arg("true")
        .output()
        .ok()?;
    probe.status.success().then_some((program, args))
}
