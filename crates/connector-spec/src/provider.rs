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

use serde::{Deserialize, Serialize};

use crate::lock::sha256_hex;
use crate::{
    AuthMethod, AuthRequirement, AuthScheme, Connector, Idempotency, JsonSchema, Operation, Param,
    ParamSet, Provenance, Quirks, Risk, Service, DEFAULT_SERVICE,
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
    operations: Vec<Operation>,
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
    let file: ProviderFile =
        toml::from_str(source).map_err(|source| crate::Error::ParseProvider {
            name: name.to_owned(),
            source: Box::new(source),
        })?;

    let loaded = assemble(file, source);

    let problems = validate(&loaded);
    if !problems.is_empty() {
        return Err(crate::Error::InvalidProvider {
            name: name.to_owned(),
            problems,
        });
    }

    Ok(loaded)
}

/// Turns the parsed file into a [`LoadedProvider`], folding `[spec]` into the connector's
/// provenance. No validation happens here — assembling and judging are separate so that validation
/// can see the finished value and report on all of it at once.
fn assemble(file: ProviderFile, source: &str) -> LoadedProvider {
    let spec = file.spec;
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
            operations: file.operations,
            provenance,
        },
        spec,
        patch: file.patch,
    }
}

/// Everything wrong with the file, in the order an author would read it: the connector itself, then
/// its credentials, then its operations, then the patch set.
///
/// Returning a `Vec` rather than short-circuiting is deliberate — see the module docs.
fn validate(loaded: &LoadedProvider) -> Vec<String> {
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
    validate_operations(connector, &mut problems);
    validate_patch(loaded, &mut problems);

    problems
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
        // The reserved name is the *implicit* service. Declaring it would be a second definition of
        // something that already exists, and the two could then disagree about a base URL or a
        // version — with nothing to say which one an operation meant.
        if name == DEFAULT_SERVICE {
            problems.push(format!(
                "`[[services]]` declares {DEFAULT_SERVICE:?}, which is reserved: it is the service an \
                 operation belongs to when it names none, and it is elided from every published \
                 address. A provider with one API surface declares no services at all"
            ));
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
            if connector.auth_method(credential).is_none() {
                problems.push(format!(
                    "{context}: auth mechanism {index} names credential {credential:?}, which no \
                     `[[auth]]` block declares"
                ));
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
