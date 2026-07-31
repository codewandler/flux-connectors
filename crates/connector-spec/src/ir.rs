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

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::auth::{AuthMethod, AuthRequirement};
use crate::config::ConfigField;
use crate::graph::Graph;
use crate::inbound::{ChannelBinding, EventDecl};

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
    /// The **caller-facing** name: what the generated op declares, and what a model passes.
    ///
    /// It is also the wire name unless [`wire`](Self::wire) says otherwise, which is the common
    /// case — a vendor that names a field the same thing its callers do needs no second name.
    pub name: String,
    /// The spelling the **vendor** sees, when it differs from [`name`](Self::name).
    ///
    /// Two shapes, one field, because they are the same fact — *where this value goes on the
    /// request* — asked of two different request positions:
    ///
    /// - a **body** field: the dot-separated JSON path it occupies in the request body.
    ///   Zendesk's comment text is `ticket.comment.body`, not `body`
    ///   (`docs/designs/provider-operation-inventory.md` §3.3.1); babelforce's agent-status update
    ///   writes `presence.name`. Without this the field is emitted at the root of the body, which
    ///   Zendesk **accepts and ignores** — a wrong result that answers 200, which is why this is a
    ///   correctness field and not a convenience one.
    /// - a **path, query or header** parameter: a plain alias, the vendor's own name for a
    ///   parameter whose caller-facing name reads better. Freshdesk's requester filter is
    ///   `req_id` to a caller and `requester_id` on the wire (inventory §4.2 op 2).
    ///
    /// A single field rather than a `body_path` and an `alias`: a JSON path with one segment *is*
    /// an alias, so two fields would be two spellings of one idea, and codegen would have to decide
    /// what a parameter carrying both meant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire: Option<String>,
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

/// The caller-facing name of the one parameter a free-form body travels in.
///
/// A convention rather than an IR field: [`ParamSet::body_schema`] says the body *is* a schema and
/// names nothing, so the parameter needs a name from somewhere. It lives here, on the IR, because
/// two surfaces now have to agree on it — `connector-flux` declares the emitted `op`'s body
/// parameter under this name, and [`Operation::input_schema`] composes the property under it. A
/// second spelling of the constant would be the two surfaces asking for one body by two names.
///
/// It is a *starting* name on the Flux side: the emitter allocates symbols through one allocator,
/// so an operation that already binds `body` elsewhere gets a disambiguated symbol rather than a
/// silently shadowed one.
pub const FREE_FORM_BODY: &str = "body";

/// How an operation's request body is encoded on the wire — C-144.
///
/// # Why it exists
///
/// Stripe parses `application/x-www-form-urlencoded` and nothing else, and so do Twilio, Mailgun,
/// PayPal classic, and **every OAuth2 token endpoint by specification** (RFC 6749 §4.3.2). Before
/// this existed the pipeline bound one media type unconditionally, which is why
/// `providers/stripe.toml` selects only operations that address everything they need in the path:
/// a Stripe write with a body field would have sent a JSON document Stripe does not parse.
///
/// # Why it is a closed enum
///
/// An open media-type string is a header nobody validates, so a typo ships a body the vendor
/// accepts and silently ignores — the failure mode this whole module is arranged against. A closed
/// set makes an unknown spelling a **load error**, and it also means the value can never carry
/// anything but one of these two words: a declared encoding cannot become a second, ungated route
/// for a credential into a request header, which is what a free-text `content_type` key would have
/// been.
///
/// # Why it sits on [`ParamSet`] rather than on a service or a provider
///
/// The axis is per-request, not per-vendor: Stripe's API is form-encoded while its *webhook*
/// payloads are JSON, and an OAuth2 token grant is form-encoded on a vendor whose whole business
/// API is JSON. It lives next to [`body`](ParamSet::body) and
/// [`body_schema`](ParamSet::body_schema) because it describes exactly what those declare and
/// nothing else.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyEncoding {
    /// A JSON document — `application/json`. The default, so no shipped provider had to change.
    #[default]
    Json,
    /// `key=value&key=value` — `application/x-www-form-urlencoded`.
    ///
    /// It has **no agreed nesting convention** (Stripe writes `metadata[key]`, PHP writes `a[b]`,
    /// Rails writes `a[b][]`, and a token grant nests nothing), so `connector-flux` refuses a nested
    /// body under this encoding rather than picking one of them silently.
    Form,
}

