//! The provider-TOML front-end: `providers/<name>.toml` in, [`Connector`] out.
//!
//! The file plays **two roles**, and the loader has to serve both from one schema:
//!
//! 1. **Hand-authored** — the whole connector is written out inline, with no vendor spec anywhere.
//!    Ollama, Freshdesk and (for now) Zendesk are in this position: there is no usable OpenAPI
//!    document to ingest. This is the role that matters most today, because it is the shortest route
//!    to an executable `.flux` module.
//! 2. **Spec pointer** — the file names a vendored spec under `specs/` and carries a *patch set*
//!    that selects and corrects operations from it. Ingest (C-4) pre-fills the IR; the overlay
//!    (C-6) applies the patches this loader parses. Neither of those exists yet, so the patch set is
//!    parsed and validated here and consumed later.
//!
//! Both roles produce the same [`LoadedProvider`], which is what "two front-ends, one IR" means in
//! practice.
//!
//! # Errors are the interface
//!
//! Nobody debugs a provider TOML with a debugger; they read the error and edit the file. So the
//! error text is a deliverable, pinned by golden files in `tests/provider_toml_errors.rs`, and the
//! loader is arranged to make it good:
//!
//! - **Shape errors are serde's**, because serde's are better. Deserializing straight into the IR
//!   types means an unknown key reports the offending key *and lists every key that would have been
//!   valid*, with a line, a column and a source snippet from `toml`. A hand-rolled checker would
//!   have to reproduce all of that and would drift from the types.
//! - **Semantic errors are ours**, and are reported **all at once** rather than one per run. Fixing
//!   a provider file one error at a time is the authoring experience this repo is written against.
//!
//! # No network, no filesystem
//!
//! [`load`] takes bytes and a display name. Reading `providers/*.toml` off disk and fetching specs
//! is `connector-cli`'s job — see the crate docs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::{parse_binding, template_variables, Binding, ConfigField};
use crate::graph::{Graph, GraphNode, NodeKind, PortRef};
use crate::inbound::{
    parse_tolerance, signed_placeholders, validate_path, validate_symbol, ChannelBinding,
    EventDecl, FieldSource, HmacSpec, ManualSetup, Reply, Selector, Subscription, Transport,
    VerificationScheme, SIGNED_PLACEHOLDERS,
};
use crate::lock::sha256_hex;
use crate::{
    AuthMethod, AuthRequirement, AuthScheme, Connector, Idempotency, JsonSchema, Operation, Param,
    ParamSet, Provenance, Quirks, Risk, Role, Service, DEFAULT_SERVICE,
};

/// The documented JSON Schema for `providers/<name>.toml`.
///
/// TOML is a JSON-shaped data model, so one schema describes both the TOML an author writes and the
/// JSON the IR encodes to. It is hand-written rather than generated (generating it would mean a
/// `schemars` dependency this crate does not take) and is therefore kept honest by a test:
/// `tests/provider_schema.rs` asks serde which keys each type actually accepts and fails if the
/// schema documents a different set.
pub const PROVIDER_TOML_JSON_SCHEMA: &str = include_str!("../schema/provider-toml.schema.json");

/// A parsed and validated `providers/<name>.toml`.
///
/// The [`connector`](Self::connector) is complete and ready for codegen when the file is
/// hand-authored. When the file points at a spec it is a *skeleton* — id, base URL, credentials,
/// provenance, plus any operations written inline — that C-4's ingest fills in and C-6's overlay
/// patches.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedProvider {
    /// The connector this file describes.
    pub connector: Connector,
    /// The vendor spec the file points at, if any. `None` for a fully hand-authored connector.
    pub spec: Option<SpecSource>,
    /// The patch set C-6 applies over the ingested spec. Empty for a hand-authored connector.
    pub patch: Patch,
}

impl LoadedProvider {
    /// Whether this file is a complete hand-authored definition — no spec, so nothing to ingest and
    /// nothing to overlay.
    pub fn is_hand_authored(&self) -> bool {
        self.spec.is_none()
    }
}

/// Where the vendor spec for this connector lives.
///
/// The path is into the **vendored, committed** cache under `specs/`, never a URL to fetch at build
/// time: builds are hermetic and offline (AGENTS.md). `source_url` records where the bytes came
/// from so C-14 can re-fetch and diff, and `sha256` is what makes that diff a fact rather than a
/// guess.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecSource {
    /// The vendored spec file, relative to the repository root
    /// (`specs/babelforce/manager-0.7.0.openapi.json`).
    pub path: String,
    /// The URL the spec was fetched from, recorded for drift-check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// The upstream version string the vendor published (`info.version`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_version: Option<String>,
    /// SHA-256 of the vendored bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// When the spec was fetched, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
}

/// The patch set applied over an ingested spec — C-6's input.
///
/// **Selection is opt-in**, which is why there is no `hide`. A 163-operation spec must not become
/// 163 LLM tools (`docs/designs/provider-operation-inventory.md` §5.2 selects 9 of them), and an
/// opt-out list would make every new upstream operation a new tool by default. Only operations
/// named by a [`OperationPatch::select`] reach the connector.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    /// The operations selected from the spec, each with its corrections.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<OperationPatch>,
}

impl Patch {
    /// Whether the file carries no patches at all.
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

/// One operation selected from the vendor spec, and everything the author corrects about it.
///
/// Every override is an `Option` so that "not stated" stays distinguishable from "stated as the
/// value that happens to equal the spec's" — the overlay must be able to tell whether the author
/// made a decision, because a spec that later changes underneath an unstated field should follow
/// the spec, while a stated one must not move.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationPatch {
    /// The spec's `operationId` this patch selects, e.g. `listReportingCalls`.
    pub select: String,
    /// The stable op id to publish it as, e.g. `babelforce.call.list`.
    ///
    /// Almost always set: `operationId` is a volatile vendor field and the op name is a public
    /// contract users and models call by name
    /// (`docs/designs/connector-pipeline.md`, "Op naming is a public contract").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename: Option<String>,
    /// Replaces the spec's `summary`/`description` as the model-facing tool description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Overrides the risk the spec implies. Specs do not carry risk, so in practice this is where
    /// risk is *stated*, not overridden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<Risk>,
    /// Overrides idempotency. As with `risk`, specs do not publish it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency: Option<Idempotency>,
    /// Overrides the operation's auth alternatives.
    ///
    /// The `Option` carries the same three-way meaning as [`Operation::auth`]: absent means "leave
    /// whatever ingest extracted", `[]` means "this operation needs no auth", and a non-empty list
    /// replaces the extracted set. Babelforce's excluded header pair
    /// (`provider-operation-inventory.md` §5.1.3) is exactly this: ingest must keep seeing it, and
    /// the overlay is the only place it may be removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Vec<AuthRequirement>>,
    /// Quirks to attach — pagination, rate limits, error envelopes. Not in any spec.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quirks: Option<Quirks>,
    /// Parameter-level corrections: a wrong type, a false `required`, a missing description.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ParamPatch>,
}

/// A correction to one parameter of a selected operation.
///
/// Identified by `name` **and** `position`, because a vendor may bind the same name in two places
/// and because the position is what decides where the value travels on the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamPatch {
    /// The parameter name as the vendor spec declares it.
    pub name: String,
    /// Where on the request it travels.
    pub position: ParamPosition,
    /// Corrects the vendor's `required` flag. Freshdesk's collection marks a path parameter
    /// optional, which produces `PUT /tickets/` when it is omitted
    /// (`provider-operation-inventory.md` §6.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// Replaces the vendor's description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Replaces the vendor's JSON Schema for this parameter — the pressure valve for a spec that
    /// types a date as a bare string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<JsonSchema>,
}

/// Where a parameter travels on the request. Mirrors the groups of [`ParamSet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ParamPosition {
    /// Interpolated into the path template.
    Path,
    /// A query-string parameter.
    Query,
    /// A caller-supplied request header.
    Header,
    /// A field of the JSON request body.
    Body,
}

/// The wire shape of `providers/<name>.toml`.
///
/// This is the *only* type that names the file's top-level keys, and it deserializes the connector
/// fields straight into the IR types rather than into shadows of them. Two consequences worth
/// stating:
///
/// - there is no translation layer to drift out of sync with the IR;
/// - `deny_unknown_fields` on the IR types is therefore what makes the *file* strict — the loader
///   could not add that from outside, which is C-2's review finding restated as a design constraint.
///
/// It is private: [`load`] is the entry point, and returning a validated [`LoadedProvider`] rather
/// than a raw parse is the point of having a loader at all.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderFile {
    id: String,
    #[serde(default)]
    authority: Option<String>,
    #[serde(default)]
    api_version: Option<String>,
    #[serde(default)]
    services: Vec<Service>,
    #[serde(default)]
    vendor: String,
    base_url: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    auth: Vec<AuthMethod>,
    #[serde(default)]
    default_auth: Vec<AuthRequirement>,
    #[serde(default)]
    const_headers: BTreeMap<String, String>,
    #[serde(default)]
    operations: Vec<Operation>,
    #[serde(default)]
    events: Vec<EventDecl>,
    #[serde(default)]
    channels: Vec<ChannelBinding>,
    #[serde(default)]
    config: Vec<ConfigField>,
    #[serde(default)]
    verify: Option<String>,
    #[serde(default)]
    graphs: Vec<Graph>,
    #[serde(default)]
    spec: Option<SpecSource>,
    #[serde(default)]
    patch: Patch,
}

/// Parses and validates one `providers/<name>.toml`.
///
/// `name` is only ever used to label errors — `providers/zendesk.toml` — so the caller decides how
/// the file identifies itself. `source` is the file's bytes as text; **no IO happens here**.
///
/// The connector's [`Provenance::toml_sha256`] is computed from `source` on the way through, which
/// is what lets `connectors.lock` (C-7) detect an edited provider file without re-reading it.
///
/// # Errors
///
/// [`Error::ParseProvider`](crate::Error::ParseProvider) when the file is not well-formed TOML or
/// does not match the schema, and [`Error::InvalidProvider`](crate::Error::InvalidProvider) — with
/// *every* problem found, not just the first — when it parses but is not a valid connector.
pub fn load(name: &str, source: &str) -> crate::Result<LoadedProvider> {
    let file: ProviderFile = match toml::from_str(source) {
        Ok(file) => file,
        // `deny_unknown_fields` has already reported `roles` as an unknown top-level key and listed
        // every key that *would* have been valid — which says the key is wrong without saying where
        // it belongs. This is the one key worth naming a destination for, because it is not wrong,
        // only one level too high. A well-formed `ProviderFile` can never carry it, so the extra
        // parse is paid on the error path alone.
        Err(parse) => {
            return Err(if declares_provider_roles(source) {
                crate::Error::InvalidProvider {
                    name: name.to_owned(),
                    problems: vec![PROVIDER_LEVEL_ROLES.to_owned()],
                }
            } else {
                crate::Error::ParseProvider {
                    name: name.to_owned(),
                    source: Box::new(parse),
                }
            });
        }
    };

    // Kept before `assemble` distributes it, so a provider-level constant header is reported once
    // rather than once per operation that inherited it.
    let provider_headers = file.const_headers.clone();
    let loaded = assemble(file, source);

    let problems = validate(&loaded, &provider_headers);
    if !problems.is_empty() {
        return Err(crate::Error::InvalidProvider {
            name: name.to_owned(),
            problems,
        });
    }

    Ok(loaded)
}

