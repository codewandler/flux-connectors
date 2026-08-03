//! The `flux-connectors` command surface.
//!
//! Hand-rolled rather than derived from a parser crate: the surface is seven subcommands, six flags
//! and one positional, and adding a dependency would collide with the connector stories in flight.
//! If the surface grows past this, swapping in a real parser is a contained change — [`parse`] is
//! the only place that knows argv exists.

use std::path::PathBuf;

use anyhow::{bail, Result};

/// The command line, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// What to do.
    pub command: Command,
    /// The repository root; `None` means the current directory.
    pub root: Option<PathBuf>,
    /// `migration-check` only: the explicit Flux checkout whose native plugin workspace is checked.
    pub flux_root: Option<PathBuf>,
    /// Restrict the run to one provider.
    pub provider: Option<String>,
    /// Restrict the run to one whole service of each provider it covers — a service name or a
    /// rendered gid (C-49). Normally paired with `--provider`, since a service is a level *within* a
    /// provider; a provider in the run that has no such service is a loud error naming what it has.
    pub service: Option<String>,
    /// Also rasterize the README snippet to PNG (`build` only). See [`crate::png`].
    pub png: bool,
    /// `scaffold` only: which operations to select, in the order they were written — C-419.
    ///
    /// Each entry is one `[[patch.select]]` statement, spelled
    /// `<service>:<path_prefix>:<METHOD,METHOD>`; see [`USAGE`] for the grammar and
    /// [`crate::scaffold`] for why selection is an argument rather than something the helper infers.
    pub selects: Vec<String>,
    /// `scaffold` only: report the document against the connector as it stands, rather than
    /// emitting TOML — C-419.
    pub diff: bool,
}

impl Invocation {
    /// A command with no options, for the paths that answer before reading any.
    fn bare(command: Command) -> Self {
        Self {
            command,
            root: None,
            flux_root: None,
            provider: None,
            service: None,
            png: false,
            selects: Vec::new(),
            diff: false,
        }
    }
}

/// A subcommand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Compile committed inputs into committed artifacts.
    Build,
    /// Show what a rebuild would change, without writing.
    Diff,
    /// Verify artifacts and the lockfile against their inputs. Lands with C-14.
    Check,
    /// Refresh the vendored spec cache from upstream. Lands with C-14.
    Fetch,
    /// Install artifacts into a local flux. Lands with C-15.
    Install,
    /// Write the provider TOML that references a vendored document, to stdout — C-419.
    Scaffold,
    /// Check the retained native-plugin inventory against an explicit Flux checkout — C-505.
    MigrationCheck,
    /// Print usage.
    Help,
    /// Print the version.
    Version,
}

impl Command {
    fn parse(token: &str) -> Result<Self> {
        Ok(match token {
            "build" => Command::Build,
            "diff" => Command::Diff,
            "check" => Command::Check,
            "fetch" => Command::Fetch,
            "install" => Command::Install,
            "scaffold" => Command::Scaffold,
            "migration-check" => Command::MigrationCheck,
            "help" | "-h" | "--help" => Command::Help,
            "version" | "-V" | "--version" => Command::Version,
            other => bail!("unknown command `{other}`\n\n{USAGE}"),
        })
    }
}

/// Usage text, printed by `help` and appended to argument errors.
pub const USAGE: &str = "\
flux-connectors — compile vendor API specs into Flux-Lang

USAGE:
    flux-connectors <COMMAND> [OPTIONS]
    flux-connectors scaffold <PROVIDER> [--select <SEL>]... [--diff]