impl BodyEncoding {
    /// The spelling a provider file's `body_encoding` key carries.
    ///
    /// The serde tag, restated as a value so an error message can name what the author wrote.
    /// `tests/ir_roundtrip.rs` holds the two together, so a variant added with a mismatched word
    /// fails rather than producing an error message that names a key nobody can find.
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Form => "form",
        }
    }

    /// The media type this encoding travels under, which the emitter puts in `content-type`.
    ///
    /// A method on the enum rather than a table in the emitter: the encoding and the media type are
    /// one fact, and separating them would let a future variant exist with no answer to "what does
    /// the vendor see".
    pub fn media_type(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Form => "application/x-www-form-urlencoded",
        }
    }

    /// Whether this is the default, in which case it is omitted from every serialization.
    ///
    /// That omission is what keeps every committed artifact byte-identical across C-144: a
    /// `body_encoding` nobody declared does not appear in a manifest, in the lockfile's hash domain,
    /// or in the published catalogue.
    pub fn is_json(&self) -> bool {
        matches!(self, Self::Json)
    }
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
    ///
    /// A header the *vendor* fixes is [`const_headers`](Self::const_headers), not one of these with
    /// a `const` in its schema: this list means caller-supplied, and reinterpreting one entry of it
    /// by keyword would make a single declaration mean two things. `connector-flux` refuses the
    /// pinned spelling rather than honouring it (C-55).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header: Vec<Param>,
    /// Request headers the **vendor** fixes: `Accept: application/vnd.github+json`,
    /// `Notion-Version: 2022-06-28`, an API version, a `User-Agent`.
    ///
    /// Not parameters, which is why they are a map of literals rather than [`Param`]s and why
    /// [`iter`](Self::iter) does not yield them: nothing is caller-supplied here, nothing reaches
    /// the emitted `op`'s signature, and a model is never asked to guess a value the vendor has
    /// already decided. The emitter binds each value as a literal and puts it in the request's
    /// header record (C-55).
    ///
    /// **Declared at two levels, resolved to one.** A provider states a header once — the file's
    /// top-level `[const_headers]` — and the loader distributes it onto every operation, an
    /// operation's own entry replacing the provider's when they name the same header (HTTP field
    /// names are case-insensitive, so `Notion-Version` and `notion-version` are one header and never
    /// two). So this map is always the *complete* set the operation sends, and no consumer has to
    /// resolve an inheritance to know what travels.
    ///
    /// **It can never carry a credential.** A value here is a literal in a committed artifact, so a
    /// secret placed in one would be a secret in the repository — the one thing generated data must
    /// never hold (AGENTS.md). The loader refuses a value that resolves from an environment variable
    /// or names a declared credential, and refuses a header name the `$auth` seam owns; `Authorization`
    /// is C-10's business and this field must not become a second, ungated path to it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub const_headers: BTreeMap<String, String>,
    /// Fields assembled into the JSON request body, each at the JSON path its
    /// [`Param::wire`] names (or at the root of the body when it names none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<Param>,
    /// The body **is** this schema, rather than being assembled from named fields.
    ///
    /// A free-form object body: `babelforce-call-session-set` and `babelforce-session-update` both
    /// take `{"type": "object"}` with no properties — a map of caller-chosen keys
    /// (`docs/designs/provider-operation-inventory.md` §6.5). A field list cannot describe that, and
    /// an operation that declared no body field at all emitted a `PUT` with **no body**, which is
    /// indistinguishable from a legitimately bodiless write.
    ///
    /// Mutually exclusive with [`body`](Self::body): "the body is these fields" and "the body is
    /// this schema" are two answers to one question, and nothing states how to merge them. Codegen
    /// refuses an operation that declares both rather than picking one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_schema: Option<JsonSchema>,
    /// How the body above is encoded on the wire. See [`BodyEncoding`].
    ///
    /// Defaulted rather than mandatory, unlike [`Risk`] and [`Idempotency`]: silence here is not a
    /// safety decision, it is the answer nineteen of nineteen shipped providers already give, and
    /// making it explicit everywhere would be nineteen files restating one fact. What silence must
    /// not do is *change* — so `json` is the default and the emitter's output for an operation that
    /// declares nothing is byte-for-byte what it was before this field existed.
    #[serde(default, skip_serializing_if = "BodyEncoding::is_json")]
    pub body_encoding: BodyEncoding,
}

impl ParamSet {
    /// Whether the operation takes no parameters at all.
    ///
    /// `body_schema` counts: an operation whose only body declaration is a schema would otherwise
    /// encode as an absent `params` and lose the whole body on the way back. So does
    /// [`const_headers`](Self::const_headers), for the same reason — an operation whose only request
    /// declaration is a pinned header would lose it on the way back, and the emitted module would
    /// stop sending the header the vendor requires. And so does a non-default
    /// [`body_encoding`](Self::body_encoding): the emitter refuses that shape rather than emitting
    /// it, so it cannot ship — but a round trip that dropped the declaration would turn a loud
    /// refusal into a silent JSON body, which is the wrong direction to fail in.
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
            && self.query.is_empty()
            && self.header.is_empty()
            && self.body.is_empty()
            && self.body_schema.is_none()
            && self.const_headers.is_empty()
            && self.body_encoding.is_json()
    }

    /// Every parameter, in request-position order: path, query, header, body.
    ///
    /// [`body_schema`](Self::body_schema) is deliberately absent — it is a schema, not a [`Param`],
    /// and a caller iterating parameters is asking about named ones.
    /// [`const_headers`](Self::const_headers) is absent for the stronger version of that reason:
    /// nothing about it is caller-supplied.
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

/// The name of the implicit service every operation belongs to unless it names another.
///
/// **Reserved.** No `[[services]]` entry may declare it (the loader refuses one), because it is the
/// name of the service that exists without being declared — a declaration of it would be a second,
/// contradictable definition. It is also **elided from every rendered address**
/// ([`crate::address::Gid`]): `default` is an internal name for "this provider has one API surface",
/// and an address is a promise that would have to be broken the day the provider grows a second one.
pub const DEFAULT_SERVICE: &str = "default";

/// A named capability shape a [`Service`] claims to implement, checked at load — C-120.
///
/// Seventeen connectors share structure that nothing named. `openai` and `openrouter` both list
/// models; `zendesk`, `freshdesk`, `intercom` and `jira` all show a ticket. Nothing in the IR said
/// so, so nothing could act on it: a UI could not group them, and a second vendor filling the same
/// shape was a coincidence rather than a contract. A role is that missing declaration. See
/// [`docs/designs/provider-roles.md`](../../../docs/designs/provider-roles.md).
///
/// # Why it is closed
///
/// An open string set is a tag system, and a tag system cannot be checked. A closed enum can, and
/// the checking is the point: `llm_catalogue` is a *promise the loader enforces*, so a consumer
/// reading the catalogue can rely on it without reading the provider's TOML. It also makes an
/// unknown name a **load error** rather than a silent no-op, which is the failure mode this whole
/// mechanism exists to prevent — a typo'd capability that quietly means "no capability" is invisible
/// to everyone downstream.
///
/// # Why it attaches to a service
///
/// `openai` is not "an LLM provider": it exposes a management surface (`models`) and an inference
/// surface (`chat`), which are different capabilities with different risk and different idempotency.
/// C-49's service level is exactly that granularity. A role declared on the *provider* would smear
/// the two together, so a provider's roles are **derived** ([`Connector::roles`]) and never authored
/// — the same rule [`Level`](crate::config::Level) follows against `binds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// A live list of the models a vendor serves — the half of the LLM story a connector may own.
    ///
    /// flux's own model metadata lives in static tables that go stale the moment a vendor ships a
    /// model; `openai-models-list` is live. So a connector *informs* the pool of `(provider, model)`
    /// tuples while `flux-providers` keeps *serving* it. Serving inference from a connector is a
    /// stated non-goal (C-123 is the charter decision), which is why there is no `llm_inference`
    /// variant here.
    LlmCatalogue,
}