/// The refusal for a provider-level `roles` key — C-120.
///
/// A provider's roles are the union of its services' and are computed, so the key does not exist at
/// that level at all. Saying only "unknown field" would leave an author to guess; the message that
/// pays for itself names the level that does own it, including for the single-surface case, which is
/// the one an author is most likely to be in when they reach for the key.
const PROVIDER_LEVEL_ROLES: &str = "\
    `roles` is not a provider-level key. A role is a capability of one API surface, so it is \
    declared on a `[[services]]` entry, and a provider's roles are derived as the union of its \
    services' — never authored, for the reason a config field's `level` is derived from its \
    `binds`. A provider with a single API surface declares `[[services]]` with `name = \"default\"` \
    and puts them there";

/// Whether the file states a **top-level** `roles` key, so [`load`] can say where it belongs.
///
/// Reached only when the typed parse has already failed, and deliberately tolerant: a file too
/// malformed to parse as a table is not a roles problem, so it falls through to `toml`'s own error.
fn declares_provider_roles(source: &str) -> bool {
    source
        .parse::<toml::Table>()
        .is_ok_and(|table| table.contains_key("roles"))
}

/// Turns the parsed file into a [`LoadedProvider`], folding `[spec]` into the connector's
/// provenance and distributing provider-level constant headers onto every operation. No validation
/// happens here — assembling and judging are separate so that validation can see the finished value
/// and report on all of it at once.
fn assemble(file: ProviderFile, source: &str) -> LoadedProvider {
    let spec = file.spec;
    let mut operations = file.operations;
    distribute_const_headers(&file.const_headers, &mut operations);
    let provenance = Provenance {
        source_url: spec.as_ref().and_then(|s| s.source_url.clone()),
        upstream_version: spec.as_ref().and_then(|s| s.upstream_version.clone()),
        fetched_at: spec.as_ref().and_then(|s| s.fetched_at.clone()),
        spec_sha256: spec.as_ref().and_then(|s| s.sha256.clone()),
        toml_sha256: Some(sha256_hex(source.as_bytes())),
    };

    LoadedProvider {
        connector: Connector {
            id: file.id,
            authority: file.authority,
            api_version: file.api_version,
            services: file.services,
            vendor: file.vendor,
            base_url: file.base_url,
            description: file.description,
            auth: file.auth,
            default_auth: file.default_auth,
            operations,
            events: file.events,
            channels: file.channels,
            config: file.config,
            verify: file.verify,
            graphs: file.graphs,
            provenance,
        },
        spec,
        patch: file.patch,
    }
}

/// Copies the provider's constant headers onto every operation, an operation's own entry winning.
///
/// **Resolved here rather than carried as inheritance**, unlike [`Connector::default_auth`]. Auth
/// inheritance has to survive into the IR because [`Operation::auth`] is a three-state field whose
/// `None` means *inherit* and carries meaning that resolving would erase. A constant header has no
/// such state — it is request content, not policy — so an operation whose IR states every header it
/// sends is one that no consumer (emitter, manifest, catalogue) has to re-derive an inheritance to
/// read. The file keeps the one-line shorthand; the IR is the normalized form, which is what it is
/// for.
///
/// The match is case-insensitive because HTTP field names are (RFC 9110 §5.1). `Notion-Version` and
/// `notion-version` are one header, so keeping both would send it twice with two values; the
/// operation's own spelling and value are the ones that survive.
fn distribute_const_headers(provider: &BTreeMap<String, String>, operations: &mut [Operation]) {
    if provider.is_empty() {
        return;
    }
    for operation in operations {
        for (name, value) in provider {
            let overridden = operation
                .params
                .const_headers
                .keys()
                .any(|own| own.eq_ignore_ascii_case(name));
            if !overridden {
                operation
                    .params
                    .const_headers
                    .insert(name.clone(), value.clone());
            }
        }
    }
}

/// Everything wrong with the file, in the order an author would read it: the connector itself, then
/// its credentials, then its operations, then the patch set.
///
/// Returning a `Vec` rather than short-circuiting is deliberate — see the module docs.
fn validate(loaded: &LoadedProvider, provider_headers: &BTreeMap<String, String>) -> Vec<String> {
    let mut problems = Vec::new();
    let connector = &loaded.connector;

    if connector.id.trim().is_empty() {
        problems.push("`id` must not be empty — it names the generated `<id>.flux`".to_owned());
    }
    if connector.base_url.trim().is_empty() {
        problems.push(
            "`base_url` must not be empty. It is stated explicitly even when a spec is present: \
             the babelforce document declares staging as `servers[0]`, so a positional default \
             would silently target the dev environment"
                .to_owned(),
        );
    }

    if loaded.spec.is_none() && connector.operations.is_empty() {
        problems.push(
            "declares neither `[spec]` nor any `[[operations]]`, so it describes no operations at \
             all. Write the operations inline for a hand-authored connector, or point `[spec]` at \
             a vendored spec and select operations with `[[patch.operations]]`"
                .to_owned(),
        );
    }
    if loaded.spec.is_none() && !loaded.patch.is_empty() {
        problems.push(
            "declares `[[patch.operations]]` but no `[spec]`; there is nothing for the patches to \
             apply to"
                .to_owned(),
        );
    }
    if let Some(spec) = &loaded.spec {
        if spec.path.trim().is_empty() {
            problems.push(
                "`[spec] path` must not be empty — it points at the vendored spec under `specs/`"
                    .to_owned(),
            );
        }
    }

    validate_services(connector, &mut problems);
    validate_credentials(connector, &mut problems);
    validate_const_headers(connector, provider_headers, &mut problems);
    validate_operations(connector, &mut problems);
    validate_events(connector, &mut problems);
    validate_channels(connector, &mut problems);
    validate_config(connector, &mut problems);
    validate_verify(connector, &mut problems);
    validate_graphs(connector, &mut problems);
    validate_member_namespace(connector, &mut problems);
    validate_patch(loaded, &mut problems);

    problems
}

/// Checks the configuration surface — what a human is asked for, and where each answer goes.
///
/// Two properties, and the first is the one that closes a defect every templated provider records in
/// a comment: **a connector must ask for everything it needs**, and **it must not ask for anything it
/// cannot use**. A template variable nobody declares is a connector that silently cannot be
/// configured; a field binding nothing real is a question whose answer is discarded.
fn validate_config(connector: &Connector, problems: &mut Vec<String>) {
    let mut seen: Vec<&str> = Vec::new();

    for field in &connector.config {
        let name = field.name.as_str();
        if name.trim().is_empty() {
            problems.push("a configuration field has an empty `name`".to_owned());
            continue;
        }
        if seen.contains(&name) {
            problems.push(format!(
                "configuration field {name:?} is declared more than once; the name is the key a host \
                 stores the collected value under"
            ));
        }
        seen.push(name);

        if let Err(reason) = crate::address::validate_member_name(name) {
            problems.push(format!(
                "configuration field {name:?} has an invalid `name`: {reason}"
            ));
        }
        validate_member_service(
            connector,
            "configuration field",
            name,
            &field.service,
            problems,
        );

        // A field with no label or no help cannot be rendered into a form that anyone can answer.
        // Defaulting either to `name` would ship `zendesk.api_token` as user-facing copy.
        if field.label.trim().is_empty() {
            problems.push(format!(
                "configuration field {name:?} has an empty `label`; it is the text a form shows \
                 beside the input, and there is no sensible default for it — {name:?} is an \
                 identifier, not a label"
            ));
        }
        if field.help.trim().is_empty() {
            problems.push(format!(
                "configuration field {name:?} has an empty `help`; a field a user cannot answer is a \
                 field that stops the installation"
            ));
        }

        // The example is a placeholder a user will copy, so it has to satisfy the field's own rule.
        if let Some(example) = &field.example {
            if let Err(reason) = field.format.validate(example) {
                problems.push(format!(
                    "configuration field {name:?} declares `format = \"{}\"` but an `example` that \
                     does not satisfy it: {reason}. A placeholder that would fail the field's own \
                     validation is worse than none, because a user copies it",
                    field.format.word()
                ));
            }
        }

        validate_binding(connector, field, problems);
    }

    validate_every_template_variable_is_asked_for(connector, problems);
}

/// Checks one field's `binds`: that it parses, that it resolves, and that `secret` agrees with it.
fn validate_binding(connector: &Connector, field: &ConfigField, problems: &mut Vec<String>) {
    let name = field.name.as_str();
    let binding = match parse_binding(&field.binds) {
        Ok(binding) => binding,
        Err(reason) => {
            problems.push(format!("configuration field {name:?}: {reason}"));
            return;
        }
    };

    match binding {
        Binding::Endpoint { variable } => {
            let declared: Vec<&str> = connector
                .service_names()
                .into_iter()
                .flat_map(|service| template_variables(connector.base_url_of(service)))
                .collect();
            if !declared.contains(&variable) {
                problems.push(format!(
                    "configuration field {name:?} binds `{{{variable}}}`, which no service's \
                     `base_url` carries. This provider's templates offer: {}",
                    if declared.is_empty() {
                        "nothing — every base URL is literal".to_owned()
                    } else {
                        declared.join(", ")
                    }
                ));
            }
        }
        Binding::Credential { name: credential } | Binding::Username { name: credential } => {
            match connector.auth_method(credential) {
                None => problems.push(format!(
                    "configuration field {name:?} binds credential {credential:?}, which no \
                     `[[auth]]` block declares"
                )),
                Some(method) => {
                    // Only `basic` has a username half; for every other scheme the whole credential
                    // is the secret, so a username field would collect a value with nowhere to go.
                    if matches!(binding, Binding::Username { .. })
                        && method.scheme != AuthScheme::Basic
                    {
                        problems.push(format!(
                            "configuration field {name:?} binds the username half of credential \
                             {credential:?}, which uses the `{}` scheme. Only `basic` sends a \
                             username — it is `base64(<user>:<secret>)`, and every other scheme \
                             sends the secret alone",
                            scheme_word(&method.scheme)
                        ));
                    }
                }
            }
        }
        Binding::OAuthClientId | Binding::OAuthClientSecret => {
            if !connector.auth.iter().any(|method| method.oauth2.is_some()) {
                problems.push(format!(
                    "configuration field {name:?} binds an OAuth app registration, but no `[[auth]]` \
                     block declares an `[auth.oauth2]` spec. There is no OAuth flow for a client id \
                     to belong to"
                ));
            }
        }
    }

    // The agreement that keeps this from becoming a second source of truth. flux partitions secret
    // from non-secret BY TYPE — an `AuthMethod` versus a `ConfigSpec` — and enforces it host-side.
    // A field that disagreed would put a contradicting claim in front of that enforcement.
    let expected = binding.is_secret();
    if field.secret != expected {
        problems.push(if expected {
            format!(
                "configuration field {name:?} binds {} but declares `secret = false`. That value is \
                 a credential: it must be masked on input, kept out of logs, and stored where a \
                 secret is stored",
                field.binds
            )
        } else {
            format!(
                "configuration field {name:?} binds {} but declares `secret = true`. That value is \
                 configuration, not a credential — marking it secret hides it from an operator who \
                 needs to read it back, and claims gating this repository does not provide",
                field.binds
            )
        });
    }
}

