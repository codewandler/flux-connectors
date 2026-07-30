//! The normalized connector IR: the single shape both front-ends produce and codegen consumes.
//!
//! Everything here is plain data with a serde encoding. Two properties are load-bearing and are
//! asserted by the tests next door rather than left to good intentions:
//!
//! 1. **Types survive.** Every [`Param`] and the response carry a real [`JsonSchema`], not a
//!    stringly-typed shadow of one. The cautionary tale is action-proxy's YAML, where `type:
//!    string` stood in for dates and ids alike and no schema ever reached the caller.
//! 2. **Serialization is deterministic.** Identical values encode to identical bytes.
//!    `connectors.lock` (C-7) hashes this encoding and `flux-connectors check` fails on a
//!    mismatch, so any leaked iteration order would surface as phantom drift on every build.
//!
//! # Why these types are strict on deserialization
//!
//! C-2 originally left them permissive, on the theory that "validation lives in the loader (C-3),
//! not here". C-2's review disproved it. The provider loader parses `providers/*.toml` straight into
//! these types, so a key the derived `Deserialize` does not recognize is discarded *before* any
//! loader-level check can see it — the strictness cannot be bolted on from outside. Two concrete
//! failures were demonstrated, and both fail in the dangerous direction:
//!
//! - a mistyped `authh` on an operation yields [`Operation::auth`] = `None`, which means *inherit
//!   the connector default*, so the operation authenticates with the connector's default
//!   credentials rather than the narrower set the author meant to name. The failure direction is
//!   credential-**sending**, not fail-closed;
//! - a mistyped `envv` on a credential yields an empty `env` list with no error at all.
//!
//! So every struct here carries `#[serde(deny_unknown_fields)]`, and
//! [`AuthMethod::scheme`](crate::AuthMethod::scheme) lost its `#[serde(default)]` for the same
//! reason [`Risk`] and [`Idempotency`] have no `Default`: how a secret reaches the wire is not a
//! decision to make by silence. `tests/strict_fields.rs` pins all of it.
//!
//! *Semantic* validation — a credential named by no declaration, a degenerate empty auth mechanism,
//! a `basic` scheme with no user half — still lives in the loader ([`crate::provider`]), because it
//! is cross-field reasoning serde cannot express.

use serde::{Deserialize, Serialize};

use crate::auth::{AuthMethod, AuthRequirement};

/// A JSON Schema, carried verbatim.
///
/// JSON Schema *is* JSON, and the pipeline's job is to move a vendor's schema from a spec document
/// into an op contract without reinterpreting it — so a faithful [`serde_json::Value`] is both the
/// simplest and the most honest representation. Modelling a subset in Rust would silently drop
/// every keyword the subset missed, which is exactly the failure this field exists to prevent.
///
/// It is also deterministic: `serde_json::Map` is a `BTreeMap` unless `serde_json/preserve_order`
/// is enabled, so object keys serialize in sorted order regardless of how the document was parsed.
/// `tests/determinism.rs::serde_json_object_keys_stay_sorted` is the tripwire that fails if any
/// future dependency turns that feature on.
pub type JsonSchema = serde_json::Value;

/// The HTTP method an operation issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
    /// `PUT`.
    Put,
    /// `PATCH`.
    Patch,
    /// `DELETE`.
    Delete,
    /// `HEAD`.
    Head,
    /// `OPTIONS`.
    Options,
}

/// How much damage an operation can do, in flux's own vocabulary (`flux_spec::Risk`).
///
/// There is deliberately **no `Default`**. flux's approval gate reads this, so letting the field be
/// omitted would mean a safety decision made by silence — an operation that forgot to declare
/// itself destructive would be waved through as low risk. Both front-ends must state it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// Reads, and writes that cannot surprise anyone.
    Low,
    /// Writes with limited blast radius.
    Medium,
    /// Writes a reviewer would want to see first.
    High,
    /// Deletes or otherwise irreversible.
    Destructive,
}

/// Whether repeating the operation is safe, in flux's own vocabulary (`flux_spec::Idempotency`).
///
/// No `Default`, for the same reason as [`Risk`]: this is what tells flux whether a `retry` around
/// the request is sound, and guessing on the operation's behalf is how a retry turns one charge
/// into three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Idempotency {
    /// Repeating the call has the same effect as making it once.
    Idempotent,
    /// Repeating the call repeats its effect.
    NonIdempotent,
    /// Idempotent only under a condition the caller supplies (e.g. an idempotency key).
    Conditional,
}

