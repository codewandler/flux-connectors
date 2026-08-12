//! The provider-TOML front-end: `providers/<name>.toml` in, [`Connector`] out.
//!
//! The file plays **two roles**, and the loader has to serve both from one schema:
//!
//! 1. **Hand-authored** — the whole connector is written out inline, with no vendor spec anywhere.
//!    Ollama, Freshdesk and (for now) Zendesk are in this position: there is no usable OpenAPI
//!    document to ingest. This is the role that matters most today, because it is the shortest route
//!    to an executable `.flux` module.
//! 2. **Spec pointer** — the file names one or more vendored specs under `specs/` and carries a
//!    *patch set* that selects and corrects operations from them. [`load_with_spec`] is that path:
//!    ingest (C-4) turns each document into every operation the vendor declares, and the patch set
//!    says which of them this connector publishes and what it corrects about each. **Selection is
//!    opt-in**, so a pointer with no patch is a connector with no operations. What one *statement*
//!    selects is wide — [`OperationSelector`] matches a set by service, path prefix and method,
//!    [`Naming`] derives op ids through one declared rule with pinned exceptions, and both `risk`
//!    and `expose` may be stated for a whole matched set (C-411, C-412, C-414) — and none of that
//!    changes opt-in, because a selector is still something an author wrote.
//!
//!    **One document is one [`Service`]** (C-410). `[spec]` names one and `[[spec]]` names several;
//!    they are one key in two TOML spellings and the table is the one-element case. A vendor that
//!    splits its API across documents — babelforce publishes five, over two API versions and two
//!    security models — is therefore one connector with a service per document, rather than five
//!    connectors. Each document is resolved, hash-checked and ingested on its own, and nothing is
//!    merged: an `operationId` is unique inside a document and nowhere else.
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

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::config::{
    parse_binding, template_variables, Approval, Binding, ConfigField, Format, Position,
};
use crate::graph::{Graph, GraphNode, NodeKind, PortRef};
use crate::inbound::{
    parse_tolerance, signed_placeholders, validate_path, validate_symbol, ChannelBinding,
    EventDecl, FieldSource, HmacSpec, ManualSetup, Reply, Selector, SocketConnectSpec,
    Subscription, Transport, VerificationScheme, PAYLOAD_PLACEHOLDERS, SIGNED_PLACEHOLDERS,
};
use crate::lock::sha256_hex;
use crate::{
    response_location_exists, AuthHazard, AuthMethod, AuthRequirement, AuthScheme, Connector,
    HttpMethod, Idempotency, JsonSchema, OAuthGrant, Operation, OperationDirection,
    OperationSpecSource, Param, ParamSet, Provenance, Quirks, Risk, Role, Runtime, SemanticEffect,
    Service, Tag, DEFAULT_SERVICE, MIN_REPEATABILITY_CONDITION,
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
    /// The vendor documents the file points at, in the order it declares them — C-410.
    ///
    /// Empty for a fully hand-authored connector. A single `[spec]` block is one entry, which is why
    /// the plural costs the single-document form nothing: `[spec]` and `[[spec]]` are two spellings
    /// of one field, and the loader treats the first as the one-element case of the second.
    pub specs: Vec<SpecSource>,
    /// The patch set applied over the ingested specs. Empty for a hand-authored connector.
    pub patch: Patch,
    /// What each vendored document said, when documents were supplied to [`load_with_spec`] — C-4,
    /// widened to several by C-410.
    ///
    /// The **whole** ingest of each, not just the part that was published: every operation the
    /// document declares including the ones no patch selected, plus the servers it names and every
    /// [`Diagnostic`](crate::openapi::Diagnostic) it earned. That is what makes "ingest makes
    /// everything *available* to patch" inspectable rather than merely claimed — and it is what a
    /// future `flux-connectors check` reads to tell an author which operations they could have
    /// selected.
    ///
    /// **One entry per document, never one merged whole.** Merging is exactly what this story
    /// exists to refuse: babelforce's manager document declares root `oauth2` with zero operation
    /// overrides while `task-automation` declares `bearerAuth`+`oauth2` on all 31 of its operations,
    /// and one field holding "the ingest" would have let whichever was read last describe both.
    ///
    /// Empty for a hand-authored connector, and also for a spec-backed one loaded through plain
    /// [`load`], which is given no document to ingest.
    pub ingested: Vec<IngestedDocument>,
    /// Members whose TOML table omitted `service` before serde normalized that omission to
    /// [`DEFAULT_SERVICE`]. Needed only for C-458's mixed legacy-default shape, where explicit
    /// `service = "default"` and omission must remain different authoring decisions.
    implicit_service_members: Vec<ImplicitServiceMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImplicitServiceMember {
    kind: &'static str,
    name: String,
}

impl LoadedProvider {
    /// Whether this file is a complete hand-authored definition — no spec, so nothing to ingest and
    /// nothing to overlay.
    pub fn is_hand_authored(&self) -> bool {
        self.specs.is_empty()
    }

    /// Everything wrong with the vendored documents that did not stop their ingest.
    ///
    /// Empty for a hand-authored connector. A real vendor document is never fully well-formed, so
    /// this being non-empty is the normal case, not a failure — see [`crate::openapi`].
    pub fn diagnostics(&self) -> Vec<&crate::openapi::Diagnostic> {
        self.ingested
            .iter()
            .flat_map(|document| document.ingested.diagnostics.iter())
            .collect()
    }

    /// The ingest of the document that joined `service`, if the file declared one.
    pub fn ingested_for(&self, service: &str) -> Option<&IngestedDocument> {
        self.ingested
            .iter()
            .find(|document| document.service == service)
    }
}

/// One vendored document, ingested, and the service its operations join — C-410.
#[derive(Debug, Clone, PartialEq)]
pub struct IngestedDocument {
    /// The repository-relative path the `[[spec]]` entry pinned.
    pub path: String,
    /// The service this document's selected operations belong to.
    ///
    /// [`DEFAULT_SERVICE`](crate::DEFAULT_SERVICE) when the entry names none, which is what keeps a
    /// single `[spec]` block meaning exactly what it meant before this field existed.
    pub service: String,
    /// Everything the document declares.
    pub ingested: crate::openapi::Ingested,
}

/// Where one vendor document for this connector lives, and which service it becomes.
///
/// The path is into the **vendored, committed** cache under `specs/`, never a URL to fetch at build
/// time: builds are hermetic and offline (AGENTS.md). `source_url` records where the bytes came
/// from so C-14 can re-fetch and diff, and `sha256` is what makes that diff a fact rather than a
/// guess.
///
/// # Provenance is per document, not per connector — C-410
///
/// A connector may declare several documents, and each carries **its own** `sha256`, `fetched_at`
/// and `upstream_version`. babelforce's five documents were pulled on two different dates and three
/// of them publish `info.version = "0.0.0-dev"`; one hash for the connector could not say which of
/// them moved, which is the only question a drift check is asked. So this whole struct is what
/// reaches [`Provenance::specs`](crate::Provenance::specs), one entry per document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecSource {
    /// The vendored spec file, relative to the repository root
    /// (`specs/babelforce/manager-2026-07-10.openapi.yaml`).
    pub path: String,
    /// The [`Service`] this document's selected operations join — C-410.
    ///
    /// Absent means the reserved [`DEFAULT_SERVICE`](crate::DEFAULT_SERVICE), which is what a single
    /// `[spec]` block meant before this key existed and must keep meaning. A named value must be one
    /// a `[[services]]` entry declares, checked by the same pass that checks an inline operation's
    /// `service` — a document is not a declaration of a service, it joins one.
    ///
    /// **This is what makes several documents a partition rather than a pile.** Two documents may
    /// declare the same `operationId` — `getUser` genuinely exists in babelforce's
    /// `manager-2026-07-10` and `user-2026-06-25`, and they are different calls — so an id is only
    /// unambiguous inside one document's service. Every [`OperationPatch`] therefore resolves
    /// against exactly one of these.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
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

impl SpecSource {
    /// The service this document's operations join — [`DEFAULT_SERVICE`](crate::DEFAULT_SERVICE)
    /// when the entry names none.
    pub fn service(&self) -> &str {
        self.service.as_deref().unwrap_or(DEFAULT_SERVICE)
    }
}

/// The patch set applied over an ingested spec — C-6's input, widened to statements about sets by
/// C-411, C-412 and C-414.
///
/// **Selection is opt-in**, which is why there is no `hide`. A 163-operation spec must not become
/// 163 LLM tools (`docs/designs/provider-operation-inventory.md` §5.2 selects 9 of them), and an
/// opt-out list would make every new upstream operation a new tool by default. Only operations a
/// [`OperationPatch::select`] names or an [`OperationSelector`] matches reach the connector, and a
/// selector widens what one *statement* selects without making anything default-selected.
///
/// # The merge order, stated once
///
/// **spec → select → per-operation patch → validate**, and it is total:
///
/// 1. ingest turns each document into every operation the vendor declares;
/// 2. every [`OperationSelector`] states what it states about the set it matched, and two selectors
///    that state different values for one operation are refused rather than ordered;
/// 3. the [`OperationPatch`] that names an operation overrides the selector **field by field** —
///    where the block is silent the selector's statement stands, and where neither speaks the rules
///    on each field decide;
/// 4. the result is validated by exactly the pass a hand-authored operation goes through.
///
/// The published order follows from the same sentence: operations a `[[patch.operations]]` block
/// names publish in file order, then everything a selector matched publishes in document order, per
/// `[[spec]]` entry. Fixed, so identical inputs produce byte-identical IR — and so a file that
/// declares no selector publishes exactly what it published before selectors existed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    /// The statements that select **sets** of operations — C-411.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub select: Vec<OperationSelector>,
    /// How an `operationId` becomes an op id, declared once — C-412.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub naming: Option<Naming>,
    /// Reviewed direction keyed by stable spec identity: service, then vendor `operationId`.
    ///
    /// Unlike a selector this map cannot change membership when an upstream method, path, name or
    /// description changes. Quoted operation ids are ordinary TOML keys:
    /// `[patch.directions.manager]` followed by `flushDialer = "write"`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub directions: BTreeMap<String, BTreeMap<String, OperationDirection>>,
    /// The operations selected one at a time, each with its corrections.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<OperationPatch>,
}

impl Patch {
    /// Whether the file carries no patches at all.
    pub fn is_empty(&self) -> bool {
        self.select.is_empty()
            && self.naming.is_none()
            && self.directions.is_empty()
            && self.operations.is_empty()
    }

    /// How to spell the block an author would go and edit, for a refusal about a patch set with no
    /// `[spec]` to apply to.
    ///
    /// Names what the file actually wrote rather than the commonest key: a message about
    /// `[[patch.operations]]` sends someone who only wrote a selector looking for a block they never
    /// authored.
    fn declared(&self) -> &'static str {
        if !self.directions.is_empty() {
            "[patch.directions]"
        } else if !self.operations.is_empty() {
            "[[patch.operations]]"
        } else if !self.select.is_empty() {
            "[[patch.select]]"
        } else {
            "[patch.naming]"
        }
    }
}

/// One statement that selects a **set** of operations — C-411.
///
/// ```toml
/// [[patch.select]]
/// service = "manager"
/// path_prefix = "/api/v2/agents"
/// methods = ["GET"]
/// risk = "low"
/// idempotency = "idempotent"
/// expose = false
/// ```
///
/// # Why this exists
///
/// [`OperationPatch`] selects exactly one `operationId`. For babelforce's canonical surface that is
/// **397** blocks, each carrying a `select`, a `rename`, a `risk` and an `idempotency` before any
/// real correction — a file nobody reviews, which means a file in which nobody notices a wrong
/// safety claim. A selector is the same statements at the grain they are actually true at: one risk
/// for 50 DELETEs, one exposure decision for the 388 operations that are callable without being
/// tools.
///
/// # What it does *not* do
///
/// It does not make anything default-selected. A file with no selector and no
/// `[[patch.operations]]` publishes nothing, and there is no `hide`: an opt-out list would make
/// every operation a vendor adds upstream a tool by default, learned about from a model's behaviour
/// rather than from a diff.
///
/// A selector that matches nothing is a **loud error**, for the same reason
/// [`OperationPatch::select`] naming an absent `operationId` is: a prefix that stops matching after
/// an upstream reshuffle would quietly empty the connector and the build would stay green.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationSelector {
    /// **Which document this selector reads** — the `service` of one `[[spec]]` entry (C-410).
    ///
    /// Absent is legal only when the file declares exactly one document, and means that one. The
    /// rule is [`OperationPatch::service`]'s and for the same reason: a path prefix is no more
    /// unique across documents than an `operationId` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// The path prefix an operation's path must carry, matched on **whole segments**.
    ///
    /// `/api/v2/agents` reaches `/api/v2/agents` and `/api/v2/agents/{id}` and does not reach
    /// `/api/v2/agentsummary` — a prefix that matched half a segment would select by spelling
    /// accident, which is the opposite of a statement.
    ///
    /// Absent means every path in the document. That is a real case (a document that *is* one
    /// resource namespace) and still an explicit statement, so it stays legal.
    ///
    /// **Path prefix rather than tag**: `Manager` tags 309 of the manager document's 356
    /// operations, while 47 distinct three-segment prefixes reproduce the SDK's 36 resource
    /// namespaces almost exactly. The vendor's tags describe the docs site; its paths describe the
    /// API.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    /// The HTTP methods to match. Empty means every method.
    ///
    /// Splitting a prefix by method is how one `risk` covers a set honestly: the reads and the
    /// deletes under one prefix are not one damage claim.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<HttpMethod>,
    /// The [`Risk`] every matched operation carries — C-414.
    ///
    /// **Silence on an authored write refuses the build.** See [`Self::idempotency`] for the whole
    /// rule, which is one rule for both fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<Risk>,
    /// The [`Idempotency`] every matched operation carries — C-414.
    ///
    /// # Silence refuses on a write and is answered on a read, and that asymmetry is the point
    ///
    /// No OpenAPI document publishes either field, so 214 of babelforce's 398 operations need both
    /// stated by someone. Deriving them from the HTTP method is the failure mode this repository has
    /// legislated against twice ([`Risk`] has no `Default`; C-186 made `conditional` state its
    /// condition or not build), because a default that *flatters* turns 214 unmade decisions into
    /// 214 claims a host reads as a licence.
    ///
    /// So: a matched operation whose identity-stable direction is `write` and about which neither
    /// this selector nor a `[[patch.operations]]` block says anything is **refused, by name**. A
    /// matched operation authored as `read` takes `low` and `idempotent` — not a method-derived
    /// direction or a flattering write default, but the only absent safety values a reviewed read
    /// can receive without widening its authority.
    ///
    /// The asymmetry belongs to **selection**, which is a statement about a set that may mix
    /// methods. A `[[patch.operations]]` block is a statement about one operation, and it still
    /// states both — one line, on the operation an author is already looking at.
    ///
    /// # `conditional` is not made bulk by this
    ///
    /// A selector may state `idempotency = "conditional"`, and every matched mutating operation
    /// then still owes the stated `repeatable_because` C-186 requires — which no selector can
    /// supply for many operations at once, because one sentence about 54 endpoints is not a
    /// condition. So the build refuses, per operation. A bulk escape hatch around C-186 is the one
    /// thing this field must not become.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency: Option<Idempotency>,
    /// Whether every matched operation reaches a model as a tool — C-413's [`Operation::expose`],
    /// declared for a set.
    ///
    /// Absent means the field's own default, which is **exposed**: silence here decides nothing, and
    /// nothing-decided must keep meaning what the repository already does. Declaring the inverse
    /// per operation is 388 lines for babelforce, which is the whole reason this key is here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expose: Option<bool>,
}

/// How an `operationId` becomes an op id, declared once — C-412.
///
/// ```toml
/// [patch.naming]
/// rule = "kebab"                     # listReportingCalls -> babelforce-list-reporting-calls
/// prefix = "babelforce"
/// [patch.naming.pin]                 # the escape hatch, and the only per-op naming cost
/// listAgents = "babelforce-agent-list"
/// ```
///
/// # Why a rule is allowed to exist beside "op naming is a public contract"
///
/// It is not allowed to exist *instead* of it. `docs/designs/connector-pipeline.md` refuses ids
/// "derived from volatile spec fields like `operationId` without a pinned override" — and this is
/// the pinned override, made bulk. Three properties are what make it safe, and all three are
/// enforced rather than intended:
///
/// - the rule is **declared**, so it is reviewable as one line rather than inferred per operation;
/// - **collisions refuse** — two `operationId`s deriving one op id is an error, never
///   last-write-wins, because the loser would silently become unreachable under a name a user or a
///   model still calls;
/// - a derived id that is not a legal flux `decl_name` is **reported, naming the operation**, never
///   mangled into something that happens to parse.
///
/// The remaining half is a test, not a type: `tests/operation_selection.rs` pins the full derived
/// id set for a fixture, so an upstream `operationId` rename moves an op id **loudly**.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Naming {
    /// The derivation to apply. Required: a rule that could be omitted would be a rule decided by
    /// silence, and silence must not name a public contract.
    pub rule: NamingRule,
    /// Prepended to every derived id, joined with `-`. Absent means no prefix.
    ///
    /// In practice this is the connector id, because an op id is global: `babelforce` +
    /// `listReportingCalls` is `babelforce-list-reporting-calls`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// `operationId` → the op id to publish it as, overriding [`Self::rule`].
    ///
    /// This is where the ids a connector already ships are held still while everything around them
    /// is derived — the nine `providers/babelforce.toml` publishes today are exactly that case.
    ///
    /// **Keyed by `operationId` alone**, which is unique inside one document and nowhere else. A
    /// key two of the connector's documents both declare is refused rather than applied twice; the
    /// way to name one of them is a `[[patch.operations]]` block with a `rename`, which is
    /// service-qualified and outranks a pin.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pin: BTreeMap<String, String>,
}