impl Role {
    /// Every role there is, in the order an error message lists them.
    ///
    /// A `const` rather than a derived iterator so that the "known set" a refusal prints and the set
    /// the loader accepts are the same value, and a variant added without extending this fails the
    /// exhaustive `match` below rather than silently going unlisted.
    pub const ALL: [Self; 1] = [Self::LlmCatalogue];

    /// The token this role serializes as, for error text.
    pub fn word(self) -> &'static str {
        match self {
            Self::LlmCatalogue => "llm_catalogue",
        }
    }

    /// The **operations** a service must have to claim this role.
    ///
    /// Named by **member name within the service** (`list`), never by full operation id
    /// (`openai-models-list`) — see [`fills_slot`] for what "within the service" means and why it is
    /// what makes a role vendor-independent. Only an operation fills a slot; see
    /// [`Connector::missing_role_members`] for why an event does not.
    pub fn required_members(self) -> &'static [&'static str] {
        match self {
            Self::LlmCatalogue => &["list"],
        }
    }
}

/// Whether an operation's name fills one of a role's required-member slots.
///
/// A role requires `list`, not `openai-models-list`. The slot is matched against the member's
/// **trailing name segments**, so `openai-models-list` and `openrouter-models-list` fill the same
/// slot and the vendor prefix nobody agreed on stays out of the contract — which is the whole reason
/// a role is worth declaring rather than inferring.
///
/// Segments split on `-`, `_` and `.`: those are the three separators a member name admits (see
/// [`Connector::member_names_of`]), so a role naming `comment.list` and a member named
/// `zendesk-ticket-comment-list` are the same slot. The match is on whole segments and never on a
/// substring, or `acme-blocklist-get` would satisfy a role it has nothing to do with.
///
/// **A one-segment slot is loose**, and known to be: a bare `list` is a suffix nine of seventeen
/// shipped providers contain somewhere. That is a weakness of how a role spells its slots, not of the
/// match, and C-121 addresses it by giving a slot a tighter spelling (`models.list`) and a set of
/// accepted ones. Multi-segment slots are already tight.
fn fills_slot(member: &str, slot: &str) -> bool {
    const SEPARATORS: [char; 3] = ['-', '_', '.'];
    let member: Vec<&str> = member.split(SEPARATORS).collect();
    let slot: Vec<&str> = slot.split(SEPARATORS).collect();
    member.len() >= slot.len() && member[member.len() - slot.len()..] == slot[..]
}

/// One API surface of a provider: the unit you address, version, select and install.
///
/// `s3` and `bedrock-runtime` under AWS; `support` under Zendesk. A service is the middle level of
/// C-37's address scheme, promoted from an anonymous path segment to a named thing with an owner —
/// see [`docs/designs/provider-services.md`](../../../docs/designs/provider-services.md).
///
/// **This is not a tag.** A free-form label cannot do the three things this level exists for: it
/// cannot *partition* (so "install the whole s3 service" would not denote a set), it cannot carry a
/// *version* (so `s3:2006-03-01` would have to be repeated on every operation and could disagree with
/// itself), and it cannot carry a *host* (so a service could not have its own egress surface).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    /// The service name, e.g. `s3`. Names the emitted `<provider>-<name>.flux`, is the first path
    /// segment of the service's gid, and is what [`Operation::service`] references.
    pub name: String,
    /// What the service is for, in one line.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The service's own API base URL, when it differs from the connector's.
    ///
    /// AWS is the motivating case: `s3.amazonaws.com` and `bedrock-runtime.<region>.amazonaws.com`
    /// share an authority and not a host. Resolve it with [`Connector::base_url_of`] rather than
    /// reading this directly — the connector's value is the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// The vendor's API version for *this* service, when it differs from the connector's.
    ///
    /// AWS versions each service on its own date (`s3:2006-03-01`,
    /// `bedrock-runtime:2023-09-30`), which is why a single connector-level version cannot describe a
    /// multi-service provider. Resolve it with [`Connector::api_version_of`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    /// The capability shapes this service claims to implement — see [`Role`].
    ///
    /// The loader checks every claim: a service declaring a role it does not satisfy is refused,
    /// naming the member that is missing. That check is what makes the declaration worth anything to
    /// a consumer.
    ///
    /// A **provider's** roles are the union of its services' ([`Connector::roles`]) and are never
    /// authored — a provider-level `roles` key is a load error, because a value that is both derived
    /// and writable is two sources of truth waiting to disagree.
    ///
    /// `skip_serializing_if` for the same reason the three fields above carry it: a service that
    /// claims no role must hash to what it hashed before roles existed, or landing C-120 moves every
    /// `ir_sha256` in the repository and churns `connectors.lock` for a provider nobody edited.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<Role>,
}