/// **Every template variable is asked for.** This is the rule that closes the `SCHEMA GAP:` comment
/// four shipped providers have carried since C-17.
///
/// A `{subdomain}` nobody declares is not a cosmetic omission: the connector has no valid destination
/// URL and no way to tell anyone what is missing. `catalog.json` already publishes an
/// `unbound-base-url-template` issue for exactly this, which is a diagnosis with no remedy attached.
fn validate_every_template_variable_is_asked_for(
    connector: &Connector,
    problems: &mut Vec<String>,
) {
    for service in connector.service_names() {
        for variable in template_variables(connector.base_url_of(service)) {
            let bound = connector.config_of(service).any(|field| {
                matches!(field.binding(), Some(Binding::Endpoint { variable: v }) if v == variable)
            });
            if !bound {
                let where_ = if service == DEFAULT_SERVICE {
                    String::new()
                } else {
                    format!(" of service {service:?}")
                };
                problems.push(format!(
                    "the base URL{where_} carries `{{{variable}}}`, which no `[[config]]` field \
                     binds. Until something asks a user for it the connector has no valid \
                     destination URL — declare a field with `binds = \"endpoint.{variable}\"`"
                ));
            }
        }
    }
}

/// Checks the declared verification operation — a host's "Test connection".
fn validate_verify(connector: &Connector, problems: &mut Vec<String>) {
    let Some(verify) = &connector.verify else {
        return;
    };
    match connector.operation(verify) {
        None => problems.push(format!(
            "`verify` names operation {verify:?}, which no `[[operations]]` block declares"
        )),
        // A "Test connection" button that could create a ticket is a button nobody dares press. The
        // check is on declared risk rather than on the HTTP method, because this provider's own
        // metadata is the thing a host will reason about.
        Some(operation) if operation.risk == Risk::High || operation.risk == Risk::Destructive => {
            problems.push(format!(
                "`verify` names operation {verify:?}, which declares `risk = \"{}\"`. A \
                 connection test runs unattended whenever someone opens a settings page, so it must \
                 be a read a user would not mind being repeated",
                match operation.risk {
                    Risk::High => "high",
                    _ => "destructive",
                }
            ));
        }
        Some(_) => {}
    }
}

/// Checks the inbound half of a service's members.
///
/// Name spelling and service membership only — an event declares no behaviour of its own, so there is
/// nothing else here to be wrong. What *uses* an event is a [`ChannelBinding`], and the
/// cross-references are checked there.
fn validate_events(connector: &Connector, problems: &mut Vec<String>) {
    let mut seen: Vec<&str> = Vec::new();

    for event in &connector.events {
        let name = event.name.as_str();
        if name.trim().is_empty() {
            problems.push("an event has an empty `name`".to_owned());
            continue;
        }
        if seen.contains(&name) {
            problems.push(format!(
                "event {name:?} is declared more than once; the event name is the trigger label a \
                 program matches on, so it must denote one event"
            ));
        }
        seen.push(name);

        if let Err(reason) = crate::address::validate_member_name(name) {
            problems.push(format!("event {name:?} has an invalid `name`: {reason}"));
        }
        validate_member_service(connector, "event", name, &event.service, problems);
    }
}

/// Checks that a member's service is one this provider has.
///
/// The operation-side equivalent is [`validate_operation_service`], which stays separate because its
/// error text names the multi-service trap specifically; this is the shorter form the other two
/// kinds need.
fn validate_member_service(
    connector: &Connector,
    kind: &str,
    name: &str,
    service: &str,
    problems: &mut Vec<String>,
) {
    let available = connector.service_names();
    if available.contains(&service) {
        return;
    }
    problems.push(format!(
        "{kind} {name:?} names service {service:?}, which no `[[services]]` entry declares. This \
         provider declares: {}",
        available.join(", ")
    ));
}

/// Checks every channel binding: its transport's own rules, and every reference it makes.
///
/// **Every rule here is a refusal, never a degradation.** A binding is a promise that an event can
/// reach a flow and that a reply can go back; a binding that half-holds is the plausible-but-wrong
/// artifact `AGENTS.md` requires the pipeline to refuse rather than emit.
fn validate_channels(connector: &Connector, problems: &mut Vec<String>) {
    let mut seen: Vec<&str> = Vec::new();

    for channel in &connector.channels {
        let name = channel.name.as_str();
        if name.trim().is_empty() {
            problems.push("a channel binding has an empty `name`".to_owned());
            continue;
        }
        if seen.contains(&name) {
            problems.push(format!(
                "channel binding {name:?} is declared more than once; the binding name is what an \
                 operator's `channel` declaration selects, so it must denote one surface"
            ));
        }
        seen.push(name);

        if let Err(reason) = crate::address::validate_member_name(name) {
            problems.push(format!(
                "channel binding {name:?} has an invalid `name`: {reason}"
            ));
        }
        validate_member_service(
            connector,
            "channel binding",
            name,
            &channel.service,
            problems,
        );

        validate_channel_events(connector, channel, problems);
        validate_channel_verification(connector, channel, problems);
        validate_channel_payload(channel, problems);
        validate_channel_reply(connector, channel, problems);
        validate_channel_transport(connector, channel, problems);
        validate_channel_setup(connector, channel, problems);

        for (label, selector) in [
            ("discriminator", &channel.discriminator),
            ("delivery_id", &channel.delivery_id),
        ] {
            if let Some(selector) = selector {
                validate_selector(name, label, selector, problems);
            }
        }
    }
}

/// Every event a binding carries must exist **in the binding's own service**.
fn validate_channel_events(
    connector: &Connector,
    channel: &ChannelBinding,
    problems: &mut Vec<String>,
) {
    let name = channel.name.as_str();

    // A push binding that carries no events delivers nothing: the transport would connect, hold, and
    // route every arrival to a label no trigger can name. A poll binding is different — its cursor
    // operation is what it carries.
    if channel.events.is_empty() && channel.transport != Transport::Poll {
        problems.push(format!(
            "channel binding {name:?} lists no `events`, so nothing it receives could reach a \
             trigger. A binding names the events it carries; only a `poll` binding may omit them, \
             because its `cursor` operation is what it carries"
        ));
    }

    for event in &channel.events {
        match connector.event(event) {
            None => problems.push(format!(
                "channel binding {name:?} carries event {event:?}, which no `[[events]]` block \
                 declares"
            )),
            Some(declared) if declared.service != channel.service => problems.push(format!(
                "channel binding {name:?} is in service {:?} but carries event {event:?}, which is \
                 in service {:?}. A binding carries the events of its own service — the two version \
                 and address independently",
                channel.service, declared.service
            )),
            Some(_) => {}
        }
    }
}

/// The tri-state on [`ChannelBinding::verification`], and the HMAC parameters when there are any.
fn validate_channel_verification(
    connector: &Connector,
    channel: &ChannelBinding,
    problems: &mut Vec<String>,
) {
    let name = channel.name.as_str();

    match (&channel.verification, channel.transport) {
        // Silence on an open endpoint is how an unverified event gets presented as a trusted one.
        // The author must say something, even if what they say is "this vendor publishes nothing".
        (None, Transport::Webhook) => problems.push(format!(
            "channel binding {name:?} uses the `webhook` transport and states no `verification`. An \
             endpoint anyone can POST to must say how it proves the caller is the vendor — write a \
             `[[channels]].verification.hmac` table, or `verification = \"none\"` to state \
             deliberately that the vendor publishes no signature"
        )),
        (Some(scheme), transport) if transport != Transport::Webhook => {
            let _ = scheme;
            problems.push(format!(
                "channel binding {name:?} states `verification`, which only the `webhook` transport \
                 uses. A `{}` binding is authenticated by the credential that opens the connection",
                transport_word(transport)
            ));
        }
        _ => {}
    }

    if let Some(VerificationScheme::Hmac(hmac)) = &channel.verification {
        validate_hmac(connector, name, hmac, problems);
    }
}