/// The declared derivation from `operationId` to op id.
///
/// A closed enum with one variant today, so a second rule is a deliberate addition with its own
/// review rather than a string the loader interprets — and so a typo is refused by serde naming
/// every rule that exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamingRule {
    /// `listReportingCalls` → `list-reporting-calls`, with case boundaries becoming `-`.
    ///
    /// Acronyms keep their shape: `listHTTPCalls` → `list-http-calls`, because the boundary is read
    /// at the *end* of a run of capitals rather than at every capital.
    Kebab,
}

impl Naming {
    /// The op id this declaration gives `operation_id`, or the reason it gives none.
    ///
    /// A pin answers directly; otherwise the rule derives one and the result is held to the same
    /// grammar an authored `rename` is. The `Err` is the *reason*, phrased to be pasted into a
    /// refusal that has already named the operation.
    fn derive(&self, operation_id: &str) -> std::result::Result<String, String> {
        if let Some(pinned) = self.pin.get(operation_id) {
            let pinned = pinned.trim();
            return legal_op_id(pinned).map(|()| pinned.to_owned());
        }

        let stem = match self.rule {
            NamingRule::Kebab => kebab(operation_id),
        };
        let derived = match self
            .prefix
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            Some(prefix) => format!("{prefix}-{stem}"),
            None => stem,
        };
        legal_op_id(&derived).map(|()| derived)
    }
}

impl OperationSelector {
    /// Whether this selector matches one of a document's operations.
    ///
    /// The `internal` guard is **not** here: matching and eligibility are different questions, and
    /// a selector that matched only internal paths must still be reported as matching nothing
    /// rather than as matching something it then dropped.
    fn matches(&self, operation: &crate::openapi::SpecOperation) -> bool {
        if !self.methods.is_empty() && !self.methods.contains(&operation.method) {
            return false;
        }
        match self.path_prefix.as_deref().map(str::trim) {
            Some(prefix) => path_has_prefix(&operation.path, prefix),
            None => true,
        }
    }

    /// How the selector reads back in a refusal — the statement, not an index nobody can find.
    fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(service) = self.service.as_deref() {
            parts.push(format!("service = {service:?}"));
        }
        if let Some(prefix) = self.path_prefix.as_deref() {
            parts.push(format!("path_prefix = {prefix:?}"));
        }
        if !self.methods.is_empty() {
            let methods: Vec<&str> = self.methods.iter().copied().map(method_word).collect();
            parts.push(format!("methods = {methods:?}"));
        }
        if parts.is_empty() {
            "`[[patch.select]]` (stating nothing)".to_owned()
        } else {
            format!("`[[patch.select]] {}`", parts.join(", "))
        }
    }
}

/// Whether `path` lies under `prefix`, matched on **whole segments**.
///
/// `/api/v2/agents` covers `/api/v2/agents/{id}` and not `/api/v2/agentsummary`. Without the
/// boundary a prefix would select by spelling accident, and the accident would be invisible: the
/// extra operations arrive silently and correctly-shaped.
fn path_has_prefix(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        return true;
    }
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The path segment no selection may ever reach.
///
/// Zero of the 398 operations babelforce's five documents declare carry it, which is exactly why it
/// is here: this is a guard against a *future* pull, and the moment a vendor publishes an internal
/// endpoint a broad selector would otherwise catalogue it as a supported call. Costing one check to
/// keep that impossible is the trade.
const INTERNAL_SEGMENT: &str = "internal";

/// Whether a path names the vendor's own internals.
fn is_internal(path: &str) -> bool {
    path.split('/').any(|segment| segment == INTERNAL_SEGMENT)
}

/// `listReportingCalls` → `list-reporting-calls`.
///
/// The boundary is read at the end of a run of capitals rather than at every capital, so
/// `listHTTPCalls` is `list-http-calls` and not `list-h-t-t-p-calls`. Characters that are neither
/// letters nor digits are **passed through unchanged** rather than substituted: the result is then
/// held to the `decl_name` grammar, so an `operationId` that cannot produce a legal id is reported
/// as itself instead of being silently mangled into something that parses.
fn kebab(operation_id: &str) -> String {
    let chars: Vec<char> = operation_id.chars().collect();
    let mut out = String::with_capacity(operation_id.len() + 8);
    for (index, ch) in chars.iter().copied().enumerate() {
        if !ch.is_ascii_uppercase() {
            out.push(ch);
            continue;
        }
        let follows_a_word = index > 0
            && (chars[index - 1].is_ascii_lowercase() || chars[index - 1].is_ascii_digit());
        let ends_an_acronym = index > 0
            && chars[index - 1].is_ascii_uppercase()
            && chars.get(index + 1).is_some_and(char::is_ascii_lowercase);
        if follows_a_word || ends_an_acronym {
            out.push('-');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

/// Whether `id` is a name flux can declare and `connector-pack` can project onto a tool name.
///
/// The charset is `flux_lang`'s `decl_name` grammar (C-8) — ASCII alphanumerics, `_` and `-` — and
/// the empty-level rule is `connector_pack::dotted_name`'s, because an id with a `--` in it becomes
/// a dotted tool name with an empty level. Re-stated here rather than imported: `connector-spec`
/// takes neither dependency, and this crate is where a bad id must be refused, since by the time
/// the emitter sees one the file that produced it is three layers away.
fn legal_op_id(id: &str) -> std::result::Result<(), String> {
    if id.is_empty() {
        return Err("it is empty".to_owned());
    }
    if let Some(offender) = id
        .chars()
        .find(|ch| !ch.is_ascii_alphanumeric() && *ch != '_' && *ch != '-')
    {
        return Err(format!(
            "it holds {offender:?}, and flux-lang's `decl_name` grammar admits ASCII \
             alphanumerics, `_` and `-` only"
        ));
    }
    if id.starts_with('-') || id.ends_with('-') || id.contains("--") {
        return Err(
            "it has an empty `-`-separated level, so `connector-pack` cannot project it onto a \
             dotted tool name"
                .to_owned(),
        );
    }
    Ok(())
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
    /// **Which document this patch reads** — the `service` of one `[[spec]]` entry (C-410).
    ///
    /// Absent is legal only when the file declares exactly one document, where it means that one.
    /// The moment a second is declared, every patch states this, and the reason is that `select`
    /// stops being a unique key: `getUser` is declared by babelforce's `manager-2026-07-10` **and**
    /// by its `user-2026-06-25`, as two different requests. Resolving an unqualified `select`
    /// against whichever document declared it would compile one of the two by accident and emit
    /// plausible, wrong Flux — so the loader refuses instead of choosing.
    ///
    /// It is also the [`Service`] the published operation lands in, because the two are the same
    /// statement: a document becomes a service, and a patch selects out of a document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// The spec's `operationId` this patch selects, e.g. `listReportingCalls`.
    pub select: String,
    /// Withhold this exact operation from a set selected in bulk, with the reason review needs.
    ///
    /// This is deliberately **not** operation selection by exclusion: it is legal only when a
    /// `[[patch.select]]` already matched the operation. The selector remains the positive review
    /// boundary; this field records why one member of that stated set cannot publish yet. Nothing
    /// else may be corrected on a deferred operation because no corrected operation would exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defer: Option<String>,
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
    /// States vendor-state direction on this exact stable operation identity. If the directions map
    /// also states it, the two values must agree or loading refuses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<OperationDirection>,
    /// Overrides the risk the spec implies. Specs do not carry risk, so in practice this is where
    /// risk is *stated*, not overridden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<Risk>,
    /// Overrides idempotency. As with `risk`, specs do not publish it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency: Option<Idempotency>,
    /// Semantic consequences stated by the author who reviewed this operation. A vendor document
    /// cannot infer business meaning, and selectors cannot state one value for a heterogeneous set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_effects: Option<Vec<SemanticEffect>>,
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
    /// Parameters the connector **drops** from what the document declares — C-422.
    ///
    /// Applied after [`params`](Self::params), so requiredness is judged as the connector states it
    /// rather than as the vendor guessed it: an author may correct a wrong `required` flag and then
    /// omit the parameter, and the two statements read in that order.
    #[serde(default, skip_serializing_if = "ParamOmission::is_empty")]
    pub omit: ParamOmission,
    /// Overrides whether this operation reaches a model as a tool — C-413's [`Operation::expose`].
    ///
    /// The counterpart of [`OperationSelector::expose`], and the reason both exist: a selector
    /// states the rule for a set (`expose = false` over 388 operations) and a block states the
    /// exception (`expose = true` on the curated nine). Absent means whatever the selector that
    /// matched this operation said, and exposed if none did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expose: Option<bool>,
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

/// The parameters a selected operation **drops**, named by position and then by name — C-422.
///
/// # Why this exists at all
///
/// A vendor document is written to describe an API, not to be a tool contract. babelforce's
/// `listReportingCalls` declares 38 query parameters, of which the vendor's own prose marks most as
/// aliases of the others (`fromNumber` *of* `from`, and a whole `filters.`-prefixed restatement of
/// the set). A model choosing arguments out of 38 synonyms chooses worse than one choosing out of
/// 14, and before this existed the only way back to 14 was to abandon the document and hand-write
/// the operation — which C-416 measured as the single place where hand-authoring beat patching
/// across an entire converted provider.
///
/// # Why this is not a contradiction of `Patch` having no `hide`
///
/// [`Patch`] refuses an operation-level opt-out because **selection is opt-in**: a `hide` list would
/// make every operation a vendor adds upstream a new tool by default, and the author would learn
/// about it from a model's behaviour rather than from a diff. That argument does not reach one level
/// down, and lands the opposite way, *because the operation is already selected*. An author writing
/// here has stated intent about this endpoint and is **narrowing** it — not opting out of reviewing
/// it — and a new upstream parameter still arrives in the tool by default, exactly as an operation
/// does.
///
/// # Why it is a list of names rather than a flag on `ParamPatch`
///
/// Dropping is not correcting: there is nothing else to say about a parameter that is going away, so
/// a three-line block per name would cost 51 lines to remove babelforce's 17 synonyms and hand a
/// reviewer 51 lines that all say the same thing. Grouping by position keeps the identity
/// [`ParamPatch`] uses — name **and** position, because a vendor may bind one name in two places —
/// and costs one line per group plus the names.
///
/// **Every omission is written down**, which is the property that survives regeneration: nothing
/// here is inferred from a description, a naming convention or a similarity between two parameters,
/// because none of those is a decision anybody made.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamOmission {
    /// Path parameters to drop. Permitted only when the same service declares an exact
    /// operator-pinned `path.<name>` configuration value; otherwise see [`omit`] for the refusal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<String>,
    /// Query-string parameters to drop. The synonym flood lives here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query: Vec<String>,
    /// Caller-supplied headers to drop.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header: Vec<String>,
    /// Named request-body fields to drop.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<String>,
}

impl ParamOmission {
    /// Whether the patch drops nothing.
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
            && self.query.is_empty()
            && self.header.is_empty()
            && self.body.is_empty()
    }

    /// Every omission as the pair that identifies it, in a fixed group order.
    ///
    /// The order is fixed rather than incidental because it is the order the refusals come out in,
    /// and a loader that reported the same file's problems in a different order on a different run
    /// would fail the determinism test this crate keeps.
    pub fn entries(&self) -> impl Iterator<Item = (ParamPosition, &str)> {
        [
            (ParamPosition::Path, &self.path),
            (ParamPosition::Query, &self.query),
            (ParamPosition::Header, &self.header),
            (ParamPosition::Body, &self.body),
        ]
        .into_iter()
        .flat_map(|(position, names)| names.iter().map(move |name| (position, name.as_str())))
    }
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
    /// `[spec]` **or** `[[spec]]` — C-410.
    ///
    /// One key, two TOML spellings, because a connector with one vendor document and a connector
    /// with five are the same thing at different sizes. The single-table form is the one-element
    /// case and is spelled `[spec]` forever: converting the 53 shipped providers to array syntax to
    /// buy a plural nobody asked for would be churn, and the golden errors pin the single form's
    /// messages verbatim.
    #[serde(rename = "spec", default, deserialize_with = "one_or_many_specs")]
    specs: Vec<SpecSource>,
    #[serde(default)]
    patch: Patch,
}

/// Accepts `[spec]` as a table and `[[spec]]` as an array of them, into one `Vec`.
///
/// Written as a visitor rather than as `#[serde(untagged)]` on purpose. An untagged enum buffers the
/// input and reports `data did not match any variant of untagged enum`, which throws away both the
/// `deny_unknown_fields` key list and `toml`'s line, column and source snippet — and this loader's
/// error text is a deliverable pinned by golden files. Dispatching on the visited shape keeps the
/// inner type's own error, whichever form the author wrote.
fn one_or_many_specs<'de, D>(deserializer: D) -> std::result::Result<Vec<SpecSource>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::value::{MapAccessDeserializer, SeqAccessDeserializer};

    struct OneOrMany;

    impl<'de> serde::de::Visitor<'de> for OneOrMany {
        type Value = Vec<SpecSource>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a `[spec]` table or a sequence of `[[spec]]` tables")
        }

        fn visit_map<A>(self, map: A) -> std::result::Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            SpecSource::deserialize(MapAccessDeserializer::new(map)).map(|spec| vec![spec])
        }

        fn visit_seq<A>(self, seq: A) -> std::result::Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            Vec::deserialize(SeqAccessDeserializer::new(seq))
        }
    }

    deserializer.deserialize_any(OneOrMany)
}