/// One request parameter, carrying its JSON Schema.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Param {
    /// The parameter name as the vendor API expects it.
    pub name: String,
    /// Human-readable description, surfaced to the model as part of the op's tool contract.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Whether the vendor requires the parameter.
    #[serde(default)]
    pub required: bool,
    /// The parameter's JSON Schema.
    ///
    /// Mandatory, with no default: a parameter whose type is unknown is a parameter that has
    /// already collapsed to a string, and that is the failure this whole crate is arranged around.
    pub schema: JsonSchema,
}

/// An operation's parameters, grouped by where they travel on the request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamSet {
    /// Parameters interpolated into the path template (`/v2/calls/{call_id}`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<Param>,
    /// Query-string parameters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query: Vec<Param>,
    /// Request headers the caller supplies. Auth headers are **not** here — they are injected by
    /// the host from an [`AuthMethod`], so no credential passes through the parameter surface.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header: Vec<Param>,
    /// Fields assembled into the JSON request body. Emitting the body is C-9's job; this is the
    /// shape it reads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<Param>,
}

impl ParamSet {
    /// Whether the operation takes no parameters at all.
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
            && self.query.is_empty()
            && self.header.is_empty()
            && self.body.is_empty()
    }

    /// Every parameter, in request-position order: path, query, header, body.
    pub fn iter(&self) -> impl Iterator<Item = &Param> {
        self.path
            .iter()
            .chain(&self.query)
            .chain(&self.header)
            .chain(&self.body)
    }
}

/// How a vendor paginates a collection endpoint.
///
/// `max_pages` is mandatory on every variant because flux's analyzer rejects unbounded loops — a
/// constraint worth honoring rather than working around. Compiling this into Flux control flow is
/// C-12; this is only its declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Pagination {
    /// Page-number pagination: `?page=2&per_page=100`.
    Page {
        /// The query parameter carrying the page number.
        page_param: String,
        /// The query parameter carrying the page size, when the vendor allows one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size_param: Option<String>,
        /// The page size to request.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page_size: Option<u32>,
        /// The hard cap on pages fetched.
        max_pages: u32,
    },
    /// Cursor pagination: the response carries the cursor for the next request.
    Cursor {
        /// The query parameter carrying the cursor.
        cursor_param: String,
        /// A JSON Pointer (RFC 6901) into the response body locating the next cursor.
        next_cursor_pointer: String,
        /// The hard cap on pages fetched.
        max_pages: u32,
    },
}

/// A vendor's published rate limit, compiled into a Flux `throttle` by C-12.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateLimit {
    /// Requests allowed per window.
    pub requests: u32,
    /// The window, in seconds.
    pub per_seconds: u32,
    /// The throttle bucket name. Buckets collide if they are not unique within a session, so when
    /// this is `None` codegen derives one from the connector and operation ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
}

/// Where a vendor hides the real error inside a non-2xx response body.
///
/// `http.request` treats a non-2xx as a *result* rather than an op failure, so the generated op has
/// to dig the message out itself. Both fields are JSON Pointers (RFC 6901) into the response body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorEnvelope {
    /// Pointer to the human-readable message.
    pub message_pointer: String,
    /// Pointer to the vendor's error code, when it publishes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_pointer: Option<String>,
}

/// The ways a real vendor API departs from what its spec implies.
///
/// Quirks are declarations, not behavior: C-12 compiles them into real Flux control flow —
/// `throttle`, a bounded pagination loop — which is the payoff for targeting a language instead of
/// interpreting config.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quirks {
    /// How the endpoint paginates, if it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Pagination>,
    /// The endpoint's rate limit, if the vendor publishes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimit>,
    /// The vendor's error envelope, if its errors are not plain HTTP status codes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_envelope: Option<ErrorEnvelope>,
}

impl Quirks {
    /// Whether the operation declares no quirks, in which case codegen emits a plain request.
    pub fn is_empty(&self) -> bool {
        self.pagination.is_none() && self.rate_limit.is_none() && self.error_envelope.is_none()
    }
}

/// Where a connector came from, so drift against upstream can be detected rather than absorbed.
///
/// **`ir_sha256` is deliberately absent.** The pipeline design lists it under provenance, but it is
/// computed *from* the serialized IR — storing it inside the value being hashed would make the hash
/// depend on itself. It belongs in `connectors.lock` alongside the generated-artifact hashes and
/// the generator version, and that is where [`LockEntry::ir_sha256`](crate::lock::LockEntry) keeps
/// it.
///
/// Note that **none of these fields is in the IR hash domain** — see [`Connector::hash_domain`].
/// They are inputs to the build, recorded and verified one by one, not part of what was compiled.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// The URL the vendor spec was fetched from. `None` for a fully hand-authored connector — the
    /// Ollama case, where no vendor OpenAPI document exists at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// The upstream version string the vendor published for that spec.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_version: Option<String>,
    /// When the spec was fetched, as an RFC 3339 timestamp. A string, not a date type: this crate
    /// takes no new dependency for a field nothing here does arithmetic on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    /// SHA-256 of the vendored spec bytes under `specs/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_sha256: Option<String>,
    /// SHA-256 of the provider TOML bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toml_sha256: Option<String>,
}

