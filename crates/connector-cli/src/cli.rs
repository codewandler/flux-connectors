//! The `flux-connectors` command surface.
//!
//! Hand-rolled rather than derived from a parser crate: the surface is five subcommands and three
//! flags, and adding a dependency would collide with the connector stories in flight. If the surface
//! grows past this, swapping in a real parser is a contained change — [`parse`] is the only place
//! that knows argv exists.

use std::path::PathBuf;

use anyhow::{bail, Result};

/// The command line, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// What to do.
    pub command: Command,
    /// The repository root; `None` means the current directory.
    pub root: Option<PathBuf>,
    /// Restrict the run to one provider.
    pub provider: Option<String>,
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

COMMANDS:
    build      Compile providers/*.toml plus the vendored spec cache into connectors/
    diff       Show what `build` would change, without writing anything
    check      Verify artifacts against their inputs           (not yet implemented — story C-14)
    fetch      Refresh the vendored spec cache from upstream   (not yet implemented — story C-14)
    install    Install artifacts into ~/.flux                  (not yet implemented — story C-15)
    help       Print this message
    version    Print the version

OPTIONS:
    --provider <NAME>   Restrict the run to one connector
    --root <DIR>        Repository root (default: the current directory)
    -h, --help          Print this message
    -V, --version       Print the version

`build`, `diff` and `check` are hermetic and offline: they compile committed bytes. `fetch` is the
only command that contacts a vendor.";

/// Parse an argument list that does **not** include the program name.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Invocation> {
    let mut args = args.into_iter();

    let Some(first) = args.next() else {
        return Ok(Invocation {
            command: Command::Help,
            root: None,
            provider: None,
        });
    };
    let command = Command::parse(&first)?;

    let mut root = None;
    let mut provider = None;
    while let Some(arg) = args.next() {
        match split_flag(&arg) {
            Some(("--root", value)) => {
                root = Some(PathBuf::from(value_of("--root", value, &mut args)?));
            }
            Some(("--provider" | "-p", value)) => {
                provider = Some(value_of("--provider", value, &mut args)?);
            }
            Some(("--help" | "-h", _)) => return Ok(help()),
            Some(("--version" | "-V", _)) => {
                return Ok(Invocation {
                    command: Command::Version,
                    root: None,
                    provider: None,
                })
            }
            Some((flag, _)) => bail!("unknown option `{flag}` for `{first}`\n\n{USAGE}"),
            None => bail!("unexpected argument `{arg}` for `{first}`\n\n{USAGE}"),
        }
    }

    Ok(Invocation {
        command,
        root,
        provider,
    })
}

fn help() -> Invocation {
    Invocation {
        command: Command::Help,
        root: None,
        provider: None,
    }
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
    fn a_stray_positional_is_an_error() {
        // Catching this matters: `flux-connectors build zendesk` must not silently build everything.
        assert!(parse_args(&["build", "zendesk"]).is_err());
    }
}
