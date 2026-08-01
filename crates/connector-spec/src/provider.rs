//! The provider-TOML front-end: `providers/<name>.toml` in, [`Connector`] out.
//!
//! The file plays **two roles**, and the loader has to serve both from one schema:
//!
//! 1. **Hand-authored** — the whole connector is written out inline, with no vendor spec anywhere.
//!    Ollama, Freshdesk and (for now) Zendesk are in this position: there is no usable OpenAPI
//!    document to ingest. This is the role that matters most today, because it is the shortest route
//!    to an executable `.flux` module.
//! 2. **Spec pointer** — the file names a vendored spec under `specs/` and carries a *patch set*
//!    that selects and corrects operations from it. [`load_with_spec`] is that path: ingest (C-4)
//!    turns the document into every operation the vendor declares, and the patch set says which of
//!    them this connector publishes and what it corrects about each. **Selection is opt-in**, so a
//!    pointer with no patch is a connector with no operations. Widening what one statement can
//!    select — a path-prefix selector, a naming rule, risk stated for a whole set — is C-411, C-412
//!    and C-414, and none of it changes that.
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

use crate::config::{parse_binding, template_variables, Binding, ConfigField, Position};
use crate::graph::{Graph, GraphNode, NodeKind, PortRef};
use crate::inbound::{
    parse_tolerance, signed_placeholders, validate_path, validate_symbol, ChannelBinding,
    EventDecl, FieldSource, HmacSpec, ManualSetup, Reply, Selector, Subscription, Transport,
    VerificationScheme, SIGNED_PLACEHOLDERS,
};
use crate::lock::sha256_hex;
use crate::{
    AuthMethod, AuthRequirement, AuthScheme, Connector, HttpMethod, Idempotency, JsonSchema,
    Operation, Param, ParamSet, Provenance, Quirks, Risk, Role, Runtime, Service, DEFAULT_SERVICE,
    MIN_REPEATABILITY_CONDITION,
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
/// The [`connector`](Self::connector) is complete and ready for codegen either way. A
/// hand-authored file describes it inline; a spec-backed one loaded through [`load_with_spec`] has
/// had ingest fill it in from the vendored document. Loaded through plain [`load`], a spec-backed
/// file is still the *skeleton* it always was — id, base URL, credentials, provenance, plus any
/// operations written inline — because no document was supplied to ingest.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedProvider {
    /// The connector this file describes.
    pub connector: Connector,
    /// The vendor spec the file points at, if any. `None` for a fully hand-authored connector.
    pub spec: Option<SpecSource>,
    /// The patch set applied over the ingested spec. Empty for a hand-authored connector.
    pub patch: Patch,
    /// What the vendored document said, when one was supplied to [`load_with_spec`] — C-4.
    ///
    /// The **whole** ingest, not just the part that was published: it carries every operation the
    /// document declares, including the ones no patch selected, plus the servers it names and every
    /// [`Diagnostic`](crate::openapi::Diagnostic) the document earned. That is what makes "ingest
    /// makes everything *available* to patch" inspectable rather than merely claimed — and it is
    /// what a future `flux-connectors check` reads to tell an author which operations they could
    /// have selected.
    ///
    /// `None` for a hand-authored connector, and also for a spec-backed one loaded through plain
    /// [`load`], which is given no document to ingest.
    pub ingested: Option<crate::openapi::Ingested>,
}

impl LoadedProvider {
    /// Whether this file is a complete hand-authored definition — no spec, so nothing to ingest and
    /// nothing to overlay.
    pub fn is_hand_authored(&self) -> bool {
        self.spec.is_none()
    }

    /// Everything wrong with the vendored document that did not stop the ingest.
    ///
    /// Empty for a hand-authored connector. A real vendor document is never fully well-formed, so
    /// this being non-empty is the normal case, not a failure — see [`crate::openapi`].
    pub fn diagnostics(&self) -> &[crate::openapi::Diagnostic] {
        self.ingested
            .as_ref()
            .map(|ingested| ingested.diagnostics.as_slice())
            .unwrap_or_default()
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
    /// **How the connector executes** — C-405. Absent means [`Runtime::Http`], and an unrecognised
    /// word is refused *here*, by `serde`, exactly as an unknown [`Role`](crate::Role) is: the enum
    /// is closed, so the error quotes what was written and lists every runtime that exists. There is
    /// no arm in [`validate`] for it, and there must not be one — a runtime that fell back to `http`
    /// on a typo is how a `process` connector ends up served by a multi-tenant host.
    #[serde(default)]
    runtime: Runtime,
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
    load_inner(name, source, None)
}

/// One vendored document available to a provider, as the spec cache holds it.
///
/// The [`path`](Self::path) is what makes this a document rather than a pile of bytes: `[spec] path`
/// names exactly one file, and the loader resolves the pin against these rather than trusting a
/// caller to have picked the right one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecDocument<'a> {
    /// The repository-relative path, spelled exactly as `[spec] path` spells it —
    /// `specs/babelforce/manager-2026-07-10.yaml`.
    pub path: &'a str,
    /// The document's bytes as text.
    pub document: &'a str,
}