/// Parses and validates one `providers/<name>.toml`.
///
/// `name` is only ever used to label errors — `providers/zendesk.toml` — so the caller decides how
/// the file identifies itself. `source` is the file's bytes as text; **no IO happens here**.
///
/// The connector's [`Provenance::toml_sha256`] is computed from `source` on the way through, which
/// is what lets `connectors.lock` (C-7) detect an edited provider file without re-reading it.
///
/// # A file that pins a `[spec]` is refused here — C-421
///
/// A spec-backed connector's operations are a function of the file's bytes **and** of the vendored
/// documents it pins. This entry point is handed only the first, so on that input it is being asked
/// a question it does not have the material to answer. Use [`load_with_spec`], which takes the cache.
///
/// Until C-421 it answered anyway. It returned `Ok` with a *skeleton* — the id, the base URL, the
/// credentials, the provenance, and **zero operations** — and every caller in this workspace treated
/// that as a compiled connector. Ninety-one files call this function and eighty-six of them are
/// tests, so the first shipped provider to convert to `[spec]` would have turned the whole
/// catalogue-wide suite into a set of assertions passing vacuously over a connector they believed
/// they had checked. `AGENTS.md`'s "a loud compile-time refusal is better than plausible but
/// incorrect Flux" decides that case, and it decides it against the skeleton.
///
/// **Why the signature did not grow a `documents` parameter instead.** The alternative considered was
/// folding [`load_with_spec`] into this function, so that "load" had one meaning everywhere. It was
/// rejected on what it does to the callers who have no cache — the majority, and every unit test
/// that authors its own TOML. The only argument they could pass is an empty slice, and an empty
/// slice against a pinned `[spec]` already refuses one layer down, in `ingest_specs`, with a message
/// about a pin that resolves to nothing. So the parameter would not give "load" one meaning; it
/// would give it one signature and two meanings, the second spelled `&[]`, and it would put a
/// vestigial argument on roughly forty golden-error tests that will never own a document. Keeping
/// the pure entry point pure and making it *say* what it is missing costs one refusal and no
/// argument, and it leaves the fifty-three hand-authored providers loading byte-identically.
///
/// The split callers face is therefore one sentence: **bytes you read from `providers/` go through
/// [`load_with_spec`] with that provider's cache; TOML you authored yourself goes through here.**
///
/// # Errors
///
/// [`Error::ParseProvider`](crate::Error::ParseProvider) when the file is not well-formed TOML or
/// does not match the schema, and [`Error::InvalidProvider`](crate::Error::InvalidProvider) — with
/// *every* problem found, not just the first — when it parses but is not a valid connector, or when
/// it pins a `[spec]` and so cannot be compiled without one.
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
/// # A connector may pin several documents, one per service — C-410
///
/// `[[spec]]` declares a document per [`Service`], and `[spec]` is the one-element case of it. Each
/// entry is resolved, hash-checked and ingested **separately**, and its selected operations join the
/// service the entry names; nothing is merged. That is what lets babelforce be one connector rather
/// than five, and it is also what keeps the manager document's root `oauth2` from describing
/// `task-automation`'s per-operation `bearerAuth`.
///
/// Because two documents may declare one `operationId` — babelforce's `getUser` does — a
/// [`OperationPatch`] states which `service` it reads from as soon as a second document exists.
///
/// # The declared `sha256` is checked against the bytes, not copied past them
///
/// [`SpecSource::sha256`] reaches [`Provenance::specs`] and from there `connectors.lock`. If nothing
/// compared it against the document actually ingested, provenance would be a claim the file makes
/// about itself — and the lockfile would record a hash for bytes it never saw. So a declared hash
/// that disagrees with the document is a refusal here, **per document**: a connector whose five
/// documents share one hash could not say which of them moved. (Comparing against *upstream* is
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
    let implicit_service_members = implicit_service_members(source);

    // Kept before `assemble` distributes it, so a provider-level constant header is reported once
    // rather than once per operation that inherited it.
    let provider_headers = file.const_headers.clone();
    let mut loaded = assemble(file, source, implicit_service_members);

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
    if !loaded.specs.is_empty() {
        match documents {
            Some(documents) => {
                ingest_specs(&mut loaded, documents, &mut problems);
                // Re-run, because selection appended operations after `assemble` distributed. The
                // pass only fills a header an operation does not already carry, so a second run over
                // the inline ones changes nothing.
                distribute_const_headers(&provider_headers, &mut loaded.connector.operations);
            }
            // **No cache was supplied at all, so this file cannot be compiled here** — C-421. See
            // [`load`] for why this is a refusal rather than the skeleton it used to be.
            None => problems.push(no_spec_cache(&loaded.specs)),
        }
    }

    // A semantic-effect list is a set. Canonicalising it before validation makes equivalent input
    // hash and emit identically; duplicates remain adjacent so the validator can refuse them rather
    // than silently absorbing an authoring error.
    for operation in &mut loaded.connector.operations {
        operation.semantic_effects.sort_unstable();
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

/// The refusal [`load`] answers a spec-backed file with — C-421.
///
/// Written to be actionable from the message alone, because the reader is as likely to be an author
/// wondering why their connector is empty as a caller who picked the wrong function: it names every
/// document the file pins, so the cache to assemble is legible, and it names [`load_with_spec`], so
/// the fix is one identifier away.
fn no_spec_cache(specs: &[SpecSource]) -> String {
    let many = specs.len() > 1;
    let pinned: Vec<String> = specs
        .iter()
        .map(|spec| format!("{:?}", spec.path.trim()))
        .collect();

    format!(
        "`{}` pins {}, so this connector's operations are a function of {} as well as of this file \
         — and `provider::load` was given no spec cache to resolve {} against. Load a spec-backed \
         provider with `provider::load_with_spec`, handing it every document under \
         `specs/<provider>/`.",
        block(many),
        pinned.join(", "),
        if many {
            "those documents"
        } else {
            "that document"
        },
        if many { "them" } else { "it" },
    )
}

/// Ingest every vendored document the file pins and publish the operations the patch set selects.
///
/// Everything here is a *statement the author made*: which documents to compile, which operations of
/// each to publish, what to call each one, how risky it is. Nothing is inferred from a document,
/// because the three fields an `Operation` needs that a specification never carries — the op id,
/// [`Risk`] and [`Idempotency`] — are the three this repository refuses to decide by silence.
///
/// # Each document is ingested on its own — C-410
///
/// One [`IngestedDocument`] per `[[spec]]` entry, keyed by the service the entry names. Nothing is
/// merged into a single "the ingest", because merging is how one document's security model would
/// come to describe another's: babelforce's manager document declares root `oauth2` and **zero**
/// operation overrides, while `task-automation` declares `bearerAuth`+`oauth2` on all 31 of its
/// operations. Whichever was folded in last would have spoken for both.
fn ingest_specs(
    loaded: &mut LoadedProvider,
    documents: &[SpecDocument<'_>],
    problems: &mut Vec<String>,
) {
    let specs = loaded.specs.clone();
    let many = specs.len() > 1;

    // **The pin, resolved — once per entry.** `specs/<provider>/` ordinarily holds more files than a
    // connector compiles: versions of one document beside the documents of another service. Only a
    // `[[spec]] path` says which of them this connector is built from. Reading whichever happened to
    // sort last is precisely the defect `Provider::spec()` carried, and it compiled an operation out
    // of a document the provider file never named, successfully and silently.
    let mut ingested: Vec<IngestedDocument> = Vec::new();
    for spec in &specs {
        let path = spec.path.clone();
        let Some(found) = documents
            .iter()
            .find(|candidate| candidate.path == path.trim())
        else {
            problems.push(format!(
                "`{} path = {path:?}` names no vendored document. {}",
                block(many),
                describe_cache(documents)
            ));
            continue;
        };
        let document = found.document;

        // **Provenance is checked, not copied, and it is checked per document.** `sha256` travels
        // from here into `connectors.lock`; a value nothing compared against the ingested bytes
        // would be the file's claim about itself, recorded as though it were a measurement. One hash
        // for a five-document connector could not say *which* document moved, which is the only
        // question a drift check is asked. Checking against upstream is C-14's — this is the local
        // claim against the local bytes.
        if let Some(declared) = spec
            .sha256
            .as_deref()
            .map(str::trim)
            .filter(|hash| !hash.is_empty())
        {
            let measured = sha256_hex(document.as_bytes());
            if !declared.eq_ignore_ascii_case(&measured) {
                problems.push(format!(
                    "`{} sha256` declares {declared:?}, but {path} hashes to {measured:?}. The \
                     declared value reaches `connectors.lock`, so a build that ignored the \
                     difference would record a hash for bytes it never read — re-vendor the \
                     document or correct the declaration",
                    block(many)
                ));
                continue;
            }
        }

        match crate::openapi::ingest(document) {
            Ok(document) => ingested.push(IngestedDocument {
                path,
                service: spec.service().to_owned(),
                ingested: document,
            }),
            Err(error) => problems.push(format!("`{} path = {path:?}`: {error}", block(many))),
        }
    }

    let (selected, operation_specs) = publish(
        &loaded.patch,
        &specs,
        &ingested,
        documents,
        &loaded.connector.config,
        problems,
    );
    loaded.connector.operations.extend(selected);
    loaded
        .connector
        .provenance
        .operation_specs
        .extend(operation_specs);
    loaded.ingested = ingested;
}

/// How to spell the block an author would go and edit — `[spec]` or `[[spec]]`.
///
/// A refusal that named the array form to someone who wrote a single table sends them looking for a
/// key they did not write; the two forms are one field, so the message follows the file.
fn block(many: bool) -> &'static str {
    if many {
        "[[spec]]"
    } else {
        "[spec]"
    }
}

/// Which ingested document one `[[patch.operations]]` block reads from — C-410.
///
/// The rule is one sentence: **a patch names its document as soon as there is more than one.** With
/// a single `[[spec]]` entry the answer is that entry, which is what keeps every single-`[spec]`
/// file loading exactly as it did. With several, an unqualified `select` is refused rather than
/// resolved, because `getUser` is declared by babelforce's `manager-2026-07-10` *and* by its
/// `user-2026-06-25` as two different requests — and a rule that searched the documents in order
/// would compile one of them by accident, exit 0, and be invisible until someone called it.
fn resolve_document<'a>(
    ingested: &'a [IngestedDocument],
    specs: &[SpecSource],
    service: Option<&str>,
    subject: &str,
    problems: &mut Vec<String>,
) -> Option<&'a IngestedDocument> {
    let Some(service) = service.map(str::trim) else {
        if specs.len() == 1 {
            // Present unless *that* document failed to resolve or ingest, which is already reported.
            return ingested.first();
        }
        problems.push(format!(
            "{subject} states no `service`, but this connector declares {} vendored documents \
             ({}). Two documents may declare one `operationId` — babelforce's `getUser` is in both \
             `manager` and `user` — so a `select` alone does not name an operation; state the \
             `service` whose document this patch reads",
            specs.len(),
            declared_services(specs)
        ));
        return None;
    };

    if let Some(document) = ingested.iter().find(|entry| entry.service == service) {
        return Some(document);
    }

    // A `service` that no `[[spec]]` entry names is a typo or a document that was removed from
    // under the patch. Silently selecting nothing is the rot `select` is already loud about.
    if !specs.iter().any(|spec| spec.service() == service) {
        problems.push(format!(
            "{subject} names service {service:?}, which no `[[spec]]` entry declares. The \
             documents this connector compiles are: {}",
            declared_services(specs)
        ));
    }
    None
}

/// The services the file's `[[spec]]` entries name, for a refusal that has to list them.
fn declared_services(specs: &[SpecSource]) -> String {
    specs
        .iter()
        .map(SpecSource::service)
        .collect::<Vec<_>>()
        .join(", ")
}

/// **The whole overlay: select, then patch, then publish** — C-6, widened by C-411/412/414.
///
/// The order is [`Patch`]'s and it is total. Selectors state what they state about the sets they
/// matched; a `[[patch.operations]]` block overrides that field by field for the one operation it
/// names; and everything neither statement covers falls to the per-field rules, each of which either
/// has a value nobody can get wrong or refuses.
///
/// Returns the operations to publish. Every failure is a pushed problem and a skipped operation
/// rather than an early return, so a file with fifty bad statements reports fifty lines.
fn publish(
    patch: &Patch,
    specs: &[SpecSource],
    ingested: &[IngestedDocument],
    documents: &[SpecDocument<'_>],
    config: &[ConfigField],
    problems: &mut Vec<String>,
) -> (Vec<Operation>, BTreeMap<String, OperationSpecSource>) {
    if let Some(naming) = patch.naming.as_ref() {
        check_pins(naming, ingested, problems);
    }
    check_directions(&patch.directions, ingested, problems);
    let naming = patch.naming.as_ref();

    // **2 · select.** What every selector states about every operation it matched, merged — and a
    // disagreement between two of them refused here rather than resolved by declaration order.
    let mut matched: BTreeMap<(&str, &str), Stated> = BTreeMap::new();
    for selector in &patch.select {
        let subject = selector.describe();
        let Some(document) = resolve_document(
            ingested,
            specs,
            selector.service.as_deref(),
            &subject,
            problems,
        ) else {
            continue;
        };

        let mut hits = 0usize;
        for operation in &document.ingested.operations {
            if !selector.matches(operation) {
                continue;
            }
            // **Matched but not eligible.** A bulk statement never asked for the vendor's
            // internals, so sweeping one up is silent; naming one by hand is a different act and is
            // refused below. Counted out of `hits` so a selector that reached *only* internal paths
            // still reports as matching nothing.
            if is_internal(&operation.path) {
                continue;
            }
            hits += 1;
            matched
                .entry((document.service.as_str(), operation.operation_id.as_str()))
                .or_default()
                .absorb(selector, &subject, &operation.operation_id, problems);
        }

        if hits == 0 {
            problems.push(format!(
                "{subject} matches no operation in {}. A selector that selects nothing is refused \
                 for the same reason a `select` naming an absent `operationId` is: a prefix that \
                 stopped matching after an upstream reshuffle would empty this connector quietly \
                 and the build would stay green",
                document.path
            ));
        }
    }

    // **3 · per-operation patch.** File order, and it wins field by field over any selector that
    // also matched — which is why the selector's statement is looked up rather than discarded.
    let mut published: Vec<Operation> = Vec::new();
    let mut operation_specs: BTreeMap<String, OperationSpecSource> = BTreeMap::new();
    let mut taken: BTreeMap<String, Claim> = BTreeMap::new();
    let mut claimed: BTreeSet<(&str, &str)> = BTreeSet::new();

    for block in &patch.operations {
        let select = block.select.as_str();
        let subject = format!("patch for {select:?}");
        let Some(document) = resolve_document(
            ingested,
            specs,
            block.service.as_deref(),
            &subject,
            problems,
        ) else {
            continue;
        };
        let Some(spec) = document.ingested.operation(select) else {
            // Loud rather than a silent no-op, because a `select` that quietly matches nothing is
            // how a patch set rots underneath a vendor's rename: the operation disappears from the
            // connector and the build stays green.
            problems.push(format!(
                "`[[patch.operations]] select = {select:?}` names no `operationId` in {}. {}",
                document.path,
                nearest(&document.ingested, select)
            ));
            continue;
        };
        claimed.insert((document.service.as_str(), select));

        if is_internal(&spec.path) {
            problems.push(format!(
                "`[[patch.operations]] select = {select:?}` names an operation whose path {:?} \
                 carries an `internal` segment. An endpoint a vendor keeps behind that word is not \
                 a supported call, so it is selectable neither in bulk nor by name",
                spec.path
            ));
            continue;
        }

        let stated = matched.get(&(document.service.as_str(), select));
        let reviewed_direction = direction_for(patch, &document.service, select);
        if let Some(reason) = block.defer.as_deref() {
            let mut incompatible = Vec::new();
            if block.rename.is_some() {
                incompatible.push("rename");
            }
            if block.description.is_some() {
                incompatible.push("description");
            }
            if block.risk.is_some() {
                incompatible.push("risk");
            }
            if block.idempotency.is_some() {
                incompatible.push("idempotency");
            }
            if block.auth.is_some() {
                incompatible.push("auth");
            }
            if block.quirks.is_some() {
                incompatible.push("quirks");
            }
            if !block.params.is_empty() {
                incompatible.push("params");
            }
            if !block.omit.is_empty() {
                incompatible.push("omit");
            }
            if block.expose.is_some() {
                incompatible.push("expose");
            }

            if stated.is_none() {
                problems.push(format!(
                    "`[[patch.operations]] select = {select:?}` uses `defer`, but no \
                     `[[patch.select]]` matched that operation. Deferral may only narrow an \
                     explicitly selected set; it is not an opt-out selection mechanism"
                ));
            }
            if reason.trim().is_empty() {
                problems.push(format!(
                    "`[[patch.operations]] select = {select:?}` uses `defer` without a non-empty \
                     reason. A withheld operation must say what model or prerequisite keeps it out"
                ));
            }
            if !incompatible.is_empty() {
                problems.push(format!(
                    "`[[patch.operations]] select = {select:?}` defers the operation and also \
                     states {}, but corrections to an operation that will not publish have no \
                     effect. Keep only `service`, `select` and `defer`",
                    incompatible.join(", ")
                ));
            }
            continue;
        }
        let source = source_of(specs, document);
        if let Some((operation, claim)) = compose(
            document,
            spec,
            ComposeOverlay {
                patch: Some(block),
                reviewed_direction,
                selected: stated.is_some(),
                stated: stated.unwrap_or(&Stated::EMPTY),
                naming,
            },
            &mut ComposeContext { config, problems },
        ) {
            offer(
                &mut taken,
                &mut published,
                &mut operation_specs,
                operation,
                claim,
                operation_source(source, document, documents, select),
                problems,
            );
        }
    }

    // Everything a selector matched that no block named, in document order per `[[spec]]` entry —
    // so the published order is a function of the inputs and of nothing else.
    for document in ingested {
        let source = source_of(specs, document);
        for spec in &document.ingested.operations {
            let key = (document.service.as_str(), spec.operation_id.as_str());
            if claimed.contains(&key) {
                continue;
            }
            let Some(stated) = matched.get(&key) else {
                continue;
            };
            let reviewed_direction = direction_for(patch, &document.service, &spec.operation_id);
            if let Some((operation, claim)) = compose(
                document,
                spec,
                ComposeOverlay {
                    patch: None,
                    reviewed_direction,
                    selected: true,
                    stated,
                    naming,
                },
                &mut ComposeContext { config, problems },
            ) {
                offer(
                    &mut taken,
                    &mut published,
                    &mut operation_specs,
                    operation,
                    claim,
                    operation_source(source, document, documents, &spec.operation_id),
                    problems,
                );
            }
        }
    }

    (published, operation_specs)
}

fn direction_for(patch: &Patch, service: &str, operation_id: &str) -> Option<OperationDirection> {
    patch
        .directions
        .get(service)
        .and_then(|directions| directions.get(operation_id))
        .copied()
}

fn check_directions(
    directions: &BTreeMap<String, BTreeMap<String, OperationDirection>>,
    ingested: &[IngestedDocument],
    problems: &mut Vec<String>,
) {
    for (service, operations) in directions {
        let Some(document) = ingested
            .iter()
            .find(|document| &document.service == service)
        else {
            problems.push(format!(
                "`[patch.directions.{service}]` names no ingested service. Direction is keyed by \
                 stable service and vendor `operationId`; available services: {}",
                ingested
                    .iter()
                    .map(|document| document.service.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            continue;
        };
        for operation_id in operations.keys() {
            if document.ingested.operation(operation_id).is_none() {
                problems.push(format!(
                    "`[patch.directions.{service}]` names no `operationId` {operation_id:?} in {}. \
                     A renamed or removed upstream operation must be reviewed rather than silently \
                     losing its direction",
                    document.path
                ));
            }
        }
    }
}

/// The exact pin that produced one ingested document.
///
/// Both path and service participate. A service-only lookup would recreate C-481's defect for a
/// mixed or multi-document provider. [`ingest_specs`] establishes this pair when it resolves each
/// [`SpecSource`], so absence here is an internal invariant failure rather than provider input.
fn source_of<'a>(specs: &'a [SpecSource], document: &IngestedDocument) -> &'a SpecSource {
    specs
        .iter()
        .find(|source| source.path == document.path && source.service() == document.service)
        .expect("every ingested document came from one exact SpecSource")
}

/// Public operation provenance projected from one pin, with no local refresh metadata.
fn operation_source(
    source: &SpecSource,
    ingested: &IngestedDocument,
    documents: &[SpecDocument<'_>],
    operation_id: &str,
) -> OperationSpecSource {
    let document = documents
        .iter()
        .find(|document| document.path == ingested.path)
        .expect("every ingested document came from one provided document");
    OperationSpecSource {
        operation_id: operation_id.to_owned(),
        source_url: source.source_url.clone(),
        upstream_version: ingested.ingested.upstream_version.clone(),
        sha256: sha256_hex(document.document.as_bytes()),
    }
}

/// What the selectors that matched one operation stated about it, and which one stated each field.
///
/// The second half of each pair is what makes a disagreement reportable: "two selectors disagree"
/// is not actionable, and "`path_prefix = "/api/v2/agents"` and `path_prefix =
/// "/api/v2/agents/{id}"` disagree about `risk`" is.
#[derive(Debug, Clone, Default)]
struct Stated {
    risk: Option<(Risk, String)>,
    idempotency: Option<(Idempotency, String)>,
    expose: Option<(bool, String)>,
}

/// The provider facts one operation composition needs beyond the overlay statements themselves.
struct ComposeContext<'a> {
    config: &'a [ConfigField],
    problems: &'a mut Vec<String>,
}

/// The identity-stable and selector-authored declarations applied to one operation.
struct ComposeOverlay<'a> {
    patch: Option<&'a OperationPatch>,
    reviewed_direction: Option<OperationDirection>,
    selected: bool,
    stated: &'a Stated,
    naming: Option<&'a Naming>,
}

impl Stated {
    /// What a selector states about an operation no selector matched: nothing.
    const EMPTY: Self = Self {
        risk: None,
        idempotency: None,
        expose: None,
    };

    /// Fold one more selector's statement in, reporting any field the two disagree about.
    fn absorb(
        &mut self,
        selector: &OperationSelector,
        subject: &str,
        operation_id: &str,
        problems: &mut Vec<String>,
    ) {
        agree(
            &mut self.risk,
            selector.risk,
            risk_word,
            "risk",
            subject,
            operation_id,
            problems,
        );
        agree(
            &mut self.idempotency,
            selector.idempotency,
            idempotency_word,
            "idempotency",
            subject,
            operation_id,
            problems,
        );
        agree(
            &mut self.expose,
            selector.expose,
            bool_word,
            "expose",
            subject,
            operation_id,
            problems,
        );
    }
}

/// Merge one field of one selector's statement into what is already held for an operation.
///
/// Silence is not disagreement — a selector that states nothing about `risk` is not fighting with
/// one that does, it is simply saying less. Two *stated* values that differ are refused, because
/// picking one would make the merge order depend on the order the selectors happen to be written
/// in, and an author would have no way to see which won.
fn agree<T: PartialEq + Copy>(
    held: &mut Option<(T, String)>,
    stated: Option<T>,
    word: fn(T) -> &'static str,
    field: &str,
    subject: &str,
    operation_id: &str,
    problems: &mut Vec<String>,
) {
    let Some(value) = stated else {
        return;
    };
    match held {
        Some((existing, first)) if *existing != value => problems.push(format!(
            "two selectors match {operation_id:?} and disagree about `{field}`: {first} states \
             {:?} and {subject} states {:?}. Overlapping selectors are legal only while they \
             agree — two statements fighting over one operation is how the merge order stops being \
             total",
            word(*existing),
            word(value)
        )),
        Some(_) => {}
        None => *held = Some((value, subject.to_owned())),
    }
}

/// Where a published op id came from, for a collision that has to explain itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdSource {
    /// A `[[patch.operations]] rename`.
    Renamed,
    /// A `[patch.naming.pin]` entry.
    Pinned,
    /// The `[patch.naming]` rule.
    Derived,
}

