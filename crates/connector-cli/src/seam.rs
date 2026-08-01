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
//! - ~~**Spec ingest is C-4.**~~ **Wired.** [`ProviderInputs`] carries the vendored document and
//!   [`load`] hands it to `connector_spec::provider::load_with_spec`, which ingests it and publishes
//!   the operations `[[patch.operations]]` selects. What is *not* here, deliberately: a selector
//!   that matches a set (C-411), a naming rule instead of a `rename` per operation (C-412), and
//!   one connector reading several documents (C-410). See [`load`].
//! - **The manifest is C-10's.** [`emit`] derives `<name>.connector.toml` from the IR here, because
//!   `connector-flux` emits Flux and nothing else. C-10 replaces its body with the real capability
//!   manifest — `http_hosts`, the endpoint env spec, and the credential declarations.

use std::collections::BTreeMap;

use anyhow::{bail, Result};
use serde::Serialize;

use connector_spec::{
    ChannelBinding, EventDecl, ManualSetup, Selector, Subscription, VerificationScheme,
};

use crate::catalog::{self, OperationRendering};
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
    /// Read for every provider whose `specs/<name>/` directory holds one, and *ingested* only when
    /// the provider file's `[spec]` asks for it — the loader makes that distinction (C-4).
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

/// What one connector compiles to.
///
/// Two files ship — [`module`](Self::module) and [`manifest`](Self::manifest) — and the rest is the
/// catalog's half (C-38): the same operations rendered one file each, plus the Rust table that
/// embeds them. The split is deliberate and is stated in the story: `connectors/<name>.flux` is
/// unchanged in role, and per-operation renderings are *additional*, never a substitution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifacts {
    /// One module and one manifest **per service** (C-49), in the connector's service order.
    ///
    /// A `default`-only provider yields exactly one entry, whose files are named `<provider>.flux`
    /// and `<provider>.connector.toml` — byte for byte what it emitted before services existed.
    pub services: Vec<ServiceArtifacts>,
    /// One `.flux` rendering per operation, in IR order — the catalog's unit (C-38).
    ///
    /// The same strings the service modules are concatenated from, so the two cannot disagree. Kept
    /// provider-wide rather than split per service, because the catalog's unit is the provider.
    pub operations: Vec<OperationRendering>,
    /// `crates/catalog/src/generated/<name>.rs`: the generated table embedding those renderings
    /// alongside the metadata a caller needs to decide whether to use an operation.
    pub catalog: String,
}

/// What one service of one connector compiles to: the pair of files that install together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceArtifacts {
    /// The service name — [`connector_spec::DEFAULT_SERVICE`] for a single-surface provider.
    pub service: String,
    /// The `.flux` module: the `op` declarations flux loads for this service.
    pub module: String,
    /// The `.connector.toml` manifest: what this service needs in order to run, including **its own**
    /// base URL rather than the union of the provider's.
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
/// # A provider that points at a spec is compiled through ingest (C-4)
///
/// `connector_spec::provider::load_with_spec` is the spec front-end: it ingests the vendored
/// document into every operation the vendor declares, publishes the ones `[[patch.operations]]`
/// selects, and validates the result through the same pass a hand-authored file goes through. This
/// function's only remaining job on that path is to hand it the bytes discovery already read, and
/// to refuse the one case the loader cannot see — a `[spec]` pointing at a document that is not in
/// the cache at all.
///
/// **A document is read only because the file asked for one.** `inputs.spec` is `Some` whenever
/// `specs/<provider>/` holds anything, which is not a declaration; `[spec] path` is. The loader
/// makes that distinction, so passing the document unconditionally is safe and a provider with no
/// `[spec]` block compiles exactly as it did before C-4.
pub fn load(inputs: &ProviderInputs) -> Result<Connector> {
    Ok(load_reported(inputs)?.connector)
}

/// [`load`], keeping what the vendored document got wrong.
///
/// The split exists because a diagnostic is **not** a failure. A real vendor document is never fully
/// well-formed — an ingest that refused one would compile nothing — so a skipped endpoint has to
/// travel beside the connector rather than instead of it. What matters is that the cost stays
/// visible: each line names the endpoint and says what it cost, so a `select` that could never have
/// matched is legible before someone goes looking for the operation it names.
pub fn load_reported(inputs: &ProviderInputs) -> Result<Loaded> {
    let label = inputs.label();
    let loaded = match &inputs.spec {
        Some(spec) => {
            connector_spec::provider::load_with_spec(&label, &inputs.definition, &spec.document)?
        }
        None => connector_spec::provider::load(&label, &inputs.definition)?,
    };

    if let Some(spec) = &loaded.spec {
        if inputs.spec.is_none() {
            bail!(
                "`{label}` points at the vendored spec `{}`, but `{}/{}/` holds no document to \
                 ingest. The spec cache is committed, so a pointer at a file that is not there is \
                 a connector that cannot be built rather than one that builds empty",
                spec.path,
                crate::workspace::SPECS_DIR,
                inputs.name
            );
        }
    }

    Ok(Loaded {
        diagnostics: loaded
            .diagnostics()
            .iter()
            .map(|diagnostic| format!("{}: {diagnostic}", inputs.name))
            .collect(),
        connector: loaded.connector,
    })
}