/// The HMAC matrix's own consistency: a fillable template, a bounded replay window, and a secret that
/// resolves to a credential declared for exactly this purpose.
fn validate_hmac(
    connector: &Connector,
    channel: &str,
    hmac: &HmacSpec,
    problems: &mut Vec<String>,
) {
    if hmac.header.trim().is_empty() {
        problems.push(format!(
            "channel binding {channel:?} declares an HMAC scheme with an empty `header`; it names \
             the header carrying the signature"
        ));
    }

    let placeholders = signed_placeholders(&hmac.signed);
    for placeholder in &placeholders {
        if !SIGNED_PLACEHOLDERS.contains(&placeholder.as_str()) {
            problems.push(format!(
                "channel binding {channel:?} has `signed = {:?}`, which interpolates \
                 {{{placeholder}}}; the host can fill only {{body}} and {{timestamp}}",
                hmac.signed
            ));
        }
    }

    // **The rule this whole struct rests on.** A template that never interpolates {body} signs a
    // string the payload never enters, so a signature captured from one delivery verifies *any*
    // forged payload — bounded only by the tolerance, and by nothing at all without one. It is the
    // same defect as the unterminated brace `signed_placeholders` reports, except that reaching it
    // needs no typo: `signed = "{timestamp}"` is well formed, and every other check here passes on
    // it. Refusing an empty template is not enough, because the hole is not emptiness.
    if !placeholders.iter().any(|p| p == "body") {
        problems.push(format!(
            "channel binding {channel:?} has `signed = {:?}`, which never interpolates {{body}}. \
             The signed string must cover the request body, or a signature captured from one \
             delivery verifies every forged payload that follows it — the signature would prove \
             only that somebody, once, held the secret",
            hmac.signed
        ));
    }

    let timestamped = placeholders.iter().any(|p| p == "timestamp");

    match (&hmac.timestamp, timestamped) {
        (None, true) => problems.push(format!(
            "channel binding {channel:?} signs over {{timestamp}} but declares no `timestamp` \
             selector. The template says the value is signed; it cannot say where the value is \
             read from, and a host left to guess would fall back to its own clock — which verifies \
             nothing"
        )),
        (Some(_), false) => problems.push(format!(
            "channel binding {channel:?} declares a `timestamp` selector, but its `signed` template \
             does not interpolate {{timestamp}} — the value would be read and never used"
        )),
        (Some(selector), true) => {
            validate_selector(channel, "verification timestamp", selector, problems);
            // Reading the timestamp out of the body inverts the order that makes verification mean
            // anything: the body would have to be parsed to find the value that decides whether the
            // body is trustworthy, which exposes a parser to any anonymous caller. flux refuses it
            // in its own request path; refusing it here puts the failure in a build instead.
            if selector.source == FieldSource::Body {
                problems.push(format!(
                    "channel binding {channel:?} reads its verification timestamp from the body \
                     ({:?}). A body-sourced timestamp has to be parsed *before* the bytes carrying \
                     it are verified, which inverts the order verification depends on; a signed \
                     timestamp is read from a header",
                    selector.name
                ));
            }
        }
        (None, false) => {}
    }

    // A timestamped scheme with no window is a signature that replays forever — strictly worse than
    // not timestamping at all, because it reads as though replay had been handled.
    match (&hmac.tolerance, timestamped) {
        (None, true) => problems.push(format!(
            "channel binding {channel:?} signs over {{timestamp}} but declares no `tolerance`. A \
             timestamped signature with no window replays forever; state how old a request may be, \
             as in `tolerance = \"5m\"`"
        )),
        (Some(_), false) => problems.push(format!(
            "channel binding {channel:?} declares a `tolerance`, but its `signed` template does not \
             interpolate {{timestamp}} — there is no timestamp to bound"
        )),
        // Requiring a window is not the same as having one. An unparseable spelling leaves the real
        // window to whatever each host decides at runtime, while reading exactly as though replay
        // had been handled.
        (Some(tolerance), true) => {
            if let Err(reason) = parse_tolerance(tolerance) {
                problems.push(format!(
                    "channel binding {channel:?} declares `tolerance = {tolerance:?}`, which is not \
                     a window a host can apply: {reason}"
                ));
            }
        }
        (None, false) => {}
    }

    // The spelling of a value nothing reads describes nothing — the same objection as an unused
    // selector or an unused window.
    if !timestamped && hmac.timestamp_format.is_some() {
        problems.push(format!(
            "channel binding {channel:?} declares a `timestamp_format`, but its `signed` template \
             does not interpolate {{timestamp}} — there is no timestamp to spell"
        ));
    }

    match connector.auth_method(&hmac.secret) {
        None => problems.push(format!(
            "channel binding {channel:?} names webhook secret {:?}, which no `[[auth]]` block \
             declares. An inbound secret is a credential like any other, so that the manifest names \
             every credential this connector requires",
            hmac.secret
        )),
        Some(method) if method.scheme != AuthScheme::Signing => problems.push(format!(
            "channel binding {channel:?} names webhook secret {:?}, which is declared with the \
             `{}` scheme. A verification secret is never placed in an outgoing request, so it is \
             declared `scheme = \"signing\"` — using an outbound credential here would spend the \
             same value in both directions",
            hmac.secret,
            scheme_word(&method.scheme)
        )),
        Some(_) => {}
    }
}

/// A payload map binds Flux symbols to dotted paths, and both halves have to be spellable.
fn validate_channel_payload(channel: &ChannelBinding, problems: &mut Vec<String>) {
    let name = channel.name.as_str();
    for (symbol, path) in &channel.payload {
        if let Err(reason) = validate_symbol(symbol) {
            problems.push(format!("channel binding {name:?}: {reason}"));
        }
        if let Err(reason) = validate_path(path) {
            problems.push(format!(
                "channel binding {name:?} maps {symbol:?} to an invalid source path: {reason}"
            ));
        }
    }
}

/// The reply must resolve, and it must be **completely** bound.
///
/// The completeness rule is the one that earns its keep. A reply missing a required parameter builds,
/// ships, passes every artifact check, and then fails on the first real delivery — at which point the
/// failure is in an operator's production channel rather than in a build they were reading.
fn validate_channel_reply(
    connector: &Connector,
    channel: &ChannelBinding,
    problems: &mut Vec<String>,
) {
    let name = channel.name.as_str();
    let Some(Reply {
        operation,
        result,
        bind,
    }) = &channel.reply
    else {
        return;
    };

    let Some(target) = connector.operation(operation) else {
        problems.push(format!(
            "channel binding {name:?} replies with operation {operation:?}, which no \
             `[[operations]]` block declares. A binding's reply is an ordinary operation of this \
             same connector — that is what makes it a composition rather than a second code path"
        ));
        return;
    };

    if target.service != channel.service {
        problems.push(format!(
            "channel binding {name:?} is in service {:?} but replies with operation {operation:?}, \
             which is in service {:?}",
            channel.service, target.service
        ));
    }

    for (param, symbol) in bind {
        if !target.params.iter().any(|p| &p.name == param) {
            problems.push(format!(
                "channel binding {name:?} binds reply parameter {param:?}, which operation \
                 {operation:?} does not declare"
            ));
        }
        if !channel.payload.contains_key(symbol) {
            problems.push(format!(
                "channel binding {name:?} binds reply parameter {param:?} to {symbol:?}, which its \
                 `payload` map does not declare. A reply is filled from the inbound payload, so \
                 every bound value has to be something the payload produced"
            ));
        }
    }

    if let Some(result) = result {
        if !target.params.iter().any(|p| &p.name == result) {
            problems.push(format!(
                "channel binding {name:?} sends its journey result to reply parameter {result:?}, \
                 which operation {operation:?} does not declare"
            ));
        }
        if bind.contains_key(result) {
            problems.push(format!(
                "channel binding {name:?} both binds reply parameter {result:?} from the payload \
                 and sends the journey result to it. One parameter carries one value — decide which"
            ));
        }
    }

    for param in target.params.iter().filter(|p| p.required) {
        let covered =
            bind.contains_key(&param.name) || result.as_deref() == Some(param.name.as_str());
        if !covered {
            problems.push(format!(
                "channel binding {name:?} replies with operation {operation:?} but leaves its \
                 required parameter {:?} unbound. Bind it from the `payload` map, or name it as \
                 `result` if it carries the journey's own output — every required parameter is \
                 settled at build time, or the reply fails on the first delivery instead of in this \
                 diff",
                param.name
            ));
        }
    }
}

/// `cursor` and `interval` belong to `poll`, and `poll` cannot do without a cursor.
///
/// See [`crate::inbound`] for the reasoning: flux's cron drops ticks across a restart and replays
/// none of them, so a poll that cannot resume from a recorded position loses events silently.
fn validate_channel_transport(
    connector: &Connector,
    channel: &ChannelBinding,
    problems: &mut Vec<String>,
) {
    let name = channel.name.as_str();

    if channel.transport == Transport::Poll {
        match &channel.cursor {
            None => problems.push(format!(
                "channel binding {name:?} uses the `poll` transport and declares no `cursor`. flux's \
                 schedule channel is best-effort — a restart drops ticks and replays none of them — \
                 so the cursor operation, not the interval, is what makes a poll correct. Name the \
                 operation that reads forward from a recorded position"
            )),
            Some(cursor) => match connector.operation(cursor) {
                None => problems.push(format!(
                    "channel binding {name:?} names cursor operation {cursor:?}, which no \
                     `[[operations]]` block declares"
                )),
                Some(target) if target.service != channel.service => problems.push(format!(
                    "channel binding {name:?} is in service {:?} but names cursor operation \
                     {cursor:?}, which is in service {:?}",
                    channel.service, target.service
                )),
                Some(_) => {}
            },
        }
    } else {
        for (field, present) in [
            ("cursor", channel.cursor.is_some()),
            ("interval", channel.interval.is_some()),
        ] {
            if present {
                problems.push(format!(
                    "channel binding {name:?} declares `{field}`, which only the `poll` transport \
                     uses. A `{}` binding is woken by the vendor, not by a schedule",
                    transport_word(channel.transport)
                ));
            }
        }
    }
}

/// How a binding gets registered — and the rule that a webhook must say.
///
/// A product that knows a callback URL and nothing about what to do with it cannot finish an
/// installation. That is the same shape as the verification rule: an open endpoint has to state
/// something, and silence is not one of the options.
fn validate_channel_setup(
    connector: &Connector,
    channel: &ChannelBinding,
    problems: &mut Vec<String>,
) {
    let name = channel.name.as_str();

    if channel.transport == Transport::Webhook
        && channel.subscription.is_none()
        && channel.setup.is_none()
    {
        problems.push(format!(
            "channel binding {name:?} uses the `webhook` transport and says neither how to register \
             it nor what a human must do. A product can show a callback URL, but with no \
             `[channels.subscription]` naming the operation that registers it and no \
             `[channels.setup]` steps to follow, nobody can finish connecting it"
        ));
    }

    // Registration belongs to a transport a vendor delivers to. A socket we opened has nothing to
    // register, and a poll is driven by our own schedule.
    for (field, present) in [
        ("subscription", channel.subscription.is_some()),
        ("setup", channel.setup.is_some()),
    ] {
        if present && channel.transport != Transport::Webhook {
            problems.push(format!(
                "channel binding {name:?} declares `{field}`, which only the `webhook` transport \
                 uses. A `{}` binding has no endpoint for the vendor to register",
                transport_word(channel.transport)
            ));
        }
    }

    if let Some(Subscription {
        subscribe,
        unsubscribe,
        list,
        callback_param,
    }) = &channel.subscription
    {
        for (label, id) in [
            ("subscribe", Some(subscribe)),
            ("unsubscribe", unsubscribe.as_ref()),
            ("list", list.as_ref()),
        ] {
            let Some(id) = id else { continue };
            match connector.operation(id) {
                None => problems.push(format!(
                    "channel binding {name:?} names `{label}` operation {id:?}, which no \
                     `[[operations]]` block declares. Registering a webhook is an ordinary \
                     authorized write, so it is an ordinary operation"
                )),
                Some(target) if target.service != channel.service => problems.push(format!(
                    "channel binding {name:?} is in service {:?} but names `{label}` operation \
                     {id:?}, which is in service {:?}",
                    channel.service, target.service
                )),
                Some(_) => {}
            }
        }

        // The callback URL is the product's, and this names where to put it. A parameter that does
        // not exist means the URL would be assembled into a request that drops it.
        if let Some(target) = connector.operation(subscribe) {
            if !target.params.iter().any(|p| &p.name == callback_param) {
                problems.push(format!(
                    "channel binding {name:?} sends its callback URL to parameter \
                     {callback_param:?}, which operation {subscribe:?} does not declare"
                ));
            }
        }
    }

    if let Some(ManualSetup { steps, .. }) = &channel.setup {
        if steps.is_empty() {
            problems.push(format!(
                "channel binding {name:?} declares `[channels.setup]` with no `steps`. An empty \
                 instruction list is the same as no instructions, stated more confidently"
            ));
        }
        for step in steps {
            if step.trim().is_empty() {
                problems.push(format!("channel binding {name:?} has an empty setup step"));
            }
        }
    }
}