/// One published op id and the statement that produced it.
#[derive(Debug, Clone)]
struct Claim {
    service: String,
    operation_id: String,
    source: IdSource,
}

/// Publish an operation unless its op id is already claimed — **collisions refuse**.
///
/// An op id is what a user or a model calls by name, so two operations deriving one id cannot be
/// resolved by order: whichever lost would still be documented, still be catalogued, and simply be
/// unreachable. The colliding operation is dropped rather than published so that
/// [`validate_operations`] does not report the same cause a second time in different words.
///
/// Two **authored** `rename`s colliding is left alone: `validate_patch` already reports that case
/// with a message about the `rename` key, which is the one an author would go and edit.
fn offer(
    taken: &mut BTreeMap<String, Claim>,
    published: &mut Vec<Operation>,
    operation_specs: &mut BTreeMap<String, OperationSpecSource>,
    operation: Operation,
    claim: Claim,
    spec_source: OperationSpecSource,
    problems: &mut Vec<String>,
) {
    if let Some(first) = taken.get(&operation.id) {
        if first.source == IdSource::Renamed && claim.source == IdSource::Renamed {
            operation_specs.insert(operation.id.clone(), spec_source);
            published.push(operation);
            return;
        }
        problems.push(format!(
            "op id {:?} is claimed twice: by `operationId` {:?} in service {:?} and by {:?} in \
             service {:?}. An op id is the public name users and models call, so two operations \
             deriving one is refused rather than resolved by order — pin one of them with \
             `[patch.naming.pin]`, or rename it with a `[[patch.operations]]` block, which states \
             its `service` and outranks a pin",
            operation.id, first.operation_id, first.service, claim.operation_id, claim.service
        ));
        return;
    }
    taken.insert(operation.id.clone(), claim);
    operation_specs.insert(operation.id.clone(), spec_source);
    published.push(operation);
}

/// One ingested operation plus everything stated about it, or a problem saying why not.
///
/// This is where the three declarations meet, and every field resolves by the same sentence:
/// **the block, then the selector, then the rule for that field.**
fn compose(
    document: &IngestedDocument,
    spec: &crate::openapi::SpecOperation,
    overlay: ComposeOverlay<'_>,
    context: &mut ComposeContext<'_>,
) -> Option<(Operation, Claim)> {
    let ComposeOverlay {
        patch,
        reviewed_direction,
        selected,
        stated,
        naming,
    } = overlay;
    let config = context.config;
    let problems = &mut *context.problems;
    let select = spec.operation_id.as_str();

    // **Naming: `rename`, then a pin, then the rule.** An op id is a public contract users and
    // models call by name and `operationId` is a volatile vendor field, so nothing here promotes
    // one into the other by silence — `docs/designs/connector-pipeline.md`, "Op naming is a public
    // contract". C-412 makes the *pinned override* bulk; it does not remove the requirement to
    // decide.
    let (id, source) = match patch.and_then(|patch| patch.rename.clone()) {
        Some(rename) => (rename, IdSource::Renamed),
        None => match naming {
            Some(naming) => match naming.derive(select) {
                Ok(id) => (
                    id,
                    if naming.pin.contains_key(select) {
                        IdSource::Pinned
                    } else {
                        IdSource::Derived
                    },
                ),
                Err(reason) => {
                    problems.push(format!(
                        "`operationId` {select:?} in {} derives no legal op id: {reason}. A name a \
                         user calls is never mangled into one that happens to parse — pin this \
                         operation with `[patch.naming.pin]`, or select it with a \
                         `[[patch.operations]]` block that states `rename`",
                        document.path
                    ));
                    return None;
                }
            },
            None if patch.is_some() => {
                problems.push(format!(
                    "patch for {select:?} states no `rename`. An op id is a public name that users \
                     and models call, and `operationId` is a volatile vendor field, so ingest will \
                     not promote one into one — state `rename`, or declare a `[patch.naming]` rule"
                ));
                return None;
            }
            None => {
                problems.push(format!(
                    "a `[[patch.select]]` matched {select:?} in {}, but this connector declares no \
                     `[patch.naming]` rule, so nothing says what to publish it as. An op id is a \
                     public name that users and models call — declare `[patch.naming]`, or select \
                     this operation with a `[[patch.operations]]` block that states `rename`",
                    document.path
                ));
                return None;
            }
        },
    };

    // **Direction: stable identity only.** An exact operation block and the service/operationId map
    // are both immune to method/path/name rematching. A selector cannot state direction.
    let exact_direction = patch.and_then(|patch| patch.direction);
    if let (Some(exact), Some(reviewed)) = (exact_direction, reviewed_direction) {
        if exact != reviewed {
            problems.push(format!(
                "{select:?} has conflicting identity-stable directions: its \
                 `[[patch.operations]]` block says {:?}, while \
                 `[patch.directions.{}]` says {:?}",
                exact.word(),
                document.service,
                reviewed.word()
            ));
            return None;
        }
    }
    let direction = exact_direction.or(reviewed_direction);
    let Some(direction) = direction else {
        problems.push(format!(
            "{select:?} states no `direction`. HTTP method, operation name, description, risk, \
             idempotency, semantic effects and exposure cannot prove whether vendor state changes \
             — state it under `[patch.directions.{}]` keyed by this vendor `operationId`, or on its \
             exact `[[patch.operations]]` block",
            document.service
        ));
        return None;
    };

    // **Risk and idempotency: the block, then the selector, then authored direction.** See
    // [`OperationSelector::idempotency`] for why the last step exists on a read and refuses on a
    // write, and why that asymmetry is the safe direction rather than a convenience.
    let risk = patch
        .and_then(|patch| patch.risk)
        .or(stated.risk.as_ref().map(|(value, _)| *value));
    let idempotency = patch
        .and_then(|patch| patch.idempotency)
        .or(stated.idempotency.as_ref().map(|(value, _)| *value));
    let mutating = direction == OperationDirection::Write;

    let (risk, idempotency) = match (risk, idempotency) {
        (Some(risk), Some(idempotency)) => (risk, idempotency),
        // A read a selector matched takes the two values a read cannot have wrong. This is the
        // only default in the whole overlay, and it is available only to an authored read.
        (risk, idempotency) if !mutating && selected => (
            risk.unwrap_or(Risk::Low),
            idempotency.unwrap_or(Idempotency::Idempotent),
        ),
        (risk, idempotency) => {
            let missing = match (risk, idempotency) {
                (None, Some(_)) => "`risk`",
                (Some(_), None) => "`idempotency`",
                _ => "`risk` and no `idempotency`",
            };
            problems.push(if selected {
                format!(
                    "{select:?} is an authored {} and states no {missing}. No OpenAPI document \
                     publishes either, and silence about damage on a write is \
                     refused rather than defaulted to `low` — state it on the `[[patch.select]]` \
                     that matched this operation, or on a `[[patch.operations]]` block for it",
                    direction.word()
                )
            } else {
                format!(
                    "patch for {select:?} states no {missing}. No OpenAPI document publishes \
                     either, so a selected operation states both or is not published; guessing on \
                     the operation's behalf is how a `retry` turns one charge into three and how a \
                     delete is waved through an approval gate"
                )
            });
            return None;
        }
    };

    let mut params = spec.params.clone();
    if let Some(patch) = patch {
        for correction in &patch.params {
            correct(&mut params, correction, select, problems);
        }
        // **Corrections first, then omissions**, because the omission rules read the corrected
        // parameter: `required` is refused as the *connector* states it, not as the vendor guessed
        // it. A document that marks a filter required when it is not would otherwise pin that
        // argument into the tool with no way out, and the way out has to stay a written statement —
        // correct the flag, then drop the parameter.
        for (position, name) in patch.omit.entries() {
            omit(
                &mut params,
                position,
                name,
                select,
                &document.service,
                config,
                problems,
            );
        }
    }

    let operation = Operation {
        id,
        // **The document decides the service, not the patch's own opinion of it** — C-410. A
        // `[[spec]]` entry becomes a service and a patch selects out of a document, so the two
        // statements are one and cannot disagree. Before C-410 every selected operation landed in
        // `DEFAULT_SERVICE`, which made a provider declaring named services beside a `[spec]` a loud
        // load error and a single-document one the only shape that worked.
        service: document.service.clone(),
        method: spec.method,
        direction,
        path: spec.path.clone(),
        description: patch
            .and_then(|patch| patch.description.clone())
            .unwrap_or_else(|| spec.description.clone()),
        risk,
        idempotency,
        semantic_effects: patch
            .and_then(|patch| patch.semantic_effects.clone())
            .unwrap_or_default(),
        // **Never stated in bulk.** A selector may declare `idempotency = "conditional"`, and each
        // matched write then still owes the condition C-186 requires — which arrives here as `None`
        // and is refused, by name, by `validate_repeatability_condition`. One sentence about 54
        // endpoints is not a condition, so there is no key here for a selector to write it in.
        repeatable_because: None,
        auth: patch.and_then(|patch| patch.auth.clone()),
        params,
        response_schema: spec.response_schema.clone(),
        // **A vendor document cannot make this claim, and no `[[patch.operations]]` key writes it
        // either** (C-430). "This response field is a credential" is a judgement about what a value
        // *is*; a document that returns a token describes it as a string like any other, which is
        // precisely how postmark's `ApiTokens` and zoom's `start_url` shipped. So the spec route
        // lands `[]` and an author who finds one states it in a `[[operations]]` block, where the
        // gate refuses it and a reviewer reads the reason beside it.
        credential_response: Vec::new(),
        // **And a vendor document cannot make this one either** (C-136). "This call's purpose is to
        // mint a credential" is a judgement about what an endpoint is *for*, and a token endpoint is
        // described by its document as an operation returning a JSON object like any other. There is
        // no `[[patch.operations]]` key for it for the same reason `credential_response` has none:
        // the declaration belongs beside the reviewer who read the vendor's own documentation, in a
        // `[[operations]]` block.
        produces_credential: None,
        quirks: patch
            .and_then(|patch| patch.quirks.clone())
            .unwrap_or_default(),
        // **The block, then the selector, then the field's own default** — which is exposed, so a
        // connector nobody said anything about behaves exactly as it did before C-413. `exposed()`
        // rather than a bare `true` so the spec route and the file route take one default from one
        // place and cannot drift into a catalogue nobody re-reads.
        expose: patch
            .and_then(|patch| patch.expose)
            .or(stated.expose.as_ref().map(|(value, _)| *value))
            .unwrap_or_else(crate::ir::exposed),
    };

    Some((
        operation,
        Claim {
            service: document.service.clone(),
            operation_id: select.to_owned(),
            source,
        },
    ))
}