/// One provider's IR, plus everything its vendored document got wrong — see [`load_reported`].
#[derive(Debug, Clone)]
pub struct Loaded {
    /// The validated connector, ready to emit.
    pub connector: Connector,
    /// One line per endpoint the ingest could not read, already prefixed with the provider name.
    /// Empty for every hand-authored connector, which today is all forty-five of them.
    pub diagnostics: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// WIRING POINT 2 of 2 — C-8
// ---------------------------------------------------------------------------------------------

/// Compile a connector into everything it produces: the two shipped artifacts, and the catalog's
/// per-operation renderings plus its generated table (C-38).
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
    let operations = renderings(connector)?;

    let mut services = Vec::new();
    for service in connector.service_names() {
        services.push(ServiceArtifacts {
            service: service.to_owned(),
            module: module(connector, service, &operations),
            manifest: manifest(connector, service)?,
        });
    }

    let catalog = catalog::render(connector, &operations)?;
    Ok(Artifacts {
        services,
        operations,
        catalog,
    })
}

/// Narrow a connector to one whole service: every operation belonging to it, and nothing else.
///
/// `selector` is a service name (`s3`) or a rendered gid (`com.amazonaws/s3:2006-03-01`). The gid form
/// is what lets an external reference name a slice without knowing our local provider spelling.
///
/// Narrowing the **IR** rather than filtering at emission time is deliberate: every backend then sees
/// a connector that simply has one service, so "the other service's operations are absent" holds for
/// the module, the manifest, the catalog and the site document at once, with no per-backend filter to
/// forget.
///
/// # Errors
///
/// An unknown service, naming the ones that exist — the same loudness `--provider` already has.
pub fn select_service(connector: &Connector, selector: &str) -> Result<Connector> {
    let available = connector.service_names();
    let matched = available
        .iter()
        .copied()
        .find(|name| *name == selector)
        .or_else(|| {
            available.iter().copied().find(|name| {
                connector
                    .gid_of(name)
                    .is_some_and(|gid| gid.to_string() == selector)
            })
        });

    let Some(service) = matched else {
        bail!(
            "connector `{}` has no service `{selector}`; {}",
            connector.id,
            describe_services(connector)
        );
    };

    Ok(Connector {
        services: connector
            .services
            .iter()
            .filter(|declared| declared.name == service)
            .cloned()
            .collect(),
        operations: connector.operations_of(service).cloned().collect(),
        // **Every member kind of the service, not just the callable one** (C-83). A selection that
        // stayed operations-only would be the worst kind of partial success: `--service s3` would
        // emit an s3 manifest carrying another service's events, and it would do so successfully.
        // The kinds partition the same way for the same reason — each member names exactly one
        // service — so one filter per kind is the whole rule. `config` and `graphs` arrived after
        // C-83 wrote that and were carried through by the tail below until C-194; each has had its
        // accessor since it landed.
        events: connector.events_of(service).cloned().collect(),
        channels: connector.channels_of(service).cloned().collect(),
        config: connector.config_of(service).cloned().collect(),
        graphs: connector.graphs_of(service).cloned().collect(),
        // `verify` is connector-level but *denotes* an operation, and an operation has exactly one
        // service — so it is service-derived, and the tail carried it through unnarrowed (C-194).
        // Neither "keep" nor "drop" is right on its own: kept across a boundary it names an operation
        // this connector no longer declares, which `connector_spec`'s own `validate_verify` refuses
        // on load; dropped unconditionally it would strip a legitimate Test-connection button from
        // the service that owns it. So it survives exactly when its operation does.
        verify: connector.verify.clone().filter(|id| {
            connector
                .operation(id)
                .is_some_and(|op| op.service == service)
        }),
        ..connector.clone()
    })
}

/// The services a connector has, with their addresses when it declares an authority — so the error
/// names both spellings a `--service` selector may use.
fn describe_services(connector: &Connector) -> String {
    let described: Vec<String> = connector
        .service_names()
        .into_iter()
        .map(|name| match connector.gid_of(name) {
            Some(gid) => format!("{name} ({gid})"),
            None => name.to_owned(),
        })
        .collect();
    format!("available services: {}", described.join(", "))
}