/// One operation: a single HTTP call, and everything a Flux `op` declaration needs to wrap it.
///
/// `description`, `risk` and `idempotency` map straight onto the metadata a Flux composite op
/// declares (`op … description "…" risk "low" idempotency "idempotent"`), which is also the
/// `ToolSpec` surface flux exposes to a model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    /// The op name, e.g. `babelforce.call.list`. This is a **stable public contract**: users and
    /// models call it by name, so it must survive regeneration and must not be derived from a
    /// volatile spec field like `operationId` without a pinned override.
    pub id: String,
    /// The HTTP method.
    pub method: HttpMethod,
    /// The path template, relative to the connector's base URL (`/v2/calls/{call_id}`).
    pub path: String,
    /// What the operation does, in one line. Reaches the model as the tool description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// How much damage the operation can do. See [`Risk`].
    pub risk: Risk,
    /// Whether repeating it is safe. See [`Idempotency`].
    pub idempotency: Idempotency,
    /// Which auth this operation requires, as a set of **alternatives** (OR); each alternative is
    /// an [`AuthRequirement`] — one mechanism — whose credentials must all be satisfied together
    /// (AND).
    ///
    /// The `Option` is the whole point, and it is OpenAPI's operation-level `security` semantics
    /// exactly:
    ///
    /// - `None` — **unset**. The operation inherits [`Connector::default_auth`]. Encodes by being
    ///   omitted entirely.
    /// - `Some(vec![])` — **explicitly none**. The operation needs no auth at all: a health or ping
    ///   endpoint. It does *not* inherit the connector default. Encodes as `[]`.
    /// - `Some(vec![a, b])` — either `a` or `b` authenticates the request.
    ///
    /// Collapsing the two empty cases into one would make an unauthenticated endpoint
    /// inexpressible on a connector that has a default, which is most of them. Use
    /// [`Connector::effective_auth`] to resolve the inheritance rather than reading this directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Vec<AuthRequirement>>,
    /// The request parameters, grouped by position.
    #[serde(default, skip_serializing_if = "ParamSet::is_empty")]
    pub params: ParamSet,
    /// The JSON Schema of a successful response body, when the spec publishes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<JsonSchema>,
    /// The ways this endpoint departs from its spec.
    #[serde(default, skip_serializing_if = "Quirks::is_empty")]
    pub quirks: Quirks,
}

/// A whole connector: the normalized form of one provider, whether it came from a vendor OpenAPI
/// document or was hand-authored in TOML.
///
/// The two front-ends produce this same shape — spec ingest merely *pre-fills* it — which is what
/// lets a vendor with no usable spec travel the identical codegen path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connector {
    /// The connector id, e.g. `babelforce`. Prefixes every operation id and names the generated
    /// `<id>.flux` and `<id>.connector.toml`.
    pub id: String,
    /// The vendor's display name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vendor: String,
    /// The API base URL, which may carry tenant templating (`https://{tenant}.babelforce.com`).
    pub base_url: String,
    /// What the connector is for, in one line.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Every credential this connector declares, each keyed by its [`AuthMethod::name`].
    ///
    /// A `Vec` rather than a map, mirroring the plugin manifest's `Vec<AuthMethod>`: the name
    /// already lives inside the method, so a keyed map would store it twice and invite the two
    /// copies to disagree. Look one up with [`auth_method`](Self::auth_method).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth: Vec<AuthMethod>,
    /// The connector-wide default requirement list — OpenAPI's document-level `security`.
    ///
    /// Every operation that does not declare its own [`Operation::auth`] inherits this. An empty
    /// list means the connector is unauthenticated by default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_auth: Vec<AuthRequirement>,
    /// The operations this connector exposes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<Operation>,
    /// Where this connector came from.
    #[serde(default, skip_serializing_if = "provenance_is_empty")]
    pub provenance: Provenance,
}

/// `skip_serializing_if` helper — a fully empty [`Provenance`] adds nothing to the encoding.
fn provenance_is_empty(provenance: &Provenance) -> bool {
    *provenance == Provenance::default()
}

impl Connector {
    /// The auth alternatives that actually apply to an operation, resolving the inheritance rule.
    ///
    /// An operation that declares nothing inherits [`default_auth`](Self::default_auth); one that
    /// declares an explicit empty list gets an empty slice and inherits nothing. This is the only
    /// correct way to read [`Operation::auth`], and C-10 resolves credentials through it.
    pub fn effective_auth<'a>(&'a self, operation: &'a Operation) -> &'a [AuthRequirement] {
        match operation.auth.as_deref() {
            Some(declared) => declared,
            None => &self.default_auth,
        }
    }

