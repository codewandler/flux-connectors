//! The two wiring points between the CLI's orchestration and the compiler crates.
//!
//! `build` and `diff` are orchestration: discover providers, read committed bytes, compile, compare,
//! write. The compiling itself belongs to two other crates:
//!
//! | Stage | Owner | Function here |
//! |---|---|---|
//! | provider TOML -> `Connector` IR | `connector-spec` (**C-3**) | [`load`] |
//! | `Connector` IR -> `.flux` + `.connector.toml` | `connector-flux` (**C-8**) | [`emit`] |
//!
//! Both are wired (C-27); this module is the only place either crate is named, and both stages are
//! pure functions of bytes, which is why all IO lives in [`crate::pipeline`].
//!
//! # What is still stubbed here, and by whom it is finished
//!
//! - **Spec ingest is C-4.** [`ProviderInputs`] already carries the vendored spec document, and
//!   `connector-spec`'s loader deliberately does not take it: turning an OpenAPI document into
//!   operations is ingest's job, not the loader's. Until C-4 lands, a provider that points at a
//!   spec compiles to *no operations at all*, so [`load`] refuses it rather than writing an empty
//!   module — see the note on [`load`].
//! - **The manifest is C-10's.** [`emit`] derives `<name>.connector.toml` from the IR here, because
//!   `connector-flux` emits Flux and nothing else. C-10 replaces its body with the real capability
//!   manifest — `http_hosts`, the endpoint env spec, and the credential declarations.

use anyhow::{bail, Result};
use serde::Serialize;

use crate::discovery::Provider;

/// A loaded, validated connector, ready to emit.
///
/// Re-exported rather than wrapped: nothing in this crate outside [`emit`] inspects it, so the IR
/// travels through orchestration untranslated.
pub use connector_spec::Connector;

/// The generator identity stamped into every artifact.
///
/// Part of the hash domain `connectors.lock` will record (C-7): a generator change must invalidate
/// generated output, or a stale artifact survives a codegen fix.
pub fn generator() -> String {
    format!("flux-connectors {}", env!("CARGO_PKG_VERSION"))
}

/// One provider's committed inputs, already read into memory.
///
/// Bytes, not paths, deliberately: it is what keeps [`load`] pure and lets `connector-spec` stay
/// fully unit-testable offline.
#[derive(Debug, Clone)]
pub struct ProviderInputs {
    /// The provider name.
    pub name: String,
    /// The contents of `providers/<name>.toml`.
    pub definition: String,
    /// The vendored spec, when the provider has one.
    ///
    /// Read but not yet consumed: ingesting it is C-4's job. See the module docs.
    pub spec: Option<SpecInput>,
}

/// A vendored spec document, already read into memory.
#[derive(Debug, Clone)]
pub struct SpecInput {
    /// The upstream version, from the cache file's stem.
    pub version: String,
    /// The document's bytes.
    pub document: String,
}

impl ProviderInputs {
    /// Read everything discovery found for one provider.
    pub fn read(provider: &Provider) -> Result<Self> {
        let definition = crate::artifact::read(&provider.definition)?;
        let spec = match provider.spec() {
            Some(spec) => Some(SpecInput {
                version: spec.version.clone(),
                document: crate::artifact::read(&spec.path)?,
            }),
            None => None,
        };
        Ok(Self {
            name: provider.name.clone(),
            definition,
            spec,
        })
    }

    /// How the definition names itself in an error — `providers/zendesk.toml`.
    ///
    /// `connector_spec::provider::load` uses its `name` argument only to label diagnostics, so the
    /// caller decides what an author sees. A path is what they can open.
    fn label(&self) -> String {
        format!("{}/{}.toml", crate::workspace::PROVIDERS_DIR, self.name)
    }
}

/// The two files one connector compiles to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifacts {
    /// The `.flux` module: the `op` declarations flux loads.
    pub module: String,
    /// The `.connector.toml` manifest: what the connector needs in order to run.
    pub manifest: String,
}

// ---------------------------------------------------------------------------------------------
// WIRING POINT 1 of 2 — C-3
// ---------------------------------------------------------------------------------------------