/// Every operation's `op` declaration, emitted **once**.
///
/// One call to `connector_flux::emit_operation` per operation, whose result feeds both the provider
/// module and the catalog. Emitting twice would make "the catalog's rendering is the one the module
/// carries" a coincidence that a future edit could break silently; here it is arithmetic.
fn renderings(connector: &Connector) -> Result<Vec<OperationRendering>> {
    connector
        .operations
        .iter()
        .map(|operation| {
            Ok(OperationRendering {
                id: operation.id.clone(),
                source: connector_flux::emit_operation(connector, operation)?,
            })
        })
        .collect()
}

/// The `.flux` module: a generated-file header, then one `op` declaration per operation.
///
/// The header is `#` comments. Flux's comment character is `#` — `//` is not a comment and does not
/// parse — and comment-only lines are trivia the lexer drops, so the header cannot affect what the
/// module declares.
/// A `default`-only provider's header is unchanged from before services existed — the byte-identity
/// property this story is asserted against. A named service adds one `# Service:` line, because a
/// reviewer opening `aws-s3.flux` should not have to infer which surface it is from the file name.
fn module(connector: &Connector, service: &str, operations: &[OperationRendering]) -> String {
    let mut module = format!(
        "# Generated by {} — do not edit.\n# Provider: {}\n",
        generator(),
        connector.id
    );
    if service != connector_spec::DEFAULT_SERVICE {
        module.push_str(&format!("# Service: {service}\n"));
    }
    module.push_str("# Regenerate with `flux-connectors build`.\n");
    // Walked in rendering order, which is IR order, so a service's declarations appear in the module
    // in the order the provider file declares them. A rendering whose operation is missing is refused
    // by `catalog::render` and `site::provider_entry`, so it cannot be quietly dropped here.
    for rendering in operations {
        if connector
            .operation(&rendering.id)
            .is_some_and(|operation| operation.service == service)
        {
            module.push('\n');
            module.push_str(&rendering.source);
        }
    }
    module
}

/// The `.connector.toml` manifest.
///
/// **A placeholder shape, and scoped like one.** The design's manifest carries `http_hosts`, the
/// endpoint env spec and the credential declarations, mirroring flux's plugin-protocol vocabulary —
/// all of which is C-10's Acceptance, and none of which can be written honestly before auth is
/// modelled. What is here is what the IR already knows and a reviewer needs in order to read a
/// diff: which connector this is, where it points, and which operations it publishes.
///
/// # The inbound half (C-83)
///
/// `[[events]]` and `[[channels]]` are the *other* call direction, and the manifest is the only
/// artifact that can carry them: a binding declares and never installs, so it reaches this file and
/// `catalog.json` and is emitted into no `.flux` module at all. Every value is the IR's own — a
/// verification block names the **credential**, never a secret, and there is no URL and no schedule
/// anywhere, because the endpoint is the operator's deployment detail and this repository runs
/// nothing.
///
/// **Two event fields deliberately stop at the catalogue.** `schema` and `when` are vendor JSON
/// Schemas, and TOML has no `null` — carrying one verbatim would make an entirely legal provider
/// file fail to build, over a value the manifest does not need. `web/public/catalog.json` is the
/// JSON-shaped backend over the same IR and carries both losslessly; a host reading this file gets
/// the identity, the address and the routing of an event, and reads its shape from the catalogue.
fn manifest(connector: &Connector, service: &str) -> Result<String> {
    /// The manifest's wire shape. Field order is the emitted order, which is what makes the output
    /// deterministic without sorting anything at runtime.
    ///
    /// `service`, `authority` and `api_version` are skipped when they are absent or reserved, so a
    /// `default`-only provider's manifest is byte for byte what it was before C-49 — see the story's
    /// byte-identity item.
    #[derive(Serialize)]
    struct Manifest<'a> {
        generator: String,
        connector: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        service: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        gid: Option<String>,
        #[serde(skip_serializing_if = "str::is_empty")]
        vendor: &'a str,
        #[serde(skip_serializing_if = "str::is_empty")]
        description: &'a str,
        /// **How this connector executes** (C-405) — `http`, `socket`, `process`, `container`,
        /// `plugin` or `remote`.
        ///
        /// Never skipped, unlike the optional fields around it, and that is the whole point of the
        /// field: a host serving more than one tenant refuses a locally-executing runtime, and a
        /// manifest that stated the runtime only when it was unusual would leave that host inferring
        /// `http` from an absence — which is exactly the derivation this replaces. So every manifest
        /// says it, including the 53 that say `http`.
        runtime: &'static str,
        /// **This service's** base URL, never the union of the provider's. A service's egress surface
        /// is its own; C-10's `http_hosts` allowlist derives from exactly this value.
        base_url: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        api_version: Option<&'a str>,
        module: String,
        operations: Vec<&'a str>,
        /// The events this service receives. Arrays of tables, so they come last — every scalar and
        /// inline value above them, which is what keeps the emitted TOML valid without sorting.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        events: Vec<ManifestEvent<'a>>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        channels: Vec<ManifestChannel<'a>>,
    }

    let named = service != connector_spec::DEFAULT_SERVICE;
    let manifest = Manifest {
        generator: generator(),
        connector: &connector.id,
        service: named.then_some(service),
        gid: connector.gid_of(service).map(|gid| gid.to_string()),
        vendor: &connector.vendor,
        description: &connector.description,
        runtime: connector.runtime.word(),
        base_url: connector.base_url_of(service),
        api_version: connector.api_version_of(service),
        module: format!(
            "{}.{}",
            if named {
                format!("{}-{service}", connector.id)
            } else {
                connector.id.clone()
            },
            crate::workspace::MODULE_EXT
        ),
        operations: connector
            .operations_of(service)
            .map(|operation| operation.id.as_str())
            .collect(),
        events: connector
            .events_of(service)
            .map(|event| manifest_event(connector, event))
            .collect(),
        channels: connector
            .channels_of(service)
            .map(|channel| manifest_channel(connector, channel))
            .collect(),
    };

    let body = toml::to_string(&manifest)?;
    Ok(format!(
        "# Generated by {} — do not edit.\n# Auth and the `http_hosts` allowlist land in C-10.\n{body}",
        generator()
    ))
}

