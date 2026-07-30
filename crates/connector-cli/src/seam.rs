//! The two wiring points between the CLI's orchestration and the compiler crates.
//!
//! `build` and `diff` are orchestration: discover providers, read committed bytes, compile, compare,
//! write. The compiling itself belongs to two other crates that are being written in parallel:
//!
//! | Stage | Owner | Function here |
//! |---|---|---|
//! | provider TOML (+ spec) -> `Connector` IR | `connector-spec` (**C-3**) | [`load`] |
//! | `Connector` IR -> `.flux` + `.connector.toml` | `connector-flux` (**C-8**) | [`emit`] |
//!
//! Until those land, both functions have placeholder bodies. **They are the only two places that
//! change when C-3 and C-8 arrive** — nothing outside this module inspects a [`Connector`], and no
//! other module knows how an artifact's text is produced. Both stages are pure functions of bytes,
//! which is why all IO lives in [`crate::pipeline`] and why the placeholders can be swapped without
//! touching a single caller.

use anyhow::{bail, Result};

use crate::discovery::Provider;

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
}

/// A loaded, validated connector, ready to emit.
///
/// **Placeholder (C-3).** This becomes `connector_spec::Connector`. It is opaque on purpose: no
/// caller reads its fields, so replacing it is a change confined to this module.
#[derive(Debug, Clone)]
pub struct Connector {
    name: String,
    spec_version: Option<String>,
    /// A digest of every input byte that produced this value.
    inputs_digest: u64,
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
/// **This body is a placeholder. Story C-3 (`crates/connector-spec`) replaces it** with a call to
/// the provider-TOML loader — roughly:
///
/// ```text
/// let connector = connector_spec::load(&inputs.definition, inputs.spec.as_ref().map(|s| s.document.as_str()))?;
/// ```
///
/// and the return type becomes `connector_spec::Connector`. The signature is already the shape that
/// requires: bytes in, IR out, no IO, no network.
///
/// Today it validates only what orchestration genuinely depends on — that the definition is not
/// empty — and records a digest of the inputs so that artifacts change when inputs change. Real
/// validation (unknown keys, missing method/path, schemeless credentials) is C-3's Acceptance.
pub fn load(inputs: &ProviderInputs) -> Result<Connector> {
    if inputs.definition.trim().is_empty() {
        bail!(
            "provider `{}` has an empty definition; a connector must declare at least an id",
            inputs.name
        );
    }

    let mut digest = Digest::new();
    digest.write(inputs.name.as_bytes());
    digest.write(inputs.definition.as_bytes());
    if let Some(spec) = &inputs.spec {
        digest.write(spec.version.as_bytes());
        digest.write(spec.document.as_bytes());
    }
    digest.write(generator().as_bytes());

    Ok(Connector {
        name: inputs.name.clone(),
        spec_version: inputs.spec.as_ref().map(|spec| spec.version.clone()),
        inputs_digest: digest.finish(),
    })
}

// ---------------------------------------------------------------------------------------------
// WIRING POINT 2 of 2 — C-8
// ---------------------------------------------------------------------------------------------

/// Compile a connector into its two artifacts.
///
/// **This body is a placeholder. Story C-8 (`crates/connector-flux`) replaces it** with a call to
/// the Flux emitter — roughly:
///
/// ```text
/// let module = connector_flux::emit_module(connector)?;   // real flux_lang AST + formatter
/// let manifest = connector_flux::emit_manifest(connector)?;
/// ```
///
/// The contract this module owes its callers, and which C-8 must preserve:
///
/// - **Deterministic.** Equal inputs produce byte-identical output, on every platform and every
///   run. `build` being a no-op over unchanged inputs rests entirely on this.
/// - **Total.** Either both artifacts are produced or the call fails; a connector is a manifest
///   *plus* a module, never one of them.
/// - **Text, not files.** Writing is [`crate::pipeline`]'s job, so a failure cannot leave a partial
///   tree behind.
///
/// Today it emits a header carrying the provider, the generator and the inputs digest, and no ops.
/// A placeholder module is deliberately *not* valid, op-bearing Flux — C-11's parse-and-analyze
/// gate is what will hold that line once C-8 supplies real content.
pub fn emit(connector: &Connector) -> Result<Artifacts> {
    let Connector {
        name,
        spec_version,
        inputs_digest,
    } = connector;
    let generator = generator();
    let digest = format!("{inputs_digest:016x}");

    let mut module = String::new();
    module.push_str(&format!("// Generated by {generator} — do not edit.\n"));
    module.push_str(&format!("// Provider: {name}\n"));
    module.push_str(&format!("// Inputs digest: {digest}\n"));
    module.push_str("//\n");
    module
        .push_str("// The `op` declarations for this connector are emitted by `connector-flux`\n");
    module
        .push_str("// (story C-8). Until that wiring point is filled this module declares none.\n");

    let mut manifest = String::new();
    manifest.push_str(&format!("# Generated by {generator} — do not edit.\n"));
    manifest.push_str(&format!("generator = \"{generator}\"\n"));
    manifest.push_str(&format!("provider = \"{name}\"\n"));
    manifest.push_str(&format!("inputs_digest = \"{digest}\"\n"));
    if let Some(version) = spec_version {
        manifest.push_str(&format!("spec_version = \"{version}\"\n"));
    }

    Ok(Artifacts { module, manifest })
}

/// FNV-1a, 64-bit.
///
/// A placeholder, and scoped like one: it exists so the placeholder artifacts vary with their
/// inputs, which is what makes the determinism and `diff` tests meaningful. The real provenance
/// hash is sha256 over the canonical IR and belongs to `connectors.lock` (C-7) — this is not it,
/// and nothing outside this module should grow to depend on it.
struct Digest(u64);

impl Digest {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
        // Length-delimit, so that concatenating fields differently cannot collide.
        self.0 ^= bytes.len() as u64;
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(definition: &str) -> ProviderInputs {
        ProviderInputs {
            name: "acme".to_string(),
            definition: definition.to_string(),
            spec: None,
        }
    }

    #[test]
    fn emission_is_deterministic() {
        let first = emit(&load(&inputs("id = \"acme\"\n")).unwrap()).unwrap();
        let second = emit(&load(&inputs("id = \"acme\"\n")).unwrap()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn a_changed_definition_changes_both_artifacts() {
        let first = emit(&load(&inputs("id = \"acme\"\n")).unwrap()).unwrap();
        let second = emit(&load(&inputs("id = \"acme\"\nvendor = \"Acme\"\n")).unwrap()).unwrap();
        assert_ne!(first.module, second.module);
        assert_ne!(first.manifest, second.manifest);
    }

    #[test]
    fn an_empty_definition_is_rejected() {
        let error = load(&inputs("   \n")).expect_err("empty definitions must not load");
        assert!(format!("{error:#}").contains("acme"));
    }

    #[test]
    fn a_spec_version_reaches_the_manifest() {
        let mut with_spec = inputs("id = \"acme\"\n");
        with_spec.spec = Some(SpecInput {
            version: "2024-06-01".to_string(),
            document: "{}".to_string(),
        });
        let artifacts = emit(&load(&with_spec).unwrap()).unwrap();
        assert!(artifacts.manifest.contains("spec_version = \"2024-06-01\""));
    }
}