/// The same, with the vendored documents the provider's spec cache holds — C-4.
///
/// This is the whole spec front-end in one call: **spec -> patch -> validate**, in that fixed order.
/// [`openapi::ingest`] turns the document into every operation it declares, the file's
/// `[[patch.operations]]` says which of them this connector publishes and what it corrects about
/// each, and the result is validated by exactly the same pass a hand-authored file goes through — so
/// a selected operation is held to every rule an inline one is.
///
/// # The pin decides which document is read, and it is resolved here
///
/// `documents` is the **cache**, not a choice already made: every file under `specs/<provider>/`,
/// and this function picks the one `[spec] path` names. That is deliberate and it is load-bearing.
/// The cache ordinarily holds more than one file — `specs/zendesk/2024-06-01.json` beside a later
/// `2025-01-01.json` is what versioning a vendored document *looks* like — so a caller that picked
/// one and passed it alone would be deciding, silently, something only the provider file may decide.
/// A build that compiled `getUser` out of a document the file never named would emit plausible,
/// wrong Flux and exit 0.
///
/// A pin naming a file the cache does not hold is refused, listing what is there.
///
/// # The declared `sha256` is checked against the bytes, not copied past them
///
/// [`SpecSource::sha256`] reaches [`Provenance::spec_sha256`] and from there `connectors.lock`. If
/// nothing compared it against the document actually ingested, provenance would be a claim the file
/// makes about itself — and the lockfile would record a hash for bytes it never saw. So a declared
/// hash that disagrees with the document is a refusal here. (Comparing against *upstream* is
/// different and is C-14's; this is the local claim against the local bytes.)
///
/// # The file decides whether any document is read at all
///
/// A provider with no `[spec]` block ignores the cache entirely. `specs/<provider>/` holding a file
/// is not a declaration; `[spec] path` is.
///
/// # Ingest selects nothing
///
/// A file that points at a 398-operation document and names none of them loads to a connector with
/// **no operations**. That is not a degenerate case to be worked around, it is the property that
/// keeps a vendor catalogue from becoming 398 LLM tools by default — see [`Patch`].
///
/// # Errors
///
/// The two [`load`] returns, plus an [`Error::InvalidProvider`](crate::Error::InvalidProvider)
/// naming the spec path when the pin resolves to nothing, when the declared hash disagrees with the
/// bytes, when the document is not an OpenAPI 3.x document at all, or when a patch selects an
/// operation the document does not declare. A document's *narrower* problems — one endpoint with an
/// unresolvable `$ref`, one parameter with no schema — are not errors: they arrive as
/// [`LoadedProvider::diagnostics`].
pub fn load_with_spec(
    name: &str,
    source: &str,
    documents: &[SpecDocument<'_>],
) -> crate::Result<LoadedProvider> {
    load_inner(name, source, Some(documents))
}

fn load_inner(
    name: &str,
    source: &str,
    documents: Option<&[SpecDocument<'_>]>,
) -> crate::Result<LoadedProvider> {
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
    let mut loaded = assemble(file, source);

    // The ids the file writes out **inline**, captured before selection appends to them. C-6's
    // `validate_patch` asks whether a `rename` collides with one, and after selection every rename
    // is trivially present — so the question has to be asked of the set that existed first.
    let inline: Vec<String> = loaded
        .connector
        .operations
        .iter()
        .map(|operation| operation.id.clone())
        .collect();

    // **spec -> patch -> validate**, in that order, so a selected operation is validated by exactly
    // the pass a hand-authored one is rather than by a second, weaker one.
    let mut problems = Vec::new();
    if loaded.spec.is_some() {
        if let Some(documents) = documents {
            ingest_spec(&mut loaded, documents, &mut problems);
            // Re-run, because selection appended operations after `assemble` distributed. The pass
            // only fills a header an operation does not already carry, so a second run over the
            // inline ones changes nothing.
            distribute_const_headers(&provider_headers, &mut loaded.connector.operations);
        }
    }

    problems.extend(validate(&loaded, &provider_headers, &inline));
    if !problems.is_empty() {
        return Err(crate::Error::InvalidProvider {
            name: name.to_owned(),
            problems,
        });
    }

    Ok(loaded)
}

/// Ingest the vendored document and publish the operations the patch set selects.
///
/// Everything here is a *statement the author made*: which operations to publish, what to call each
/// one, how risky it is. Nothing is inferred from the document, because the three fields an
/// `Operation` needs that a specification never carries — the op id, [`Risk`] and [`Idempotency`] —
/// are the three this repository refuses to decide by silence.
fn ingest_spec(
    loaded: &mut LoadedProvider,
    documents: &[SpecDocument<'_>],
    problems: &mut Vec<String>,
) {
    let Some(spec) = loaded.spec.clone() else {
        return;
    };
    let path = spec.path.clone();

    // **The pin, resolved.** `specs/<provider>/` ordinarily holds several files — versions of one
    // document — and only `[spec] path` says which of them this connector is compiled from. Reading
    // whichever happened to sort last would compile an operation out of a document the provider file
    // never named, successfully and silently.
    let Some(found) = documents
        .iter()
        .find(|candidate| candidate.path == path.trim())
    else {
        problems.push(format!(
            "`[spec] path = {path:?}` names no vendored document. {}",
            describe_cache(documents)
        ));
        return;
    };
    let document = found.document;

    // **Provenance is checked, not copied.** `sha256` travels from here into `connectors.lock`; a
    // value nothing compared against the ingested bytes would be the file's claim about itself,
    // recorded as though it were a measurement. Checking upstream drift is a different question and
    // is C-14's — this is the local claim against the local bytes.
    if let Some(declared) = spec
        .sha256
        .as_deref()
        .map(str::trim)
        .filter(|hash| !hash.is_empty())
    {
        let measured = sha256_hex(document.as_bytes());
        if !declared.eq_ignore_ascii_case(&measured) {
            problems.push(format!(
                "`[spec] sha256` declares {declared:?}, but {path} hashes to {measured:?}. The \
                 declared value reaches `connectors.lock`, so a build that ignored the difference \
                 would record a hash for bytes it never read — re-vendor the document or correct \
                 the declaration"
            ));
            return;
        }
    }

    let ingested = match crate::openapi::ingest(document) {
        Ok(ingested) => ingested,
        Err(error) => {
            problems.push(format!("`[spec] path = {path:?}`: {error}"));
            return;
        }
    };

    let mut selected = Vec::new();
    for patch in &loaded.patch.operations {
        if let Some(operation) = select(&ingested, patch, &path, problems) {
            selected.push(operation);
        }
    }
    loaded.connector.operations.extend(selected);
    loaded.ingested = Some(ingested);
}

/// One `[[patch.operations]]` block against the ingested document, or a problem saying why not.
///
/// Returns `None` on every failure rather than short-circuiting the caller, so a file with five bad
/// patches reports five lines — the same "every problem at once" contract the rest of this loader
/// keeps.
fn select(
    ingested: &crate::openapi::Ingested,
    patch: &OperationPatch,
    path: &str,
    problems: &mut Vec<String>,
) -> Option<Operation> {
    let select = patch.select.as_str();
    let Some(spec) = ingested.operation(select) else {
        // Loud rather than a silent no-op, because a `select` that quietly matches nothing is how a
        // patch set rots underneath a vendor's rename: the operation disappears from the connector
        // and the build stays green.
        problems.push(format!(
            "`[[patch.operations]] select = {select:?}` names no `operationId` in {path}. {}",
            nearest(ingested, select)
        ));
        return None;
    };

    // `rename` is required, and this is the one place the requirement is stated. An op id is a
    // public contract users and models call by name, and `operationId` is a volatile vendor field —
    // promoting one to the other silently is exactly what `docs/designs/connector-pipeline.md`'s "Op
    // naming is a public contract" refuses. C-412 replaces the per-operation `rename` with a naming
    // rule declared once; until it lands, an author states each one.
    let Some(id) = patch.rename.clone() else {
        problems.push(format!(
            "patch for {select:?} states no `rename`. An op id is a public name that users and \
             models call, and `operationId` is a volatile vendor field, so ingest will not promote \
             one into one — state `rename`"
        ));
        return None;
    };
    let (Some(risk), Some(idempotency)) = (patch.risk, patch.idempotency) else {
        problems.push(format!(
            "patch for {select:?} states no {}. No OpenAPI document publishes either, so a \
             selected operation states both or is not published; guessing on the operation's \
             behalf is how a `retry` turns one charge into three and how a delete is waved through \
             an approval gate",
            match (patch.risk, patch.idempotency) {
                (None, Some(_)) => "`risk`",
                (Some(_), None) => "`idempotency`",
                _ => "`risk` and no `idempotency`",
            }
        ));
        return None;
    };

    let mut params = spec.params.clone();
    for correction in &patch.params {
        correct(&mut params, correction, select, problems);
    }

    Some(Operation {
        id,
        service: crate::DEFAULT_SERVICE.to_owned(),
        method: spec.method,
        path: spec.path.clone(),
        description: patch
            .description
            .clone()
            .unwrap_or_else(|| spec.description.clone()),
        risk,
        idempotency,
        repeatable_because: None,
        auth: patch.auth.clone(),
        params,
        response_schema: spec.response_schema.clone(),
        quirks: patch.quirks.clone().unwrap_or_default(),
        // Resolved at integration: C-4 built this literal and C-413 added the field, and neither
        // branch could see the other. `exposed()` rather than a bare `true` so an ingested operation
        // takes the same default a hand-authored one does from serde, in one place — and so a spec
        // route that silently diverged from the file route would fail here rather than in a
        // catalogue nobody re-reads. Declaring exposure *per selector* is C-411's, not ingest's:
        // `OperationPatch` carries no `expose` key today, so silence here means the connector-wide
        // default and never a decision ingest made on an author's behalf.
        expose: crate::ir::exposed(),
    })
}

/// Apply one [`ParamPatch`] to a selected operation's parameters.
///
/// A correction that matches nothing is a problem, not a no-op: it is the same rot a `select`
/// naming an absent operation is, one level down — the vendor renamed a field and the correction
/// that used to fix its type silently stopped applying.
fn correct(
    params: &mut ParamSet,
    correction: &ParamPatch,
    select: &str,
    problems: &mut Vec<String>,
) {
    let group = match correction.position {
        ParamPosition::Path => &mut params.path,
        ParamPosition::Query => &mut params.query,
        ParamPosition::Header => &mut params.header,
        ParamPosition::Body => &mut params.body,
    };
    let Some(param) = group.iter_mut().find(|param| param.name == correction.name) else {
        problems.push(format!(
            "patch for {select:?} corrects a `{:?}` parameter named {:?}, which the vendored spec \
             does not declare there",
            correction.position, correction.name
        ));
        return;
    };
    if let Some(required) = correction.required {
        param.required = required;
    }
    if let Some(description) = &correction.description {
        param.description = description.clone();
    }
    if let Some(schema) = &correction.schema {
        param.schema = schema.clone();
    }
}

/// What the spec cache actually holds, for a refusal about a pin that resolved to nothing.
///
/// The paths, not a count: an author who mistyped a pin needs to see the spelling that would have
/// worked, and one who vendored nothing needs to be told that rather than left comparing a number
/// against a directory listing.
fn describe_cache(documents: &[SpecDocument<'_>]) -> String {
    if documents.is_empty() {
        "The spec cache holds no document for this provider at all — the cache is committed, so a \
         pointer at a file that is not there is a connector that cannot be built rather than one \
         that builds empty"
            .to_owned()
    } else {
        format!(
            "The cache holds {}",
            documents
                .iter()
                .map(|document| document.path)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// What to say after "names no `operationId`" — the closest spellings, or how many there were.
///
/// A document with 356 operations cannot have them all listed in a refusal, and a bare count helps
/// nobody. A prefix match catches the overwhelmingly common cause, which is a typo or a vendor's
/// casing change.
fn nearest(ingested: &crate::openapi::Ingested, select: &str) -> String {
    let folded = select.to_lowercase();
    let near: Vec<&str> = ingested
        .operation_ids()
        .into_iter()
        .filter(|id| {
            let id = id.to_lowercase();
            id.starts_with(&folded) || folded.starts_with(&id) || id.contains(&folded)
        })
        .take(5)
        .collect();
    if near.is_empty() {
        format!(
            "The document declares {} operations, none of them by that name",
            ingested.operations.len()
        )
    } else {
        format!("Did you mean {}?", near.join(", "))
    }
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
            runtime: file.runtime,
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
        // Filled by `ingest_spec` when a document was supplied; assembling reads the TOML alone.
        ingested: None,
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
fn validate(
    loaded: &LoadedProvider,
    provider_headers: &BTreeMap<String, String>,
    inline: &[String],
) -> Vec<String> {
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
    validate_patch(loaded, inline, &mut problems);

    problems
}

/// Checks the configuration surface — what a human is asked for, and where each answer goes.
///
/// Two properties, and the first is the one that closes a defect every templated provider records in
/// a comment: **a connector must ask for everything it needs**, and **it must not ask for anything it
/// cannot use**. A template variable nobody declares is a connector that silently cannot be
/// configured; a field binding nothing real is a question whose answer is discarded.
///
/// # Why `secret` + `example` is refused here and not asserted over `providers/` — C-231
///
/// The case against putting it in the loader is that an `example` is *documentation*, and a loader
/// that polices documentation is a loader with an opinion about prose. **That argument is rejected,
/// on three grounds:**
///
/// 1. **This loader already treats `example` as a checked property, not as prose.** It is validated
///    against the field's own [`Format`](crate::Format) six lines below, and against the request
///    position it pins in [`validate_pin`]. The precedent is not merely nearby, it is the same
///    field; the documentation argument was already answered when those landed.
/// 2. **The property being checked is not "is this placeholder good".** It is "no credential-shaped
///    literal is committed", which is the rule
///    `no_provider_file_carries_a_credential_value` states over the same files and which
///    `validate_const_headers` already enforces at the loader for a header value. A secret field's
///    `example` is the one remaining place in `[[config]]` where a credential-shaped literal is
///    invited by the schema.
/// 3. **A test over `providers/` protects this repository's 53 files and nobody else's.** These
///    crates are published, so a downstream author writing their own provider TOML is a real
///    person, and a refusal at [`load`] is the only form of this rule that reaches them. The cost
///    asymmetry is what settles it: a placeholder that merely *looks* like a token blocks a push
///    and costs an hour, and one that *is* a token is a disclosed credential.
///
/// Catalogue-wide, the rule is named by `no_shipped_provider_gives_a_secret_field_an_example`
/// (`crates/connector-spec/tests/config_fields.rs`), which enumerates `providers/` from disk — the
/// same shape `no_shipped_provider_has_an_unbound_template_variable` beside it already uses for a
/// rule the loader also refuses, and the reason a provider landing tomorrow is covered without
/// anyone adding it to a list. What does *not* belong anywhere is a **per-connector** restatement:
/// C-219's `no_secret_config_field_carries_an_example` was reduced to the claim that is actually
/// about Confluence. Measured while landing this: **24** per-connector tests spelled this rule out,
/// and 14 of the 38 providers that declare a secret field had none — one rule with two dozen
/// spellings that still missed a third of its surface, which is the defect C-230 is about.
///
/// **Scope.** Secret fields only. Whether a *non-secret* field's example is realistic is a
/// documentation question, and those placeholders stay welcome.
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

        // A secret field takes no placeholder at all — C-231. See `validate_config`'s own doc
        // comment for why this is a loader refusal rather than a test over `providers/`.
        if field.secret && field.example.is_some() {
            problems.push(format!(
                "configuration field {name:?} declares `secret = true` and an `example`. A secret \
                 takes no placeholder: a token-shaped literal in a committed file has tripped \
                 GitHub push protection and blocked a release here before, and a placeholder that \
                 is a real token is a disclosed credential rather than a blocked push. Nobody \
                 recognises their own secret from an example of someone else's — put the shape in \
                 `help` instead"
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
        Binding::Request {
            position,
            name: pinned,
        } => validate_pin(connector, field, position, pinned, problems),
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

/// **An operator-pinned request value resolves, is mandatory, and is not also an argument** —
/// C-187.
///
/// A pin says "this connector is installed for *this* zone / *this* team". Three ways that can be
/// declared and mean nothing, each refused here rather than discovered later:
///
/// 1. **It reaches nothing.** A `path.<variable>` no operation's path carries, a header name that
///    is not an HTTP field name — the pin would be collected from a human and dropped. This is the
///    request-position twin of the endpoint rule two functions down.
/// 2. **It is also a caller argument.** If any operation of the service declares a parameter the
///    pin already claims, the pin is *advisory*: the emitted op still takes the value, and a model
///    passing its own overrides the operator's. That is the opposite of the point, so it is a
///    refusal and not a precedence rule — there is no reading under which two declarations of one
///    request slot are both right.
/// 3. **It is optional.** A host substitutes a pinned placeholder into a string literal and refuses
///    the whole request when it has no value (`connector-pack`'s `Error::MissingConfig`), so
///    `required = false` describes a connector that composes no URL. For a *query* pin it is worse
///    than useless: Vercel's `teamId` is dangerous precisely because omitting it silently redirects
///    the call to a personal account, and an optional pin would reintroduce that.
///
/// The fourth check is about *addressing* rather than reach. A host keys a configuration value by
/// `(tenant, provider, service, kind, name)` and the emitted module carries one `{placeholder}` per
/// pinned value, so two values of one service sharing a placeholder name are **one slot** — the
/// exact collapse C-197 found between Contentful's two spaces, where a management write landed in
/// whichever space the delivery reads were configured with. Two declarations that would share a
/// placeholder are refused here so that they can never share a slot there.
fn validate_pin(
    connector: &Connector,
    field: &ConfigField,
    position: Position,
    pinned: &str,
    problems: &mut Vec<String>,
) {
    let name = field.name.as_str();
    let service = field.service.as_str();
    let word = position.word();

    // A pin whose value never arrives is a connector with no URL, not one that sends less.
    if !field.required {
        problems.push(format!(
            "configuration field {name:?} pins `{word}.{pinned}` but declares `required = false`. A \
             pinned value is substituted into the emitted module, and a host with no value refuses \
             the whole request rather than omitting the pin — so an optional pin is a connector that \
             composes no URL. Drop `required` or drop the pin"
        ));
    }

    // The example is what a user copies into the field, so it is held to the rule the position
    // imposes on the real value — the same reasoning `format`/`example` already carries.
    if let Some(example) = &field.example {
        if let Err(reason) = position.validate_value(example) {
            problems.push(format!(
                "configuration field {name:?} pins a {word} value but gives an `example` that could \
                 not be one: {reason}"
            ));
        }
    }

    match position {
        Position::Path => {
            let carried = connector
                .operations_of(service)
                .any(|operation| template_variables(&operation.path).contains(&pinned));
            if !carried {
                problems.push(format!(
                    "configuration field {name:?} pins `{{{pinned}}}` in the request path, which no \
                     operation of service {service:?} carries. A pin nothing interpolates is a \
                     question whose answer is discarded"
                ));
            }
        }
        Position::Query => {
            if let Err(reason) = position.validate_value(pinned) {
                problems.push(format!(
                    "configuration field {name:?} pins query parameter {pinned:?}, which is not a \
                     query parameter name: {reason}"
                ));
            }
        }
        Position::Header => {
            let folded = pinned.to_ascii_lowercase();
            if !is_http_field_name(pinned) {
                problems.push(format!(
                    "configuration field {name:?} pins header {pinned:?}, which is not an HTTP \
                     field name — only ASCII token characters are allowed (RFC 9110 §5.1), and a \
                     request carrying it could never be built"
                ));
            }
            if folded == "content-type" {
                problems.push(format!(
                    "configuration field {name:?} pins `content-type`, which is the emitter's: it \
                     is derived from the request body, so pinning it would describe an encoding the \
                     emitted module does not produce"
                ));
            }
            // The line this binding exists **not** to cross. A pinned value is non-secret by
            // construction and reaches no redactor, so letting one land in an auth-owned header
            // would be a second, ungated route for a credential — the thing `const_headers` is
            // already refused for.
            if AUTH_OWNED_HEADERS.contains(&folded.as_str()) {
                problems.push(format!(
                    "configuration field {name:?} pins header {pinned:?}, which carries a \
                     credential. A pinned value is configuration: it is never masked, never \
                     redacted, and readable back by anyone who can open a settings page. \
                     Credentials are declared in `[[auth]]` and injected by the host at the `$auth` \
                     seam"
                ));
            }
            for method in &connector.auth {
                if let AuthScheme::Header { name: owned, .. } = &method.scheme {
                    if owned.eq_ignore_ascii_case(pinned) {
                        problems.push(format!(
                            "configuration field {name:?} pins header {pinned:?}, which is where \
                             credential {:?} is injected. One of the two would overwrite the other, \
                             and which one depends on an order nothing declares",
                            method.name
                        ));
                    }
                }
            }
        }
    }

    // A pin that is also an argument is advisory, and an advisory pin is not a pin.
    for operation in connector.operations_of(service) {
        let claimed = match position {
            Position::Path => operation
                .params
                .path
                .iter()
                .any(|param| wire_of(param) == pinned),
            Position::Query => operation
                .params
                .query
                .iter()
                .any(|param| wire_of(param) == pinned),
            Position::Header => {
                operation
                    .params
                    .header
                    .iter()
                    .any(|param| wire_of(param).eq_ignore_ascii_case(pinned))
                    || operation
                        .params
                        .const_headers
                        .keys()
                        .any(|header| header.eq_ignore_ascii_case(pinned))
            }
        };
        if claimed {
            problems.push(format!(
                "configuration field {name:?} pins `{word}.{pinned}`, but operation {:?} already \
                 declares it. A value an operator pins at install time and a caller may also pass is \
                 not pinned — the caller's wins, and the operator's choice of tenant becomes a \
                 suggestion. Declare it on one side only",
                operation.id
            ));
        }
    }

    // One placeholder, one host-side slot. See this function's documentation for the C-197 collapse
    // this refusal exists to make unreachable.
    for other in connector.config_of(service) {
        if std::ptr::eq(other, field) {
            continue;
        }
        let shared = match other.binding() {
            Some(Binding::Endpoint { variable }) => variable == pinned,
            Some(Binding::Request { name: other, .. }) => other == pinned,
            _ => false,
        };
        if shared {
            problems.push(format!(
                "configuration fields {name:?} and {:?} both resolve `{{{pinned}}}` in service \
                 {service:?}, so a host would key them to one value under one slot. Two questions \
                 that share an answer are one question — bind one of them to a different name",
                other.name
            ));
        }
    }
}

/// The spelling the vendor sees for a parameter: its `wire` alias when it declares one.
///
/// A pin is compared against this rather than against `name`, because it is the wire name that
/// occupies the request slot the pin would claim.
fn wire_of(param: &crate::Param) -> &str {
    param.wire.as_deref().unwrap_or(&param.name)
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

/// Checks a header credential's literal prefix (C-184).
///
/// The prefix is the closest this repository gets to authoring a credential value — it is the text
/// immediately before one — so it is the seam where "no credential value" has to be enforced rather
/// than assumed. Everything refused here is refused because it is either an attempt to reach the
/// secret through the prefix, or a request the connector did not describe.
///
/// # The separator rule is the load-bearing one
///
/// **The host appends the credential to the prefix with nothing in between.** So a prefix that ends
/// in an alphanumeric would be glued onto the secret — `SSWS` + `<token>` is `SSWS<token>`, a header
/// no vendor accepts. A well-formed prefix therefore *always* ends in a separator, and requiring
/// that catches two failures one rule apart:
///
/// - **A pasted credential.** `Bearer sk-live-51H8…` ends in an opaque blob, so it is refused. This
///   is the case a `CREDENTIAL_VALUE_PREFIXES` check would only half-catch: that list is matched
///   with `starts_with` and holds `"bearer "`, `"basic "`, `"token "`, `"apikey "`, `"digest "`, so
///   it would refuse a pasted `Bearer …` but not a pasted `SSWS …` or `OAuth …` — one of C-184's
///   three vendors, not three. The separator rule is indifferent to the scheme word and catches all
///   of them.
/// - **A missing trailing space.** `prefix = "SSWS"` was previously uncatchable, and
///   `crates/connector-flux/tests/okta_connector.rs` says so in as many words. It is the same rule:
///   `SSWS` does not end in a separator.
///
/// `Token token=` passes, because `=` *is* a separator — which is the point. The rule is about the
/// boundary between connector data and the secret, not about the vendor's choice of syntax.
fn validate_auth_prefix(
    connector: &Connector,
    method: &AuthMethod,
    prefix: &str,
    problems: &mut Vec<String>,
) {
    if prefix.is_empty() {
        return;
    }
    let name = method.name.as_str();
    let folded = prefix.to_ascii_lowercase();

    // A prefix is emitted as a literal and nothing interpolates it, so a marker is either a broken
    // request or an author reaching for the value. Both end the same way: the only spelling that
    // "works" is the credential itself, pasted in.
    if let Some(marker) = RESOLUTION_MARKERS
        .iter()
        .find(|marker| folded.contains(*marker))
    {
        problems.push(format!(
            "credential {name:?} declares a header `prefix` spelling {marker:?}, but a prefix is a \
             literal and nothing interpolates it — the vendor would receive those characters. The \
             prefix carries the vendor's scheme word (`SSWS `, `Token token=`); the credential is \
             appended by the host and is never written here"
        ));
    }

    // Every credential the connector declares, not just this one, and folded — matching the sibling
    // `credential_shaped_value`, which has always iterated `connector.auth`. A prefix naming another
    // credential's variable is the same mistake spelled sideways, and case never made it less of one.
    for other in &connector.auth {
        if folded.contains(&other.name.to_ascii_lowercase()) {
            problems.push(format!(
                "credential {name:?} declares a header `prefix` naming credential {:?}. A prefix is \
                 a literal, not a reference — nothing resolves the name, and the value that would \
                 make it work is one this file must never hold",
                other.name
            ));
        }
        for key in other.env.iter().chain(&other.user_env) {
            if !key.trim().is_empty() && folded.contains(&key.to_ascii_lowercase()) {
                problems.push(format!(
                    "credential {name:?} declares a header `prefix` naming the environment variable \
                     {key:?}, which resolves credential {:?}. A prefix is emitted as a literal, so \
                     the name would travel as text and the value it stands for must never be \
                     written here at all",
                    other.name
                ));
            }
        }
    }

    // See the separator rule on this function. `SSWS ` ends in a space, `Token token=` in `=`; a
    // prefix ending in an alphanumeric would be concatenated onto the secret.
    if prefix
        .chars()
        .next_back()
        .is_some_and(|last| last.is_ascii_alphanumeric())
    {
        problems.push(format!(
            "credential {name:?} declares a header `prefix` ending in an alphanumeric character. \
             The host appends the credential directly, so this would send the prefix and the secret \
             glued together. A scheme word ends in a separator — `\"SSWS \"` with the trailing \
             space, `\"Token token=\"` with the `=`. If the text after the scheme word is the \
             credential itself, it does not belong in this file at all"
        ));
    }

    // A prefix of only spaces contributes no scheme word and puts leading whitespace in front of the
    // credential, which `field-content` does not allow at the edges of a header value.
    if prefix.trim().is_empty() {
        problems.push(format!(
            "credential {name:?} declares a header `prefix` of only whitespace, which carries no \
             scheme word and would send a header value beginning with a space (RFC 9110 §5.5 \
             field-content permits no leading or trailing whitespace). Omit `prefix` for a header \
             whose whole value is the secret"
        ));
    }

    // **The whitespace-corruption class, found by C-184's own review.**
    //
    // The separator rule above catches a prefix with *no* trailing separator. It does not catch one
    // with too many, and neither did anything else: `"SSWS  "` and `" SSWS "` both loaded, and both
    // send a header the vendor answers `401` to. Worse, nothing downstream could catch them either —
    // a connector's own suite asserts the prefix against a constant in the same file, so an author
    // editing both together leaves every test green.
    //
    // Deliberately narrow. It refuses *whitespace* corruption, which is an HTTP hygiene rule that
    // holds for every vendor, and says nothing about repeated punctuation: `"Token token=="` is
    // wrong for PagerDuty but this model has no basis to declare `==` wrong in general, and guessing
    // at a vendor's syntax is how a checker starts refusing correct connectors.
    if prefix.starts_with([' ', '\t']) {
        problems.push(format!(
            "credential {name:?} declares a header `prefix` beginning with whitespace. It would send \
             a header value whose first character is a space, which RFC 9110 §5.5 field-content does \
             not permit at the edges — and which a vendor answers with `401` rather than a message \
             naming the space"
        ));
    }
    if let Some(run) = prefix
        .as_bytes()
        .windows(2)
        .position(|pair| pair.iter().all(|byte| matches!(byte, b' ' | b'\t')))
    {
        problems.push(format!(
            "credential {name:?} declares a header `prefix` with two consecutive whitespace \
             characters at byte {run}. One separator is what the scheme word needs — `\"SSWS \"`, \
             `\"OAuth \"` — and a second one travels to the vendor verbatim, which answers `401` \
             without saying why. Nothing downstream catches this: a connector's own test asserts the \
             prefix against a constant beside it, so editing both together leaves the suite green"
        ));
    }

    // The value half of the grammar check `name` has had since C-3. A prefix reaches a header value
    // verbatim, so a CR or LF in one ends the header and begins another — header injection, from a
    // committed artifact. RFC 9110 §5.5 field-content: visible ASCII, plus space and horizontal tab.
    if let Some(bad) = prefix
        .chars()
        .find(|c| !matches!(c, ' ' | '\t') && !c.is_ascii_graphic())
    {
        problems.push(format!(
            "credential {name:?} declares a header `prefix` containing {bad:?}, which is not \
             visible ASCII, space or tab. A prefix is placed into a header value verbatim, so a \
             newline in one would end the header and begin another (RFC 9110 §5.5 field-content)"
        ));
    }
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

        if let AuthScheme::Header { prefix, .. } = &method.scheme {
            validate_auth_prefix(connector, method, prefix, problems);
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
            if let AuthScheme::Header { name: owned, .. } = &method.scheme {
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

        validate_repeatability_condition(operation, problems);

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

/// **A `conditional` write must state its condition, and the condition must mean something.**
///
/// flux's I3 (`flux_spec::coherence`) names [`Idempotency::Conditional`] as the escape hatch for a
/// mutation that is genuinely replay-safe — "safe to repeat under **stated** conditions". This is
/// what makes "stated" true. Before C-186 nothing did: six operations declared `conditional` with
/// the condition written in no field and no artifact, so a host learned that a condition existed
/// and nothing about what it was.
///
/// Four refusals, each a different author mistake:
///
/// - **a mutating `conditional` with no condition** — the claim without the thing that makes it
///   checkable, and the reason this validator exists;
/// - **a condition on a non-mutating method** — there is no repeat hazard to condition, so the
///   field would spread as cargo-culted decoration until no reviewer read any of them;
/// - **a condition on an operation not declaring `conditional`** — prose asserting what its own
///   field denies, which is precisely the drift this story removes, arriving from the other side;
/// - **a condition that says nothing** — `"yes"` unlocks the claim while telling a reviewer no more
///   than silence did.
///
/// `connector-flux` refuses all four again on the IR rather than on the file. That overlap is
/// deliberate and each layer is pinned on its own: this is the loud, early refusal an author sees,
/// and `check_write_metadata` is the one an IR assembled in memory cannot walk past.
fn validate_repeatability_condition(operation: &Operation, problems: &mut Vec<String>) {
    let id = operation.id.as_str();
    let mutating = matches!(
        operation.method,
        HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch | HttpMethod::Delete
    );

    if !operation.states_repeatability_condition() {
        if mutating && operation.idempotency == Idempotency::Conditional {
            problems.push(format!(
                "operation {id:?} declares `idempotency = \"conditional\"` but no \
                 `repeatable_because`. `conditional` is flux's escape hatch for a write that is \
                 genuinely safe to repeat *under a stated condition* (`flux_spec::coherence`, I3), \
                 and a condition stated nowhere leaves a host knowing only that one exists — say \
                 what makes repeating this call safe"
            ));
        }
        return;
    }

    if !mutating {
        problems.push(format!(
            "operation {id:?} is a {} and declares `repeatable_because`, but a method that changes \
             nothing has no repeat hazard to put a condition on. The field exists only to state the \
             condition behind `idempotency = \"conditional\"` on a write; remove it",
            method_word(operation.method)
        ));
        return;
    }

    if operation.idempotency != Idempotency::Conditional {
        problems.push(format!(
            "operation {id:?} declares `repeatable_because` but `idempotency = {:?}`. The condition \
             describes a repeat that is safe while the field says otherwise — one of the two is \
             wrong, and shipping both is the contradiction C-186 exists to end",
            idempotency_word(operation.idempotency)
        ));
        return;
    }

    if operation.repeatability_condition().is_none() {
        problems.push(format!(
            "operation {id:?} declares `repeatable_because` = {:?}, which is shorter than \
             {MIN_REPEATABILITY_CONDITION} characters once trimmed and states no vendor behaviour. \
             This is what a reviewer reads beside a retry-safety claim on a write; say what \
             repeating the call actually does, as `cloudflare-cache-purge` and \
             `launchdarkly-flag-toggle` do",
            operation.repeatable_because.as_deref().unwrap_or_default()
        ));
    }
}

/// The `idempotency` value as an author spells it in a provider file. Exhaustive so a fourth variant
/// is a compile error here rather than a refusal quoting the wrong word.
fn idempotency_word(idempotency: Idempotency) -> &'static str {
    match idempotency {
        Idempotency::Idempotent => "idempotent",
        Idempotency::NonIdempotent => "non_idempotent",
        Idempotency::Conditional => "conditional",
    }
}

/// The method as an author spells it in a provider file.
fn method_word(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Head => "HEAD",
        HttpMethod::Options => "OPTIONS",
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
fn validate_patch(loaded: &LoadedProvider, inline: &[String], problems: &mut Vec<String>) {
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
            // Asked of the **inline** ids, not of the connector's — after selection every rename is
            // among the connector's operations by construction, which would make this fire on every
            // successful patch (C-4).
            } else if inline.iter().any(|id| id == rename) {
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