/// One `[[events]]` block: what a host needs in order to *name* an event.
///
/// The payload schema is deliberately absent — see [`manifest`] on why it stops at the catalogue.
#[derive(Serialize)]
struct ManifestEvent<'a> {
    name: &'a str,
    /// The rendered address, when the connector publishes one.
    #[serde(skip_serializing_if = "Option::is_none")]
    oip: Option<String>,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    /// Whether a product offers this event on when a user connects. Skipped when it is `true`, so
    /// the common case adds nothing and the firehose case is the one that shows up in a diff.
    #[serde(skip_serializing_if = "is_true")]
    default: bool,
    #[serde(skip_serializing_if = "str::is_empty")]
    group: &'a str,
}

/// One `[[channels]]` block.
///
/// Field order is the emitted order, and every scalar precedes every table — TOML places a nested
/// table after the key/value pairs of its parent, so a scalar declared after one would land inside
/// it. That is why `cursor` and `interval` sit above `verification` rather than beside the other
/// poll-shaped fields.
#[derive(Serialize)]
struct ManifestChannel<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    oip: Option<String>,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    transport: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    events: Vec<&'a str>,
    /// The cursor operation a `poll` binding resumes from — the field that carries a poll's
    /// correctness, since flux's schedule drops ticks across a restart.
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<&'a str>,
    /// Advisory only.
    #[serde(skip_serializing_if = "Option::is_none")]
    interval: Option<&'a str>,
    /// **Always emitted**, and always naming its kind. A binding that states nothing here would be
    /// indistinguishable from one whose vendor publishes no signature — the exact confusion the
    /// tri-state exists to prevent.
    verification: ManifestVerification<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discriminator: Option<ManifestSelector<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_id: Option<ManifestSelector<'a>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    payload: &'a BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply: Option<ManifestReply<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subscription: Option<&'a Subscription>,
    #[serde(skip_serializing_if = "Option::is_none")]
    setup: Option<&'a ManualSetup>,
}

/// How a delivery proves it came from the vendor, in the manifest's own encoding.
///
/// `kind` and `verified` are the pair `crate::inbound` documents: a consumer reads one boolean and
/// does not have to learn the vocabulary, and neither answer is ever carried by an absent key.
#[derive(Serialize)]
struct ManifestVerification<'a> {
    kind: &'static str,
    verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    hmac: Option<ManifestHmac<'a>>,
}

/// The HMAC parameters, in the vendor's own terms. `secret` is a **credential name**, never a value.
///
/// **Every field of `HmacSpec` must appear here**, and this restatement of them is why that is a
/// promise rather than a habit: a field the IR gains and this struct does not is dropped from every
/// manifest, silently, because both halves still compile. `HmacSpec` cannot simply be flattened in —
/// TOML places a nested table after its parent's key/value pairs, so a scalar declared after
/// `timestamp` would reparse as a field *of* the timestamp table, and `HmacSpec` declares `secret`
/// and `tolerance` after it. So the field set is held to the IR's by a test instead:
/// `inbound_artifacts.rs::neither_projection_can_lose_a_field_hmac_spec_declares` derives the
/// authoritative list from `HmacSpec`'s own `Deserialize` impl and fails until this struct carries
/// all of it (C-151).
#[derive(Serialize)]
struct ManifestHmac<'a> {
    algorithm: &'static str,
    encoding: &'static str,
    header: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<&'a str>,
    signed: &'a str,
    secret: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tolerance: Option<&'a str>,
    /// How the signed timestamp is spelled — the **resolved** answer, so a host never has to know
    /// the IR's default. A scalar, so it sits above the `timestamp` table for the reason
    /// [`ManifestChannel`] records.
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp_format: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<ManifestSelector<'a>>,
}