/// Every `[patch.naming.pin]` entry names an operation, and names exactly one — C-412.
///
/// A pin that matches nothing is the rot `select` is already loud about, one field over: the vendor
/// renames an `operationId`, the pin stops applying, and the op id it was holding still quietly
/// moves to whatever the rule derives. A pin that matches *two* is the C-410 problem in a key that
/// cannot carry a service — babelforce declares `getUser` in `manager` and in `user` — so it is
/// refused rather than applied to both, which would only collide one step later with a worse
/// message.
fn check_pins(naming: &Naming, ingested: &[IngestedDocument], problems: &mut Vec<String>) {
    for operation_id in naming.pin.keys() {
        let declaring: Vec<&str> = ingested
            .iter()
            .filter(|document| document.ingested.operation(operation_id).is_some())
            .map(|document| document.service.as_str())
            .collect();

        match declaring.len() {
            0 => problems.push(format!(
                "`[patch.naming.pin]` pins {operation_id:?}, which no vendored document declares. \
                 A pin that matches nothing is how a public name rots underneath a vendor's \
                 rename: the pin stops applying, the rule derives a different id, and the build \
                 stays green"
            )),
            1 => {}
            count => problems.push(format!(
                "`[patch.naming.pin]` pins {operation_id:?}, which {count} of this connector's \
                 documents declare ({}). An `operationId` is unique inside one document and \
                 nowhere else, so one pin cannot say which of them it means — name the one you \
                 mean with a `[[patch.operations]]` block, which states its `service` and outranks \
                 a pin",
                declaring.join(", ")
            )),
        }
    }
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

/// Drop one parameter a [`ParamOmission`] names from a selected operation — C-422.
///
/// Three refusals, and each is the same sentence pointed somewhere different: **an omission may only
/// drop a parameter the request can still be composed without, and only one the document actually
/// declares.**
///
/// - **A name the document does not declare there** is a problem rather than a no-op, for the reason
///   [`correct`] gives one: the vendor renames a parameter, the line that used to drop it stops
///   applying, and the argument this connector removed on purpose is silently back in the tool with
///   the build green. It is also what catches a name listed twice — the second lookup finds nothing,
///   because the first one removed it.
/// - **A required parameter** composes a request the vendor rejects. Every other consequence of
///   omission is a wider or narrower tool; this one is a runtime failure, so it is the case where
///   silence would be actively unsafe rather than merely unhelpful. Judged *after* corrections, so
///   an author who believes the vendor's flag is wrong says so in `params` and is then free to drop
///   it.
/// - **A path parameter without an exact configuration pin.** The path template keeps its
///   placeholder, so dropping `id` from `/tickets/{id}` leaves a URL nothing can fill — unless the
///   operation's service declares `path.id`, in which case omission is what prevents the same tenant
///   scope from also being a caller argument. The pin is exact and service-scoped; no name or service
///   inference is performed.
fn omit(
    params: &mut ParamSet,
    position: ParamPosition,
    name: &str,
    select: &str,
    service: &str,
    config: &[ConfigField],
    problems: &mut Vec<String>,
) {
    let group = match position {
        ParamPosition::Path => &mut params.path,
        ParamPosition::Query => &mut params.query,
        ParamPosition::Header => &mut params.header,
        ParamPosition::Body => &mut params.body,
    };
    let Some(index) = group.iter().position(|param| param.name == name) else {
        problems.push(format!(
            "patch for {select:?} omits a `{position:?}` parameter named {name:?}, which the \
             vendored spec does not declare there"
        ));
        return;
    };
    if position == ParamPosition::Path {
        let pinned = config
            .iter()
            .filter(|field| field.service == service)
            .any(|field| {
                field
                    .pins()
                    .iter()
                    .any(|pin| pin.position == Position::Path && pin.name == name)
            });
        if pinned {
            group.remove(index);
            return;
        }
        problems.push(format!(
            "patch for {select:?} omits the path parameter {name:?}, which cannot be dropped: the \
             path template still carries `{{{name}}}` and nothing composes a URL with that left in \
             it. A path parameter leaves only when the path does, or when this service declares an \
             exact `path.{name}` configuration pin"
        ));
        return;
    }
    if group[index].required {
        problems.push(format!(
            "patch for {select:?} omits {name:?}, which the vendored spec declares **required** — \
             dropping it composes a request the vendor rejects. If the vendor's flag is wrong, \
             correct it with a `[[patch.operations.params]]` block stating `required = false` and \
             then omit it"
        ));
        return;
    }
    group.remove(index);
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
fn assemble(
    file: ProviderFile,
    source: &str,
    implicit_service_members: Vec<ImplicitServiceMember>,
) -> LoadedProvider {
    let specs = file.specs;
    let mut operations = file.operations;
    distribute_const_headers(&file.const_headers, &mut operations);
    // **The four scalar fields describe a connector, so they are filled only when one document
    // describes the connector** — C-410. With several documents there is no single `sha256`,
    // `fetched_at` or `upstream_version` that is true, and filling them from the first would record
    // one document's provenance as the whole connector's. `Provenance::specs` is the per-document
    // record and is filled either way.
    let sole = specs.first().filter(|_| specs.len() == 1);
    let provenance = Provenance {
        source_url: sole.and_then(|s| s.source_url.clone()),
        upstream_version: sole.and_then(|s| s.upstream_version.clone()),
        fetched_at: sole.and_then(|s| s.fetched_at.clone()),
        spec_sha256: sole.and_then(|s| s.sha256.clone()),
        specs: specs.clone(),
        operation_specs: BTreeMap::new(),
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
        specs,
        patch: file.patch,
        // Filled by `ingest_specs` when documents were supplied; assembling reads the TOML alone.
        ingested: Vec::new(),
        implicit_service_members,
    }
}

/// Records which service-bearing TOML tables omitted `service` before serde turns that omission
/// into `default`.
///
/// The normalized IR deliberately has one spelling for the default service. That stays correct for
/// default-only connectors; C-458 adds one authoring-time distinction in a mixed connector, so the
/// loader retains only the presence bit and discards the raw TOML immediately after validation.
fn implicit_service_members(source: &str) -> Vec<ImplicitServiceMember> {
    let table: toml::Table = source
        .parse()
        .expect("ProviderFile already parsed this source as TOML");
    let mut omitted = Vec::new();

    for (key, kind, identity) in [
        ("operations", "operation", "id"),
        ("events", "event", "name"),
        ("channels", "channel binding", "name"),
        ("config", "configuration field", "name"),
        ("graphs", "graph", "name"),
    ] {
        let Some(entries) = table.get(key).and_then(toml::Value::as_array) else {
            continue;
        };
        for entry in entries.iter().filter_map(toml::Value::as_table) {
            if entry.contains_key("service") {
                continue;
            }
            omitted.push(ImplicitServiceMember {
                kind,
                name: entry
                    .get(identity)
                    .and_then(toml::Value::as_str)
                    .unwrap_or("<unnamed>")
                    .to_owned(),
            });
        }
    }

    match table.get("spec") {
        Some(toml::Value::Table(spec)) if !spec.contains_key("service") => {
            omitted.push(ImplicitServiceMember {
                kind: "spec document",
                name: spec
                    .get("path")
                    .and_then(toml::Value::as_str)
                    .unwrap_or("<unnamed>")
                    .to_owned(),
            });
        }
        Some(toml::Value::Array(specs)) => {
            for spec in specs.iter().filter_map(toml::Value::as_table) {
                if spec.contains_key("service") {
                    continue;
                }
                omitted.push(ImplicitServiceMember {
                    kind: "spec document",
                    name: spec
                        .get("path")
                        .and_then(toml::Value::as_str)
                        .unwrap_or("<unnamed>")
                        .to_owned(),
                });
            }
        }
        _ => {}
    }

    omitted
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

    // The two messages below are pinned verbatim by `tests/golden/nothing-to-generate.error` and
    // `tests/golden/patch-without-spec.error`, and they are about the *absence* of any `[spec]` —
    // which C-410 did not change. A file with no spec block has none in either spelling.
    if loaded.specs.is_empty() && connector.operations.is_empty() {
        problems.push(
            "declares neither `[spec]` nor any `[[operations]]`, so it describes no operations at \
             all. Write the operations inline for a hand-authored connector, or point `[spec]` at \
             a vendored spec and select operations with `[[patch.operations]]`"
                .to_owned(),
        );
    }
    if loaded.specs.is_empty() && !loaded.patch.is_empty() {
        // The key is the one the file actually wrote: a message about `[[patch.operations]]` sends
        // an author who only declared a selector looking for a block they never authored. The
        // `[[patch.operations]]` rendering is the golden's, byte for byte.
        problems.push(format!(
            "declares `{}` but no `[spec]`; there is nothing for the patches to apply to",
            loaded.patch.declared()
        ));
    }
    validate_specs(loaded, &mut problems);

    validate_services(connector, &mut problems);
    validate_legacy_default_members(loaded, &mut problems);
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

/// Checks the `[spec]` / `[[spec]]` declarations themselves — C-410.
///
/// Everything here is about the *set* of documents rather than about any one of them, which is why
/// it cannot live in [`ingest_specs`]: those checks must hold whether or not the cache was supplied,
/// so `load` refuses a contradictory declaration exactly as `load_with_spec` does.
///
/// # Why a document's service must be declared, and is not declared *by* the document
///
/// A `[[spec]]` entry **joins** a service; it does not create one. A service carries a description,
/// possibly its own base URL and API version, and the roles it claims — none of which an OpenAPI
/// document supplies — and it names the emitted `<provider>-<service>.flux`. Letting a `service` key
/// conjure one would make a typo a silently-emitted extra module rather than a refusal, which is the
/// rule [`validate_member_service`] already keeps for every other member kind.
fn validate_specs(loaded: &LoadedProvider, problems: &mut Vec<String>) {
    let many = loaded.specs.len() > 1;
    let available = loaded.connector.service_names();
    let mut seen_paths: Vec<&str> = Vec::new();
    let mut seen_services: Vec<&str> = Vec::new();

    for spec in &loaded.specs {
        let path = spec.path.trim();
        if path.is_empty() {
            problems.push(format!(
                "`{} path` must not be empty — it points at the vendored spec under `specs/`",
                block(many)
            ));
        } else if seen_paths.contains(&path) {
            problems.push(format!(
                "`{}` names {path:?} more than once. One document is one service, so compiling it \
                 twice would put one vendor's operations in two places with no way to say which a \
                 caller meant",
                block(many)
            ));
        } else {
            seen_paths.push(path);
        }

        if spec
            .service
            .as_deref()
            .is_some_and(|name| name.trim().is_empty())
        {
            problems.push(format!(
                "`{} service` is empty for {path:?}; omit the key to join the reserved \
                 {DEFAULT_SERVICE:?} service, or name one a `[[services]]` entry declares",
                block(many)
            ));
            continue;
        }

        // Asked of the resolved name, so two entries that both omit the key are caught: they both
        // join `default`, which is one namespace and cannot hold two documents.
        let service = spec.service();
        if !available.contains(&service) {
            problems.push(if service == DEFAULT_SERVICE {
                format!(
                    "`{}` for {path:?} names no `service`, which means the reserved \
                     {DEFAULT_SERVICE:?} service — but this provider declares named services and no \
                     `[[services]]` entry declares {DEFAULT_SERVICE:?}. Each document of a \
                     multi-service provider names one of: {}",
                    block(many),
                    available.join(", ")
                )
            } else {
                format!(
                    "`{} service = {service:?}` for {path:?} names a service no `[[services]]` \
                     entry declares. A document joins a service, it does not declare one — a \
                     service carries a description, a base URL, an API version and its roles, none \
                     of which an OpenAPI document supplies. This provider declares: {}",
                    block(many),
                    available.join(", ")
                )
            });
        }
        if seen_services.contains(&service) {
            problems.push(format!(
                "`{}` gives service {service:?} two documents. A service is one name namespace, so \
                 two documents joining it could declare one `operationId` twice with nothing to \
                 tell them apart — give each document its own service",
                block(many)
            ));
        } else {
            seen_services.push(service);
        }
    }
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

        // A whole HTTPS origin is the explicit C-402 self-managed exception: the connector owns
        // the path while deployment policy approves the scheme+authority. It is deliberately a
        // generic configuration shape, not a GitLab branch in a consumer.
        if field.format == Format::Origin {
            if field.approval != Approval::Operator {
                problems.push(format!(
                    "configuration field {name:?} declares `format = \"origin\"` without \
                     `approval = \"operator\"`. A caller-selected whole authority is an unbounded \
                     egress grant; a non-default origin becomes active only after deployment policy \
                     approves and pins it"
                ));
            }
            match field.binding() {
                Some(Binding::Endpoint { variable }) => {
                    let base_url = connector.base_url_of(&field.service);
                    let placeholder = format!("{{{variable}}}");
                    if !base_url.starts_with(&placeholder)
                        || base_url[placeholder.len()..]
                            .chars()
                            .next()
                            .is_some_and(|next| next != '/')
                    {
                        problems.push(format!(
                            "configuration field {name:?} declares an HTTPS origin but service {:?} \
                             has base URL {base_url:?}. An origin must be the entire leading \
                             endpoint placeholder (`{{{variable}}}`); the connector may append a \
                             path after it, but input may not replace that path",
                            field.service
                        ));
                    }
                }
                _ => problems.push(format!(
                    "configuration field {name:?} declares `format = \"origin\"` but does not bind \
                     an `endpoint.<variable>`. An origin is one resolved endpoint, not an operation \
                     argument or request field"
                )),
            }
        } else if field.approval == Approval::Operator {
            problems.push(format!(
                "configuration field {name:?} declares `approval = \"operator\"` but format `{}`. \
                 Operator approval on this surface is the explicit whole-HTTPS-origin policy; use \
                 `format = \"origin\"` so consumers and the runtime can enforce the same rule",
                field.format.word()
            ));
        }

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

        if field.secret && field.default.is_some() {
            problems.push(format!(
                "configuration field {name:?} declares a secret `default`; a default is a literal \
                 sent on the wire and credentials never belong in provider TOML"
            ));
        }
        if field.required && field.default.is_some() {
            problems.push(format!(
                "configuration field {name:?} is required and also declares a `default`. A value \
                 the connector can supply itself is optional; set `required = false`"
            ));
        }
        if let Some(default) = &field.default {
            if let Err(reason) = field.format.validate(default) {
                problems.push(format!(
                    "configuration field {name:?} declares a `default` that does not satisfy \
                     format `{}`: {reason}",
                    field.format.word()
                ));
            }
            if let Err(reason) = field.permits(default) {
                problems.push(format!(
                    "configuration field {name:?} declares a `default` outside its choices: {reason}"
                ));
            }
        }

        validate_choices(field, problems);
        validate_binding(connector, field, problems);
    }

    validate_every_template_variable_is_asked_for(connector, problems);
}

/// **A closed set of values is a narrowing of the field, not a second field beside it** — C-225.
///
/// `choices` answers *which values are legal*; [`Format`](crate::Format) answers *what shape a value
/// has*. Keeping them separate is what makes the rules below derivations rather than preferences:
///
/// 1. **Every permitted value satisfies the field's own `format`.** A set that could admit a value
///    the format rejects would let a closed field be *wider* than the open one it narrows, and the
///    renderer's fallback input — built from `format` — would refuse a value the select offers.
/// 2. **A set has at least two values.** A set of one is a constant: the field asks a question with
///    one answer, which belongs in the base URL rather than in front of a human. An empty
///    `choices = []` is an open field spelled the long way, and reads in a diff as a set someone
///    emptied by accident.
/// 3. **Every entry is renderable and distinguishable.** A blank label is a dropdown row with
///    nothing in it; a repeated value is one member wearing two names; a repeated label is two rows
///    a user cannot tell apart. Each of the three produces a form that cannot be answered
///    correctly, which is the same standard `label` and `help` are mandatory under.
/// 4. **A `secret` declares none.** The values would be credentials, enumerated in a committed file.
///    That is the C-231 rule about `example` in its stronger form — an example is one such literal,
///    a set is all of them — and the same push-protection and disclosure argument settles it.
/// 5. **The `example` is one of the choices.** It is the placeholder a user copies, so on a closed
///    field it has to be an answer they are allowed to give. Exactly the defect class the
///    format/example rule already refuses, one level narrower.
///
/// The sixth rule is not here: a value pinned into a request position is checked against that
/// position in [`validate_pin`], beside the `example` check it mirrors, because the rule belongs to
/// the binding rather than to the set.
fn validate_choices(field: &ConfigField, problems: &mut Vec<String>) {
    let name = field.name.as_str();
    if field.choices.is_empty() {
        // Distinguishable from "no `choices` key at all" only in the source, so an explicit empty
        // list is called out rather than silently read as an open field.
        return;
    }

    if field.secret {
        problems.push(format!(
            "configuration field {name:?} declares `secret = true` and `choices`. A closed set of \
             secret values is a list of credentials in a committed file — the same defect a \
             secret's `example` is refused for (C-231), and a stronger form of it, because a set is \
             exhaustive where an example is one literal"
        ));
        return;
    }

    if field.choices.len() < 2 {
        problems.push(format!(
            "configuration field {name:?} declares `choices` with one value. A set of one is a \
             constant, not a choice: put the value in the `base_url` (or wherever the field binds) \
             rather than asking a human to confirm the only answer"
        ));
    }

    let mut values: Vec<&str> = Vec::new();
    let mut labels: Vec<&str> = Vec::new();
    for choice in &field.choices {
        if let Err(reason) = field.format.validate(&choice.value) {
            problems.push(format!(
                "configuration field {name:?} declares `format = \"{}\"` but a choice that does not \
                 satisfy it: {reason}. A closed set narrows the field's format; it cannot widen it, \
                 or the input a renderer falls back to would reject a value the set offers",
                field.format.word()
            ));
        }
        if choice.label.trim().is_empty() {
            problems.push(format!(
                "configuration field {name:?} declares a choice {:?} with an empty `label`; a set \
                 of raw values is a dropdown nobody can read, which is why the label is the whole \
                 reason a choice is a table rather than a string",
                choice.value
            ));
        }
        if values.contains(&choice.value.as_str()) {
            problems.push(format!(
                "configuration field {name:?} lists the choice {:?} more than once; one value under \
                 two labels is a set a user cannot select from unambiguously",
                choice.value
            ));
        }
        if labels.contains(&choice.label.as_str()) {
            problems.push(format!(
                "configuration field {name:?} uses the label {:?} more than once; two rows a user \
                 cannot tell apart is a choice they cannot make",
                choice.label
            ));
        }
        values.push(&choice.value);
        labels.push(&choice.label);
    }

    if let Some(example) = &field.example {
        if let Err(reason) = field.permits(example) {
            problems.push(format!(
                "configuration field {name:?} declares an `example` that is not one of its own \
                 choices: {reason}. A placeholder a user copies and is then refused for is worse \
                 than none"
            ));
        }
    }
}

/// Checks one field's `binds`: that it parses, that it resolves, and that `secret` agrees with it.
///
/// **And each of its `also_binds`, on the same terms** (C-229). A field reaching several
/// destinations is validated once per destination rather than once per field, so "every position is
/// checked, not only the first" is how the loop is written rather than a claim beside it. The
/// per-field questions — the destination set is well-formed, and the slot collides with nothing —
/// are [`validate_destinations`] and [`validate_slot_is_not_shared`].
fn validate_binding(connector: &Connector, field: &ConfigField, problems: &mut Vec<String>) {
    validate_destinations(field, problems);
    for binds in
        std::iter::once(field.binds.as_str()).chain(field.also_binds.iter().map(String::as_str))
    {
        validate_one_binding(connector, field, binds, problems);
    }
    validate_slot_is_not_shared(connector, field, problems);
    validate_also_services(connector, field, problems);
}

/// **A shared endpoint slot names real sibling services, and only an endpoint slot may be shared**
/// (C-529).
///
/// Four refusals, each closing a way the declaration could name something without doing anything:
///
/// 1. **Only an `endpoint.` binding may be shared.** A credential or a request pin has no
///    per-service placeholder for a second service to fill, so an entry on one is a service named
///    for no effect — which reads like coverage and is not.
/// 2. **Every named service is declared.** A typo would leave the real service's `{variable}`
///    unbound, and the coverage check would then report the *other* service as the problem.
/// 3. **The head is not repeated.** `service = "default"` with `also_services = ["default"]` is one
///    slot spelled twice; harmless to resolve and a sign the author meant a different name.
/// 4. **No service is named twice.**
///
/// What this deliberately does *not* refuse is two fields reaching one service with different
/// variables — that is ordinary, and Contentful's two `space_id` fields stay two slots because they
/// share no field, not because anything here stops them.
fn validate_also_services(connector: &Connector, field: &ConfigField, problems: &mut Vec<String>) {
    if field.also_services.is_empty() {
        return;
    }
    let name = field.name.as_str();

    if !matches!(field.binding(), Some(Binding::Endpoint { .. })) {
        problems.push(format!(
            "configuration field {name:?} declares `also_services`, but binds {:?} rather than an \
             `endpoint.<variable>`. Only a base-URL placeholder exists once per service and can \
             therefore be filled for a sibling service; a credential or a request pin has no \
             per-service slot, so the entry would name a service without reaching anything there",
            field.binds
        ));
        return;
    }

    let declared = connector.service_names();
    let mut seen: Vec<&str> = Vec::new();
    for extra in &field.also_services {
        let extra = extra.as_str();
        if extra == field.service {
            problems.push(format!(
                "configuration field {name:?} lists its own service {extra:?} in `also_services`. \
                 The head `service` already carries the address; listing it again is one slot \
                 spelled twice"
            ));
            continue;
        }
        if seen.contains(&extra) {
            problems.push(format!(
                "configuration field {name:?} lists service {extra:?} twice in `also_services`"
            ));
            continue;
        }
        seen.push(extra);
        if !declared.contains(&extra) {
            problems.push(format!(
                "configuration field {name:?} lists service {extra:?} in `also_services`, which \
                 this connector does not declare. A misspelled sibling leaves the real service's \
                 base-URL placeholder unbound, and the failure would then be reported against that \
                 service rather than against this typo"
            ));
        }
    }
}

/// **The destination set itself is well-formed**, before any of its members is resolved.
///
/// Three rules, and each is a consequence of the slot being `binds`' own target:
///
/// 1. **A further destination is a request position and nothing else.** An `endpoint.` entry here
///    would be a second `{placeholder}` in a `base_url` the emitter fills from a slot that is not
///    its own, so it would arrive at the vendor as text; a `credential.`, `username.` or `oauth.`
///    entry resolves through a *different port* under a different address, which is the one thing a
///    single slot cannot be. The `endpoint.` case has a spelling that works and it is `binds`.
/// 2. **No destination is named twice.** One value written into one position twice is either a
///    duplicate the emitter drops or a header sent twice; either way the second entry says nothing
///    the first did not.
/// 3. **`also_binds` on its own means nothing** — it is only ever the tail of `binds`, so an entry
///    that fails to parse is reported against the field like `binds` is.
fn validate_destinations(field: &ConfigField, problems: &mut Vec<String>) {
    let name = field.name.as_str();
    if let Ok(head) = parse_binding(&field.binds) {
        if !matches!(head, Binding::Username { .. }) && head.target().starts_with("username.") {
            problems.push(format!(
                "configuration field {name:?} binds {:?}, whose target uses the reserved \
                 `username.` placeholder prefix. That prefix identifies the non-secret half of a \
                 Basic credential when a value also pins a request; choose a target that does not \
                 impersonate another configuration kind",
                field.binds
            ));
        }
    }
    let mut seen: Vec<&str> = vec![field.binds.as_str()];
    for binds in field.also_binds.iter().map(String::as_str) {
        if seen.contains(&binds) {
            problems.push(format!(
                "configuration field {name:?} names the destination {binds:?} twice. One collected \
                 value reaches a position once; a repeat is either dropped by the emitter or sent \
                 twice, and neither says anything the first one did not"
            ));
        }
        seen.push(binds);
        match parse_binding(binds) {
            Ok(Binding::Request { .. }) | Err(_) => {}
            Ok(other) => problems.push(format!(
                "configuration field {name:?} declares `also_binds = [… {binds:?} …]`, which is a \
                 `{}` destination. Only a request position — `path.`, `query.` or `header.` — may \
                 be a further destination: every other kind is resolved under its own address by a \
                 different port, and one collected value has exactly one address. A `base_url` \
                 variable belongs in `binds`, where it becomes the placeholder every other \
                 destination carries",
                other.kind()
            )),
        }
    }
}

/// One destination of one field: that it parses, that it resolves, and that `secret` agrees with it.
fn validate_one_binding(
    connector: &Connector,
    field: &ConfigField,
    binds: &str,
    problems: &mut Vec<String>,
) {
    let name = field.name.as_str();
    let binding = match parse_binding(binds) {
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
            // The host half of "every value is checked where it lands" (C-214/C-229), and the strict
            // one: `acme.example@evil.example` is a legal header value and a legal path segment, and
            // substituted into an authority it sends the request — and the operator's own
            // credential — to a host nobody named. See `config::validate_host_value`.
            if field.format != Format::Origin {
                validate_substituted_values(field, binding, "composes a host", problems);
            }
        }
        Binding::Request {
            position,
            name: pinned,
        } => validate_pin(connector, field, position, pinned, problems),
        Binding::ChannelQuery { channel, parameter } => {
            match connector.channel(channel) {
                None => problems.push(format!(
                    "configuration field {name:?} binds channel {channel:?}, which names no \
                     channel binding"
                )),
                Some(channel_binding) if channel_binding.service != field.service => {
                    problems.push(format!(
                        "configuration field {name:?} is in service {:?} but binds channel \
                         {channel:?} in service {:?}",
                        field.service, channel_binding.service
                    ));
                }
                Some(channel_binding) => match &channel_binding.connect {
                    None => problems.push(format!(
                        "configuration field {name:?} binds socket query parameter {parameter:?} \
                         on channel {channel:?}, which declares no generic `connect` block"
                    )),
                    Some(connect) if !connect.query.contains_key(parameter) => {
                        problems.push(format!(
                            "configuration field {name:?} binds socket query parameter \
                             {parameter:?} on channel {channel:?}, but its `connect.query` \
                             declares no such parameter"
                        ));
                    }
                    Some(connect) => {
                        let value = &connect.query[parameter];
                        if !template_variables(value).contains(&name) {
                            problems.push(format!(
                                "configuration field {name:?} binds socket query parameter \
                                 {parameter:?} on channel {channel:?}, but its value {value:?} \
                                 does not interpolate {{{name}}}"
                            ));
                        }
                    }
                },
            }
            validate_substituted_values(field, binding, "fills a socket query value", problems);
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
        Binding::OAuthClientId | Binding::OAuthClientSecret | Binding::OAuthRedirectUri => {
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
                "configuration field {name:?} binds {binds} but declares `secret = false`. That \
                 value is a credential: it must be masked on input, kept out of logs, and stored \
                 where a secret is stored"
            )
        } else {
            format!(
                "configuration field {name:?} binds {binds} but declares `secret = true`. That \
                 value is configuration, not a credential — marking it secret hides it from an \
                 operator who needs to read it back, and claims gating this repository does not \
                 provide"
            )
        });
    }
}

