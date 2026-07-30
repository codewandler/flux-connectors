//! The catalog's committed artifacts are a **checked** artifact (C-38).
//!
//! `crates/catalog` embeds its data at compile time, which means the bytes under
//! `crates/catalog/ops/` and `crates/catalog/src/generated/` are inputs to a *Rust build* rather
//! than files anyone reads at runtime. A stale one is therefore invisible: the crate still compiles,
//! still answers every query, and hands out Flux that no longer matches `providers/*.toml`. Nothing
//! in `cargo build` would say a word.
//!
//! So staleness is detected here instead, by recomputing the artifacts from the shipped provider
//! definitions and comparing byte for byte. `flux-connectors check` is C-14's and does not exist
//! yet; when it lands it should call the same comparison rather than reimplement it. Until then
//! this test *is* the check, and `flux-connectors diff` — which already plans the catalog artifacts
//! alongside the provider modules — is its interactive half.
//!
//! The test reads `providers/` and `crates/catalog/` from the repository root rather than from a
//! fixture, because the thing under test is what ships.

use std::path::{Path, PathBuf};

use connector_cli::pipeline;
use connector_cli::workspace::Workspace;
use connector_spec::Connector;

/// Every provider this repository ships: C-17's original three, then each connector added
/// since — `github` (C-52), `openai` (C-51), `slack` (C-53).
const SHIPPED: &[&str] = &["zendesk", "freshdesk", "babelforce", "github", "openai", "slack"];

/// The repository root, derived from this crate's manifest directory so the test is independent of
/// the working directory a runner happens to use.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

/// `crates/catalog/ops/<provider>` — one `.flux` rendering per operation.
fn ops_dir(provider: &str) -> PathBuf {
    repo_root().join("crates/catalog/ops").join(provider)
}

/// `path` relative to the repository root, for a message a reader can act on.
fn rel(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn load(provider: &str) -> Connector {
    let path = repo_root()
        .join("providers")
        .join(format!("{provider}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    connector_spec::provider::load(&format!("providers/{provider}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{provider}.toml does not load: {error}"))
        .connector
}

/// The file names actually present under `crates/catalog/ops/<provider>`, sorted.
fn committed_renderings(provider: &str) -> Vec<String> {
    let dir = ops_dir(provider);
    let mut names: Vec<String> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .map(|entry| entry.expect("readable directory entry").file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".flux"))
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("cannot read {}: {error}", rel(&dir)),
    };
    names.sort();
    names
}

/// **The catalog's source of truth, pinned.** Every shipped operation has exactly one committed
/// `.flux` rendering, and that rendering is byte-identical to what the emitter produces today.
///
/// Both halves matter and they fail differently. A *missing* file means the catalog does not carry
/// an operation the provider publishes — a consumer asking for it gets `None` while
/// `connectors/<provider>.flux` declares it happily. A *stale* file means the catalog hands out
/// Flux the provider definition no longer describes, which is worse: it looks like an answer.
///
/// The orphan check is the third failure. Nothing in the pipeline deletes, so an operation removed
/// from a provider TOML leaves its rendering behind; the file stays on disk, out of the generated
/// module, and reads to a human as though it were still shipped.
#[test]
fn every_shipped_operation_has_a_committed_flux_rendering() {
    for provider in SHIPPED {
        let connector = load(provider);
        let mut expected: Vec<String> = Vec::new();

        for operation in &connector.operations {
            let rendered = connector_flux::emit_operation(&connector, operation)
                .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
            let path = ops_dir(provider).join(format!("{}.flux", operation.id));

            let committed = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!(
                    "{} is missing or unreadable ({error}) — the catalog does not carry `{}`. Run \
                     `cargo run -p connector-cli -- build`",
                    rel(&path),
                    operation.id
                )
            });
            assert_eq!(
                committed,
                rendered,
                "{} is stale: the committed rendering of `{}` is not what the emitter produces \
                 today. Run `cargo run -p connector-cli -- build`",
                rel(&path),
                operation.id
            );

            expected.push(format!("{}.flux", operation.id));
        }

        expected.sort();
        assert_eq!(
            committed_renderings(provider),
            expected,
            "{} holds renderings that no operation in providers/{provider}.toml claims — an \
             orphaned file is one nothing regenerates and nothing deletes",
            rel(&ops_dir(provider))
        );
    }
}

/// **A rebuild from unchanged inputs is byte-identical**, over every artifact a build writes — the
/// six that ship *and* the catalog's thirty-odd.
///
/// [`pipeline::plan`] compiles everything and compares it against the committed tree without
/// writing, so this is the same comparison `flux-connectors diff` renders and `build` acts on. An
/// up-to-date plan means every generated byte in the repository is reproducible from
/// `providers/*.toml` alone.
///
/// This is the assertion standing in for `flux-connectors check`, which is C-14's and does not
/// exist yet. When it lands it should call `plan` and fail on `!is_up_to_date()` rather than grow
/// its own notion of staleness.
#[test]
fn the_committed_tree_is_a_fixed_point_of_a_build() {
    let workspace = Workspace::new(repo_root());
    let plan = pipeline::plan(&workspace, None).expect("every shipped provider compiles");

    let stale: Vec<String> = plan
        .changes()
        .map(|artifact| {
            format!(
                "{} ({:?})",
                workspace.display_path(&artifact.path).display(),
                artifact.change
            )
        })
        .collect();

    assert!(
        stale.is_empty(),
        "a rebuild would change committed artifacts — run `cargo run -p connector-cli -- build`:\n  {}",
        stale.join("\n  ")
    );
}

/// **The catalog's generated tables are checked too**, not merely the Flux they embed.
///
/// The table is where the metadata lives — risk, idempotency, credentials, hosts — so a stale one
/// answers a caller's "may I run this?" from a provider definition that has since changed. It is
/// also the file that decides *which* renderings are embedded at all, which is how an operation
/// removed upstream could keep being handed out.
#[test]
fn every_generated_catalog_module_matches_its_provider() {
    let workspace = Workspace::new(repo_root());

    for provider in SHIPPED {
        let expected = connector_cli::seam::emit(&load(provider))
            .unwrap_or_else(|error| panic!("providers/{provider}.toml does not compile: {error:#}"))
            .catalog;
        let path = workspace.catalog_module_path(provider);
        let committed = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "{} is missing or unreadable ({error}) — run `cargo run -p connector-cli -- build`",
                rel(&path)
            )
        });

        assert_eq!(
            committed,
            expected,
            "{} is stale. Run `cargo run -p connector-cli -- build`",
            rel(&path)
        );
    }
}

/// **The per-operation renderings are additional, not a substitution.** Each one is byte for byte
/// a slice of the provider module that ships, so the catalog and `~/.flux/flows` can never describe
/// two different requests for the same operation.
///
/// The story settles this explicitly: `connectors/<name>.flux` is unchanged in role, and it is what
/// a user installs. This is the assertion that keeps a later refactor from quietly making the
/// per-operation files the real artifact and the module a summary of them.
#[test]
fn every_rendering_is_the_text_the_shipped_module_carries() {
    for provider in SHIPPED {
        let module_path = repo_root()
            .join("connectors")
            .join(format!("{provider}.flux"));
        let module = std::fs::read_to_string(&module_path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", rel(&module_path)));

        for name in committed_renderings(provider) {
            let path = ops_dir(provider).join(&name);
            let rendering = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", rel(&path)));
            assert!(
                module.contains(&rendering),
                "{} is not the declaration connectors/{provider}.flux carries — the module that \
                 ships and the catalog have diverged",
                rel(&path)
            );
        }
    }
}