/// Where on an inbound request one named value is read from.
#[derive(Serialize)]
struct ManifestSelector<'a> {
    source: &'static str,
    name: &'a str,
}

/// The outbound half of a binding — **the reply as a rendered oip**, beside the local id a host
/// resolves against this same manifest's `operations`.
#[derive(Serialize)]
struct ManifestReply<'a> {
    operation: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    oip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<&'a str>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    bind: &'a BTreeMap<String, String>,
}

/// `skip_serializing_if` for [`ManifestEvent::default`].
fn is_true(value: &bool) -> bool {
    *value
}

fn manifest_event<'a>(connector: &Connector, event: &'a EventDecl) -> ManifestEvent<'a> {
    ManifestEvent {
        name: &event.name,
        oip: member_oip(connector, &event.service, &event.name),
        description: &event.description,
        default: event.default,
        group: &event.group,
    }
}

fn manifest_channel<'a>(connector: &Connector, channel: &'a ChannelBinding) -> ManifestChannel<'a> {
    let verification = crate::inbound::verification_of(channel);
    ManifestChannel {
        name: &channel.name,
        oip: member_oip(connector, &channel.service, &channel.name),
        description: &channel.description,
        transport: crate::inbound::transport_token(channel.transport),
        events: channel.events.iter().map(String::as_str).collect(),
        cursor: channel.cursor.as_deref(),
        interval: channel.interval.as_deref(),
        verification: ManifestVerification {
            kind: verification.kind(),
            verified: verification.verified(),
            hmac: match &channel.verification {
                Some(VerificationScheme::Hmac(spec)) => Some(ManifestHmac {
                    algorithm: crate::inbound::digest_token(spec.algorithm),
                    encoding: crate::inbound::encoding_token(spec.encoding),
                    header: &spec.header,
                    prefix: spec.prefix.as_deref(),
                    signed: &spec.signed,
                    secret: &spec.secret,
                    tolerance: spec.tolerance.as_deref(),
                    timestamp_format: crate::inbound::timestamp_format_of(spec),
                    timestamp: spec.timestamp.as_ref().map(manifest_selector),
                }),
                Some(VerificationScheme::None) | None => None,
            },
        },
        discriminator: channel.discriminator.as_ref().map(manifest_selector),
        delivery_id: channel.delivery_id.as_ref().map(manifest_selector),
        payload: &channel.payload,
        reply: channel.reply.as_ref().map(|reply| ManifestReply {
            operation: &reply.operation,
            oip: member_oip(connector, &channel.service, &reply.operation),
            result: reply.result.as_deref(),
            bind: &reply.bind,
        }),
        subscription: channel.subscription.as_ref(),
        setup: channel.setup.as_ref(),
    }
}

fn manifest_selector(selector: &Selector) -> ManifestSelector<'_> {
    ManifestSelector {
        source: crate::inbound::source_token(selector.source),
        name: &selector.name,
    }
}

/// A member's rendered address, or `None` when the connector declares no authority or no API
/// version. All three member kinds share one namespace per service and therefore one address form.
fn member_oip(connector: &Connector, service: &str, name: &str) -> Option<String> {
    connector
        .oip_of_member(service, name)
        .map(|oip| oip.to_string())
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

    /// A two-service provider, AWS-shaped: one authority, two hosts, one API date each.
    const TWO_SERVICE: &str = r#"
id = "aws"
vendor = "Amazon Web Services"
authority = "com.amazonaws"
base_url = "https://amazonaws.com"

[[services]]
name = "s3"
description = "Object storage."
base_url = "https://s3.amazonaws.com"
api_version = "2006-03-01"

[[services]]
name = "bedrock-runtime"
description = "Model inference."
base_url = "https://bedrock-runtime.us-east-1.amazonaws.com"
api_version = "2023-09-30"

[[operations]]
id = "aws-object-get"
service = "s3"
method = "GET"
path = "/objects/{key}"
description = "Fetch one object."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "key"
required = true
schema = { type = "string" }

[[operations]]
id = "aws-model-invoke"
service = "bedrock-runtime"
method = "POST"
path = "/model/{model_id}/invoke"
description = "Invoke a model."
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.path]]
name = "model_id"
required = true
schema = { type = "string" }
"#;

    /// The same two-service shape, now declaring **every** service-partitioned surface: one
    /// `[[config]]` field and one `[[graphs]]` flow per service, plus a connector-level `verify`
    /// naming an s3 operation.
    ///
    /// Constructed rather than shipped, because no single shipped provider carries all three.
    /// `anthropic`, `contentful` and `postmark` are multi-service *and* declare per-service
    /// `[[config]]`, and `anthropic`, `contentful` and `microsoft_graph` declare a `verify` — those
    /// are covered against what ships, by
    /// `tests/service_units.rs::narrowing_a_shipped_provider_carries_no_other_services_config_graphs_or_verify`.
    /// **No provider in `providers/` declares `[[graphs]]` at all**, so a graph has no shipped case
    /// and only a fixture can assert it.
    ///
    /// It goes through the real loader, so the value a narrowing starts from is one `connector_spec`
    /// accepts — which is what makes "the narrowed value would not load" a statement about the
    /// narrowing rather than about the fixture.
    ///
    /// Each service's `base_url` carries **its own** template variable, bound by its own field. That
    /// is the detail that makes the leak observable rather than merely untidy: carry `region` into an
    /// s3-only connector and it binds `{region}`, which no remaining base URL has anywhere.
    const TWO_SERVICE_CONFIGURED: &str = r#"