COMMANDS:
    build      Compile providers/*.toml plus the vendored spec cache into connectors/
    diff       Show what `build` would change, without writing anything
    scaffold   Write the provider TOML that references a vendored document, to stdout
    migration-check
               Check native-plugin inventory and retirement evidence against a Flux checkout
    check      Verify artifacts against their inputs           (not yet implemented — story C-14)
    fetch      Refresh the vendored spec cache from upstream   (not yet implemented — story C-14)
    install    Install artifacts into ~/.flux                  (not yet implemented — story C-15)
    help       Print this message
    version    Print the version

OPTIONS:
    --select <SEL>      `scaffold` only, repeatable: one `[[patch.select]]` statement,
                        spelled <service>:<path_prefix>:<METHOD,METHOD> — for example
                        `manager:/api/v2/agents:GET,POST`. Fields may be dropped from
                        the right (`manager:/api/v2`, `manager`) and left empty
                        (`:/api/v2:GET`); an empty field states nothing, which is
                        C-411's absent key. Without any --select every document is
                        selected whole, split by method class
    --diff              `scaffold` only: report what the documents declare that this
                        connector does not publish, and the reverse, instead of
                        emitting TOML
    --provider <NAME>   Restrict the run to one connector
    --service <NAME>    Restrict the run to one whole service of that connector,
                        by service name (`s3`) or by its address
                        (`com.amazonaws/s3:2006-03-01`). A provider with a single
                        API surface has one service, `default`, and needs no flag
    --root <DIR>        Repository root (default: the current directory)
    --flux-root <DIR>   `migration-check` only: the Flux repository checkout to inspect
    --png               `build` only: also rasterize the README snippet to
                        assets/readme-snippet.png with the `flux` binary. Skipped
                        with a message when flux is not installed; the README's
                        SVGs are rendered by every build either way
    -h, --help          Print this message
    -V, --version       Print the version

`build`, `diff`, `check`, `scaffold` and `migration-check` are hermetic and offline: they read
committed bytes. `fetch` is the only command that contacts a vendor. `scaffold` writes to stdout and
never over a file in place — the author diffs and pastes, so a bad run costs nothing.";

/// Parse an argument list that does **not** include the program name.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Invocation> {
    let mut args = args.into_iter();

    let Some(first) = args.next() else {
        return Ok(Invocation::bare(Command::Help));
    };
    let command = Command::parse(&first)?;

    let mut root = None;
    let mut flux_root = None;
    let mut provider = None;
    let mut service = None;
    let mut png = false;
    let mut selects = Vec::new();
    let mut diff = false;
    while let Some(arg) = args.next() {
        match split_flag(&arg) {
            Some(("--root", value)) => {
                root = Some(PathBuf::from(value_of("--root", value, &mut args)?));
            }
            Some(("--flux-root", value)) if command == Command::MigrationCheck => {
                flux_root = Some(PathBuf::from(value_of("--flux-root", value, &mut args)?));
            }
            Some(("--provider" | "-p", value)) => {
                provider = Some(value_of("--provider", value, &mut args)?);
            }
            Some(("--service" | "-s", value)) => {
                service = Some(value_of("--service", value, &mut args)?);
            }
            // Only `build` writes anything, so only `build` can be asked to write one more thing.
            // Accepting it elsewhere would promise a raster that `diff` has no way to produce.
            Some(("--png", _)) if command == Command::Build => png = true,
            // `scaffold` is the only command that states a selection, and the only one that reports
            // instead of compiling. Accepting either elsewhere would promise a `build` that selects
            // — which is the provider file's decision and only its.
            Some(("--select", value)) if command == Command::Scaffold => {
                selects.push(value_of("--select", value, &mut args)?);
            }
            Some(("--diff", _)) if command == Command::Scaffold => diff = true,
            Some(("--help" | "-h", _)) => return Ok(Invocation::bare(Command::Help)),
            Some(("--version" | "-V", _)) => return Ok(Invocation::bare(Command::Version)),
            Some((flag, _)) => bail!("unknown option `{flag}` for `{first}`\n\n{USAGE}"),
            // `scaffold <provider>`, the one positional in the surface. It is spelled positionally
            // because the provider is not a *restriction* on a whole-catalogue run the way
            // `build --provider` is — a scaffold of nothing in particular has no meaning — and
            // `--provider` stays accepted so the two spellings do not have to be remembered apart.
            None if command == Command::Scaffold && provider.is_none() => {
                provider = Some(arg);
            }
            None => bail!("unexpected argument `{arg}` for `{first}`\n\n{USAGE}"),
        }
    }

    Ok(Invocation {
        command,
        root,
        flux_root,
        provider,
        service,
        png,
        selects,
        diff,
    })
}

/// Split `--flag=value` into its parts; `None` when the argument is not a flag at all.
fn split_flag(arg: &str) -> Option<(&str, Option<&str>)> {
    if !arg.starts_with('-') {
        return None;
    }
    match arg.split_once('=') {
        Some((flag, value)) => Some((flag, Some(value))),
        None => Some((arg, None)),
    }
}

/// The value of a flag, whether it arrived as `--flag=value` or `--flag value`.
fn value_of(
    flag: &str,
    inline: Option<&str>,
    rest: &mut impl Iterator<Item = String>,
) -> Result<String> {
    if let Some(value) = inline {
        if value.is_empty() {
            bail!("`{flag}` needs a value");
        }
        return Ok(value.to_string());
    }
    match rest.next() {
        Some(value) if !value.starts_with('-') => Ok(value),
        _ => bail!("`{flag}` needs a value"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Invocation> {
        parse(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn no_arguments_prints_help() {
        assert_eq!(parse_args(&[]).unwrap().command, Command::Help);
    }

    #[test]
    fn every_command_parses() {
        for (token, expected) in [
            ("build", Command::Build),
            ("diff", Command::Diff),
            ("check", Command::Check),
            ("fetch", Command::Fetch),
            ("install", Command::Install),
            ("scaffold", Command::Scaffold),
            ("migration-check", Command::MigrationCheck),
        ] {
            assert_eq!(parse_args(&[token]).unwrap().command, expected);
        }
    }

    #[test]
    fn provider_accepts_both_spellings() {
        let separate = parse_args(&["build", "--provider", "zendesk"]).unwrap();
        let inline = parse_args(&["build", "--provider=zendesk"]).unwrap();
        assert_eq!(separate.provider.as_deref(), Some("zendesk"));
        assert_eq!(separate, inline);
    }

    #[test]
    fn root_is_captured() {
        let invocation = parse_args(&["diff", "--root", "/tmp/repo"]).unwrap();
        assert_eq!(invocation.root, Some(PathBuf::from("/tmp/repo")));
    }

    #[test]
    fn migration_check_requires_its_own_explicit_flux_root_flag() {
        let invocation = parse_args(&["migration-check", "--flux-root", "/src/flux"]).unwrap();
        assert_eq!(invocation.flux_root, Some(PathBuf::from("/src/flux")));
        assert!(parse_args(&["build", "--flux-root", "/src/flux"]).is_err());
    }

    #[test]
    fn a_flag_without_a_value_is_an_error() {
        assert!(parse_args(&["build", "--provider"]).is_err());
        assert!(parse_args(&["build", "--provider="]).is_err());
    }

    #[test]
    fn an_unknown_command_is_an_error_that_shows_usage() {
        let error = parse_args(&["bulid"]).unwrap_err();
        let rendered = format!("{error:#}");
        assert!(rendered.contains("bulid"));
        assert!(rendered.contains("USAGE"));
    }

    #[test]
    fn an_unknown_option_is_an_error() {
        assert!(parse_args(&["build", "--verbose"]).is_err());
    }

    #[test]
    fn png_is_a_build_only_flag() {
        assert!(parse_args(&["build", "--png"]).unwrap().png);
        assert!(!parse_args(&["build"]).unwrap().png);
        // Offering it on a command that writes nothing would promise a raster `diff` cannot make.
        assert!(parse_args(&["diff", "--png"]).is_err());
    }

    #[test]
    fn a_stray_positional_is_an_error() {
        // Catching this matters: `flux-connectors build zendesk` must not silently build everything.
        assert!(parse_args(&["build", "zendesk"]).is_err());
    }

    #[test]
    fn scaffold_names_its_provider_positionally() {
        let positional = parse_args(&["scaffold", "babelforce"]).unwrap();
        assert_eq!(positional.provider.as_deref(), Some("babelforce"));
        assert_eq!(
            positional,
            parse_args(&["scaffold", "--provider", "babelforce"]).unwrap()
        );
        // A second one is an error rather than a silent last-wins: two provider names is a command
        // line whose author meant something this surface cannot do.
        assert!(parse_args(&["scaffold", "babelforce", "zendesk"]).is_err());
    }

    #[test]
    fn select_is_repeatable_and_scaffold_only() {
        let invocation = parse_args(&[
            "scaffold",
            "babelforce",
            "--select",
            "manager:/api/v2:GET",
            "--select=auth:/oauth",
        ])
        .unwrap();
        assert_eq!(
            invocation.selects,
            vec!["manager:/api/v2:GET".to_owned(), "auth:/oauth".to_owned()]
        );
        // `build` compiles what the provider file selects; a selection on its command line would be
        // a second, invisible source of truth for the same decision.
        assert!(parse_args(&["build", "--select", "manager:/api/v2"]).is_err());
    }

    #[test]
    fn diff_is_a_scaffold_flag_and_a_command_of_its_own() {
        assert!(
            parse_args(&["scaffold", "babelforce", "--diff"])
                .unwrap()
                .diff
        );
        assert!(!parse_args(&["scaffold", "babelforce"]).unwrap().diff);
        assert_eq!(parse_args(&["diff"]).unwrap().command, Command::Diff);
        assert!(parse_args(&["build", "--diff"]).is_err());
    }
}