/// A selector reads one named value off an inbound request; a body selector addresses it by path.
fn validate_selector(channel: &str, label: &str, selector: &Selector, problems: &mut Vec<String>) {
    if selector.name.trim().is_empty() {
        problems.push(format!(
            "channel binding {channel:?} has a `{label}` with an empty `name`"
        ));
        return;
    }
    if selector.source == crate::inbound::FieldSource::Body {
        if let Err(reason) = validate_path(&selector.name) {
            problems.push(format!(
                "channel binding {channel:?} has a `{label}` reading an invalid body path: {reason}"
            ));
        }
    }
}

/// Checks every flow graph: that its references resolve, and that it has a lowering at all.
///
/// **The structural rules are not style.** Flux has no `goto`, so a cyclic graph and a graph whose
/// control regions overlap have no expressible form — a compiler that accepted them would have to
/// guess, and guessing produces plausible-but-wrong Flux, which is the one output this pipeline
/// refuses everywhere else.
fn validate_graphs(connector: &Connector, problems: &mut Vec<String>) {
    let mut seen: Vec<&str> = Vec::new();

    for graph in &connector.graphs {
        let name = graph.name.as_str();
        if name.trim().is_empty() {
            problems.push("a graph has an empty `name`".to_owned());
            continue;
        }
        if seen.contains(&name) {
            problems.push(format!(
                "graph {name:?} is declared more than once; the name becomes an emitted `op`, so it \
                 must denote one flow"
            ));
        }
        seen.push(name);

        if let Err(reason) = crate::address::validate_member_name(name) {
            problems.push(format!("graph {name:?} has an invalid `name`: {reason}"));
        }
        validate_member_service(connector, "graph", name, &graph.service, problems);

        validate_graph_nodes(connector, graph, problems);
        validate_graph_structure(graph, problems);
        validate_graph_edges(graph, problems);
    }
}

/// Every node's references resolve, in the graph's own service.
fn validate_graph_nodes(connector: &Connector, graph: &Graph, problems: &mut Vec<String>) {
    let name = graph.name.as_str();
    let mut ids: Vec<&str> = Vec::new();

    for node in &graph.nodes {
        let id = node.id.as_str();
        if id.trim().is_empty() {
            problems.push(format!("graph {name:?} has a node with an empty `id`"));
            continue;
        }
        if ids.contains(&id) {
            problems.push(format!(
                "graph {name:?} declares node {id:?} more than once; a node id is the stable \
                 identity an editor and a diagnostic both key on"
            ));
        }
        ids.push(id);

        match &node.kind {
            NodeKind::Operation { operation } => {
                resolve_member(
                    connector,
                    graph,
                    name,
                    id,
                    "operation",
                    operation,
                    connector
                        .operation(operation)
                        .map(|target| target.service.as_str()),
                    problems,
                );
            }
            NodeKind::Trigger { event } => {
                resolve_member(
                    connector,
                    graph,
                    name,
                    id,
                    "event",
                    event,
                    connector.event(event).map(|target| target.service.as_str()),
                    problems,
                );
            }
            NodeKind::Endpoint { binding } => {
                resolve_member(
                    connector,
                    graph,
                    name,
                    id,
                    "channel binding",
                    binding,
                    connector
                        .channel(binding)
                        .map(|target| target.service.as_str()),
                    problems,
                );
            }
            NodeKind::Select { path } => {
                if let Err(reason) = crate::inbound::validate_path(path) {
                    problems.push(format!(
                        "graph {name:?} node {id:?} selects an invalid path: {reason}"
                    ));
                }
            }
            NodeKind::Object { fields } => {
                for (field, port) in fields {
                    if !node.inputs.iter().any(|p| &p.name == port) {
                        problems.push(format!(
                            "graph {name:?} node {id:?} builds field {field:?} from port {port:?}, \
                             which it does not declare as an input"
                        ));
                    }
                }
            }
            NodeKind::Retry { max, .. } if *max == 0 => problems.push(format!(
                "graph {name:?} node {id:?} retries 0 times. flux's analyzer rejects unbounded loops \
                 and a zero bound is not a loop at all — remove the node or give it a real maximum"
            )),
            NodeKind::Throttle { max, window_ms } if *max == 0 || *window_ms == 0 => {
                problems.push(format!(
                    "graph {name:?} node {id:?} throttles to {max} per {window_ms}ms, which admits \
                     nothing. A throttle bounds a rate; it is not a way to disable a branch"
                ));
            }
            _ => {}
        }

        // A boundary node declares what wakes the flow. It is emitted nowhere, so it can neither
        // consume a value nor sit inside a region that only exists at runtime.
        if node.kind.is_boundary() {
            if !node.inputs.is_empty() {
                problems.push(format!(
                    "graph {name:?} node {id:?} is a `{}` boundary and declares inputs. A boundary \
                     says what wakes the flow; nothing inside the flow can feed it",
                    node.kind.word()
                ));
            }
            if node.region.is_some() {
                problems.push(format!(
                    "graph {name:?} node {id:?} is a `{}` boundary inside a region. A boundary is \
                     emitted nowhere, so it cannot be conditional, retried or rate-limited",
                    node.kind.word()
                ));
            }
        }

        // The rule with teeth. See the module docs on `graph`.
        if matches!(node.kind, NodeKind::Gate { .. }) && !node.outputs.is_empty() {
            problems.push(format!(
                "graph {name:?} node {id:?} is a gate declaring outputs. A gate lowers to Flux's \
                 `when`, which has no else branch here — a symbol bound inside it is *unbound* when \
                 the condition is false, and reading it afterwards fails at runtime. A value that \
                 must escape a conditional needs a branch with a default"
            ));
        }
        if !node.kind.is_region() && !node.outputs.is_empty() && node.region.is_some() {
            // Non-region nodes may have outputs; this only checks that they are reachable, which
            // `validate_graph_edges` covers. Nothing to add here.
        }
    }
}

/// One member reference: it exists, and it belongs to this graph's service.
#[allow(clippy::too_many_arguments)]
fn resolve_member(
    _connector: &Connector,
    graph: &Graph,
    name: &str,
    id: &str,
    kind: &str,
    reference: &str,
    found: Option<&str>,
    problems: &mut Vec<String>,
) {
    match found {
        None => problems.push(format!(
            "graph {name:?} node {id:?} names {kind} {reference:?}, which this connector does not \
             declare"
        )),
        Some(service) if service != graph.service => problems.push(format!(
            "graph {name:?} is in service {:?} but node {id:?} names {kind} {reference:?}, which is \
             in service {service:?}",
            graph.service
        )),
        Some(_) => {}
    }
}

/// No cycles, and every region containment resolves.
fn validate_graph_structure(graph: &Graph, problems: &mut Vec<String>) {
    let name = graph.name.as_str();

    for node in &graph.nodes {
        let Some(region) = node.region.as_deref() else {
            continue;
        };
        match graph.node(region) {
            None => problems.push(format!(
                "graph {name:?} node {:?} names region {region:?}, which is not a node of this graph",
                node.id
            )),
            Some(container) if !container.kind.is_region() => problems.push(format!(
                "graph {name:?} node {:?} is inside {region:?}, which is a `{}` and contains nothing",
                node.id,
                container.kind.word()
            )),
            Some(_) => {}
        }
        if graph.enclosing(&node.id).is_none() {
            problems.push(format!(
                "graph {name:?} node {:?} is contained in itself, directly or through a chain of \
                 regions",
                node.id
            ));
        }
    }

    if graph.topological_order().is_none() {
        problems.push(format!(
            "graph {name:?} has a cycle. Flux has no `goto` and its control flow is strictly nested, \
             so a cyclic graph has no lowering at all — an iteration is a bounded loop node, not an \
             edge pointing backwards"
        ));
    }
}

/// Every edge connects declared ports, and no edge crosses a region boundary.
fn validate_graph_edges(graph: &Graph, problems: &mut Vec<String>) {
    let name = graph.name.as_str();

    for edge in &graph.edges {
        let from = endpoint(graph, name, &edge.from, Side::Output, problems);
        let to = endpoint(graph, name, &edge.to, Side::Input, problems);
        let (Some(from), Some(to)) = (from, to) else {
            continue;
        };

        // A value may enter a region freely — an inner statement reads an outer symbol, which Flux
        // allows. It may only *leave* through a port the region declares, because that is the one
        // place a bound symbol is guaranteed to exist after the block closes.
        let (Some(source_regions), Some(sink_regions)) =
            (graph.enclosing(&from.id), graph.enclosing(&to.id))
        else {
            continue; // a containment cycle, already reported
        };

        for region in &source_regions {
            if sink_regions.contains(region) {
                continue; // the sink is inside the same region; nothing escapes
            }
            let Some(container) = graph.node(region) else {
                continue;
            };
            let escapes_through = container
                .outputs
                .iter()
                .any(|port| port.name == edge.from.port);
            if !escapes_through {
                problems.push(format!(
                    "graph {name:?} has an edge from {:?}.{:?} out of region {region:?} to {:?}, but \
                     {region:?} declares no output port {:?}. A value leaves a region only through a \
                     port the region declares — otherwise the symbol it lowers to may not be bound \
                     when the block closes",
                    from.id, edge.from.port, to.id, edge.from.port
                ));
            }
        }
    }

    if let Some(output) = &graph.output {
        endpoint(graph, name, output, Side::Output, problems);
    }
}

enum Side {
    Input,
    Output,
}

/// Resolve one end of an edge, reporting a missing node or a missing port.
fn endpoint<'a>(
    graph: &'a Graph,
    name: &str,
    reference: &PortRef,
    side: Side,
    problems: &mut Vec<String>,
) -> Option<&'a GraphNode> {
    let Some(node) = graph.node(&reference.node) else {
        problems.push(format!(
            "graph {name:?} has an edge naming node {:?}, which it does not declare",
            reference.node
        ));
        return None;
    };
    let (ports, word) = match side {
        Side::Input => (&node.inputs, "input"),
        Side::Output => (&node.outputs, "output"),
    };
    if !ports.iter().any(|port| port.name == reference.port) {
        problems.push(format!(
            "graph {name:?} node {:?} has no {word} port {:?}",
            reference.node, reference.port
        ));
    }
    Some(node)
}