id = "aws"
vendor = "Amazon Web Services"
authority = "com.amazonaws"
base_url = "https://amazonaws.com"
description = "Object storage and model inference."
verify = "aws-object-get"
default_auth = [{ credentials = ["aws.token"] }]

[[auth]]
name = "aws.token"
scheme = "bearer"
env = ["AWS_TOKEN"]

[[services]]
name = "s3"
description = "Object storage."
base_url = "https://{bucket}.s3.amazonaws.com"
api_version = "2006-03-01"

[[services]]
name = "bedrock-runtime"
description = "Model inference."
base_url = "https://bedrock-runtime.{region}.amazonaws.com"
api_version = "2023-09-30"

[[config]]
name = "bucket"
service = "s3"
label = "Bucket name"
help = "The bucket this connector reads objects from."
binds = "endpoint.bucket"

[[config]]
name = "region"
service = "bedrock-runtime"
label = "Model region"
help = "The AWS region hosting the models you invoke."
binds = "endpoint.region"

[[operations]]
id = "aws-object-get"
service = "s3"
method = "GET"
path = "/objects/{key}"
description = "Fetch one object."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "key"
required = true
schema = { type = "string" }

[[operations]]
id = "aws-model-invoke"
service = "bedrock-runtime"
method = "POST"
path = "/model/{model_id}/invoke"
description = "Invoke a model."
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.path]]
name = "model_id"
required = true
schema = { type = "string" }

[[graphs]]
name = "object-fetch"
service = "s3"
description = "Read one object."

[[graphs.nodes]]
id = "get"
kind = { operation = { operation = "aws-object-get" } }
outputs = [{ name = "out" }]

[[graphs]]
name = "model-call"
service = "bedrock-runtime"
description = "Invoke one model."

[[graphs.nodes]]
id = "invoke"
kind = { operation = { operation = "aws-model-invoke" } }
outputs = [{ name = "out" }]
"#;

    /// A provider that points at a vendored spec and selects one operation out of it — the shape
    /// C-4 exists to compile.
    const SPEC_BACKED: &str = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"

[spec]
path = "specs/acme/v1.json"