/// **The `example`, and every permitted choice, held to the rule of one destination they reach.**
///
/// Both are values a human will end up supplying: an `example` is the placeholder a user copies, and
/// a choice is a value the connector *invites* an operator to pick. Neither may be one the position
/// it lands in would refuse — a permitted value that escaped its path segment would be a sanctioned
/// way to address another resource on the same host with the same credential, and one that moved the
/// authority would be the same thing with a different host.
///
/// Called once per destination (C-229), which is what makes a multi-destination field satisfy
/// **every** rule rather than the first: the intersection is taken by checking each, not by picking
/// one. `did` names the destination in the refusal — "pins a header value", "composes a host" — so
/// an author told their example is illegal is also told *which* of the field's destinations refused
/// it.
fn validate_substituted_values(
    field: &ConfigField,
    binding: Binding<'_>,
    did: &str,
    problems: &mut Vec<String>,
) {
    let name = field.name.as_str();
    if let Some(example) = &field.example {
        if let Err(reason) = binding.validate_value(example) {
            problems.push(format!(
                "configuration field {name:?} {did} but gives an `example` that could not be one: \
                 {reason}"
            ));
        }
    }
    for choice in &field.choices {
        if let Err(reason) = binding.validate_value(&choice.value) {
            problems.push(format!(
                "configuration field {name:?} {did} but offers a choice that could not be one: \
                 {reason}"
            ));
        }
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
/// The addressing check that used to be the fourth is now [`validate_slot_is_not_shared`], run once
/// per field rather than once per pin: a field that reaches several positions (C-229) has one slot,
/// not one per destination, so asking the question here would have asked it several times and
/// answered it about the wire spelling rather than about the slot.
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

    // The example is what a user copies into the field, and every value a closed set permits is one
    // an operator is *invited* to pick (C-225) — so both are held to the rule this position imposes
    // on the real value, once per position the field reaches (C-229). See
    // `validate_substituted_values`.
    validate_substituted_values(
        field,
        Binding::Request {
            position,
            name: pinned,
        },
        &format!("pins a {word} value"),
        problems,
    );

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
}

/// **Two configuration fields may not share a slot, and may not share a destination** — the C-197
/// addressing rule, and the door C-229 must not reopen.
///
/// A host keys a configuration value by `(tenant, provider, service, kind, name)` and the emitted
/// module carries one `{placeholder}` per field, so two fields of one service whose slots collide are
/// **one slot** — the exact collapse C-197 found between Contentful's two spaces, where a management
/// write landed in whichever space the delivery reads were configured with. That is the refusal
/// C-164 measured and quoted: *two questions that share an answer are one question*.
///
/// **It still fires, and it means the same thing.** C-229 does not weaken it; it answers the other
/// half. One field naming two destinations is *one* question with one answer and one slot, which is
/// what the rule was protecting. Two *fields* keyed to one slot is still one field's answer silently
/// discarded, so the comparison is between slots — [`ConfigField::slot`] — and a further destination
/// is not a slot and cannot become one.
///
/// The second clause is what a further destination makes newly possible and is refused for its own
/// reason: two fields, two slots, one *wire position*. Two answers written into one header on the
/// same request is not an addressing collapse, it is a request that carries one of two values
/// depending on an order nothing declares. `connector-flux` refuses the emitted shape independently
/// (`Error::HeaderConflict`); this is the declaration-level half, so the refusal names the two
/// fields rather than an operation.
///
/// **Scope, deliberately unchanged.** It runs for a field that reaches at least one request
/// position, exactly as it did when it lived inside `validate_pin`. Two `endpoint.` fields of one
/// service sharing a variable is a shape this has never refused — Contentful ships two `space_id`
/// fields under two *different* services, which is precisely why the check is service-scoped — and
/// widening it is not this story's to do.
fn validate_slot_is_not_shared(
    connector: &Connector,
    field: &ConfigField,
    problems: &mut Vec<String>,
) {
    let pins = field.pins();
    if pins.is_empty() {
        return;
    }
    let name = field.name.as_str();
    let service = field.service.as_str();
    let Some(binding) = field.binding() else {
        return;
    };

    for other in connector.config_of(service) {
        if std::ptr::eq(other, field) {
            continue;
        }
        if other.binding().is_some_and(|other| {
            config_address_kind(other) == config_address_kind(binding)
                && other.target() == binding.target()
        }) {
            problems.push(format!(
                "configuration fields {name:?} and {:?} both resolve `{}.{}` in service \
                 {service:?}, so a host would key them to one value under one address. Two questions \
                 that share an answer are one question — bind one of them to a different name, or \
                 make them one field with an `also_binds`",
                other.name,
                config_address_kind(binding),
                binding.target()
            ));
        }
        for theirs in other.pins() {
            let collides = pins.iter().any(|ours| {
                ours.position == theirs.position
                    && match ours.position {
                        Position::Header => ours.name.eq_ignore_ascii_case(theirs.name),
                        Position::Path | Position::Query => ours.name == theirs.name,
                    }
            });
            if collides {
                problems.push(format!(
                    "configuration fields {name:?} and {:?} both send {:?} on the {} of every \
                     request of service {service:?}. They are two questions with two slots writing \
                     one position, so which value the vendor sees depends on an order nothing \
                     declares — declare it on one side only",
                    other.name,
                    theirs.name,
                    theirs.position.word()
                ));
            }
        }
    }
}

/// The host-side kind one binding is stored under.
///
/// Bare request pins are carried through the established endpoint-configuration port, so an
/// endpoint and a request pin of one target still share an address (C-229). A Basic username and a
/// credential secret are separate ports and therefore separate addresses even when both name the
/// same credential — the distinction C-475's qualified placeholder preserves.
fn config_address_kind(binding: Binding<'_>) -> &'static str {
    match binding {
        Binding::Request { .. } => "endpoint",
        other => other.kind(),
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
            // Only `binds` can answer, and never an `also_binds`: a further destination is a request
            // position by construction (`validate_destinations`), so a header pin still does not
            // bind a hostname. That is C-164's third measured shape, and C-229 does not move it —
            // the field that binds Algolia's hostname *and* its header binds the hostname in
            // `binds`, which is what makes `{app_id}` the one placeholder both destinations carry.
            // `config_of` is the head-service lookup; `also_services` extends the same field's one
            // address to a sibling surface of the same deployment (C-529). Both are consulted, and
            // neither admits an `also_binds` — a further destination is a request position by
            // construction, so a header pin still does not bind a hostname.
            let bound = connector.config_filling(service).any(|field| {
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
        // A "Test connection" button that could change vendor state is a button nobody dares press.
        // Direction is connector truth; neither method nor risk may stand in for it.
        Some(operation) if operation.direction == OperationDirection::Write => {
            problems.push(format!(
                "`verify` names operation {verify:?}, which declares `direction = \"write\"`. A \
                 connection test runs unattended whenever someone opens a settings page, so it must \
                 be a read a user would not mind being repeated"
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
        validate_socket_connect(connector, channel, problems);
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

/// Validate the declarative RFC 6455 handshake without resolving a host or reading credentials.
fn validate_socket_connect(
    connector: &Connector,
    channel: &ChannelBinding,
    problems: &mut Vec<String>,
) {
    let name = channel.name.as_str();
    let Some(connect) = &channel.connect else {
        return;
    };

    if channel.transport != Transport::Socket {
        problems.push(format!(
            "channel binding {name:?} declares `connect`, which only the `socket` transport uses"
        ));
    }

    let path = connect.path.as_str();
    if !path.starts_with('/')
        || path.starts_with("//")
        || path.contains("://")
        || path.contains(['?', '#'])
        || path.split('/').any(|segment| matches!(segment, "." | ".."))
    {
        problems.push(format!(
            "channel binding {name:?} declares socket path {path:?}, which is not a relative \
             WebSocket path rooted at the service `base_url`"
        ));
    }

    const HANDSHAKE_HEADERS: &[&str] = &[
        "host",
        "connection",
        "upgrade",
        "sec-websocket-key",
        "sec-websocket-version",
        "sec-websocket-protocol",
        "authorization",
    ];
    for (header, value) in &connect.headers {
        if HANDSHAKE_HEADERS
            .iter()
            .any(|reserved| header.eq_ignore_ascii_case(reserved))
        {
            problems.push(format!(
                "channel binding {name:?} fixes handshake-owned header {header:?}; the guarded \
                 host owns upgrade, subprotocol and authentication headers"
            ));
        }
        if header.is_empty()
            || !header
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
        {
            problems.push(format!(
                "channel binding {name:?} declares invalid fixed header name {header:?}"
            ));
        }
        if value.chars().any(|c| !c.is_ascii() || c.is_ascii_control())
            || value.contains(['{', '}'])
        {
            problems.push(format!(
                "channel binding {name:?} declares fixed header {header:?} with an invalid or \
                 templated value; fixed headers are public literals"
            ));
        }
    }

    let mut seen_protocols: Vec<&str> = Vec::new();
    for protocol in &connect.subprotocols {
        let valid = !protocol.is_empty()
            && protocol
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte));
        if !valid {
            problems.push(format!(
                "channel binding {name:?} declares invalid WebSocket subprotocol {protocol:?}"
            ));
        }
        if seen_protocols.contains(&protocol.as_str()) {
            problems.push(format!(
                "channel binding {name:?} declares WebSocket subprotocol {protocol:?} twice"
            ));
        }
        seen_protocols.push(protocol);
    }

    for (parameter, value) in &connect.query {
        if parameter.is_empty()
            || parameter
                .chars()
                .any(|c| c.is_control() || c.is_whitespace() || "&=?#".contains(c))
        {
            problems.push(format!(
                "channel binding {name:?} declares invalid socket query parameter {parameter:?}"
            ));
        }
        for variable in template_variables(value) {
            let declared = connector.config.iter().any(|field| {
                field.service == channel.service
                    && field.name == variable
                    && matches!(
                        field.binding(),
                        Some(Binding::ChannelQuery { channel: owner, parameter: target })
                            if owner == name && target == parameter
                    )
            });
            if !declared {
                problems.push(format!(
                    "channel binding {name:?} query parameter {parameter:?} needs configuration \
                     {{{variable}}}, but no `[[config]]` field binds \
                     `channel.{name}.query.{parameter}` under that name"
                ));
            }
        }
    }

    validate_requirements(
        connector,
        &connect.auth,
        &format!("channel binding {name:?}"),
        problems,
    );
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

    let mut seen_events: Vec<&str> = Vec::new();
    let mut seen_wire_values: Vec<&str> = Vec::new();
    for event in &channel.events {
        if seen_events.contains(&event.as_str()) {
            problems.push(format!(
                "channel binding {name:?} carries event {event:?} twice"
            ));
        }
        seen_events.push(event);
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
            Some(declared) => {
                let wire = declared.wire_value.as_deref().unwrap_or(&declared.name);
                if wire.trim().is_empty() {
                    problems.push(format!(
                        "event {event:?} carried by channel {name:?} declares an empty `wire_value`"
                    ));
                }
                if seen_wire_values.contains(&wire) {
                    problems.push(format!(
                        "channel binding {name:?} maps more than one event to wire value {wire:?}; \
                         a discriminator value must select exactly one declared event"
                    ));
                }
                seen_wire_values.push(wire);
            }
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

/// [`SIGNED_PLACEHOLDERS`] as an author reads them, so the refusal lists what it will accept rather
/// than only what it rejected. Derived from the list rather than restated beside it — a hand-written
/// copy is how an error message comes to name a vocabulary that has since moved.
fn fillable_placeholders() -> String {
    let names: Vec<String> = SIGNED_PLACEHOLDERS
        .iter()
        .map(|name| format!("{{{name}}}"))
        .collect();
    match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => names.join(""),
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
                 {{{placeholder}}}; the host can fill only {}",
                hmac.signed,
                fillable_placeholders()
            ));
        }
    }

    // **The rule this whole struct rests on.** A template that puts no payload into the signed
    // string signs something the request never enters, so a signature captured from one delivery
    // verifies *any* forged payload — bounded only by the tolerance, and by nothing at all without
    // one. It is the same defect as the unterminated brace `signed_placeholders` reports, except
    // that reaching it needs no typo: `signed = "{timestamp}"` is well formed, and every other check
    // here passes on it. Refusing an empty template is not enough, because the hole is not emptiness.
    //
    // The test is `PAYLOAD_PLACEHOLDERS`, not the literal `{body}`, and C-188 is why: `{url}` is a
    // per-endpoint constant, so `signed = "{url}"` is this exact hole under a placeholder that
    // *looks* request-specific — and a URL-signing vendor carries no timestamp, so there is not even
    // a window bounding it.
    if !placeholders
        .iter()
        .any(|p| PAYLOAD_PLACEHOLDERS.contains(&p.as_str()))
    {
        problems.push(format!(
            "channel binding {channel:?} has `signed = {:?}`, which never interpolates {{body}} or \
             {{sorted_form}}. The signed string must cover the request payload, or a signature \
             captured from one delivery verifies every forged payload that follows it — the \
             signature would prove only that somebody, once, held the secret",
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
    if channel.payload_root && !channel.payload.is_empty() {
        problems.push(format!(
            "channel binding {name:?} declares `payload_root = true` and a `payload` projection. \
             A delivery is either the complete JSON event or one projected object, never both"
        ));
    }
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
        if service.legacy && name != DEFAULT_SERVICE {
            problems.push(format!(
                "service {name:?} sets `legacy = true`, but the marker belongs only to the reserved \
                 {DEFAULT_SERVICE:?} service whose already-published addresses must stay elided"
            ));
        }
        // The reserved name is normally the *implicit* service. C-458 also admits an explicitly
        // marked legacy default beside named siblings without changing its published addresses.
        // See `validate_default_service_entry`.
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
        validate_service_tags(service, problems);
    }
}

/// Checks that every tag a service declares is stated once — C-153.
///
/// There is no satisfaction check here and there deliberately cannot be one: a [`Tag`] carries no
/// required members, because no operation makes a service `storage`. The unknown-name case is not
/// here either — `serde` refuses it first at the parse, since [`Tag`] is a closed enum — so the only
/// thing left to refuse is a repeat.
fn validate_service_tags(service: &Service, problems: &mut Vec<String>) {
    let name = service.name.as_str();
    let mut seen: Vec<Tag> = Vec::new();

    for tag in &service.tags {
        let word = tag.word();
        if seen.contains(tag) {
            problems.push(format!(
                "service {name:?} declares tag {word:?} more than once. A tag is a label, and a set \
                 that tolerates repeats is a list pretending to be a set. Known tags: {}",
                Tag::known_set()
            ));
            continue;
        }
        seen.push(*tag);
    }
}

/// Checks the one `[[services]]` entry that may name the reserved [`DEFAULT_SERVICE`] — C-120,
/// C-458.
///
/// C-49 refused the name outright, and the reason was sound: `default` is the service an operation
/// belongs to when it names none, so declaring it is a second definition of something that already
/// exists, and the two could disagree about a base URL or a version.
///
/// Roles and tags are the one thing that argument does not cover for **a provider with a single API
/// surface**, which has no other service to attach either to. C-458 adds a second, explicitly marked
/// shape for a published default service growing named siblings. The exceptions are scoped along
/// two axes:
///
/// 1. **What the entry may carry.** `roles` and `tags`, and nothing else. Neither has a
///    connector-level spelling, so neither has anything to contradict, while `base_url`,
///    `api_version` and `description` all do. `tags` joined the exception with C-153, which is what
///    makes the *forty-seven* single-surface providers taggable at all.
/// 2. **Whether the provider has any other service.** A `default` entry beside a named one remains
///    refused unless it sets `legacy = true`. That marker says the elided address already exists and
///    must not move. In that shape every service-bearing source table must state `service`, so the
///    declaration cannot silently catch omissions.
///
/// The reserved service stays address-elided in both admitted forms. A single-surface provider still
/// satisfies [`Connector::is_default_only`]; a mixed legacy provider does not, but artifact and
/// address rendering elide `default` by name and therefore keep its existing `<provider>.flux`.
fn validate_default_service_entry(
    connector: &Connector,
    service: &Service,
    problems: &mut Vec<String>,
) {
    // Scoped by "a service other than `default` is declared" rather than by a count, so that a file
    // declaring `default` twice reports the duplicate once and does not also report this twice.
    let named_sibling = connector
        .services
        .iter()
        .find(|other| other.name != DEFAULT_SERVICE);

    if let Some(other) = named_sibling.filter(|_| !service.legacy) {
        problems.push(format!(
            "`[[services]]` declares {DEFAULT_SERVICE:?} beside the named service {:?}. \
             Set `legacy = true` only when this is an already-published, address-elided service that \
             must retain its old GID, OIP, credential address and unsuffixed artifacts while named \
             siblings are added. Otherwise declare the roles and tags on the named service that \
             actually has them",
            other.name
        ));
        return;
    }

    if named_sibling.is_none() && service.legacy {
        problems.push(format!(
            "`[[services]]` declares {DEFAULT_SERVICE:?} with `legacy = true` but has no named \
             sibling. The marker is an address-migration capability for preserving an \
             already-published default while named services are added, not shorthand for a new or \
             default-only connector"
        ));
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
             elided from every published address — so the entry may carry `roles` and `tags`, and \
             nothing else. A role and a tag attach to a service and a single-surface provider has \
             nowhere else to put either; everything else is already stated at connector level, and a \
             second definition could disagree with it",
            overreaching.join("`, `")
        ));
    } else if named_sibling.is_none()
        && !service.legacy
        && service.roles.is_empty()
        && service.tags.is_empty()
    {
        problems.push(format!(
            "`[[services]]` declares {DEFAULT_SERVICE:?} and nothing else. {DEFAULT_SERVICE:?} is \
             reserved: it is the service an operation belongs to when it names none, and a provider \
             with one API surface declares no services at all. The two reasons to write the entry \
             are to carry `roles` and to carry `tags`"
        ));
    }
}

/// Refuses the ambiguity C-458's explicit legacy-default shape would otherwise reintroduce.
///
/// Serde intentionally normalizes an omitted `service` to `default` for every existing connector.
/// Only a mixed connector preserving an old default needs to distinguish those authoring forms, so
/// [`implicit_service_members`] retains that presence bit until this check and nowhere beyond it.
fn validate_legacy_default_members(loaded: &LoadedProvider, problems: &mut Vec<String>) {
    let connector = &loaded.connector;
    let preserves_legacy_default = connector
        .services
        .iter()
        .any(|service| service.name == DEFAULT_SERVICE && service.legacy)
        && connector
            .services
            .iter()
            .any(|service| service.name != DEFAULT_SERVICE);

    if !preserves_legacy_default {
        return;
    }

    for member in &loaded.implicit_service_members {
        problems.push(format!(
            "{} {:?} names no `service` in a connector preserving legacy {DEFAULT_SERVICE:?} \
             beside named services. State `service = {DEFAULT_SERVICE:?}` for the published legacy \
             owner or name its sibling; omission remains refused so a new member cannot silently \
             enter the address-elided service",
            member.kind, member.name
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

/// **One credential, one acquisition** (C-525) — the sibling of
/// [`validate_one_credential_disposition`], one axis over.
///
/// That function refuses two declarations of *where a credential appears in a response*. This one
/// refuses two declarations of *how a credential is obtained at all*: `[auth.oauth2]` says the host
/// runs a grant against the vendor's own OAuth endpoints, and an operation's `produces_credential`
/// naming the same credential says one of this connector's own calls mints it. Both cannot be true,
/// and the cost of not refusing falls on the emitter — `catalog::Acquisition` has one variant per
/// credential, so something downstream would have to *choose*, silently, and publish an acquisition
/// the author never declared.
///
/// The refusal carries the discriminator, because it is the one thing neither field's own
/// documentation supplies: an authorize or token endpoint is **never a connector operation**
/// (`AGENTS.md` § Authentication contract), so a credential obtained from the vendor's OAuth
/// endpoints is always the `[auth.oauth2]` case. `produces_credential` is for a credential minted by
/// an ordinary operation the connector genuinely declares — a session login, a device registration.
fn validate_one_credential_acquisition(
    connector: &Connector,
    method: &AuthMethod,
    problems: &mut Vec<String>,
) {
    if method.oauth2.is_none() {
        return;
    }
    let name = method.name.as_str();
    for operation in &connector.operations {
        let Some(produced) = &operation.produces_credential else {
            continue;
        };
        if produced.credential != method.name {
            continue;
        }
        problems.push(format!(
            "credential {name:?} declares an `[auth.oauth2]` grant, and operation {:?} declares \
             `produces_credential` naming it. Those state two different acquisitions of one \
             credential — the host runs a token grant, or this connector's own call mints it — and \
             exactly one governs. An authorize or token endpoint is never a connector operation, so \
             a credential obtained from the vendor's OAuth endpoints declares only `[auth.oauth2]` \
             and the minting operation is removed; `produces_credential` is for a credential minted \
             by an ordinary operation this connector declares, and such a credential declares no \
             `[auth.oauth2]` block",
            operation.id
        ));
    }
}

/// **An OAuth2 `token_endpoint` names a declared service, or the loader refuses it** (C-556).
///
/// The token endpoint may live on a different host from the authorize endpoint — Anthropic's
/// subscription flow authorizes on `claude.ai` and redeems its token on `platform.claude.com`. The
/// second host is declared by *reference*: [`OAuth2Spec::token_endpoint`] names a `[[services]]`
/// entry whose base URL the token exchange resolves against. That is what keeps the host set derived
/// from declared services rather than from a URL nothing admitted — so a name no service declares is
/// a typo pointing the token exchange at a host the allow-list never admitted, and it is refused
/// loudly. An empty value is the common case and means the exchange resolves against `endpoint`,
/// which needs no check here.
fn validate_one_credential_token_endpoint(
    connector: &Connector,
    method: &AuthMethod,
    problems: &mut Vec<String>,
) {
    let Some(spec) = &method.oauth2 else {
        return;
    };
    if spec.token_endpoint.is_empty()
        || connector
            .service_names()
            .contains(&spec.token_endpoint.as_str())
    {
        return;
    }
    let listed = connector.service_names().join(", ");
    problems.push(format!(
        "credential {:?} resolves its token exchange against token_endpoint {:?}, which is not a \
         declared service — a `token_endpoint` names the declared service whose base URL the token \
         exchange resolves against, and a name nothing declares reaches a host the allow-list never \
         admitted. This provider declares: {listed}. Leaving it empty is the other legal answer, and \
         means the token exchange resolves against the `endpoint` service",
        method.name, spec.token_endpoint
    ));
}

/// **A grant that carries a declared weakness must declare it** (C-440).
///
/// The closed [`AuthHazard`] vocabulary is only worth having if a connector cannot opt out of it by
/// silence. A host's deployment filter refuses on the *presence* of a hazard, so a connector that
/// allows the resource-owner password grant and declares no hazard is admitted by the very
/// deployment that set out to refuse exactly this — and the omission is one line nobody wrote rather
/// than anything a reviewer sees. `AGENTS.md` puts it generally: a marking that reads as a safety
/// decision while recording only that the question was never asked is worse than no marking at all.
///
/// The rule runs one way. A hazard on a credential whose grants do not include `password` is not
/// refused here: the vocabulary is about how a credential is *obtained*, and a future hazard need
/// not be an OAuth grant at all.
fn validate_one_credential_hazard(method: &AuthMethod, problems: &mut Vec<String>) {
    let Some(spec) = &method.oauth2 else {
        return;
    };
    if !spec.grants.contains(&OAuthGrant::Password) || method.hazard.is_some() {
        return;
    }
    problems.push(format!(
        "credential {:?} allows the `password` grant and declares no `hazard`. The resource owner's \
         own password reaching this host is a named weakness — RFC 9700 §2.4 says the grant MUST \
         NOT be used, and OAuth 2.1 drops it — and a host refuses it by declared property rather \
         than by connector name, so an undeclared one is admitted by the deployment that set out to \
         refuse it. Declare `hazard = {:?}` beside the grant, or remove `password` from `grants`",
        method.name,
        AuthHazard::ResourceOwnerSecretShared.word()
    ));
}

/// **Every auth quirk names a grant, says what was measured, and says who measured it when**
/// (C-440).
///
/// A quirk is asserted against a vendor's implementation and contradicted by that vendor's own
/// document, so the two provenance fields are what separate it from a guess that aged. They are
/// checked rather than trusted because the cost of an unattributed one is already on the record:
/// `providers/babelforce.toml` carries an open question to a vendor's API owners that nobody can now
/// answer, because whoever raised it did not write down what they had read.
fn validate_one_credential_quirks(method: &AuthMethod, problems: &mut Vec<String>) {
    let name = method.name.as_str();
    if method.quirks.is_empty() {
        return;
    }

    // A token endpoint the connector never declared is one nothing will ever read — the same rule
    // an `oauth.redirect_uri` binding already carries.
    if method.oauth2.is_none() {
        problems.push(format!(
            "credential {name:?} declares a `quirks.token_endpoint` measurement and no \
             `[auth.oauth2]` block. A token-endpoint quirk describes an endpoint the host reaches to \
             run a grant, and a credential declaring no grant has no such endpoint, so nothing would \
             ever read it"
        ));
    }

    let mut seen: Vec<&str> = Vec::new();
    for quirk in &method.quirks.token_endpoint {
        let grant = quirk.grant.trim();
        if grant.is_empty() {
            problems.push(format!(
                "credential {name:?} declares a `quirks.token_endpoint` measurement with an empty \
                 `grant`. The vendor's own `grant_type` word is what says which of the endpoint's \
                 behaviours was measured; one endpoint answers differently per grant, which is the \
                 whole reason these are recorded one at a time"
            ));
        } else if seen.contains(&grant) {
            problems.push(format!(
                "credential {name:?} declares two `quirks.token_endpoint` measurements for grant \
                 {grant:?}. That is two answers to one question, and nothing downstream could say \
                 which was measured last — record one, and supersede it in place when the vendor \
                 changes"
            ));
        }
        seen.push(grant);

        for (field, value) in [
            ("behaviour", quirk.behaviour.as_str()),
            ("attribution", quirk.attribution.as_str()),
        ] {
            if value.trim().is_empty() {
                problems.push(format!(
                    "credential {name:?}'s `quirks.token_endpoint` measurement for grant \
                     {grant:?} declares an empty `{field}`. A quirk contradicts the vendor's own \
                     document, so a reader a year from now needs to know what was measured and \
                     against what — an unattributed one is indistinguishable from a guess"
                ));
            }
        }

        if !is_iso_date(&quirk.measured) {
            problems.push(format!(
                "credential {name:?}'s `quirks.token_endpoint` measurement for grant {grant:?} \
                 declares `measured = {:?}`, which is not a date. It must be `YYYY-MM-DD`: a quirk \
                 is a timestamped claim about a vendor's running implementation, and \"recently\" \
                 does not let a reader decide whether it predates the release they are debugging",
                quirk.measured
            ));
        }
    }
}

/// Whether `value` is a calendar date spelled `YYYY-MM-DD`.
///
/// Deliberately a shape-and-range check rather than a date library: the question is whether an
/// author wrote a date at all, and a leap-year rule would be a dependency bought to reject
/// `2026-02-30` in a provenance field no arithmetic is ever done on.
fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return false;
    }
    let number = |range: std::ops::Range<usize>| value[range].parse::<u32>().unwrap_or(0);
    (1..=12).contains(&number(5..7)) && (1..=31).contains(&number(8..10))
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

        validate_one_credential_acquisition(connector, method, problems);
        validate_one_credential_token_endpoint(connector, method, problems);
        validate_one_credential_hazard(method, problems);
        validate_one_credential_quirks(method, problems);
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
        validate_semantic_effects(operation, problems);
        // The two credential declarations are checked as a pair before either is checked alone:
        // when both are present the operation is incoherent at the root, and the rules downstream
        // of each would render two contradicting instructions for one fact. See
        // `validate_one_credential_disposition`, which returns whether it took the decision.
        if !validate_one_credential_disposition(operation, problems) {
            validate_credential_response(operation, problems);
            validate_produces_credential(connector, operation, problems);
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

/// Semantic effects are a closed, policy-bearing set and must agree with the metadata Flux gates.
fn validate_semantic_effects(operation: &Operation, problems: &mut Vec<String>) {
    let id = operation.id.as_str();

    for pair in operation.semantic_effects.windows(2) {
        if pair[0] == pair[1] {
            problems.push(format!(
                "operation {id:?} declares semantic effect {:?} more than once; semantic effects \
                 are a set, so remove the duplicate rather than relying on a consumer to dedupe it",
                pair[0].tag()
            ));
        }
    }

    if operation.semantic_effects.contains(&SemanticEffect::Pure) {
        problems.push(format!(
            "operation {id:?} declares semantic effect `pure`, but every connector operation makes \
             an external HTTP call. `pure` means deterministic and side-effect free, so it cannot \
             describe a connector operation"
        ));
    }

    for effect in &operation.semantic_effects {
        if matches!(effect, SemanticEffect::Money | SemanticEffect::Delete)
            && operation.risk != Risk::Destructive
        {
            problems.push(format!(
                "operation {id:?} declares semantic effect {:?} but risk {:?}; Flux requires \
                 `money` and `delete` to be `destructive` so policy and the approval preview cannot \
                 understate them",
                effect.tag(),
                risk_word(operation.risk)
            ));
        } else if effect.is_consequential() && operation.risk == Risk::Low {
            problems.push(format!(
                "operation {id:?} declares consequential semantic effect {:?} but risk `low`; Flux \
                 does not permit a consequence that outlives the call to use its harmless tier",
                effect.tag()
            ));
        }
    }

    if operation
        .semantic_effects
        .iter()
        .any(|effect| effect.is_consequential())
        && operation.idempotency == Idempotency::Idempotent
    {
        problems.push(format!(
            "operation {id:?} declares a consequential semantic effect but `idempotency = \
             \"idempotent\"`; that value licenses Flux to skip execution in favour of a cached \
             result, so a consequence-bearing operation must be `conditional` or `non_idempotent`"
        ));
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
/// - **an authored `conditional` write with no condition** — the claim without the thing that makes it
///   checkable, and the reason this validator exists;
/// - **a condition on an authored read** — there is no repeat hazard to condition, so the
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
    let mutating = operation.direction == OperationDirection::Write;

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
            "operation {id:?} is an authored {} and declares `repeatable_because`, but a read has \
             no repeat hazard to put a condition on. The field exists only to state the \
             condition behind `idempotency = \"conditional\"` on a write; remove it",
            operation.direction.word()
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

/// The `risk` value as an author spells it in a provider file. Exhaustive for the reason
/// [`idempotency_word`] is: a fifth variant must be a compile error here, not a refusal quoting a
/// word the file cannot contain.
fn risk_word(risk: Risk) -> &'static str {
    match risk {
        Risk::Low => "low",
        Risk::Medium => "medium",
        Risk::High => "high",
        Risk::Destructive => "destructive",
    }
}

/// A boolean as an author spells it, so `expose` reads the same way in a refusal as in the file.
fn bool_word(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

/// **One fact, one disposition** — C-432's reconciliation of C-430 and C-136.
///
/// [`Operation::credential_response`] and [`Operation::produces_credential`] state the same fact —
/// a credential arrives in this operation's response — and prescribe opposite outcomes. C-430's
/// field withholds the operation; C-136's ships it and returns a handle. Declared together they ask
/// for both, and before this story the loader obliged: `credential_response`'s stock refusal told
/// the author to withhold an operation the sibling declaration says ships, with nothing stating
/// which governs. Two rules in one repository is the thing C-432 exists to remove.
///
/// **The discriminator is purpose, not shape.** Both fields point at a credential in a response; no
/// inspection of the pointer, the schema or the field name can tell them apart, because the
/// difference is what the operation is *for*:
///
/// - the credential **is** the answer — a token exchange, a login — so diverting it into the store
///   and returning the handle costs the caller nothing. That is `produces_credential`.
/// - the credential arrives **incidentally**, beside the meeting or the server the operation exists
///   to deliver. Diverting the whole result would delete the answer, so the operation is withheld
///   until the value can be redacted where it sits — `credential_response`, and C-79.
///
/// That sentence is what the refusal has to carry, because it is the one thing an author cannot
/// re-derive by reading either field's own documentation.
///
/// Returns `true` when it refused, and the pair-wise check is then the *only* thing said about this
/// operation's credential declarations: the per-field rules downstream are all conditioned on a
/// disposition this operation has not yet chosen, so running them would bury the choice under
/// consequences of both branches.
fn validate_one_credential_disposition(operation: &Operation, problems: &mut Vec<String>) -> bool {
    if operation.credential_response.is_empty() || operation.produces_credential.is_none() {
        return false;
    }
    let id = operation.id.as_str();
    problems.push(format!(
        "operation {id:?} declares both `credential_response` (at {}) and `produces_credential`, \
         which state one fact — a credential arrives in this response — and prescribe opposite \
         dispositions. Exactly one governs, and which one is a question about the operation's \
         **purpose**, not about the shape of the value: if the credential *is* the answer, as a \
         token exchange's is, declare only `produces_credential` and the value is diverted into the \
         bound `CredentialStore` with the caller receiving the handle. If the credential arrives \
         **incidentally**, beside the result the operation exists to deliver, declare only \
         `credential_response` — diverting the whole result would delete the answer, so the \
         operation is withheld until the value can be redacted where it sits (C-79)",
        quoted(&operation.credential_response)
    ));
    true
}

/// **No operation returns a secret** (C-430) — the gate, reading the declaration that says one does.
///
/// `AGENTS.md` § Authentication contract states the rule this enforces, and states it once: an
/// operation whose declared response carries a token is withheld until C-136's diversion lands,
/// because the host's redactor holds only values the host itself resolved and cannot know a secret
/// minted by the very call returning it. Four operations shipped in v0.9.0 against it — postmark's
/// server pair returning `ApiTokens` in plaintext, zoom's meeting pair returning a `start_url` with
/// the host's ZAK token embedded — every one of them accurately describing the hazard in its own
/// `response_schema` and returning the field anyway. Describing a credential is not withholding it.
///
/// # It reads a declaration, and that is the design rather than a shortcut
///
/// A catalogue-wide scan for token-shaped property names found 31 candidates and **28 of them were
/// correct as they stood**, each documented as harmless by its own connector. A regex over field
/// names would refuse all 28, and a gate that is wrong nine times in ten is one authors learn to
/// spell around — so the only thing that trips this is [`Operation::credential_response`], which
/// nothing but a connector can write. The cost is stated rather than hidden: this does not catch an
/// author who never declares. `crates/connector-spec/tests/credential_response.rs` carries the other
/// half — the four withheld operations, named, so reinstating one is a red build.
///
/// # Three refusals, and the first two are what keep the third honest
///
/// - **A location with no `response_schema` to resolve against**, which is a claim about a shape
///   nothing states.
/// - **A location that matches nothing**, which is the shape a vendor rename takes: a declaration
///   that quietly stopped applying reads as protection while being none. C-79 names this one
///   explicitly, and it is the reason the walk descends into arrays — `ApiTokens` sits under
///   `Servers[]`, and a resolver stopping at the root would call the true declaration a typo.
/// - **The declaration itself**, which is the withholding.
fn validate_credential_response(operation: &Operation, problems: &mut Vec<String>) {
    if operation.credential_response.is_empty() {
        return;
    }
    let id = operation.id.as_str();

    match &operation.response_schema {
        None => problems.push(format!(
            "operation {id:?} declares `credential_response` but no `response_schema`, so there is \
             nothing for {} to resolve against. A location naming a shape the operation does not \
             declare cannot be checked by anything",
            quoted(&operation.credential_response)
        )),
        Some(schema) => {
            for location in &operation.credential_response {
                if !response_location_exists(schema, location) {
                    problems.push(format!(
                        "operation {id:?} declares a credential response location {location:?} \
                         that matches nothing in its `response_schema`. A location resolving to \
                         nothing protects nothing, and this is the shape a vendor rename takes — \
                         spell each segment as the response spells it, and `*` for every element \
                         of an array"
                    ));
                }
            }
        }
    }

    problems.push(format!(
        "operation {id:?} declares that its own response carries a credential at {}, so it cannot \
         ship. `AGENTS.md` § Authentication contract: an operation whose declared response carries \
         a token is withheld until C-136's diversion lands, because the host's redactor holds only \
         values the host itself resolved and cannot know a secret minted by the very call returning \
         it. Withhold the operation and name it as an exclusion carrying that reason — `expose = \
         false` is not the mechanism, since `connector_pack::resolve` admits any named operation \
         whatever its exposure (C-413)",
        quoted(&operation.credential_response)
    ));
}

/// **A credential-producing operation returns a handle, or it does not load** — C-136's refusals.
///
/// [`Operation::produces_credential`] is the declaration that makes a login shippable: the secret
/// travels from the vendor's response into the host's bound `CredentialStore` and the caller
/// receives `{ "credential": "tenants/…" }`. Every rule below exists because the guarantee is
/// *structural* — it comes from the declared shape rather than from a filter — and a declaration the
/// loader accepted while the shape did not hold would be the worst of both: an operation documented
/// as safe to call, shipping the token.
///
/// # The three the story names, and the three that make them possible
///
/// - **The declared output still exposes the secret.** A `response_schema` beside this declaration
///   documents the vendor's wire body; if the secret's own location resolves in it, the operation is
///   describing an output it does not have and one that carries a credential. Refused, and this is
///   C-430's mechanism read from the other side — that story established that *deleting* the
///   location from the schema removes the disclosure and leaves the exposure, so the schema is
///   cross-checked rather than silently rewritten.
/// - **No secret field is named.** The extractor would not know what to divert, and an operation
///   that diverts nothing returns the vendor's body — which is the unsafe operation wearing the safe
///   operation's declaration.
/// - **`idempotency = "idempotent"`.** Minting a token is a write, and some vendors invalidate the
///   previous one; `Idempotent` additionally licenses flux's op cache to serve a stored result
///   *instead of executing*, which for a login means handing back an address whose value was
///   replaced.
///
/// The other three are the ones without which the first three cannot be enforced at all: a
/// credential the connector does not declare has no leaf and therefore no address; a connector with
/// no `authority` has no second path segment, so nothing composes; and two operations minting one
/// credential leave "which call put the value there" unanswerable, which is the same ambiguity
/// C-406 refuses for two connections of one vendor.
fn validate_produces_credential(
    connector: &Connector,
    operation: &Operation,
    problems: &mut Vec<String>,
) {
    let Some(produced) = &operation.produces_credential else {
        return;
    };
    let id = operation.id.as_str();

    // **Refusal 1 — names no secret field.** A pointer must start with `/`, exactly as
    // `credential_response` does: one spelling of "a location in a response", not two.
    if produced.secret.trim().is_empty() || !produced.secret.starts_with('/') {
        problems.push(format!(
            "operation {id:?} declares `produces_credential` with `secret = {:?}`, which names no \
             field of the vendor's response. The extractor would not know what to divert, so the \
             operation would return the vendor's body — state a location as `credential_response` \
             spells one: a JSON Pointer into the response body, `/access_token`",
            produced.secret
        ));
    }

    // **And it names exactly one value.** `credential_response`'s vocabulary admits `*` for every
    // element of an array, because that field describes *where credentials appear* and an array of
    // them is a real shape — postmark's `Servers[].ApiTokens` is the case that forced it. A **mint**
    // is the other question: one call, one value, one address. `*` here would name several secrets
    // for one credential with nothing to say which is stored, so it is refused at load rather than
    // left to fail at every call. Refusing is also what keeps the runtime honest — the diversion
    // resolves the location with `serde_json::Value::pointer`, which has no wildcard, so a `*` this
    // validator admitted would be a documented behaviour the code does not have.
    if produced.secret.contains('*') {
        problems.push(format!(
            "operation {id:?} declares `produces_credential` at {:?}, which uses `*`. A `*` names \
             every element of an array and a mint stores exactly one value at exactly one address, \
             so there would be nothing to say which element is the credential. That spelling \
             belongs to `credential_response`, which describes where credentials *appear*; name a \
             single location here",
            produced.secret
        ));
    }

    // **Refusal 2 — the declared output still exposes the secret.** Read against what the author
    // wrote, because `Operation::effective_response_schema` already answers the handle here; the
    // question is whether the connector's own description of the wire body promises a caller the
    // value.
    if let Some(schema) = &operation.response_schema {
        if response_location_exists(schema, &produced.secret) {
            problems.push(format!(
                "operation {id:?} declares `produces_credential` at {:?} and its `response_schema` \
                 still describes that location, so its published contract offers a caller the \
                 secret the diversion exists to withhold. A `response_schema` here documents the \
                 vendor's wire body and must not carry the minted value — note that deleting the \
                 location is not enough on its own (C-430): what makes the operation safe is that \
                 the value never reaches the result, which is what `produces_credential` does",
                produced.secret
            ));
        }
    }

    // **Refusal 3 — a write declared safe to repeat.**
    if operation.idempotency == Idempotency::Idempotent {
        problems.push(format!(
            "operation {id:?} declares `produces_credential` and `idempotency = \"idempotent\"`. \
             Minting a credential is a write — some vendors invalidate the previous token — and \
             `idempotent` additionally licenses flux's op cache to serve a stored result instead of \
             executing, which would hand back an address whose value has since been replaced. \
             Declare `non_idempotent`"
        ));
    }

    // The credential must be one the connector declares, or there is no leaf to address it by.
    if !connector
        .auth
        .iter()
        .any(|method| method.name == produced.credential)
    {
        problems.push(format!(
            "operation {id:?} declares `produces_credential` storing {:?}, which this connector \
             declares no `[[auth]]` credential for. The value would have nowhere to be put: the \
             address is composed from the connector's `authority` and the credential's own leaf, \
             and neither exists for a name nothing declares",
            produced.credential
        ));
    }

    // And the connector must have an authority, or the second segment of the address does not
    // exist. `connector-pack` refuses the same arrangement at resolve time with
    // `Error::NoCredentialAddress`; refusing here makes it a build failure instead of a first-call
    // one.
    if connector.authority.is_none() {
        problems.push(format!(
            "operation {id:?} declares `produces_credential` but this connector declares no \
             `authority`, so `tenants/<tenant>/<authority>/…` has no second segment and the minted \
             value has no address to be stored at"
        ));
    }

    // One producer per credential. Two would leave a reader with no way to say which call put the
    // value there, and the catalogue's own record of the mint names exactly one operation.
    for other in &connector.operations {
        if other.id == operation.id {
            break;
        }
        if other
            .produces_credential
            .as_ref()
            .is_some_and(|earlier| earlier.credential == produced.credential)
        {
            problems.push(format!(
                "operations {:?} and {id:?} both declare `produces_credential` storing {:?}. Two \
                 calls minting into one address leave \"which one put the value there\" \
                 unanswerable, and a downstream operation naming the credential cannot say which \
                 login it needs — give each grant its own credential",
                other.id, produced.credential
            ));
        }
    }
}

/// Locations as a refusal lists them: `"/a", "/b"`. One spelling, so two refusals about the same
/// operation read alike.
fn quoted(locations: &[String]) -> String {
    locations
        .iter()
        .map(|location| format!("{location:?}"))
        .collect::<Vec<_>>()
        .join(", ")
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
    // **Keyed by service as well as by `operationId`** — C-410. An `operationId` is unique inside
    // one document and nowhere else: babelforce declares `getUser` in `manager-2026-07-10` and again
    // in `user-2026-06-25`, as two different requests, so selecting both is the ordinary case and
    // only a repeat *within* one document is the duplicate this refuses.
    let mut selected: Vec<(&str, &str)> = Vec::new();
    let mut renamed: Vec<&str> = Vec::new();

    for patch in &loaded.patch.operations {
        let select = patch.select.as_str();
        let service = patch.service.as_deref().map(str::trim).unwrap_or_else(|| {
            loaded
                .specs
                .first()
                .filter(|_| loaded.specs.len() == 1)
                .map_or(DEFAULT_SERVICE, SpecSource::service)
        });
        if select.trim().is_empty() {
            problems.push(
                "a `[[patch.operations]]` entry has an empty `select`; it names the spec's \
                 `operationId`"
                    .to_owned(),
            );
        } else if selected.contains(&(service, select)) {
            problems.push(format!(
                "`[[patch.operations]]` selects {select:?} more than once from service {service:?}"
            ));
        }
        selected.push((service, select));

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

        for (position, name) in patch.omit.entries() {
            if name.trim().is_empty() {
                problems.push(format!(
                    "patch for {select:?} omits a `{position:?}` parameter with an empty `name`"
                ));
            }
        }
    }

    validate_selectors(&loaded.patch, problems);
    if let Some(naming) = loaded.patch.naming.as_ref() {
        validate_naming(naming, problems);
    }
}

/// The `[[patch.select]]` statements themselves — C-411.
///
/// Only what can be judged without a document. Whether a selector *matches* anything is
/// [`publish`]'s, because it needs the ingest; whether it is a well-formed statement is here, so
/// `load` refuses a malformed one exactly as `load_with_spec` does.
fn validate_selectors(patch: &Patch, problems: &mut Vec<String>) {
    for selector in &patch.select {
        let subject = selector.describe();
        if let Some(prefix) = selector.path_prefix.as_deref() {
            let prefix = prefix.trim();
            if prefix.is_empty() {
                problems.push(format!(
                    "{subject} states an empty `path_prefix`. Omit the key to match every path in \
                     the document — an empty string is the same statement written so it reads like \
                     a mistake"
                ));
            } else if !prefix.starts_with('/') {
                problems.push(format!(
                    "{subject} states `path_prefix = {prefix:?}`, which must start with `/`: it is \
                     matched against the document's own path templates, and those do"
                ));
            }
        }
    }
}

/// The `[patch.naming]` declaration itself — C-412.
///
/// The pins are checked against the documents in [`check_pins`]; this is the half that holds
/// without one, so a prefix or a pinned value that could never produce a legal op id is refused
/// even by [`load`].
fn validate_naming(naming: &Naming, problems: &mut Vec<String>) {
    if let Some(prefix) = naming.prefix.as_deref() {
        let prefix = prefix.trim();
        if prefix.is_empty() {
            problems.push(
                "`[patch.naming] prefix` is empty. Omit the key for no prefix — an empty string is \
                 the same statement written so it reads like a mistake"
                    .to_owned(),
            );
        } else if let Err(reason) = legal_op_id(prefix) {
            problems.push(format!(
                "`[patch.naming] prefix = {prefix:?}` cannot begin a legal op id: {reason}"
            ));
        }
    }

    for (operation_id, pinned) in &naming.pin {
        if operation_id.trim().is_empty() {
            problems.push(
                "`[patch.naming.pin]` has an entry with an empty key; a pin is keyed by the spec's \
                 `operationId`"
                    .to_owned(),
            );
            continue;
        }
        if let Err(reason) = legal_op_id(pinned.trim()) {
            problems.push(format!(
                "`[patch.naming.pin]` pins {operation_id:?} to {pinned:?}, which is not a legal op \
                 id: {reason}"
            ));
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
        ("operationSelector", probe::<OperationSelector>()),
        ("naming", probe::<Naming>()),
        ("operationPatch", probe::<OperationPatch>()),
        ("paramPatch", probe::<ParamPatch>()),
        ("paramOmission", probe::<ParamOmission>()),
        ("authMethod", probe::<AuthMethod>()),
        ("oauth2", probe::<crate::OAuth2Spec>()),
        ("oauthRedirect", probe::<crate::OAuthRedirect>()),
        ("authQuirks", probe::<crate::AuthQuirks>()),
        ("tokenEndpointQuirk", probe::<crate::TokenEndpointQuirk>()),
        ("authRequirement", probe::<AuthRequirement>()),
        ("operation", probe::<Operation>()),
        ("producedCredential", probe::<crate::ProducedCredential>()),
        ("event", probe::<EventDecl>()),
        ("channel", probe::<ChannelBinding>()),
        ("socketConnect", probe::<SocketConnectSpec>()),
        ("configField", probe::<ConfigField>()),
        ("choice", probe::<crate::Choice>()),
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
        ("operationSpecSource", probe::<OperationSpecSource>()),
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