/// The three member kinds share one namespace per service — see [`Connector::member_names_of`].
///
/// **Cross-kind collisions only.** A name repeated *within* one kind is already reported by that
/// kind's own pass, in its own vocabulary ("the op id is the public name callers and models use"),
/// and reporting it twice would make an author fix one problem and see two. What no single pass can
/// see is an operation and an event that happen to share a name — neither list has a duplicate, and
/// only the union does.
fn validate_member_namespace(connector: &Connector, problems: &mut Vec<String>) {
    for service in connector.service_names() {
        // (name, kind), in the order `member_names_of` yields them.
        let mut seen: Vec<(&str, &str)> = Vec::new();
        let mut reported: Vec<&str> = Vec::new();

        let members = connector
            .operations_of(service)
            // The labels carry their own article, because they are interpolated into a sentence that
            // reads "names both {other} and {kind}" — "a operation" and "a event" otherwise.
            .map(|operation| (operation.id.as_str(), "an operation"))
            .chain(
                connector
                    .events_of(service)
                    .map(|event| (event.name.as_str(), "an event")),
            )
            .chain(
                connector
                    .channels_of(service)
                    .map(|channel| (channel.name.as_str(), "a channel binding")),
            )
            .chain(
                connector
                    .config_of(service)
                    .map(|field| (field.name.as_str(), "a configuration field")),
            )
            .chain(
                connector
                    .graphs_of(service)
                    .map(|graph| (graph.name.as_str(), "a graph")),
            );

        for (name, kind) in members {
            if let Some((_, other)) = seen
                .iter()
                .find(|(seen_name, seen_kind)| *seen_name == name && *seen_kind != kind)
            {
                if !reported.contains(&name) {
                    let where_ = if service == DEFAULT_SERVICE {
                        String::new()
                    } else {
                        format!(" of service {service:?}")
                    };
                    problems.push(format!(
                        "{name:?} names both {other} and {kind}{where_}. The three member kinds \
                         share one namespace: all of them render into the same address \
                         (`…#{name}`) and into flux's declaration namespace, so a name has to \
                         denote exactly one thing"
                    ));
                    reported.push(name);
                }
            }
            seen.push((name, kind));
        }
    }
}

/// The `kind` word for a transport, for error text.
fn transport_word(transport: Transport) -> &'static str {
    match transport {
        Transport::Webhook => "webhook",
        Transport::Socket => "socket",
        Transport::Poll => "poll",
    }
}

/// The `scheme` word for a credential, for error text.
fn scheme_word(scheme: &AuthScheme) -> &'static str {
    match scheme {
        AuthScheme::Bearer => "bearer",
        AuthScheme::Basic => "basic",
        AuthScheme::Header { .. } => "header",
        AuthScheme::Query { .. } => "query",
        AuthScheme::Signing => "signing",
    }
}

/// Checks the connector's address components and its `[[services]]` declarations — C-49.
///
/// The operation-side half of the rule (every operation belongs to a declared service) is in
/// [`validate_operations`], because that is where an operation is already being read.
///
/// # Why the grammar is enforced *here*
///
/// The [`address`](crate::address) module owns the spelling of an authority, a service name and an
/// API version, and this is the only place that can refuse a bad one while the author is still
/// looking at the file. Two things go wrong if it does not:
///
/// 1. **A service name reaches the output filesystem path.** It names the emitted
///    `<provider>-<service>.flux`, and a build creates that file's parent directories. A name
///    carrying `/` or `..` would therefore let a *content* field of a provider TOML decide where a
///    build writes — including outside the repository root. Before services existed, no content field
///    could influence an output path at all: paths came from the discovered file stem. That invariant
///    is worth keeping, and keeping it costs one call to a validator that already exists.
/// 2. **An unspellable component publishes a malformed address.** [`Connector::gid_of`] renders
///    whatever the loader accepted, and that string reaches every service manifest and
///    `catalog.json`. An authority of `com.acme/s3` renders `com.acme/s3:v2`, which *reparses* — as a
///    different address. That is exactly the "a typo in a segment cannot masquerade as a valid
///    address" property the address module claims, and only validation here makes the claim true.
fn validate_services(connector: &Connector, problems: &mut Vec<String>) {
    if let Some(authority) = &connector.authority {
        if let Err(reason) = crate::address::validate_authority(authority) {
            problems.push(format!(
                "`authority` is not a valid reverse-DNS authority: {reason}. It is the leading \
                 component of every service address"
            ));
        }
    }
    if let Some(api_version) = &connector.api_version {
        if let Err(reason) = crate::address::validate_api_version(api_version) {
            problems.push(format!(
                "`api_version` cannot travel in an address: {reason}"
            ));
        }
    }

    let mut seen: Vec<&str> = Vec::new();

    for service in &connector.services {
        let name = service.name.as_str();
        if let Err(reason) = crate::address::validate_service_name(name) {
            problems.push(format!(
                "a `[[services]]` entry has an invalid `name`: {reason}"
            ));
            continue;
        }
        // The reserved name is the *implicit* service. A second definition of something that already
        // exists could disagree with it about a base URL or a version, with nothing to say which one
        // an operation meant — so the entry is admitted for exactly one purpose, and refused for
        // every other. See `validate_default_service_entry`.
        if name == DEFAULT_SERVICE {
            validate_default_service_entry(connector, service, problems);
        }
        if seen.contains(&name) {
            problems.push(format!(
                "service {name:?} is declared more than once; an operation naming it could not say \
                 which declaration it meant"
            ));
        }
        seen.push(name);

        if let Some(base_url) = &service.base_url {
            if base_url.trim().is_empty() {
                problems.push(format!(
                    "service {name:?} declares an empty `base_url`; omit it to inherit the \
                     connector's"
                ));
            }
        }
        if let Some(api_version) = &service.api_version {
            if let Err(reason) = crate::address::validate_api_version(api_version) {
                problems.push(format!(
                    "service {name:?} declares an `api_version` that cannot travel in an address: \
                     {reason}. Omit it to inherit the connector's"
                ));
            }
        }

        validate_service_roles(connector, service, problems);
    }
}

/// Checks the one `[[services]]` entry that may name the reserved [`DEFAULT_SERVICE`] — C-120.
///
/// C-49 refused the name outright, and the reason was sound: `default` is the service an operation
/// belongs to when it names none, so declaring it is a second definition of something that already
/// exists, and the two could disagree about a base URL or a version.
///
/// Roles are the one thing that argument does not cover, and only for **a provider with a single API
/// surface**, which has no other service to attach a role to. The exception is scoped to exactly that
/// case, along two axes:
///
/// 1. **What the entry may carry.** `roles` and nothing else. `roles` has no connector-level
///    spelling, so it has nothing to contradict, while `base_url`, `api_version` and `description`
///    all do.
/// 2. **Whether the provider has any other service.** A `default` entry beside a named one would
///    hand back the implicit `default` that a multi-service provider must not have — and the harm is
///    concrete, not doctrinal: [`validate_operation_service`] refuses an operation that omits
///    `service` in a multi-service file precisely so it is not emitted into a
///    `<provider>-default.flux` nobody declared. Declaring the entry would make that operation legal
///    again. So the entry is refused unless `default` is the only service there is.
///
/// It stays the *implicit* service when it is admitted: [`Connector::is_default_only`] remains true,
/// so a provider that writes the entry emits the same `<provider>.flux` it emitted before.
fn validate_default_service_entry(
    connector: &Connector,
    service: &Service,
    problems: &mut Vec<String>,
) {
    // Scoped by "no service other than `default` is declared" rather than by a count, so that a file
    // declaring `default` twice reports the duplicate once and does not also report this twice.
    if let Some(other) = connector
        .services
        .iter()
        .find(|other| other.name != DEFAULT_SERVICE)
    {
        problems.push(format!(
            "`[[services]]` declares {DEFAULT_SERVICE:?} beside the named service {:?}. \
             {DEFAULT_SERVICE:?} may be declared only by a provider whose *only* API surface it is, \
             and only to carry `roles` — which a single-surface provider has nowhere else to put. \
             A provider that declares named services has no implicit {DEFAULT_SERVICE:?} for an \
             operation to fall into, and declaring one here would hand it back: an operation that \
             omitted `service` would become legal and be emitted into a \
             `<provider>-{DEFAULT_SERVICE}.flux` nobody asked for. Declare the roles on the service \
             that actually has them",
            other.name
        ));
        return;
    }

    let mut overreaching: Vec<&str> = Vec::new();
    if !service.description.is_empty() {
        overreaching.push("description");
    }
    if service.base_url.is_some() {
        overreaching.push("base_url");
    }
    if service.api_version.is_some() {
        overreaching.push("api_version");
    }

    if !overreaching.is_empty() {
        problems.push(format!(
            "`[[services]]` declares {DEFAULT_SERVICE:?} with `{}`. {DEFAULT_SERVICE:?} is \
             reserved — it is the service an operation belongs to when it names none, and it is \
             elided from every published address — so the entry may carry `roles` and nothing else. \
             A role attaches to a service and a single-surface provider has nowhere else to put one; \
             everything else is already stated at connector level, and a second definition could \
             disagree with it",
            overreaching.join("`, `")
        ));
    } else if service.roles.is_empty() {
        problems.push(format!(
            "`[[services]]` declares {DEFAULT_SERVICE:?} and nothing else. {DEFAULT_SERVICE:?} is \
             reserved: it is the service an operation belongs to when it names none, and a provider \
             with one API surface declares no services at all. The one reason to write the entry is \
             to carry `roles`"
        ));
    }
}

/// Checks that every role a service claims is one it satisfies, and claimed once — C-120.
///
/// A role is a *contract*, and the checking is the whole value of declaring one: a consumer reading
/// the catalogue relies on `llm_catalogue` without reading the provider's TOML, so an unsatisfied
/// claim would make the catalogue lie. The unknown-name case is not here because `serde` refuses it
/// first, at the parse — [`Role`] is a closed enum, and serde's error already quotes the name that
/// was written and lists the ones that exist.
fn validate_service_roles(connector: &Connector, service: &Service, problems: &mut Vec<String>) {
    let name = service.name.as_str();
    let mut seen: Vec<Role> = Vec::new();

    for role in &service.roles {
        let word = role.word();
        if seen.contains(role) {
            problems.push(format!(
                "service {name:?} declares role {word:?} more than once. A role is a claim; stating \
                 it twice states nothing the first one did not, and a set that tolerates repeats is \
                 a list pretending to be a set"
            ));
            continue;
        }
        seen.push(*role);

        for missing in connector.missing_role_members(name, *role) {
            problems.push(format!(
                "service {name:?} claims role {word:?} but has no {missing:?} operation. A role \
                 names what it requires by the member's name *within the service* — the trailing \
                 segments, so that `openai-models-list` and `openrouter-models-list` fill one slot \
                 and the shape is the same whatever the vendor calls its endpoint. It must be an \
                 `[[operations]]` entry: a role is a claim that something is callable, and an event \
                 or a channel binding is emitted into no module, so filling the slot with one would \
                 publish a capability nothing can call. {word:?} requires: {}",
                role.required_members().join(", ")
            ));
        }
    }
}