/// Parse and validate a provider's inputs into the connector IR.
///
/// This is `connector_spec::provider::load`: bytes in, a validated [`Connector`] out, no IO and no
/// network. Every rule an author can break — an unknown key, a missing method or path, a
/// requirement naming an undeclared credential — is diagnosed there, with *every* problem in the
/// file reported at once rather than one per run.
///
/// # A provider that points at a spec is refused, for now
///
/// The loader returns such a file as a *skeleton*: id, base URL, credentials, and whatever
/// operations were written inline. Its operations come from ingesting the vendored document
/// (C-4) and applying the overlay (C-6), neither of which is wired. Emitting the skeleton would
/// produce a syntactically valid module declaring nothing, which is worse than a failure — it
/// would pass C-11's parse-and-analyze gate while silently exposing none of the connector's
/// operations. So it fails loudly here and C-4 removes the refusal.
pub fn load(inputs: &ProviderInputs) -> Result<Connector> {
    let loaded = connector_spec::provider::load(&inputs.label(), &inputs.definition)?;

    if let Some(spec) = &loaded.spec {
        bail!(
            "`{}` points at the vendored spec `{}`, and compiling a spec-backed provider needs \
             spec ingest (story C-4), which is not wired yet. Until it is, only a fully \
             hand-authored connector — one that writes its `[[operations]]` inline — can be built",
            inputs.label(),
            spec.path
        );
    }

    Ok(loaded.connector)
}

// ---------------------------------------------------------------------------------------------
// WIRING POINT 2 of 2 — C-8
// ---------------------------------------------------------------------------------------------

/// Compile a connector into its two artifacts.
///
/// The module comes from `connector_flux::emit_operation`, which builds real `flux_lang` AST nodes
/// and formats them with flux-lang's own formatter — never string templates (AGENTS.md). Only the
/// file *envelope* is assembled here, and it is comments, which have no AST to build.
///
/// The contract this module owes its callers:
///
/// - **Deterministic.** Equal inputs produce byte-identical output, on every platform and every
///   run. `build` being a no-op over unchanged inputs rests entirely on this. Both halves are
///   functions of the IR alone, and the IR's own ordering is fixed, so nothing here can vary.
/// - **Total.** Either both artifacts are produced or the call fails; a connector is a manifest
///   *plus* a module, never one of them. Both are built before either is returned.
/// - **Text, not files.** Writing is [`crate::pipeline`]'s job, so a failure cannot leave a partial
///   tree behind.
pub fn emit(connector: &Connector) -> Result<Artifacts> {
    let module = module(connector)?;
    let manifest = manifest(connector)?;
    Ok(Artifacts { module, manifest })
}

/// The `.flux` module: a generated-file header, then one `op` declaration per operation.
///
/// The header is `#` comments. Flux's comment character is `#` — `//` is not a comment and does not
/// parse — and comment-only lines are trivia the lexer drops, so the header cannot affect what the
/// module declares.
fn module(connector: &Connector) -> Result<String> {
    let mut module = format!(
        "# Generated by {} — do not edit.\n# Provider: {}\n# Regenerate with `flux-connectors build`.\n",
        generator(),
        connector.id
    );
    for operation in &connector.operations {
        module.push('\n');
        module.push_str(&connector_flux::emit_operation(connector, operation)?);
    }
    Ok(module)
}