[[patch.operations]]
select = "showTicket"
rename = "acme-ticket-show"
risk = "low"
idempotency = "idempotent"
"#;

    /// The vendored document `SPEC_BACKED` points at: two operations, one of which is selected.
    const SPEC_DOCUMENT: &str = r#"{
  "openapi": "3.0.3",
  "info": { "title": "Acme", "version": "1.0.0" },
  "servers": [{ "url": "https://{tenant}.acme.example" }],
  "paths": {
    "/v2/tickets/{ticket_id}": {
      "get": {
        "operationId": "showTicket",
        "summary": "Show one Acme ticket.",
        "parameters": [
          {
            "name": "ticket_id",
            "in": "path",
            "required": true,
            "schema": { "type": "integer" }
          }
        ]
      }
    },
    "/v2/tickets": {
      "get": { "operationId": "listTickets", "summary": "List Acme tickets." }
    }
  }
}
"#;

    fn inputs(definition: &str) -> ProviderInputs {
        ProviderInputs {
            name: "acme".to_string(),
            definition: definition.to_string(),
            spec: None,
        }
    }

    /// The same inputs, with the vendored document discovery would have read beside them.
    fn spec_backed(definition: &str, document: &str) -> ProviderInputs {
        ProviderInputs {
            name: "acme".to_string(),
            definition: definition.to_string(),
            spec: Some(SpecInput {
                version: "v1".to_string(),
                document: document.to_string(),
            }),
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
        assert_ne!(default_unit(&first).module, default_unit(&second).module);
        assert_ne!(
            default_unit(&first).manifest,
            default_unit(&second).manifest
        );
    }

    /// The single unit a `default`-only provider emits. Every fixture here is one, which is also the
    /// shape of every provider the repository ships.
    fn default_unit(artifacts: &Artifacts) -> &ServiceArtifacts {
        assert_eq!(
            artifacts.services.len(),
            1,
            "a `default`-only provider emits exactly one service unit"
        );
        let unit = &artifacts.services[0];
        assert_eq!(unit.service, connector_spec::DEFAULT_SERVICE);
        unit
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

    /// **C-4's wiring point.** A provider that points at a vendored document compiles: ingest turns
    /// the document into operations, the patch set says which of them are published, and what
    /// reaches the IR carries the parameters and schemas the document declared.
    ///
    /// This is the failing-first test the story names. Before C-4 it failed with "spec ingest
    /// (story C-4), which is not wired yet".
    #[test]
    fn a_spec_backed_provider_ingests_its_operations() {
        let connector = load(&spec_backed(SPEC_BACKED, SPEC_DOCUMENT))
            .expect("a spec-backed provider compiles once ingest is wired");

        let ids: Vec<&str> = connector
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["acme-ticket-show"],
            "only the selected operation reaches the IR"
        );

        let operation = &connector.operations[0];
        assert_eq!(operation.path, "/v2/tickets/{ticket_id}");
        assert_eq!(operation.description, "Show one Acme ticket.");
        let param = &operation.params.path[0];
        assert_eq!(param.name, "ticket_id");
        assert!(param.required);
        assert_eq!(param.schema, serde_json::json!({ "type": "integer" }));
    }

    /// **Ingest selects nothing.** A document with operations in it and a file that names none of
    /// them publishes none of them — selection is opt-in, and stays C-6's and C-411's to widen.
    #[test]
    fn a_spec_backed_provider_with_no_patch_publishes_no_operations() {
        let definition = "\
id = \"acme\"
base_url = \"https://api.acme.example\"

[spec]
path = \"specs/acme/v1.json\"
";
        let connector = load(&spec_backed(definition, SPEC_DOCUMENT))
            .expect("a spec with nothing selected is a connector with no operations, not an error");
        assert!(
            connector.operations.is_empty(),
            "ingest made two operations available and the file selected neither: {:?}",
            connector.operations
        );
    }

    #[test]
    fn the_module_declares_the_operation() {
        let artifacts = emit(&load(&inputs(HAND_AUTHORED)).unwrap()).unwrap();
        let module = &default_unit(&artifacts).module;
        assert!(
            module.contains("op acme-ticket-show(ticket_id: Number) -> Any"),
            "{module}"
        );
    }

    /// A two-service provider emits one unit per service, each carrying only its own operations and
    /// its own base URL. This is the emitted-unit half of C-49, at the seam that produces the bytes.
    #[test]
    fn a_multi_service_provider_emits_one_unit_per_service() {
        let artifacts = emit(&load(&inputs(TWO_SERVICE)).unwrap()).unwrap();

        let names: Vec<&str> = artifacts
            .services
            .iter()
            .map(|unit| unit.service.as_str())
            .collect();
        assert_eq!(names, vec!["s3", "bedrock-runtime"]);

        let s3 = &artifacts.services[0];
        assert!(s3.module.contains("# Service: s3"), "{}", s3.module);
        assert!(s3.module.contains("op aws-object-get"), "{}", s3.module);
        assert!(
            !s3.module.contains("op aws-model-invoke"),
            "the s3 module carries bedrock's operation:\n{}",
            s3.module
        );
        assert!(
            s3.manifest
                .contains("base_url = \"https://s3.amazonaws.com\""),
            "the manifest must carry the service's own base URL, never the provider's:\n{}",
            s3.manifest
        );
        assert!(
            s3.manifest
                .contains("gid = \"com.amazonaws/s3:2006-03-01\""),
            "{}",
            s3.manifest
        );
        assert!(
            s3.manifest.contains("module = \"aws-s3.flux\""),
            "{}",
            s3.manifest
        );
        assert!(
            !s3.manifest.contains('*'),
            "a service's host claim is never widened:\n{}",
            s3.manifest
        );

        let bedrock = &artifacts.services[1];
        assert!(
            bedrock.manifest.contains("api_version = \"2023-09-30\""),
            "each service versions itself:\n{}",
            bedrock.manifest
        );
    }

    /// Selecting a service yields a connector holding that service and nothing else — the property
    /// that makes `--service s3` mean "the whole s3 service".
    #[test]
    fn selecting_a_service_drops_every_other_operation() {
        let connector = load(&inputs(TWO_SERVICE)).unwrap();

        for selector in ["s3", "com.amazonaws/s3:2006-03-01"] {
            let selected = select_service(&connector, selector)
                .unwrap_or_else(|error| panic!("`{selector}` must select the s3 service: {error}"));
            assert_eq!(selected.service_names(), vec!["s3"]);
            let ids: Vec<&str> = selected
                .operations
                .iter()
                .map(|operation| operation.id.as_str())
                .collect();
            assert_eq!(ids, vec!["aws-object-get"]);

            let artifacts = emit(&selected).unwrap();
            assert_eq!(artifacts.services.len(), 1);
            assert!(!artifacts.services[0].module.contains("aws-model-invoke"));
        }
    }

    /// **C-194.** The narrowing carries the selected service's surfaces and *nothing else* — the
    /// three the C-83 comment above [`select_service`] did not cover.
    ///
    /// The assertions are stated as the loader's own invariants rather than as field values, because
    /// that is what the leak actually broke: an endpoint-bound field for a `{variable}` the remaining
    /// base URL does not carry is refused by `validate_config`, and a `verify` naming an operation
    /// the connector does not declare is refused by `validate_verify`. Before the fix,
    /// `select_service` returned a `Connector` failing both.
    #[test]
    fn selecting_a_service_carries_no_other_services_config_graphs_or_verify() {
        let connector = load(&inputs(TWO_SERVICE_CONFIGURED)).unwrap();

        for (service, config, graph) in [
            ("s3", "bucket", "object-fetch"),
            ("bedrock-runtime", "region", "model-call"),
        ] {
            let selected = select_service(&connector, service).expect("a declared service");

            let names: Vec<&str> = selected
                .config
                .iter()
                .map(|field| field.name.as_str())
                .collect();
            assert_eq!(
                names,
                vec![config],
                "`--service {service}` carries another service's configuration fields"
            );

            let flows: Vec<&str> = selected
                .graphs
                .iter()
                .map(|flow| flow.name.as_str())
                .collect();
            assert_eq!(
                flows,
                vec![graph],
                "`--service {service}` carries another service's flow graphs"
            );

            assert!(
                selected.config.iter().all(|field| field.service == service),
                "`--service {service}` kept a configuration field naming another service"
            );
            assert!(
                selected.graphs.iter().all(|flow| flow.service == service),
                "`--service {service}` kept a graph naming another service"
            );
            if let Some(verify) = &selected.verify {
                assert!(
                    selected.operation(verify).is_some(),
                    "`--service {service}` kept `verify = {verify:?}`, an operation it no longer \
                     declares — a Test-connection pointer into a service this build is not producing"
                );
            }
        }

        // `verify` is connector-level but *denotes* an operation, and an operation has exactly one
        // service — so it is service-derived, and narrowing it is neither "keep" nor "drop". Both
        // directions are asserted, because dropping it unconditionally would strip a legitimate
        // Test-connection button from the very service that owns it.
        assert_eq!(
            select_service(&connector, "s3").unwrap().verify.as_deref(),
            Some("aws-object-get"),
            "the service owning the verify operation keeps it"
        );
        assert_eq!(
            select_service(&connector, "bedrock-runtime")
                .unwrap()
                .verify,
            None,
            "a service that does not declare the verify operation must not point at it"
        );
    }

    /// The other half of the same rule: what is **not** service-partitioned still comes through the
    /// `..connector.clone()` tail untouched.
    ///
    /// `AuthMethod` carries no `service` and an `AuthRequirement` names a credential connector-wide,
    /// so narrowing auth would need a reachability computation rather than a filter. That is a
    /// different story; this test is what says C-194 did not quietly take it.
    #[test]
    fn selecting_a_service_keeps_the_connector_level_surfaces() {
        let connector = load(&inputs(TWO_SERVICE_CONFIGURED)).unwrap();
        let selected = select_service(&connector, "s3").expect("`s3` is a declared service");

        assert_eq!(selected.auth, connector.auth);
        assert_eq!(selected.default_auth, connector.default_auth);
        assert_eq!(selected.id, connector.id);
        assert_eq!(selected.vendor, connector.vendor);
        assert_eq!(selected.description, connector.description);
        assert_eq!(selected.authority, connector.authority);
        // The connector-level default, not the service's own — a service resolves its base URL
        // through `base_url_of`, which the narrowed connector still answers correctly.
        assert_eq!(selected.base_url, connector.base_url);
        assert_eq!(
            selected.base_url_of("s3"),
            "https://{bucket}.s3.amazonaws.com"
        );
    }

    #[test]
    fn an_unknown_service_is_an_error_that_names_what_exists() {
        let connector = load(&inputs(TWO_SERVICE)).unwrap();
        let error = select_service(&connector, "s4").expect_err("`s4` is not a service");
        let rendered = format!("{error:#}");
        assert!(rendered.contains("s4"), "{rendered}");
        assert!(
            rendered.contains("s3 (com.amazonaws/s3:2006-03-01)"),
            "{rendered}"
        );
        assert!(rendered.contains("bedrock-runtime"), "{rendered}");
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