/// **The shortest justification that counts as one**, in characters after trimming.
///
/// Not a measure of truth — nothing here can check whether a vendor really deduplicates — but a
/// floor on effort, and the number is calibrated rather than invented: it is the length of
/// `"purging twice is a no-op"`, the shortest honest reason anyone working on C-186 actually wrote.
/// Below it live `"yes"`, `"idempotent"` and `"see above"`, which unlock the claim while telling a
/// reviewer nothing, and an escape hatch that costs nothing is a deleted guard.
pub const MIN_IDEMPOTENCY_JUSTIFICATION: usize = 24;

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
    /// The [`Service`] this operation belongs to — **exactly one**, always a concrete name.
    ///
    /// A `String` rather than an `Option<String>`: `None` and `Some("default")` would be two
    /// spellings of one meaning, which is the objection [`AuthRequirement`] already records against an
    /// empty mechanism inside a non-empty alternatives list. "Unset" exists in the *file*, where it
    /// deserializes to [`DEFAULT_SERVICE`]; the IR always names a service.
    ///
    /// Not a set, either. A set would leave the gid ambiguous (which segment renders?) and would make
    /// selection non-partitioning, so "the s3 service" would stop being answerable by set membership.
    /// An operation that genuinely serves two services is duplicated deliberately with two ids —
    /// which is visible in a diff — rather than resolved by a rule nobody can see.
    #[serde(
        default = "default_service",
        skip_serializing_if = "is_default_service"
    )]
    pub service: String,
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
    /// **Why this `POST` or `PATCH` is idempotent when its method is not** — the one thing that
    /// unlocks [`Idempotency::Idempotent`] on a method RFC 9110 §9.2.2 does not make idempotent.
    ///
    /// `connector-flux` refuses `idempotent` on `POST` and `PATCH` by method, and that refusal is
    /// correct almost every time: most `POST`s create something, and a `retry` wrapped around one
    /// that does is unsound. It was also correct for `cloudflare-cache-purge` (purging an
    /// already-purged cache is a no-op) and `launchdarkly-flag-toggle` (a `replace` onto the same
    /// boolean), which are idempotent by their vendors' own behaviour. Both shipped
    /// `non_idempotent` with a comment stating the opposite, and **a comment is not what a host
    /// reads** — the field travels to flux's `ToolSpec`, and a host deciding whether a retry is safe
    /// reads the field (C-186).
    ///
    /// So the guard stays and gains an addressee. Stating a reason is the whole cost of the claim,
    /// and it is deliberately not free:
    ///
    /// - **Silence still refuses.** An author who copies a read's metadata onto a write says
    ///   nothing here, so the build fails exactly as it did before. The careless case is the case
    ///   the method rule was always for.
    /// - **The reason must be one.** Blank, or shorter than [`MIN_IDEMPOTENCY_JUSTIFICATION`], is
    ///   refused: an escape hatch anyone can take without saying anything is a removal of the guard
    ///   wearing its clothes. What no compiler can check is whether the sentence is *true* — that is
    ///   what publishing it into `web/public/catalog.json`, where a reviewer reads it beside the
    ///   claim it licenses, is for.
    /// - **It is refused where nothing was refusing the claim.** `GET`, `PUT` and `DELETE` may
    ///   declare `idempotent` freely, so a justification on one addresses no refusal; and a
    ///   justification on an operation not claiming `idempotent` is prose contradicting its own
    ///   field, which is the defect this story exists to remove, arriving backwards.
    ///
    /// `Option`, not a defaulted `String`: "stated nothing" and "stated the empty string" must not
    /// be two spellings of one thing, and `skip_serializing_if` keeps every `ir_sha256` in the
    /// repository exactly where it was for the operations that do not use it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotent_because: Option<String>,
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

impl Operation {
    /// **The justification that licenses `idempotent` on this operation**, or `None`.
    ///
    /// One reading of [`Operation::idempotent_because`], shared by the loader (which refuses a file
    /// stating a reason that says nothing) and by `connector-flux`'s `check_write_metadata` (which
    /// must not trust an IR assembled in memory rather than loaded). Two implementations of "is this
    /// a real reason" would be two places for the floor to drift apart, and the emitter is the one
    /// that has to be right.
    ///
    /// Returns the trimmed reason, so a caller never publishes an author's stray whitespace.
    pub fn idempotency_justification(&self) -> Option<&str> {
        let reason = self.idempotent_because.as_deref()?.trim();
        (reason.chars().count() >= MIN_IDEMPOTENCY_JUSTIFICATION).then_some(reason)
    }

    /// Whether this operation *states* a justification at all, however poor.
    ///
    /// The difference from [`Operation::idempotency_justification`] is what the loader needs to tell
    /// an author apart from silence: "you wrote nothing" and "you wrote `yes`" are different
    /// mistakes and deserve different refusals, and a field present-but-rejected must never read as
    /// a field absent.
    pub fn states_idempotency_justification(&self) -> bool {
        self.idempotent_because.is_some()
    }