    /// The declared credential of that name, or `None` when nothing declares it.
    ///
    /// A requirement naming an undeclared credential is an authoring error the loader rejects
    /// (C-3); codegen must never invent one, because that would be flux-connectors deciding on its
    /// own how to spend a credential.
    pub fn auth_method(&self, name: &str) -> Option<&AuthMethod> {
        self.auth.iter().find(|method| method.name == name)
    }

    /// An operation by id.
    pub fn operation(&self, id: &str) -> Option<&Operation> {
        self.operations.iter().find(|op| op.id == id)
    }

    /// The connector's canonical JSON encoding: compact, and byte-identical for equal values.
    ///
    /// This is the connector's **complete** encoding, provenance included, and it is what a round
    /// trip through disk must preserve. It is deliberately *not* what `connectors.lock` hashes —
    /// see [`hash_domain`](Self::hash_domain) for why. Every ordering decision in this module
    /// exists to make this function total in the mathematical sense: the same value in, the same
    /// bytes out, on every machine and every run.
    pub fn canonical_json(&self) -> crate::Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// The exact bytes [`ir_sha256`](Self::ir_sha256) hashes — the **hash domain**, stated as a
    /// value rather than left implicit in whatever `Serialize` happens to emit.
    ///
    /// # What is in it
    ///
    /// The connector's *compiled meaning*: `id`, `vendor`, `base_url`, `description`, `auth`,
    /// `default_auth`, `operations`. Change any of those and the generated `.flux` module changes,
    /// so the hash must move.
    ///
    /// # What is out of it, and why
    ///
    /// **All of [`Provenance`]** — including `fetched_at`, which C-2's review caught leaking into
    /// [`canonical_json`](Self::canonical_json). Provenance records *where the bytes came from*;
    /// this hash records *what was compiled from them*. Two reasons, and the first is the one the
    /// story exists for:
    ///
    /// 1. `fetched_at` moves on every re-fetch of a byte-identical document, so hashing it would
    ///    report drift where nothing drifted — phantom drift on every build, which is precisely
    ///    what `connectors.lock` is supposed to rule out.
    /// 2. `source_url`, `upstream_version`, `spec_sha256` and `toml_sha256` are recorded verbatim
    ///    as their own fields of the lockfile entry
    ///    ([`LockEntry`](crate::lock::LockEntry)), and `check` (C-14) verifies each of them
    ///    directly. Folding them in here would hash the same facts twice and would make a
    ///    comment-only edit to a provider TOML — which changes `toml_sha256` and not one generated
    ///    byte — move the IR hash as well. Each recorded hash covers exactly one input; that is
    ///    what lets `check` name *which* input moved.
    ///
    /// A [`Connector`] field added later is a compile error inside this function until someone
    /// decides which side of that line it falls on.
    pub fn hash_domain(&self) -> crate::Result<String> {
        Ok(serde_json::to_string(&HashDomain::of(self))?)
    }

    /// Lowercase-hex SHA-256 of [`hash_domain`](Self::hash_domain) — the `ir_sha256` of a
    /// [`LockEntry`](crate::lock::LockEntry).
    pub fn ir_sha256(&self) -> crate::Result<String> {
        Ok(crate::lock::sha256_hex(self.hash_domain()?.as_bytes()))
    }
}

/// The serialized shape of the hash domain: an explicit projection of [`Connector`], not the
/// connector itself. See [`Connector::hash_domain`] for what it includes and why.
///
/// Borrowed rather than cloned so that hashing is allocation-cheap, and with no
/// `skip_serializing_if` of its own — the fields it names are always present, and the types it
/// borrows already encode canonically.
#[derive(Serialize)]
struct HashDomain<'a> {
    id: &'a str,
    vendor: &'a str,
    base_url: &'a str,
    description: &'a str,
    auth: &'a [AuthMethod],
    default_auth: &'a [AuthRequirement],
    operations: &'a [Operation],
}

impl<'a> HashDomain<'a> {
    fn of(connector: &'a Connector) -> Self {
        // The exhaustive destructuring is the tripwire, and it is the whole reason this is a
        // separate type instead of a `serde` attribute: a field added to `Connector` fails to
        // compile here until someone states whether it belongs in the hash domain. Silently
        // inheriting the decision is how `fetched_at` got into the hash in the first place.
        let Connector {
            id,
            vendor,
            base_url,
            description,
            auth,
            default_auth,
            operations,
            provenance,
        } = connector;

        // Deliberately excluded — see `Connector::hash_domain`.
        let _excluded = provenance;

        Self {
            id,
            vendor,
            base_url,
            description,
            auth,
            default_auth,
            operations,
        }
    }
}