/// Checks that an operation's service is one this provider has.
///
/// The set is the declared names, or exactly `default` when nothing is declared — so a
/// single-surface provider needs no `[[services]]` block, and a multi-service provider has no
/// implicit `default` for an operation to fall into. That second half is the important one: an
/// operation that omitted `service` in a multi-service file would otherwise be emitted into an
/// `<provider>-default.flux` nobody declared or asked for.
fn validate_operation_service(
    connector: &Connector,
    operation: &Operation,
    problems: &mut Vec<String>,
) {
    let available = connector.service_names();
    if available.contains(&operation.service.as_str()) {
        return;
    }
    let listed = available.join(", ");
    let id = operation.id.as_str();
    problems.push(if operation.service == DEFAULT_SERVICE {
        format!(
            "operation {id:?} names no `service`, which means the reserved {DEFAULT_SERVICE:?} \
             service — but this provider declares named services and no `[[services]]` entry \
             declares {DEFAULT_SERVICE:?}. Every operation of a multi-service provider names one of: \
             {listed}"
        )
    } else {
        format!(
            "operation {id:?} names service {:?}, which no `[[services]]` entry declares. This \
             provider declares: {listed}",
            operation.service
        )
    });
}

/// Checks the connector's own credential declarations.
fn validate_credentials(connector: &Connector, problems: &mut Vec<String>) {
    let mut seen: Vec<&str> = Vec::new();

    for method in &connector.auth {
        let name = method.name.as_str();
        if name.trim().is_empty() {
            problems.push("a credential has an empty `name`".to_owned());
            continue;
        }
        if seen.contains(&name) {
            problems.push(format!(
                "credential {name:?} is declared more than once; a requirement naming it could not \
                 say which declaration it meant"
            ));
        }
        seen.push(name);

        // A Basic credential's user half is not optional: without it the host has nothing to put
        // before the colon, and `base64(":secret")` authenticates as nobody.
        if method.scheme == AuthScheme::Basic && method.user_env.is_empty() {
            problems.push(format!(
                "credential {name:?} uses the `basic` scheme but declares no `user_env`. Basic \
                 sends `base64(<user>:<secret>)`, and the user half comes from `user_env`; for \
                 zendesk that is `user_env = [\"ZENDESK_USER\"]` with `user_suffix = \"/token\"`"
            ));
        }
        if method.scheme != AuthScheme::Basic && !method.user_env.is_empty() {
            problems.push(format!(
                "credential {name:?} declares `user_env`, which only the `basic` scheme uses"
            ));
        }
        if method.scheme != AuthScheme::Basic && method.user_suffix.is_some() {
            problems.push(format!(
                "credential {name:?} declares `user_suffix`, which only the `basic` scheme uses"
            ));
        }

        // A credential resolved from no env var and minted by no grant can never produce a value.
        if method.env.is_empty() && method.oauth2.is_none() {
            problems.push(format!(
                "credential {name:?} names no `env` keys, so nothing can resolve it to a value"
            ));
        }
        for key in method.env.iter().chain(&method.user_env) {
            if key.trim().is_empty() {
                problems.push(format!("credential {name:?} lists an empty env-var key"));
            }
        }
    }

    validate_requirements(
        connector,
        &connector.default_auth,
        "`default_auth`",
        problems,
    );
}

/// Header names the `$auth` seam owns. A constant header may not spell one whatever its value is.
///
/// `authorization` and `proxy-authorization` are where a credential goes; `cookie` is a session, which
/// is the same thing arriving by another route. Any of the three declared as a literal would be a
/// credential written into a committed artifact — see [`validate_const_headers`].
const AUTH_OWNED_HEADERS: &[&str] = &["authorization", "proxy-authorization", "cookie"];

/// Value prefixes that spell a credential rather than a constant, whatever header carries them.
const CREDENTIAL_VALUE_PREFIXES: &[&str] = &["bearer ", "basic ", "token ", "apikey ", "digest "];

/// Spellings that say "resolve this from somewhere else". None of them resolves: a constant header is
/// a literal, emitted verbatim, so a value in one of these shapes reaches the vendor as its own text.
const RESOLUTION_MARKERS: &[&str] = &["${", "{{", "$secret", "$auth", "env:", "secret:"];

/// Checks every constant request header — the vendor-fixed `Accept`, `Notion-Version`, `User-Agent`
/// (C-55).
///
/// **The rule that earns its keep is the credential one.** Every other field in this file that could
/// hold a secret is a *reference* — a credential name, an env-var key — resolved by the host at
/// request time and never written down. This one is a literal that reaches generated Flux, the
/// capability manifest and the public catalogue verbatim, so an author who reached for it to send
/// `Authorization: Bearer sk-…` would be committing the token to the repository, and the pipeline
/// would carry it all the way to a published artifact without a word. That is precisely the failure
/// `AGENTS.md` forbids ("no credential value enters provider TOML, generated Flux, a manifest, the
/// public catalogue, or the lockfile"), and the refusals below are what keep the field from becoming
/// a second, ungated path to the `$auth` seam C-10 owns.
///
/// The provider-level table is checked once and the operations' own entries after it, so a header
/// declared once for the whole provider is reported once.
fn validate_const_headers(
    connector: &Connector,
    provider_headers: &BTreeMap<String, String>,
    problems: &mut Vec<String>,
) {
    check_const_header_table(connector, provider_headers, "`[const_headers]`", problems);

    for operation in &connector.operations {
        // Entries the provider contributed are already reported above, spelling and value alike.
        let own: BTreeMap<String, String> = operation
            .params
            .const_headers
            .iter()
            .filter(|(name, value)| provider_headers.get(*name) != Some(*value))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        check_const_header_table(
            connector,
            &own,
            &format!("operation {:?}: `const_headers`", operation.id),
            problems,
        );
    }
}

/// One table of constant headers, provider-level or operation-level.
fn check_const_header_table(
    connector: &Connector,
    headers: &BTreeMap<String, String>,
    context: &str,
    problems: &mut Vec<String>,
) {
    let mut seen: Vec<String> = Vec::new();

    for (name, value) in headers {
        // A map is keyed by exact spelling and HTTP field names are case-insensitive, so two
        // spellings of one header would reach the request record as two entries and be sent twice.
        let folded = name.to_ascii_lowercase();
        if seen.contains(&folded) {
            problems.push(format!(
                "{context}: header {name:?} is declared twice under two spellings. HTTP field names \
                 are case-insensitive (RFC 9110 §5.1), so both would travel as one header sent twice"
            ));
        }
        seen.push(folded.clone());

        if !is_http_field_name(name) {
            problems.push(format!(
                "{context}: header name {name:?} is not an HTTP field name — only ASCII token \
                 characters are allowed (RFC 9110 §5.1), and a request carrying it could never be \
                 built"
            ));
        }
        // Emitted verbatim into a header record, so a CR or LF would append a header of the
        // author's choosing to every request — and a non-ASCII byte is not a field value at all.
        if let Some(bad) = value
            .chars()
            .find(|c| !c.is_ascii() || (c.is_ascii_control() && *c != '\t'))
        {
            problems.push(format!(
                "{context}: header {name:?} has a value carrying {bad:?}, which is not an HTTP \
                 field value (RFC 9110 §5.5). A newline in particular would append a header of its \
                 own to every request"
            ));
        }
        if value.trim().is_empty() {
            problems.push(format!(
                "{context}: header {name:?} has an empty value. A header that says nothing is a \
                 header the vendor did not ask for — remove it, or state what it sends"
            ));
        }

        if folded == "content-type" {
            problems.push(format!(
                "{context}: `content-type` is the emitter's, not a provider's. It is derived from \
                 the request body — `application/json` for every body this pipeline builds — so \
                 declaring it here would describe an encoding the emitted module does not produce"
            ));
        }
        if AUTH_OWNED_HEADERS.contains(&folded.as_str()) {
            problems.push(format!(
                "{context}: header {name:?} carries a credential, and a constant header is a \
                 literal in a committed artifact. Credentials are declared in `[[auth]]` and \
                 injected by the host at the `$auth` seam, which is what keeps the value out of the \
                 generated module, the manifest and the public catalogue"
            ));
        }
        for method in &connector.auth {
            if let AuthScheme::Header { name: owned } = &method.scheme {
                if owned.eq_ignore_ascii_case(name) {
                    problems.push(format!(
                        "{context}: header {name:?} is where credential {:?} is injected, so a \
                         constant would either be overwritten by the host or overwrite the \
                         credential. Declare the header on one side only",
                        method.name
                    ));
                }
            }
        }

        credential_shaped_value(connector, name, value, context, problems);
    }
}

/// Whether a constant header's *value* is a credential, or something the author expects to resolve
/// into one.
///
/// Nothing here resolves. A constant header is emitted as a literal, so `${GITHUB_TOKEN}` reaches
/// GitHub as those fourteen characters — the benign reading is a broken request, and the dangerous
/// one is an author who pastes the real value in once the placeholder does not work. Both are
/// refused at the declaration.
fn credential_shaped_value(
    connector: &Connector,
    name: &str,
    value: &str,
    context: &str,
    problems: &mut Vec<String>,
) {
    let folded = value.to_ascii_lowercase();

    if let Some(marker) = RESOLUTION_MARKERS
        .iter()
        .find(|marker| folded.contains(*marker))
    {
        problems.push(format!(
            "{context}: header {name:?} has a value spelling {marker:?}, but a constant header is a \
             literal and nothing interpolates it — the vendor would receive those characters. A \
             value that has to be resolved is a credential or configuration: declare it in \
             `[[auth]]` or `[[config]]`"
        ));
    }
    if let Some(prefix) = CREDENTIAL_VALUE_PREFIXES
        .iter()
        .find(|prefix| folded.starts_with(*prefix))
    {
        problems.push(format!(
            "{context}: header {name:?} has a value beginning {prefix:?}, which is a credential. It \
             would be committed to this repository verbatim and published in the catalogue. \
             Credentials are declared in `[[auth]]` and injected by the host"
        ));
    }
    for method in &connector.auth {
        if value.contains(&method.name) {
            problems.push(format!(
                "{context}: header {name:?} has a value naming credential {:?}. A constant header is \
                 a literal, not a reference — nothing resolves the name, and the value that would \
                 make it work is one this file must never hold",
                method.name
            ));
        }
        for key in method.env.iter().chain(&method.user_env) {
            if !key.trim().is_empty() && value.contains(key.as_str()) {
                problems.push(format!(
                    "{context}: header {name:?} has a value naming the environment variable \
                     {key:?}, which resolves credential {:?}. A constant header is emitted as a \
                     literal, so the name would travel as text and the value it stands for must \
                     never be written here at all",
                    method.name
                ));
            }
        }
    }
}

