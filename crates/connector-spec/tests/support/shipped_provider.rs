//! **The one spec-aware way to load a shipped provider from a test** — C-421.
//!
//! # The split, stated once
//!
//! `connector_spec::provider` has two entry points and this repository's tests choose between them
//! by a single rule:
//!
//! - **bytes read from `providers/<name>.toml`** — or derived from those bytes, as the
//!   substitution-based refusal tests do — go through here, which resolves that provider's spec
//!   cache off disk and calls `provider::load_with_spec`;
//! - **TOML a test authored itself** — the fixtures, the golden-error cases, the `acme.toml`
//!   inventions — goes through `provider::load`, which is the pure loader and is the thing those
//!   tests are actually about.
//!
//! The rule is not stylistic. Before C-421 every one of those callers used `provider::load`, and
//! that was correct for exactly as long as all fifty-three shipped providers were hand-authored: a
//! file with no `[spec]` block ignores the cache, so passing one changes nothing. The moment one
//! shipped provider converts to `[spec]`, a definition read off disk stops being compilable from its
//! own bytes — its operations come from the vendored document too — and `provider::load` now refuses
//! it rather than answering with the zero-operation skeleton it used to return. C-416 measured what
//! that costs when the convention is wrong: **53 tests across 18 binaries in 4 crates**, from one
//! provider converting.
//!
//! Routing every disk-read load through here is what makes a conversion cost nothing. The helper
//! passes the **whole** cache and lets the loader resolve the pins, so a provider that gains,
//! changes or drops a `[spec]` entry needs no test change at all — which is the property C-417 and
//! C-420 depend on, since between them they convert the entire catalogue.
//!
//! # Why this file is `#[path]`-included rather than a crate
//!
//! Three crates' integration tests need it — `connector-spec`, `connector-flux`, `connector-cli` —
//! and a shared test crate would be a fourth workspace member and a `dev-dependencies` edge in each
//! of the three. One file, included by path, costs no manifest change and keeps the rule in one
//! place. It lives under `connector-spec` because that is the crate that owns provider-TOML loading.
//!
//! `env!("CARGO_MANIFEST_DIR")` is read where the module is *included*, not where it is written, so
//! it expands to the including crate's directory. Every crate that includes it lives at
//! `crates/<name>`, so `../..` is the repository root from all of them.
//!
//! # No filesystem IO moved into the library
//!
//! `AGENTS.md` records that `connector-spec` "ingest accepts bytes so it remains fully
//! unit-testable", and that the crate's IO belongs to `connector-cli`. This is test scaffolding, not
//! library code: it reads bytes and hands them to the same pure function the CLI's `seam::load`
//! hands them to. `crates/connector-cli/src/seam.rs` is still the only *shipping* caller.

// Each including binary uses a different part of this module.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use connector_spec::{provider, Connector, LoadedProvider, SpecDocument};

/// The repository root. See the module docs on `CARGO_MANIFEST_DIR`.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `providers/`, where every shipped definition lives.
pub fn providers_dir() -> PathBuf {
    root().join("providers")
}

/// `providers/<name>.toml`.
pub fn path(name: &str) -> PathBuf {
    providers_dir().join(format!("{name}.toml"))
}

/// How a shipped definition names itself in a diagnostic — `providers/zendesk.toml`.
///
/// The loader uses its `name` argument only to label errors, so this is what an author sees. A
/// repository-relative path is what they can open.
pub fn label(name: &str) -> String {
    format!("providers/{name}.toml")
}

/// One vendored document, read.
pub struct CachedDocument {
    /// The repository-relative path, spelled exactly as `[spec] path` spells it.
    pub path: String,
    /// The document's bytes as text.
    pub document: String,
}

/// One shipped provider's inputs, read: the definition and **every** document in its spec cache.
pub struct Sources {
    /// The provider name — `babelforce`.
    pub name: String,
    /// The contents of `providers/<name>.toml`.
    pub definition: String,
    /// Every file under `specs/<name>/`, sorted by file stem. Empty when the directory is absent,
    /// which is the ordinary case and not a distinct code path.
    pub cache: Vec<CachedDocument>,
}

impl Sources {
    /// The cache in the shape the loader takes.
    pub fn documents(&self) -> Vec<SpecDocument<'_>> {
        self.cache
            .iter()
            .map(|entry| SpecDocument {
                path: &entry.path,
                document: &entry.document,
            })
            .collect()
    }

    /// Load `definition` against this provider's cache.
    ///
    /// `definition` is a parameter rather than `self.definition` so a test that *doctors* the
    /// shipped bytes — the refusal tests that substitute one key and assert the loader objects —
    /// still compiles against the real cache instead of falling back to the pure loader.
    pub fn load(&self, definition: &str) -> connector_spec::Result<LoadedProvider> {
        provider::load_with_spec(&label(&self.name), definition, &self.documents())
    }
}

/// Read `providers/<name>.toml` and everything under `specs/<name>/`.
///
/// **The whole cache, unfiltered.** Which of its documents a connector compiles from — and how many
/// — is the provider file's decision and the loader's to resolve. A caller that picked one would be
/// deciding silently what only `[spec] path` may decide, which is the defect C-410 deleted
/// `Provider::spec()` for: it returned the last document by file stem, so a pin at an older version
/// compiled a newer one, successfully and with no diagnostic.
pub fn sources(name: &str) -> Sources {
    let definition_path = path(name);
    let definition = std::fs::read_to_string(&definition_path)
        .unwrap_or_else(|error| panic!("cannot read {} ({error})", definition_path.display()));

    let dir = root().join("specs").join(name);
    let mut cache = Vec::new();
    if dir.is_dir() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("cannot read {} ({error})", dir.display()))
            .map(|entry| entry.expect("readable directory entry").path())
            .filter(|entry| entry.is_file())
            .filter(|entry| {
                // A dotfile is not a vendored document, and neither is a directory. Mirrors
                // `connector_cli::discovery::discover_specs`, which is what the real build walks.
                entry
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| !stem.starts_with('.'))
            })
            .collect();
        files.sort();

        for file in files {
            let file_name = file
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_else(|| panic!("spec file name is not valid UTF-8: {}", file.display()))
                .to_owned();
            let document = std::fs::read_to_string(&file)
                .unwrap_or_else(|error| panic!("cannot read {} ({error})", file.display()));
            cache.push(CachedDocument {
                // Spelled as `[spec] path` spells it, which is what makes the pin resolvable.
                path: format!("specs/{name}/{file_name}"),
                document,
            });
        }
    }

    Sources {
        name: name.to_owned(),
        definition,
        cache,
    }
}

/// **The drop-in for `provider::load` at a site that already read the definition itself.**
///
/// This is the shape most of the migrated call sites took, because it changes one expression and
/// leaves each test's own `expect`/`unwrap_or_else` message — and there are eighty-odd of those,
/// each saying something specific about why that provider must load.
///
/// `definition` is passed rather than re-read for the same reason [`Sources::load`] takes one: a
/// test that doctors the shipped bytes to prove a refusal must still be compiled against the real
/// cache.
pub fn load_definition(name: &str, definition: &str) -> connector_spec::Result<LoadedProvider> {
    sources(name).load(definition)
}

/// Load one shipped provider, cache and all — for a caller that has not read the file yet.
pub fn load(name: &str) -> LoadedProvider {
    let sources = sources(name);
    sources
        .load(&sources.definition)
        .unwrap_or_else(|error| panic!("{} does not load: {error}", label(name)))
}

/// [`load`], keeping only the IR — the shape most callers want.
pub fn connector(name: &str) -> Connector {
    load(name).connector
}
