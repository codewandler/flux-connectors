//! Finding out which providers exist, from committed files alone.
//!
//! Discovery is deterministic and total: the same tree always yields the same providers in the same
//! order, and anything it cannot classify is an error rather than a silent omission. A provider
//! that quietly disappears from a build is the failure mode this repo exists to prevent.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::workspace::Workspace;

/// One vendored spec document in the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecFile {
    /// The upstream version, taken from the file stem: `specs/zendesk/2024-06-01.json` -> `2024-06-01`.
    pub version: String,
    /// Where it lives.
    pub path: PathBuf,
}

/// A provider as the committed tree describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    /// The provider name, taken from the definition's file stem.
    pub name: String,
    /// `providers/<name>.toml`.
    pub definition: PathBuf,
    /// **Every** vendored spec for this provider, ordered by version — the cache, whole.
    ///
    /// There is deliberately no accessor that picks one of these, and that absence is C-410's whole
    /// point at this layer. `Provider::spec()` used to return `self.specs.last()` — the highest file
    /// stem — which for babelforce's five documents selected the four-operation `user-2026-06-25`
    /// over the 356-operation `manager-2026-07-10`, silently and with exit 0. Discovery cannot make
    /// that choice correctly because the choice is not discovery's: which documents a connector
    /// compiles from, and how many, is stated by `[spec]` / `[[spec]]` in the provider file and is
    /// resolved where those are read (`connector_spec::provider::load_with_spec`).
    ///
    /// Empty is normal, not an error — a hand-authored connector such as Ollama has no vendor
    /// document at all. That is the "two front-ends, one IR" requirement.
    pub specs: Vec<SpecFile>,
}

/// Every provider in the workspace, ordered by name; `only` restricts the result to one.
pub fn discover(workspace: &Workspace, only: Option<&str>) -> Result<Vec<Provider>> {
    let dir = workspace.providers_dir();
    if !dir.is_dir() {
        bail!(
            "no `{}` directory at {} — a build compiles committed provider definitions, so there \
             must be at least one",
            crate::workspace::PROVIDERS_DIR,
            dir.display()
        );
    }

    let mut names = Vec::new();
    for entry in read_dir_sorted(&dir)? {
        if entry.is_dir() {
            continue;
        }
        if entry.extension().is_none_or(|ext| ext != "toml") {
            continue;
        }
        names.push(provider_name(&entry)?);
    }
    names.sort();

    if let Some(wanted) = only {
        if !names.iter().any(|name| name == wanted) {
            bail!(
                "unknown provider `{wanted}`; {}",
                describe_available(&names)
            );
        }
        names.retain(|name| name == wanted);
    }

    if names.is_empty() {
        bail!("no provider definitions found in {}", dir.display());
    }

    names
        .into_iter()
        .map(|name| {
            let specs = discover_specs(workspace, &name)?;
            Ok(Provider {
                definition: dir.join(format!("{name}.toml")),
                name,
                specs,
            })
        })
        .collect()
}

/// The vendored specs for one provider, ordered by version. An absent directory means none.
fn discover_specs(workspace: &Workspace, provider: &str) -> Result<Vec<SpecFile>> {
    let dir = workspace.spec_dir(provider);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut specs = Vec::new();
    for entry in read_dir_sorted(&dir)? {
        if entry.is_dir() {
            continue;
        }
        let Some(version) = entry.file_stem().and_then(|stem| stem.to_str()) else {
            bail!("spec file name is not valid UTF-8: {}", entry.display());
        };
        if version.starts_with('.') {
            continue;
        }
        specs.push(SpecFile {
            version: version.to_string(),
            path: entry,
        });
    }
    specs.sort_by(|a, b| a.version.cmp(&b.version));
    Ok(specs)
}

/// The provider name a definition path carries, rejecting anything that would not round-trip.
fn provider_name(path: &Path) -> Result<String> {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        bail!("provider file name is not valid UTF-8: {}", path.display());
    };
    let valid = !stem.is_empty()
        && stem
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !valid {
        bail!(
            "invalid provider name `{stem}` ({}): names must be lowercase ASCII letters, digits, \
             `-` or `_`, because the name becomes the artifact file name and the op-id prefix",
            path.display()
        );
    }
    Ok(stem.to_string())
}

/// Directory entries in a stable order — `read_dir` yields filesystem order, which is not one.
fn read_dir_sorted(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(dir)
        .with_context(|| format!("cannot read {}", dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("cannot read {}", dir.display()))?;
    paths.sort();
    Ok(paths)
}

fn describe_available(names: &[String]) -> String {
    if names.is_empty() {
        "no provider definitions exist in this workspace".to_string()
    } else {
        format!("available providers: {}", names.join(", "))
    }
}
