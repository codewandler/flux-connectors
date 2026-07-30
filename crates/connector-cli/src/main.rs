//! The `flux-connectors` binary: fetch, check, build, diff, install.
//!
//! A thin shell over the [`connector_cli`] library, which holds the whole command surface so that
//! integration tests drive exactly the code path this entry point does.
//!
//! This is the only crate in the workspace permitted to touch the network — `connector-spec` and
//! `connector-flux` stay pure so they remain unit-testable offline — and within it, only
//! `connector_cli::net` may. `build` and `diff` never do.

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = std::env::args().skip(1);

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let result = connector_cli::cli::parse(args)
        .and_then(|invocation| connector_cli::run(&invocation, &mut out));

    if let Err(error) = result.and_then(|()| Ok(out.flush()?)) {
        // `{:#}` renders the whole `anyhow` context chain, which is where the provider name and the
        // failing path live.
        eprintln!("error: {error:#}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