    /// **One JSON Schema describing everything the operation receives.**
    ///
    /// Derived, never authored: there is no `input_schema` key in a provider file, and one is a
    /// load error — `Operation` is `deny_unknown_fields` and the editor-facing
    /// [`crate::PROVIDER_TOML_JSON_SCHEMA`] closes the operation object as well. The same rule
    /// `Level` follows in `docs/designs/connector-configuration.md`, for the same reason: a value
    /// that is both derived and writable is two sources of truth waiting to disagree.
    ///
    /// # Why this exists at all
    ///
    /// The data was already here — [`Param::schema`] on every parameter, plus
    /// [`ParamSet::body_schema`] — but split across four request positions with nothing saying what
    /// the operation *receives*. Every consumer therefore composed its own, and each disagreed at
    /// the corners: whether a path parameter belongs in the same object as a body field, what
    /// `required` means, how a free-form body merges with named ones. One composition settles it.
    ///
    /// # The rules, each of which was a corner someone got wrong
    ///
    /// - **Keyed by the caller-facing [`Param::name`], never the wire name.** The vendor's own
    ///   spelling stays exactly where it already lives, in [`Param::wire`]; a schema keyed by it
    ///   would name arguments no caller passes. Where the caller-facing name is not a spellable
    ///   Flux symbol — babelforce's `time.start` — the emitted `op` declares a normalized symbol
    ///   instead, and that mapping stays in `connector-flux`'s `names` module, which owns it.
    /// - **Each property is the declared schema, verbatim.** This composes; it does not enrich.
    ///   Folding [`Param::description`] in would be this function improving a vendor's declaration,
    ///   and raising declaration quality is its own work.
    /// - **A free-form [`ParamSet::body_schema`] is one property, [`FREE_FORM_BODY`]** — the same
    ///   name the emitted `op` declares it under, so both surfaces ask for the body by one name.
    ///   It is required: the body *is* the request for the operations that declare one.
    /// - **`required` is exactly the parameters marked [`Param::required`].** Not everything (which
    ///   is what a consumer reading the emitted Flux must say, since flux has no optional
    ///   composite-op parameter) and not nothing.
    /// - **[`ParamSet::const_headers`] are absent.** They are not parameters and nothing about them
    ///   is caller-supplied, exactly as [`ParamSet::iter`] already has it.
    /// - **An operation with no parameters composes an empty object schema, not absence.** "This
    ///   takes nothing" is a derived answer and deserves to be stated; the output side publishes
    ///   absence instead precisely because "we do not know" is *not* an answer
    ///   (`docs/designs/member-io-schemas.md`).
    ///
    /// # The merge rule for a body declared twice
    ///
    /// Named [`ParamSet::body`] fields and a free-form [`ParamSet::body_schema`] are two answers to
    /// one question, and **the answer is that an operation declaring both is refused at load**
    /// (`provider::load`; `connector-flux` refuses it a second time at emission). So the case does
    /// not arise for any connector that came through the loader, and this function does not invent
    /// a merge for it. An IR assembled in memory can still carry both, and then both appear here —
    /// stated so it is not a surprise, and deliberately *not* a rule that drops one silently, which
    /// would make the refusal look optional.
    pub fn input_schema(&self) -> JsonSchema {
        let mut properties = serde_json::Map::new();
        let mut required: Vec<JsonSchema> = Vec::new();

        for param in self.params.iter() {
            properties.insert(param.name.clone(), param.schema.clone());
            if param.required {
                required.push(JsonSchema::String(param.name.clone()));
            }
        }
        if let Some(schema) = &self.params.body_schema {
            properties.insert(FREE_FORM_BODY.to_owned(), schema.clone());
            required.push(JsonSchema::String(FREE_FORM_BODY.to_owned()));
        }

        serde_json::json!({
            "type": "object",
            "properties": JsonSchema::Object(properties),
            "required": JsonSchema::Array(required),
        })
    }
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
    /// The reverse-DNS authority this provider publishes under (`com.amazonaws`) — the `pid` of
    /// C-37's address scheme, and the leading component of every service gid.
    ///
    /// Optional: a connector that declares none has no rendered address at all. Every shipped
    /// provider was in that state until `slack` needed one — a channel binding's reply renders as an
    /// oip, and an address missing its authority is not an address. See [`crate::address`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    /// The vendor's API version for this connector, as the *default* for its services.
    ///
    /// A multi-service provider overrides it per [`Service`]; a single-surface provider states it
    /// once here. It is the vendor's version, never ours — our own connector version lives in
    /// `connectors.lock` — so an address stays stable across our regenerations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    /// The API surfaces this provider exposes, each owning its operations.
    ///
    /// Empty means the provider has exactly one surface, the reserved [`DEFAULT_SERVICE`], which no
    /// entry declares and which is elided from every rendered address. A provider that declares any
    /// service must place **every** operation in one of them — an operation that omits `service` in
    /// such a file is a loader error, not an implicit `default` sitting beside the declared ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<Service>,
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
    /// The operations this connector exposes — the **outbound** call direction.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<Operation>,
    /// The events this connector receives — the **inbound** call direction.
    ///
    /// An operation is flux calling the vendor; an [`EventDecl`] is the vendor calling flux. Both are
    /// members of a [`Service`], and both share one name namespace within it — see
    /// [`member_names_of`](Self::member_names_of).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<EventDecl>,
    /// The ingress surfaces this connector describes, each composing events with a reply operation.
    ///
    /// The third member kind, and the one that is not a primitive: see [`crate::inbound`] for why a
    /// [`ChannelBinding`] is a composition of the two above rather than a thing of its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<ChannelBinding>,
    /// What a **human** must supply before any of the above can run — see [`crate::config`].
    ///
    /// Not a member kind in the addressing sense: a configuration field is not something a program
    /// calls or a trigger matches, it is a question a settings page asks. It shares the member
    /// namespace anyway, because it shares the *host's* namespace — a config value and an operation
    /// resolving to one name would be ambiguous wherever a host looks either up.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config: Vec<ConfigField>,
    /// The flows this connector composes from its own members — see [`crate::graph`].
    ///
    /// A graph is the only member kind that is itself a *composition of members*: its nodes name
    /// operations, events and channel bindings this connector already declares. It lowers to one Flux
    /// `op`, which is the only declaration flux lifts from a module.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graphs: Vec<Graph>,
    /// The operation a host calls to prove the configuration works — a "Test connection" button.
    ///
    /// Read-shaped and cheap by construction: `freshdesk-test` is a bounded contact read,
    /// `babelforce-agent-list` doubles as one. Both already existed as a convention nothing declared,
    /// so nothing could use them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<String>,
    /// Where this connector came from.
    #[serde(default, skip_serializing_if = "provenance_is_empty")]
    pub provenance: Provenance,
}

/// `skip_serializing_if` helper — a fully empty [`Provenance`] adds nothing to the encoding.
fn provenance_is_empty(provenance: &Provenance) -> bool {
    *provenance == Provenance::default()
}

/// `serde(default)` for [`Operation::service`]: an operation that names none belongs to the reserved
/// [`DEFAULT_SERVICE`]. Shared with the two other member kinds in [`crate::inbound`], which carry the
/// same field for the same reason.
pub(crate) fn default_service() -> String {
    DEFAULT_SERVICE.to_owned()
}