/// The `.connector.toml` manifest.
///
/// **A placeholder shape, and scoped like one.** The design's manifest carries `http_hosts`, the
/// endpoint env spec and the credential declarations, mirroring flux's plugin-protocol vocabulary —
/// all of which is C-10's Acceptance, and none of which can be written honestly before auth is
/// modelled. What is here is what the IR already knows and a reviewer needs in order to read a
/// diff: which connector this is, where it points, and which operations it publishes.
fn manifest(connector: &Connector) -> Result<String> {
    /// The manifest's wire shape. Field order is the emitted order, which is what makes the output
    /// deterministic without sorting anything at runtime.
    #[derive(Serialize)]
    struct Manifest<'a> {
        generator: String,
        connector: &'a str,
        #[serde(skip_serializing_if = "str::is_empty")]
        vendor: &'a str,
        #[serde(skip_serializing_if = "str::is_empty")]
        description: &'a str,
        base_url: &'a str,
        module: String,
        operations: Vec<&'a str>,
    }

    let manifest = Manifest {
        generator: generator(),
        connector: &connector.id,
        vendor: &connector.vendor,
        description: &connector.description,
        base_url: &connector.base_url,
        module: format!("{}.{}", connector.id, crate::workspace::MODULE_EXT),
        operations: connector
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect(),
    };

    let body = toml::to_string(&manifest)?;
    Ok(format!(
        "# Generated by {} — do not edit.\n# Auth and the `http_hosts` allowlist land in C-10.\n{body}",
        generator()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete hand-authored connector, in the form an author writes it.
    const HAND_AUTHORED: &str = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"

[[operations]]
id = "acme-ticket-show"
method = "GET"
path = "/v2/tickets/{ticket_id}"
description = "Fetch one Acme ticket."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "ticket_id"
required = true
schema = { type = "integer" }
"#;

    fn inputs(definition: &str) -> ProviderInputs {
        ProviderInputs {
            name: "acme".to_string(),
            definition: definition.to_string(),
            spec: None,
        }
    }

    #[test]
    fn emission_is_deterministic() {
        let first = emit(&load(&inputs(HAND_AUTHORED)).unwrap()).unwrap();
        let second = emit(&load(&inputs(HAND_AUTHORED)).unwrap()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn a_changed_definition_changes_both_artifacts() {
        let first = emit(&load(&inputs(HAND_AUTHORED)).unwrap()).unwrap();
        let changed = HAND_AUTHORED
            .replace("https://api.acme.example", "https://api.acme.test")
            .replace(r#"vendor = "Acme""#, r#"vendor = "Acme Inc.""#);
        let second = emit(&load(&inputs(&changed)).unwrap()).unwrap();
        assert_ne!(first.module, second.module);
        assert_ne!(first.manifest, second.manifest);
    }

    #[test]
    fn an_empty_definition_is_rejected() {
        let error = load(&inputs("   \n")).expect_err("empty definitions must not load");
        assert!(format!("{error:#}").contains("acme"));
    }

    /// The loader's diagnosis must reach the user with the file it is about, not be flattened into
    /// something this crate invented.
    #[test]
    fn the_loaders_own_diagnosis_survives() {
        let error = load(&inputs("id = \"acme\"\n")).expect_err("a connector needs a base URL");
        let rendered = format!("{error:#}");
        assert!(rendered.contains("providers/acme.toml"), "{rendered}");
        assert!(rendered.contains("base_url"), "{rendered}");
    }

    /// Until C-4 lands there is nothing to compile a spec-backed provider *from*, and an empty
    /// module would pass a parse gate while publishing nothing.
    #[test]
    fn a_spec_backed_provider_is_refused_rather_than_emitted_empty() {
        let definition = "\
id = \"acme\"
base_url = \"https://api.acme.example\"

[spec]
path = \"specs/acme/v1.json\"
";
        let error = load(&inputs(definition)).expect_err("spec ingest is not wired");
        let rendered = format!("{error:#}");
        assert!(rendered.contains("C-4"), "{rendered}");
        assert!(rendered.contains("specs/acme/v1.json"), "{rendered}");
    }

    #[test]
    fn the_module_declares_the_operation() {
        let artifacts = emit(&load(&inputs(HAND_AUTHORED)).unwrap()).unwrap();
        assert!(
            artifacts
                .module
                .contains("op acme-ticket-show(ticket_id: Number) -> Any"),
            "{}",
            artifacts.module
        );
    }

    /// An operation the emitter cannot spell must fail the whole call, not yield a module missing
    /// one op — that is what "total" means here.
    #[test]
    fn an_unemittable_operation_fails_the_whole_emission() {
        let definition = HAND_AUTHORED.replace("acme-ticket-show", "acme.ticket.show");
        let connector =
            load(&inputs(&definition)).expect("a dotted id loads; only Flux refuses it");
        emit(&connector).expect_err("a dotted op id cannot be declared in Flux");
    }
}