/// Whether `name` is a valid HTTP field name (RFC 9110 §5.1 `token`).
///
/// The emitter checks the same grammar on the way out (`connector-flux`'s `is_http_token`), for the
/// same reason the member-name rules are split: this guards what an author may *declare*, and the
/// emitter guards what may reach `http.request`.
fn is_http_field_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// Checks every operation, and the auth it names.
fn validate_operations(connector: &Connector, problems: &mut Vec<String>) {
    let mut seen: Vec<&str> = Vec::new();

    for operation in &connector.operations {
        let id = operation.id.as_str();
        if id.trim().is_empty() {
            problems.push("an operation has an empty `id`".to_owned());
        } else if seen.contains(&id) {
            problems.push(format!(
                "operation {id:?} is declared more than once; the op id is the public name callers \
                 and models use, so it must be unique"
            ));
        }
        seen.push(id);

        validate_operation_service(connector, operation, problems);

        if operation.path.trim().is_empty() {
            problems.push(format!("operation {id:?} has an empty `path`"));
        } else if !operation.path.starts_with('/') {
            problems.push(format!(
                "operation {id:?} has path {:?}, which must start with `/` — it is joined onto the \
                 connector's `base_url`",
                operation.path
            ));
        }

        for param in operation.params.iter() {
            if param.name.trim().is_empty() {
                problems.push(format!(
                    "operation {id:?} has a parameter with an empty `name`"
                ));
            }
        }

        // Two answers to one question, refused rather than merged. "The body is these named fields"
        // and "the body *is* this schema" cannot both hold, and nothing states how they would
        // combine — so an operation declaring both has no derivable request body and no derivable
        // `input_schema` (C-125). `connector-flux` refuses it again at emission, which is the
        // narrower gate: this one also covers a definition nobody has emitted yet.
        if operation.params.body_schema.is_some() && !operation.params.body.is_empty() {
            problems.push(format!(
                "operation {id:?} declares both named `params.body` fields and a free-form \
                 `params.body_schema`. Those are two answers to one question — what the request \
                 body is — and there is no rule for merging them, so declare either the fields or \
                 the schema"
            ));
        }
        for param in &operation.params.path {
            // The placeholder is written in the vendor's spelling, so a parameter that declares a
            // `wire` alias is looked up under that — matching on the caller-facing name would
            // reject `{requester_id}` for a parameter a caller knows as `req_id`.
            let wire = param.wire.as_deref().unwrap_or(&param.name);
            let placeholder = format!("{{{wire}}}");
            if !operation.path.contains(&placeholder) {
                problems.push(format!(
                    "operation {id:?} declares path parameter {:?}, but its path {:?} has no \
                     `{placeholder}` to interpolate it into",
                    param.name, operation.path
                ));
            }
        }

        if let Some(alternatives) = &operation.auth {
            validate_requirements(
                connector,
                alternatives,
                &format!("operation {id:?}"),
                problems,
            );
        }
    }
}

/// Checks one alternatives list — the OR of mechanisms an operation or the connector default names.
///
/// Two rules, and the first is the one that keeps the encoding unambiguous.
fn validate_requirements(
    connector: &Connector,
    alternatives: &[AuthRequirement],
    context: &str,
    problems: &mut Vec<String>,
) {
    for (index, mechanism) in alternatives.iter().enumerate() {
        // An empty mechanism inside a non-empty list is a *second spelling of "no auth"*, and the
        // IR already has one: an empty alternatives list. Two encodings of one meaning is how
        // ambiguity gets baked in — and here the two would not even be equivalent downstream, since
        // C-10 picks "the first satisfiable mechanism" and an empty mechanism is trivially
        // satisfiable, so it would silently disable auth for the whole operation.
        if mechanism.is_empty() {
            problems.push(format!(
                "{context}: auth mechanism {index} names no credentials. \"No auth\" is written as \
                 an empty alternatives list (`auth = []`), never as a list holding an empty \
                 mechanism — an empty mechanism is always satisfiable, so it would disable auth for \
                 every alternative beside it"
            ));
            continue;
        }
        for credential in mechanism {
            match connector.auth_method(credential) {
                None => problems.push(format!(
                    "{context}: auth mechanism {index} names credential {credential:?}, which no \
                     `[[auth]]` block declares"
                )),
                // The complement of the rule in `validate_hmac`: a signing secret has no placement
                // on an outgoing request, so an operation naming one is asking the host to inject a
                // value that has nowhere to go — and to spend an inbound secret outbound.
                Some(method) if method.scheme == AuthScheme::Signing => problems.push(format!(
                    "{context}: auth mechanism {index} names credential {credential:?}, which is \
                     declared `scheme = \"signing\"`. A signing secret verifies an inbound request \
                     and is never placed in an outgoing one, so no operation can authenticate with it"
                )),
                Some(_) => {}
            }
        }
    }
}

/// Checks the patch set the overlay (C-6) will consume.
fn validate_patch(loaded: &LoadedProvider, problems: &mut Vec<String>) {
    let mut selected: Vec<&str> = Vec::new();
    let mut renamed: Vec<&str> = Vec::new();

    for patch in &loaded.patch.operations {
        let select = patch.select.as_str();
        if select.trim().is_empty() {
            problems.push(
                "a `[[patch.operations]]` entry has an empty `select`; it names the spec's \
                 `operationId`"
                    .to_owned(),
            );
        } else if selected.contains(&select) {
            problems.push(format!(
                "`[[patch.operations]]` selects {select:?} more than once"
            ));
        }
        selected.push(select);

        if let Some(rename) = &patch.rename {
            if rename.trim().is_empty() {
                problems.push(format!("patch for {select:?} has an empty `rename`"));
            } else if renamed.contains(&rename.as_str()) {
                problems.push(format!(
                    "`[[patch.operations]]` renames two operations to {rename:?}; the op id is a \
                     public name and must be unique"
                ));
            } else if loaded.connector.operation(rename).is_some() {
                problems.push(format!(
                    "patch for {select:?} renames to {rename:?}, which an inline `[[operations]]` \
                     block already declares"
                ));
            }
            renamed.push(rename);
        }

        if let Some(alternatives) = &patch.auth {
            validate_requirements(
                &loaded.connector,
                alternatives,
                &format!("patch for {select:?}"),
                problems,
            );
        }

        for param in &patch.params {
            if param.name.trim().is_empty() {
                problems.push(format!(
                    "patch for {select:?} has a parameter correction with an empty `name`"
                ));
            }
        }
    }
}

/// The keys the loader actually accepts, per documented object, **as serde reports them**.
///
/// This is the machinery behind "a JSON Schema kept in sync by a test". Hand-written schemas rot;
/// the only cure is to ask the code rather than the author. Each entry is produced by handing the
/// type a key it cannot possibly know and reading the field list out of `deny_unknown_fields`'
/// own error — so the answer is derived from the `Deserialize` impl that will parse real provider
/// files, not from a second list that could disagree with it.
///
/// `tests/provider_schema.rs` asserts that this map and the schema's `$defs` describe the same
/// objects with the same properties. Adding a field to any IR type therefore fails that test until
/// the schema documents it.
///
/// The object names are the schema's `$defs` keys, not Rust type names — the schema is the
/// published artifact, so it gets to choose the vocabulary.
pub fn accepted_keys() -> Vec<(&'static str, Vec<String>)> {
    vec![
        ("provider", probe::<ProviderFile>()),
        ("service", probe::<Service>()),
        ("spec", probe::<SpecSource>()),
        ("patch", probe::<Patch>()),
        ("operationPatch", probe::<OperationPatch>()),
        ("paramPatch", probe::<ParamPatch>()),
        ("authMethod", probe::<AuthMethod>()),
        ("oauth2", probe::<crate::OAuth2Spec>()),
        ("oauthRedirect", probe::<crate::OAuthRedirect>()),
        ("authRequirement", probe::<AuthRequirement>()),
        ("operation", probe::<Operation>()),
        ("event", probe::<EventDecl>()),
        ("channel", probe::<ChannelBinding>()),
        ("configField", probe::<ConfigField>()),
        ("graph", probe::<Graph>()),
        ("graphNode", probe::<GraphNode>()),
        ("port", probe::<crate::graph::Port>()),
        ("portRef", probe::<PortRef>()),
        ("edge", probe::<crate::graph::Edge>()),
        ("condition", probe::<crate::graph::Condition>()),
        ("subscription", probe::<Subscription>()),
        ("manualSetup", probe::<ManualSetup>()),
        ("hmac", probe::<HmacSpec>()),
        ("selector", probe::<Selector>()),
        ("reply", probe::<Reply>()),
        ("paramSet", probe::<ParamSet>()),
        ("param", probe::<Param>()),
        ("quirks", probe::<Quirks>()),
        ("rateLimit", probe::<crate::RateLimit>()),
        ("errorEnvelope", probe::<crate::ErrorEnvelope>()),
        ("provenance", probe::<Provenance>()),
    ]
}

/// A key no provider TOML will ever contain, used to make `deny_unknown_fields` name its alternatives.
const UNKNOWN_KEY_PROBE: &str = "__connector_spec_unknown_key_probe__";

/// Asks `T` which keys it accepts, by feeding it one it does not.
///
/// Panics if `T` accepts the probe key or reports no alternatives — either would mean the type is
/// not `deny_unknown_fields`, which is the very property this crate's strictness rests on, so a
/// panic in a test helper is the right loudness.
fn probe<T: serde::de::DeserializeOwned>() -> Vec<String> {
    let document = format!("{{\"{UNKNOWN_KEY_PROBE}\": null}}");
    let error = serde_json::from_str::<T>(&document)
        .err()
        .unwrap_or_else(|| {
            panic!(
                "{} accepted an unknown key — it is missing `deny_unknown_fields`",
                std::any::type_name::<T>()
            )
        })
        .to_string();

    let keys = expected_fields(&error);
    assert!(
        !keys.is_empty(),
        "could not read the accepted keys of {} out of: {error}",
        std::any::type_name::<T>()
    );
    keys
}

/// Extracts the backtick-quoted field names serde lists after "expected one of" (or "expected", for
/// a single-field struct).
fn expected_fields(error: &str) -> Vec<String> {
    let Some(offset) = error.find("expected") else {
        return Vec::new();
    };
    error[offset..]
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}