/// `skip_serializing_if` for [`Operation::service`], so that omitting the key and writing
/// `service = "default"` produce **identical bytes**. One meaning, one encoding — and the encoding of
/// every operation shipped before services existed is unchanged, which is what keeps
/// `connectors.lock` from churning for a connector nobody edited.
pub(crate) fn is_default_service(service: &str) -> bool {
    service == DEFAULT_SERVICE
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

    /// Every service name this connector has, in declaration order.
    ///
    /// A connector that declares none has exactly one service, the reserved [`DEFAULT_SERVICE`]. This
    /// is the set an operation's [`service`](Operation::service) must be a member of — the loader
    /// enforces it — and therefore the set that makes the partition below total.
    pub fn service_names(&self) -> Vec<&str> {
        if self.services.is_empty() {
            vec![DEFAULT_SERVICE]
        } else {
            self.services
                .iter()
                .map(|service| service.name.as_str())
                .collect()
        }
    }

    /// The declared [`Service`] of that name, or `None`.
    ///
    /// `None` for [`DEFAULT_SERVICE`] on a connector that declares no services: the implicit service
    /// has no declaration by construction. Use [`service_names`](Self::service_names) to ask *which*
    /// services exist and the resolvers below to ask about their fields.
    pub fn service(&self, name: &str) -> Option<&Service> {
        self.services.iter().find(|service| service.name == name)
    }

    /// Whether this connector has exactly one service and it is the reserved [`DEFAULT_SERVICE`].
    ///
    /// The shape of every provider shipped today, and the shape whose emitted artifacts are named
    /// `<provider>.flux` rather than `<provider>-<service>.flux`.
    ///
    /// Declaring no services at all is the usual spelling, but not the only one: C-120 lets a
    /// single-surface provider write a `[[services]]` entry named [`DEFAULT_SERVICE`] **to carry
    /// roles**, because a role attaches to a service and there is no other service to attach it to.
    /// Such an entry names the service that already exists rather than adding one, so it must not
    /// rename a single artifact — which is what this predicate decides.
    pub fn is_default_only(&self) -> bool {
        self.services
            .iter()
            .all(|service| service.name == DEFAULT_SERVICE)
    }

    /// The provider's roles: the union of its services', deduplicated, in declaration order.
    ///
    /// **Derived, never authored.** A provider-level `roles` key is a load error, for the reason
    /// [`Level`](crate::config::Level) is derived from `binds`: an author who could state it could
    /// state it wrongly, and then two sources of truth would disagree about what the connector can
    /// do. It is also the wrong *level* — `openai`'s model-listing surface and its chat surface are
    /// different capabilities of one vendor, and a provider-level role would smear them together.
    ///
    /// A `Vec` rather than a set type: the order is the declaration order, which is what a catalogue
    /// and a UI want, and the loader has already refused a repeat within one service.
    pub fn roles(&self) -> Vec<Role> {
        let mut union: Vec<Role> = Vec::new();
        for role in self.services.iter().flat_map(|service| &service.roles) {
            if !union.contains(role) {
                union.push(*role);
            }
        }
        union
    }

    /// The required members of `role` that `service` does not have, in the role's own order.
    ///
    /// Empty when the service satisfies the claim. The loader calls this to refuse a claim that does
    /// not hold; a consumer can call it to explain one. Matching is by member name *within the
    /// service* — see [`fills_slot`].
    ///
    /// # Only an operation fills a slot
    ///
    /// Deliberately [`operations_of`](Self::operations_of) and **not**
    /// [`member_names_of`](Self::member_names_of), even though the latter is the per-service
    /// namespace every other rule here uses. A role is a claim that something is *callable*: a
    /// consumer resolving `llm_catalogue` intends to call the listing.
    ///
    /// The other member kinds cannot answer that. An [`EventDecl`] and a [`ChannelBinding`] reach the
    /// manifest and the catalogue and are emitted into no module at all — flux lifts `op`
    /// declarations only — so a service whose only `list` is an event named `models.list` would
    /// publish a live-listing capability that nothing can call. That is "an event dressed up as a
    /// pollable op", which this repository refuses on sight. A [`ConfigField`](crate::ConfigField) is
    /// not addressable in the calling sense either.
    ///
    /// [`Graph`](crate::graph::Graph) is the one arguable exclusion: it *does* lower to an emitted
    /// `op`. It stays out because a role is a vendor's API shape and a graph is a composition this
    /// repository wrote over it — admitting one would let a provider satisfy a role by wrapping
    /// members that do not implement it. Nothing needs it today; widening later is cheap.
    pub fn missing_role_members(&self, service: &str, role: Role) -> Vec<&'static str> {
        let callable: Vec<&str> = self
            .operations_of(service)
            .map(|operation| operation.id.as_str())
            .collect();
        role.required_members()
            .iter()
            .copied()
            .filter(|slot| !callable.iter().any(|member| fills_slot(member, slot)))
            .collect()
    }

    /// The operations belonging to one service, in IR order.
    ///
    /// Together with [`service_names`](Self::service_names) this is the partition: the sets are
    /// pairwise disjoint because [`Operation::service`] holds one name, and their union is every
    /// operation because the loader refuses an operation naming a service that does not exist.
    pub fn operations_of<'a>(&'a self, service: &'a str) -> impl Iterator<Item = &'a Operation> {
        self.operations
            .iter()
            .filter(move |operation| operation.service == service)
    }

    /// The events belonging to one service, in IR order. The inbound half of the partition
    /// [`operations_of`](Self::operations_of) documents.
    pub fn events_of<'a>(&'a self, service: &'a str) -> impl Iterator<Item = &'a EventDecl> {
        self.events
            .iter()
            .filter(move |event| event.service == service)
    }

    /// The channel bindings belonging to one service, in IR order.
    pub fn channels_of<'a>(&'a self, service: &'a str) -> impl Iterator<Item = &'a ChannelBinding> {
        self.channels
            .iter()
            .filter(move |channel| channel.service == service)
    }

    /// The configuration fields belonging to one service, in IR order.
    pub fn config_of<'a>(&'a self, service: &'a str) -> impl Iterator<Item = &'a ConfigField> {
        self.config
            .iter()
            .filter(move |field| field.service == service)
    }

    /// A configuration field by name.
    pub fn config_field(&self, name: &str) -> Option<&ConfigField> {
        self.config.iter().find(|field| field.name == name)
    }

    /// The graphs belonging to one service, in IR order.
    pub fn graphs_of<'a>(&'a self, service: &'a str) -> impl Iterator<Item = &'a Graph> {
        self.graphs.iter().filter(move |g| g.service == service)
    }

    /// A graph by name.
    pub fn graph(&self, name: &str) -> Option<&Graph> {
        self.graphs.iter().find(|g| g.name == name)
    }

    /// Every member name of one service — operations, then events, then channels, then config.
    ///
    /// **The three kinds share one namespace**, which is why this returns them together rather than
    /// leaving a caller to concatenate three lists and get the rule wrong. Two reasons it has to be
    /// one namespace, and the second is the load-bearing one:
    ///
    /// 1. All three project into the same address space. `com.slack.api:v1#slack` must denote one
    ///    thing, and an [`Oip`](crate::Oip) carries no kind discriminator to tell two apart.
    /// 2. All three project into flux's declaration namespace. An operation is an `op` a model calls
    ///    by name and an event is a trigger label; a collision would resolve differently depending on
    ///    which surface asked, which is the kind of ambiguity that is cheap to forbid now and
    ///    impossible to fix after publication.
    ///
    /// The loader refuses a duplicate, so on a loaded connector this list has no repeats.
    pub fn member_names_of<'a>(&'a self, service: &'a str) -> Vec<&'a str> {
        self.operations_of(service)
            .map(|operation| operation.id.as_str())
            .chain(self.events_of(service).map(|event| event.name.as_str()))
            .chain(
                self.channels_of(service)
                    .map(|channel| channel.name.as_str()),
            )
            .chain(self.config_of(service).map(|field| field.name.as_str()))
            .chain(self.graphs_of(service).map(|graph| graph.name.as_str()))
            .collect()
    }

    /// An event by name.
    pub fn event(&self, name: &str) -> Option<&EventDecl> {
        self.events.iter().find(|event| event.name == name)
    }

    /// A channel binding by name.
    pub fn channel(&self, name: &str) -> Option<&ChannelBinding> {
        self.channels.iter().find(|channel| channel.name == name)
    }

    /// The base URL a call to that service reaches: the service's own override, else the connector's.
    pub fn base_url_of(&self, service: &str) -> &str {
        self.service(service)
            .and_then(|service| service.base_url.as_deref())
            .unwrap_or(&self.base_url)
    }

    /// The vendor API version of that service: the service's own, else the connector's.
    ///
    /// `None` when neither states one, in which case the service has no renderable
    /// [`gid`](Self::gid_of) — stated at the accessor rather than papered over with a placeholder
    /// version that would become part of a published address.
    pub fn api_version_of(&self, service: &str) -> Option<&str> {
        self.service(service)
            .and_then(|service| service.api_version.as_deref())
            .or(self.api_version.as_deref())
    }

    /// The provider's `pid` — C-37's top address level — when it declares an authority.
    pub fn pid(&self) -> Option<crate::address::Pid> {
        Some(crate::address::Pid::new(self.authority.as_deref()?))
    }

    /// One service's `gid`: `com.amazonaws/s3:2006-03-01`, with [`DEFAULT_SERVICE`] elided.
    ///
    /// `None` unless the connector declares an authority *and* the service resolves an API version:
    /// an address missing either half is not an address, and inventing one is how a wrong identifier
    /// gets published under a stability contract that forbids reusing it.
    pub fn gid_of(&self, service: &str) -> Option<crate::address::Gid> {
        Some(crate::address::Gid::new(
            self.authority.as_deref()?,
            service,
            self.api_version_of(service)?,
        ))
    }

    /// One operation's `oip`: `com.amazonaws/s3:2006-03-01#object-get`.
    ///
    /// The fragment is [`Operation::id`], which stays the declarable Flux symbol — C-37's design
    /// records why the two identifiers are separate rather than one richer one.
    pub fn oip_of(&self, operation: &Operation) -> Option<crate::address::Oip> {
        self.oip_of_member(&operation.service, &operation.id)
    }

    /// One member's `oip`, whatever its kind: `com.slack.api:v1#app-mention`.
    ///
    /// Operations, events and channel bindings share one namespace per service
    /// ([`member_names_of`](Self::member_names_of)), so they share one address form and the `#`
    /// fragment needs no kind discriminator. `name` is assumed to be a member of `service` — the
    /// loader has already refused one that is not.
    pub fn oip_of_member(&self, service: &str, name: &str) -> Option<crate::address::Oip> {
        Some(crate::address::Oip {
            gid: self.gid_of(service)?,
            operation: name.to_owned(),
        })
    }

    /// Where one tenant's value for one of this connector's credentials is kept.
    ///
    /// ```text
    /// tenants/<tenant>/com.zendesk.api/api_token
    /// ```
    ///
    /// The three outcomes are distinct on purpose, because they have different owners:
    ///
    /// - `Err` — **the caller's tenant id is unusable.** It is untrusted input, so this is the
    ///   expected failure and it names what is wrong with it.
    /// - `Ok(None)` — **this connector has no `authority`**, so no address renders. The same answer
    ///   [`gid_of`](Self::gid_of) gives, for the same reason: inventing one publishes a wrong
    ///   identifier under a contract that forbids reusing it. Fifteen of the sixteen shipped
    ///   providers are in this state.
    /// - `Ok(Some)` — the address.
    ///
    /// # The service segment
    ///
    /// A credential is declared at **provider** level ([`Connector::auth`]), so this always renders
    /// the reserved [`DEFAULT_SERVICE`] — which is elided, giving the three-segment form above. The
    /// path *can* carry a service ([`CredentialRef::new`]), and that is deliberate headroom: a vendor
    /// whose surfaces authenticate separately is expressible the day one appears, without moving
    /// every path that already exists. Nothing today needs it, because credential names are unique
    /// within a provider and already distinguish the case.
    ///
    /// # Errors
    ///
    /// A reason string when the tenant id is unusable, or when `credential` is not one this connector
    /// declares.
    pub fn credential_ref_for(
        &self,
        tenant: &str,
        credential: &str,
    ) -> crate::Result<Option<crate::credential::CredentialRef>> {
        let leaf = self.local_credential_name(credential)?;
        let Some(authority) = self.authority.as_deref() else {
            return Ok(None);
        };
        crate::credential::CredentialRef::new(tenant, authority, DEFAULT_SERVICE, leaf)
            .map(Some)
            .map_err(crate::Error::Invalid)
    }

    /// The local part of a declared credential's name — `api_token` from `zendesk.api_token`.
    ///
    /// Credential names are vendor-prefixed because they live in **one flat namespace** across the
    /// connector. A path does not need the prefix: the authority above it already says which vendor,
    /// so carrying it would say the same thing twice and put a `.` inside what must be one segment.
    ///
    /// # Errors
    ///
    /// When nothing declares `credential`, or when its prefix disagrees with the connector id — the
    /// latter is an authoring slip that would otherwise produce a plausible path pointing at the
    /// wrong vendor's namespace.
    pub fn local_credential_name<'a>(&self, credential: &'a str) -> crate::Result<&'a str> {
        if self.auth_method(credential).is_none() {
            return Err(crate::Error::Invalid(format!(
                "credential {credential:?} is not declared by connector {:?}",
                self.id
            )));
        }
        let prefix = format!("{}.", self.id);
        credential.strip_prefix(&prefix).ok_or_else(|| {
            crate::Error::Invalid(format!(
                "credential {credential:?} is not prefixed {prefix:?}, so its local name cannot be \
                 derived. A credential name is `<connector>.<name>` because credentials share one \
                 flat namespace; a disagreeing prefix would render a path under the wrong vendor"
            ))
        })
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
    /// The connector's *compiled meaning*: `id`, `authority`, `api_version`, `services`, `vendor`,
    /// `base_url`, `description`, `auth`, `default_auth`, `operations`, `events`, `channels` — and,
    /// inside each member, its `service`. Change any of those and a generated artifact changes, so
    /// the hash must move. C-49's service fields are in for exactly this reason: a service owns the
    /// base URL a call reaches and names the module it is emitted into.
    ///
    /// The inbound members are in even though **nothing about them reaches the `.flux` module** — a
    /// channel binding is declared into the manifest and the catalogue, never emitted as code. The
    /// hash domain is the connector's compiled meaning, not the module's bytes, and a binding whose
    /// reply operation or verification scheme moved is a connector that changed. Leaving them out
    /// would make `flux-connectors check` blind to a drifting event schema, which is the one thing
    /// the lockfile exists to catch.
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
/// The three service fields carry `skip_serializing_if`, which is the one exception to the rule
/// above, and it exists for the same reason [`Provenance`] is excluded altogether: a connector that
/// declares no service, no authority and no version must hash to what it hashed *before* services
/// existed. Otherwise landing C-49 moves every `ir_sha256` in the repository and every
/// `connectors.lock` entry churns for a provider nobody edited — phantom drift, which is precisely
/// what the lockfile exists to rule out. [`Operation::service`] holds the same property through its
/// own `skip_serializing_if`.
#[derive(Serialize)]
struct HashDomain<'a> {
    id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    authority: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_version: &'a Option<String>,
    #[serde(skip_serializing_if = "<[Service]>::is_empty")]
    services: &'a [Service],
    vendor: &'a str,
    base_url: &'a str,
    description: &'a str,
    auth: &'a [AuthMethod],
    default_auth: &'a [AuthRequirement],
    operations: &'a [Operation],
    /// Inbound members carry `skip_serializing_if` for the same reason the service fields do: a
    /// connector that declares none must hash to what it hashed *before* they existed, or landing
    /// them moves every `ir_sha256` in the repository and churns `connectors.lock` for 14 providers
    /// nobody edited.
    #[serde(skip_serializing_if = "<[EventDecl]>::is_empty")]
    events: &'a [EventDecl],
    #[serde(skip_serializing_if = "<[ChannelBinding]>::is_empty")]
    channels: &'a [ChannelBinding],
    /// In the domain, and the reasoning is worth stating because it is not obvious: a configuration
    /// field is presentation, and presentation usually is not compiled meaning. But a field's `binds`
    /// decides *where a value goes* — a changed binding sends a token to a different place — and its
    /// `secret` flag decides whether a value is masked. Both are behaviour. Splitting the struct so
    /// that only those two hashed would make `flux-connectors check` silent about a label that had
    /// drifted from the vendor's own name for the field, which is a change a user sees.
    #[serde(skip_serializing_if = "<[ConfigField]>::is_empty")]
    config: &'a [ConfigField],
    #[serde(skip_serializing_if = "Option::is_none")]
    verify: &'a Option<String>,
    /// In the domain without argument: a graph *is* compiled meaning — it becomes an emitted `op`,
    /// so a changed edge is a changed module.
    #[serde(skip_serializing_if = "<[Graph]>::is_empty")]
    graphs: &'a [Graph],
}

impl<'a> HashDomain<'a> {
    fn of(connector: &'a Connector) -> Self {
        // The exhaustive destructuring is the tripwire, and it is the whole reason this is a
        // separate type instead of a `serde` attribute: a field added to `Connector` fails to
        // compile here until someone states whether it belongs in the hash domain. Silently
        // inheriting the decision is how `fetched_at` got into the hash in the first place.
        let Connector {
            id,
            authority,
            api_version,
            services,
            vendor,
            base_url,
            description,
            auth,
            default_auth,
            operations,
            events,
            channels,
            config,
            verify,
            graphs,
            provenance,
        } = connector;

        // Deliberately excluded — see `Connector::hash_domain`.
        let _excluded = provenance;

        Self {
            id,
            authority,
            api_version,
            services,
            vendor,
            base_url,
            description,
            auth,
            default_auth,
            operations,
            events,
            channels,
            config,
            verify,
            graphs,
        }
    }
}
