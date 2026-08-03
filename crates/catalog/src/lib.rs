//! Every generated connector operation, embedded at compile time.
//!
//! This is flux-connectors made consumable with `cargo add` instead of by copying artifacts into
//! `~/.flux/flows`. Adding the crate *is* getting the catalog: every operation's Flux source and
//! the metadata a caller needs to decide whether to run it are `&'static` data baked into the
//! binary by `include_str!` and a generated table. There is no filesystem lookup, no parsing, no
//! initialization, and no dependency.
//!
//! It stays inside the charter (`AGENTS.md`): a library that hands out **text**, not a runtime.
//! Nothing here executes an operation or touches the network — flux does that, from the module it
//! loads.
//!
//! ```
//! use catalog::{OperationKey, ProviderKey, Risk};
//!
//! let show = catalog::operation(OperationKey::id("zendesk-ticket-show")).unwrap();
//! assert_eq!(show.risk, Risk::Low);
//! assert!(show.flux.starts_with("op zendesk-ticket-show("));
//!
//! // "every operation in this provider" is one call.
//! let zendesk = catalog::operations_of(ProviderKey::id("zendesk"));
//! assert!(zendesk.iter().all(|operation| operation.provider == "zendesk"));
//! ```
//!
//! # What is generated and what is not
//!
//! `flux-connectors build` writes three kinds of file into this crate, all committed and reviewed
//! like every other generated artifact in the repository. **Nothing here is hand-written.**
//!
//! - `ops/<provider>/<operation>.flux` — one rendering per operation, byte for byte the same text
//!   the provider's module carries for it;
//! - `src/generated/<provider>.rs` — the table that embeds those renderings alongside their
//!   metadata;
//! - `src/generated.rs` — [`generated`], the index naming every provider's module.
//!
//! The first two are **per provider**, so `flux-connectors build --provider zendesk` regenerates
//! exactly what it compiled. The index is not: it names every provider at once, so it is written by
//! a **full** build only and a scoped run leaves the committed file untouched rather than rendering
//! an index that drops the providers the run never looked at. `web/public/catalog.json` follows the
//! same rule for the same reason.
//!
//! It was hand-written until C-104 on exactly that reasoning, and the cost was structural: two lines
//! per provider, appended by hand, made this the one file every provider story touched — so any two
//! provider stories conflicted and connectors shipped one at a time. `tests/embedded_operations.rs`
//! still holds the list against `providers/`, which is now a staleness check on a committed artifact
//! rather than a reminder to add a line.
//!
//! `crates/connector-cli/tests/catalog_artifacts.rs` is what makes the embedded data a *checked*
//! artifact: it recomputes everything here from `providers/*.toml` and compares byte for byte, so a
//! stale catalog — which still compiles and still answers every query — fails the build.
//!
//! # The per-provider module is still what ships
//!
//! `connectors/<provider>.flux` is unchanged in role: it is the artifact flux loads from
//! `~/.flux/flows`, and it declares every one of that provider's operations. The renderings here
//! are **additional** — the catalog's unit, not a substitute for the module — and each one is a
//! substring of it (`tests/embedded_operations.rs` pins that).
//!
//! # Addressing, and what C-37 changes
//!
//! Operations are keyed today by [`Operation::id`], the declarable Flux symbol
//! (`zendesk-ticket-show`). C-37 introduces a global address — an `oip` such as
//! `com.zendesk.api/support/tickets:v2#show` — and the key types here exist so that lands
//! **additively**: [`OperationKey`] and [`ProviderKey`] are opaque, constructed only through named
//! constructors, so C-37 adds `OperationKey::oip` and `ProviderKey::pid` and no signature in this
//! crate moves. There is deliberately no `From<&str>` for either: a bare string cannot say whether
//! it is a symbol or an address, and guessing is exactly the ambiguity two identifiers exist to
//! avoid.

mod generated;

/// How much damage an operation can do, in flux's own vocabulary.
///
/// A mirror of `connector_spec::Risk`, not a re-export: this crate has no dependencies, which is
/// what makes it cheap to add. The mapping is exhaustive at the point of generation
/// (`connector-cli`'s `catalog` module matches on every variant), so a variant added upstream is a
/// compile error there rather than a silent omission here, and
/// `tests/embedded_operations.rs::metadata_agrees_with_the_embedded_flux` pins every value against
/// the `risk "…"` line of the Flux it describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

impl Risk {
    /// The spelling flux's approval gate reads, and the one the embedded Flux declares.
    pub const fn as_str(self) -> &'static str {
        match self {
            Risk::Low => "low",
            Risk::Medium => "medium",
            Risk::High => "high",
            Risk::Destructive => "destructive",
        }
    }
}

/// Whether repeating an operation is safe, in flux's own vocabulary.
///
/// Mirrors `connector_spec::Idempotency` for the same reason [`Risk`] does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Idempotency {
    /// Repeating the call has the same effect as making it once.
    Idempotent,
    /// Repeating the call repeats its effect.
    NonIdempotent,
    /// Idempotent only under a condition the caller supplies (e.g. an idempotency key).
    Conditional,
}

impl Idempotency {
    /// The spelling the embedded Flux declares.
    pub const fn as_str(self) -> &'static str {
        match self {
            Idempotency::Idempotent => "idempotent",
            Idempotency::NonIdempotent => "non_idempotent",
            Idempotency::Conditional => "conditional",
        }
    }
}

/// **Why an operation names the credentials it does** — and, when it names none, which of the two
/// opposite reasons that is (C-235).
///
/// [`Operation::credentials`] is an empty slice in two situations that mean the reverse of each
/// other, and a consumer reading only that slice gets one value for both. This field is what a
/// connector *declares*, carried across rather than left to be inferred from an absence — the trap
/// C-206 closed in the published catalogue and this closes in the embedded one.
///
/// # The vocabulary is C-206's
///
/// The two named states are the two codes `web/public/catalog.json` already publishes for the same
/// distinction (`docs/designs/catalog-json.md`), and [`as_str`](Self::as_str) returns those tokens
/// character for character. A host restating them in words of its own is how two surfaces
/// describing one fact come to disagree, so this crate restates nothing:
/// `connectors_api::api::Wiring` serializes these tokens directly.
///
/// [`Declared`](Self::Declared) is the ordinary case and has no published *status* code, because
/// there is nothing to report about it — the operation names its mechanisms and the slice beside
/// this field is them. It is given a token anyway so that every state has one name rather than two
/// having names and one being "the other one".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CredentialRequirement {
    /// The operation authenticates: [`Operation::credentials`] names at least one mechanism.
    Declared,
    /// **The vendor requires no credential**, declared positively by the connector (`auth = []` on
    /// the operation). Nothing is withheld and nothing is missing: the unauthenticated call is the
    /// correct working call.
    NoneRequired,
    /// **A credential is withheld.** Nothing is declared and nothing declares that nothing is
    /// needed, so the request would go out unauthenticated — the fail-closed outcome, not a working
    /// one. `freshdesk` is the shipped case: its API key occupies the Basic *username* position,
    /// which this repository's model treats as non-secret config, so declaring it would route a
    /// live key outside the secret gate (`AGENTS.md`, Intentional gaps).
    ///
    /// The operator-facing consequence is the part worth carrying: there is nothing for an operator
    /// to supply *and* the operation does not work, which is neither of the other two states.
    Withheld,
}

impl CredentialRequirement {
    /// The token this state travels as — C-206's published spellings for the two it named.
    ///
    /// `no-credential-required` is `connector_cli::status::NO_CREDENTIAL_REQUIRED` and
    /// `no-credential` is `connector_cli::status::NO_CREDENTIAL`, both of which are published
    /// contract tokens that are extended but never renamed once shipped.
    pub const fn as_str(self) -> &'static str {
        match self {
            CredentialRequirement::Declared => "declared",
            CredentialRequirement::NoneRequired => "no-credential-required",
            CredentialRequirement::Withheld => "no-credential",
        }
    }
}

/// One operation: its Flux source, and what a caller needs in order to decide whether to use it.
///
/// `#[non_exhaustive]` because C-37 adds the global address to this struct and C-10 adds the
/// resolved endpoint spec; neither should be a breaking change for a consumer. C-197's
/// [`service`](Self::service) is the first field to land on that headroom, and it is what the
/// attribute was for: a consumer outside this crate can neither construct one with a struct literal
/// nor destructure one exhaustively, so a field arriving is additive for them and breaking only for
/// the generated tables in this crate, which the same build rewrites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Operation {
    /// The Flux symbol the operation is declared and called by, e.g. `zendesk-ticket-show`. Unique
    /// across the whole catalog, and the key [`OperationKey::id`] matches on.
    pub id: &'static str,
    /// The [`Provider::id`] this operation belongs to.
    pub provider: &'static str,
    /// The **service** this operation belongs to — `delivery`, `s3`, or the reserved `default` for
    /// a provider with a single API surface. Never empty: the IR always names a service
    /// (`connector_spec::ir::DEFAULT_SERVICE`), and the reserved name is elided from *addresses*
    /// only, never from this field.
    ///
    /// # Why it is here, and what was wrong without it (C-197)
    ///
    /// Services partition the operation set, and a service owns its own `base_url` — so
    /// [`Operation::hosts`] is already per service. What was missing was the *name*, and a consumer
    /// that cannot read it cannot key anything by service.
    ///
    /// `connector-pack`'s configuration port is where that bit. It keyed a tenant's endpoint values
    /// on `(tenant, provider, kind, name)`, because there was no service here to key on, so two
    /// services of one connector that spell the same `{variable}` in their own `base_url` collapsed
    /// to a single value. `contentful` is the shipped case: `delivery_space_id` and
    /// `management_space_id` are two declared configuration fields, both binding `endpoint.space_id`
    /// under different services, and `contentful-entry-create` on `api.contentful.com` resolved
    /// whatever `contentful-entry-get` on `cdn.contentful.com` had. A tenant whose two environments
    /// differ got a `200` from the wrong one rather than a refusal.
    ///
    /// `web/public/catalog.json` had carried the service all along; this is the embedded catalogue
    /// catching up, which is why every provider's generated table moved when it landed.
    pub service: &'static str,
    /// What the operation does, in one line — the same text the model sees as the tool description.
    pub description: &'static str,
    /// How much damage it can do.
    pub risk: Risk,
    /// Whether repeating it is safe.
    pub idempotency: Idempotency,
    /// Flux semantic-effect tags (`money`, `delete`, `send_external`, …), distinct from the host
    /// effects carried by [`Self::flux`]. Empty means no semantic consequence is declared.
    pub semantic_effects: &'static [&'static str],
    /// The credentials the operation needs, as **alternatives of mechanisms**: the outer slice is
    /// an OR over ways to authenticate, and each inner slice is the set of credentials that must
    /// all be satisfied on the same request (AND).
    ///
    /// `&[&["babelforce.access_id", "babelforce.access_token"]]` is one mechanism needing two
    /// headers together, not two ways to authenticate — the distinction babelforce forces and that
    /// a flat list of credential names cannot express. An empty outer slice means the operation
    /// needs no credential at all.
    ///
    /// The names are credential *references*. No secret, and no environment variable's value, is
    /// ever in this crate — flux resolves the reference and applies the scheme.
    ///
    /// **An empty outer slice does not say why**, which is what [`credential_requirement`] is for.
    ///
    /// [`credential_requirement`]: Self::credential_requirement
    pub credentials: &'static [&'static [&'static str]],
    /// **Why [`credentials`](Self::credentials) is what it is** — and, when it is empty, which of
    /// the two opposite reasons that is (C-235). See [`CredentialRequirement`].
    ///
    /// It is a separate field rather than a richer `credentials` because the type of that field is
    /// this crate's published API: a consumer already matching on the mechanism list keeps working,
    /// and one that needs the reason reads a field that was not there before. `Operation` is
    /// `#[non_exhaustive]` precisely so that is additive.
    ///
    /// The invariant, held at emission and pinned by
    /// `tests/embedded_operations.rs::the_declared_requirement_agrees_with_the_mechanism_list`:
    /// this is [`Declared`](CredentialRequirement::Declared) exactly when `credentials` is
    /// non-empty.
    pub credential_requirement: CredentialRequirement,
    /// The hosts a call reaches, as the connector's base URL spells them — templating included, so
    /// Zendesk is `{subdomain}.zendesk.com` rather than a tenant nobody has chosen yet. What a
    /// caller does with this is decide whether their egress policy admits the call.
    pub hosts: &'static [&'static str],
    /// The operation's Flux source: exactly the `op` declaration
    /// `connectors/<provider>.flux` carries for it.
    pub flux: &'static str,
}

/// **Where a credential goes on the way out** — the *placement* axis of
/// `docs/designs/unified-auth.md`.
///
/// The design's central claim is carried by one field: `prefix` on [`Header`](Self::Header) is what
/// turns `Bearer` from an enum variant into data, so `Bearer `, `Basic `, `Token `, `GenieKey ` and
/// the empty prefix are one code path rather than five variants. A flat scheme enum conflates
/// *where* a secret goes with *what it is prefixed by*, and needs a new variant the moment a vendor
/// wants `Token <t>` in `Authorization`.
///
/// Deliberately **not** `#[non_exhaustive]`, unlike [`Provider`] and [`Operation`]. A consumer
/// placing a credential must match every variant, and a new placement must be a compile error at
/// every such site rather than a silently skipped credential — a request that quietly went out
/// without one is exactly the failure this type exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Placement {
    /// `<name>: <prefix><value>`. `Authorization: Bearer <t>` is
    /// `Header { name: "Authorization", prefix: "Bearer " }`; a raw-value header such as
    /// `X-Shopify-Access-Token` is the same variant with an empty prefix.
    Header {
        /// The header name.
        name: &'static str,
        /// The literal text the value is prefixed with, **trailing space included**. Empty for a
        /// raw-value header.
        prefix: &'static str,
    },
    /// `?<name>=<value>` — a query-parameter key.
    Query {
        /// The parameter name.
        name: &'static str,
    },
    /// **Never placed on an outgoing request.** A webhook signing secret verifies bytes that
    /// arrived, so it has no answer to "where does this go on the way out" — the one deliberate
    /// divergence from flux's vocabulary, recorded in `AGENTS.md`'s authentication contract. It is
    /// here so that one credential namespace covers both directions.
    Inbound,
}

/// **How stored material becomes the value that is placed** — the *acquisition* axis.
///
/// Two of the three are pure: the stored secret is placed unchanged, or it is joined into a Basic
/// pair. The third, [`Minted`](Self::Minted), is the one effectful acquisition this repository
/// models, and it is modelled as *provenance* rather than as a step a connector performs — see its
/// own documentation. The remaining effectful ones — token refresh, expiry, `session` — are still
/// the host's, by the line `docs/designs/unified-auth.md` draws: they need a cache and a
/// refresh-on-401, and C-90 put both out of scope.
///
/// Not `#[non_exhaustive]`, for the reason [`Placement`] is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Acquisition {
    /// The stored secret, unchanged.
    Static,
    /// **The stored secret, unchanged — and one of this connector's own operations put it there**
    /// (C-136).
    ///
    /// Read as a *placement* instruction this is [`Static`](Self::Static): a minted credential goes
    /// onto a request exactly as a statically provisioned one does, because by the time anything
    /// places it, it is a value in the store like any other. What the variant adds is the fact the
    /// diversion needs, and which nothing else in the catalogue carries: **which call mints it, and
    /// where in that call's answer the value arrives**.
    ///
    /// `connector-pack` reads it from the minting side. Projecting an operation, it looks for the
    /// declared credential whose [`by`](Self::Minted::by) is that operation's own id; finding one
    /// makes the operation credential-producing, and its result stops being the vendor's body and
    /// becomes the handle `{ "credential": "tenants/…" }`. A downstream operation that *uses* the
    /// credential names it in `credentials` like any other and never sees a value — which is the
    /// property C-136 exists for: **a caller can use a credential it can never read.**
    ///
    /// # Why the fact lives on the credential rather than on the operation
    ///
    /// It reads as though it belongs on [`Operation`], and semantically it is the operation's
    /// declaration — `produces_credential` in the provider file sits in the `[[operations]]` block.
    /// It is carried here because it is also, exactly, an answer on this axis, and because a field
    /// on `Operation` is a field every one of the catalogue's generated tables must state: adding
    /// one rewrites every `crates/catalog/src/generated/<provider>.rs`, every artifact hash under
    /// it, and `connectors.lock` — for a fact no shipped connector declares. A variant costs
    /// nothing until something uses it, which is the same reason `skip_serializing_if` is on the IR
    /// field.
    Minted {
        /// The [`Operation::id`] whose call mints this credential.
        by: &'static str,
        /// Where the secret arrives in that operation's response body — **one** JSON Pointer into
        /// the response value, `/access_token`.
        ///
        /// Exactly one location, and the loader refuses anything else. `credential_response`'s
        /// sibling vocabulary admits `*` for every element of an array because it describes where
        /// credentials *appear*; a mint stores one value at one address, so there would be nothing
        /// to say which element it was. That is also what keeps this field's documented behaviour
        /// and the runtime's the same thing: the diversion resolves it with
        /// `serde_json::Value::pointer`, which has no wildcard.
        from: &'static str,
    },
    /// `base64(<user><user_suffix>:<secret>)` — HTTP Basic, composed rather than pre-composed.
    ///
    /// The user half is **config, not a gated secret**: it is an email address or an account name,
    /// and it resolves from the declared environment variables rather than from the secret store.
    /// `user_suffix` is Zendesk's `/token` marker — public API syntax, and the reason a bare env
    /// value is not enough (`docs/designs/auth-seam.md` §7.5).
    ///
    /// The secret occupies the **password** position. Freshdesk's shape, where the API key occupies
    /// the *username* position, is not expressible here because the IR cannot yet say so either
    /// (C-16 owns that decision); Freshdesk therefore declares no credential at all, which
    /// `AGENTS.md` records as an intentional gap rather than an oversight.
    BasicJoin {
        /// Environment-variable **keys** holding the user half, tried in order. Never a value.
        user_env: &'static [&'static str],
        /// A literal appended to the resolved user half before the `user:secret` join.
        user_suffix: &'static str,
    },
}

/// One credential a connector declares: what it is called, where its value is kept, and how it
/// reaches the wire.
///
/// **No value, ever**, and no field it could occupy: [`Credential::leaf`] is the last segment of a
/// *path*, [`Acquisition`] names environment-variable *keys*, and [`Placement`] names a header, a
/// prefix or a query key. This crate is `&'static` data compiled from the IR, so "it holds no secret"
/// is a property of the shape rather than a habit of whoever last edited the emitter.
///
/// Not `#[non_exhaustive]`: a consumer constructs one in a test to prove its own placement code, and
/// the type is small, declarative and complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Credential {
    /// The flat-namespace name an operation references, e.g. `zendesk.api_token`. Vendor-prefixed,
    /// because credentials share one namespace across the connector.
    pub name: &'static str,
    /// The last segment of the credential's address — `api_token`, not `zendesk.api_token`. The
    /// path already carries the authority, so the vendor prefix would be said twice.
    pub leaf: &'static str,
    /// How stored material becomes the value that is placed.
    pub acquire: Acquisition,
    /// Where that value goes on the request.
    pub place: Placement,
}

/// One string pair in a generated declaration (query, header or payload mapping).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pair {
    pub name: &'static str,
    pub value: &'static str,
}

/// How an event reaches a connector binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelTransport {
    Webhook,
    Socket,
    Poll,
}

/// One event declaration, including the local/wire identity split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub name: &'static str,
    pub service: &'static str,
    pub description: &'static str,
    pub wire_value: Option<&'static str>,
    pub default: bool,
    pub group: &'static str,
    /// Canonical JSON Schema text, or `None` when the vendor publishes none.
    pub schema: Option<&'static str>,
    /// Canonical JSON for the complete source declaration, including narrowing rules.
    pub declaration_json: &'static str,
}

/// The generic RFC 6455 handshake facts for one socket binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketConnect {
    pub path: &'static str,
    pub query: &'static [Pair],
    pub headers: &'static [Pair],
    pub auth: &'static [&'static [&'static str]],
    pub subprotocols: &'static [&'static str],
}

/// One selector into an inbound envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selector {
    pub source: &'static str,
    pub name: &'static str,
}

/// One generated channel binding. Verification/reply/setup remain available in the manifest and
/// public catalogue; this embedded shape carries every fact the zero-I/O socket planner and generic
/// event router require.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channel {
    pub name: &'static str,
    pub service: &'static str,
    /// The service base URL against which [`SocketConnect::path`] is derived.
    pub base_url: &'static str,
    pub description: &'static str,
    pub transport: ChannelTransport,
    pub events: &'static [&'static str],
    pub connect: Option<SocketConnect>,
    pub discriminator: Option<Selector>,
    /// Stable vendor delivery identity used for deduplication, when declared.
    pub delivery_id: Option<Selector>,
    pub payload: &'static [Pair],
    pub payload_root: bool,
    /// Canonical JSON for the complete binding declaration, including verification and reply.
    pub declaration_json: &'static str,
}

/// One complete configuration field, with no stored value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigField {
    pub name: &'static str,
    pub service: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    pub example: Option<&'static str>,
    pub format: &'static str,
    pub required: bool,
    pub default: Option<&'static str>,
    pub secret: bool,
    pub docs_url: Option<&'static str>,
    pub binds: &'static str,
    pub also_binds: &'static [&'static str],
    /// Canonical JSON for the complete form declaration, including choices.
    pub declaration_json: &'static str,
}

/// **How a connector executes** — flux's runtime axis, as the catalogue publishes it (C-405).
///
/// The vocabulary mirrors flux's `docs/designs/ecosystem.md`, which replaces the plugin-versus-
/// connector dichotomy with one axis: a plugin is a runtime kind a connector may declare, not a
/// rival of one. It is the same closed set `connector_spec::Runtime` accepts at load; the emitter
/// translates one into the other, so a variant that existed on only one side would not compile.
///
/// # What a consumer does with it
///
/// HTTP is easy to multi-tenant because the effect leaves the machine. Process spawning, container
/// exec and raw sockets do not — they consume the host's own identity, network position, filesystem
/// and descriptors — so **a host serving more than one tenant refuses a locally-executing runtime**.
/// That refusal is mechanical only if the runtime is a declared fact, which is what this field is:
/// before it existed a consumer derived [`Http`](Self::Http) and was right, and would have stayed
/// right until the first connector that was not, at which point it was silently wrong for exactly
/// the case the refusal exists to catch.
///
/// Deliberately carries **no** `is_local()` predicate. Which runtimes a deployment admits is the
/// host's judgment (flux-exchange's `Deployment::admits`), and this crate publishes vendor facts;
/// putting the policy here would make the catalogue an authority on a question it cannot see the
/// inputs to.
///
/// Not `#[non_exhaustive]`, for the reason [`Placement`] is not: it is a closed set mirrored from a
/// design, and a consumer matching exhaustively on it *should* be told when the set grows, because
/// a new runtime is a new effect they have not decided about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Runtime {
    /// A guarded HTTP request. Every connector this repository generates.
    #[default]
    Http,
    /// A guarded dial — TCP, UDP or ICMP. **Locally executing.**
    Socket,
    /// A guarded, argv-only spawn under a sandbox backend. **Locally executing.**
    Process,
    /// A spawn inside docker or kubernetes. **Locally executing.**
    Container,
    /// The flux plugin protocol over stdio — a special case of [`Process`](Self::Process).
    /// **Locally executing.**
    Plugin,
    /// Delegation to another substrate. Not locally executing: the effect leaves the machine.
    Remote,
}

impl Runtime {
    /// The token this runtime is published as, in `catalog.json` and in every `.connector.toml`.
    pub fn word(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Socket => "socket",
            Self::Process => "process",
            Self::Container => "container",
            Self::Plugin => "plugin",
            Self::Remote => "remote",
        }
    }
}

/// One connector, and every operation the catalog carries for it.
///
/// `#[non_exhaustive]` for the same reason [`Operation`] is: C-37's `pid` lands here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Provider {
    /// The connector id, e.g. `zendesk`. Names `connectors/<id>.flux`, and is the key
    /// [`ProviderKey::id`] matches on.
    pub id: &'static str,
    /// The vendor's display name.
    pub vendor: &'static str,
    /// What the connector is for, in one line.
    pub description: &'static str,
    /// The reverse-DNS authority the connector publishes under (`com.slack.api`) — C-37's `pid`,
    /// and the second segment of every credential address this connector's secrets live at.
    ///
    /// `None` for a connector that has not declared one yet. That is not cosmetic: without an
    /// authority no credential address renders at all, so a consumer resolving credentials can only
    /// refuse. Refusing is the correct answer — a request sent without the credential it declares is
    /// the failure worth preventing — but the diagnostic must say *which* fact is missing.
    pub authority: Option<&'static str>,
    /// **How this connector executes** — see [`Runtime`]. `Http` for every connector shipped today,
    /// and stated rather than assumed, which is the whole of C-405.
    pub runtime: Runtime,
    /// The API base URL, templating included.
    pub base_url: &'static str,
    /// Every credential the connector declares, in declaration order and keyed by
    /// [`Credential::name`] — the vocabulary [`Operation::credentials`] references by name.
    ///
    /// Declared at **provider** level, exactly as the IR declares them: a credential belongs to the
    /// connector, not to one of its services, which is why its address elides the service segment.
    pub auth: &'static [Credential],
    /// Every operation, in the order the provider declares them — which is also the order they
    /// appear in `connectors/<id>.flux`.
    pub operations: &'static [Operation],
    /// Complete configuration declarations, never values.
    pub config: &'static [ConfigField],
    /// Inbound events in declaration order.
    pub events: &'static [Event],
    /// Channel bindings in declaration order.
    pub channels: &'static [Channel],
    /// **Every configuration field whose value comes from a closed set** — C-225.
    ///
    /// Empty for most connectors. Present for the ones whose value is a *choice* rather than a
    /// string the operator knows: New Relic's two region hosts, Intercom's three.
    ///
    /// This is deliberately **not** the whole configuration surface — labels, help text, `binds` and
    /// the derived level are C-87's, and that story is a breaking change to `catalog.json`'s OAuth
    /// key. What is here is the part a closed set is useless without: a form that cannot see the
    /// choices renders a text box, which moves the declaration without moving the benefit.
    pub config_choices: &'static [ConfigChoices],
}

/// One configuration field that permits a closed set of values, and the set — C-225.
///
/// Addressed the way the runtime configuration port addresses a stored value: by
/// `(service, kind, name)`, where `kind` is `endpoint`, `path`, `query`, `header`, `username` or
/// `oauth` and `name` is the binding target within it. `field` is the declaration's own name, which
/// is what a form labels the row with and what an error message quotes.
///
/// Not `#[non_exhaustive]`: a consumer builds one in a test to prove its own rendering, exactly as
/// it does with [`Credential`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigChoices {
    /// The service whose configuration this is. Written out, `default` included, because a consumer
    /// grouping by service needs a name in every row (C-197).
    pub service: &'static str,
    /// The declared field name — `host`. The key a host stores the collected value under.
    pub field: &'static str,
    /// The form label — `New Relic API host`.
    pub label: &'static str,
    /// Where the value goes: `endpoint`, `path`, `query`, `header`, `username` or `oauth`.
    pub kind: &'static str,
    /// The name within `kind` — the base-URL `{variable}`, the pinned wire name, the credential.
    pub name: &'static str,
    /// The permitted values, in the vendor's own order.
    pub choices: &'static [Choice],
}

/// One permitted value of a [`ConfigChoices`] set, and the text a renderer shows for it.
///
/// The label is what makes the set usable: an operator knows their account is in Frankfurt and does
/// not know that `api.eu.newrelic.com` is what that means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    /// The value itself — what a host stores and what substitution puts on the request.
    pub value: &'static str,
    /// The human name for it — `European Union`.
    pub label: &'static str,
}

impl Provider {
    /// One of this provider's operations, by key.
    pub fn operation(&self, key: OperationKey<'_>) -> Option<&'static Operation> {
        self.operations
            .iter()
            .find(|operation| key.matches(operation))
    }

    /// The credential this connector declares under `name`, or `None` when nothing declares it.
    ///
    /// A lookup rather than a map because the list is short, ordered and generated; the ordering is
    /// the connector's own, and an alternative-selection rule that depends on declaration order
    /// needs it preserved.
    pub fn credential(&self, name: &str) -> Option<&'static Credential> {
        self.auth.iter().find(|credential| credential.name == name)
    }

    /// One declared channel binding, by its service-local name.
    pub fn channel(&self, name: &str) -> Option<&'static Channel> {
        self.channels.iter().find(|channel| channel.name == name)
    }

    /// The closed set governing one configuration slot, addressed exactly as the runtime port
    /// addresses a stored value — `(service, kind, name)`.
    ///
    /// `None` means the slot is open, **not** that the value is unconstrained by anything: a field
    /// still has a `format`, which this surface does not publish (C-87). A caller uses this to
    /// decide between a select and an input, and to refuse a value it was not offered.
    pub fn choices_for(
        &self,
        service: &str,
        kind: &str,
        name: &str,
    ) -> Option<&'static ConfigChoices> {
        self.config_choices
            .iter()
            .find(|entry| entry.service == service && entry.kind == kind && entry.name == name)
    }
}

/// How a caller names one operation.
///
/// A key type rather than a bare `&str` so that C-37's global address becomes a *constructor* —
/// `OperationKey::oip(…)` — instead of a reshape of every lookup in the crate. The field is
/// private and there is no `From<&str>`: a caller states which kind of name they hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperationKey<'a> {
    /// The Flux symbol. C-37 turns this field into an enum over `{ id, oip }`; because the field is
    /// private and every constructor is named, that is an additive change.
    id: &'a str,
}

impl<'a> OperationKey<'a> {
    /// Name an operation by its Flux symbol, e.g. `zendesk-ticket-show`.
    pub const fn id(id: &'a str) -> Self {
        Self { id }
    }

    /// Whether this key names `operation`.
    fn matches(&self, operation: &Operation) -> bool {
        operation.id == self.id
    }
}

/// How a caller names one provider.
///
/// The sibling of [`OperationKey`], and additive in the same way: C-37's `pid`
/// (`com.zendesk.api`) lands as `ProviderKey::pid`, and its middle level — the `gid`, a versioned
/// resource group — lands as a `GroupKey` beside it without disturbing anything here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderKey<'a> {
    /// The connector id. C-37 turns this field into an enum over `{ id, pid }`.
    id: &'a str,
}

impl<'a> ProviderKey<'a> {
    /// Name a provider by its connector id, e.g. `zendesk`.
    pub const fn id(id: &'a str) -> Self {
        Self { id }
    }

    /// Whether this key names `provider`.
    fn matches(&self, provider: &Provider) -> bool {
        provider.id == self.id
    }
}

/// Every provider in the catalog, ordered by [`Provider::id`].
pub fn providers() -> &'static [&'static Provider] {
    generated::PROVIDERS
}

/// One provider, by key.
pub fn provider(key: ProviderKey<'_>) -> Option<&'static Provider> {
    providers()
        .iter()
        .copied()
        .find(|provider| key.matches(provider))
}

/// **Every operation in one provider** — the listing that makes a provider addressable rather than
/// merely nameable.
///
/// An unknown provider yields an empty slice rather than an error; use [`provider`] when the
/// difference between "no such connector" and "a connector with nothing in it" matters.
pub fn operations_of(key: ProviderKey<'_>) -> &'static [Operation] {
    provider(key).map_or(&[], |provider| provider.operations)
}

/// Every operation in the catalog, provider by provider.
pub fn operations() -> impl Iterator<Item = &'static Operation> {
    providers()
        .iter()
        .flat_map(|provider| provider.operations.iter())
}

/// **The lookup.** One operation by key, with its Flux source and its metadata.
///
/// A linear scan: the catalog holds tens of operations today and low hundreds once spec ingest
/// selects more (a spec-ingested babelforce alone offers 163), which is well inside the range where
/// a scan over contiguous `&'static` data beats any index worth maintaining — and an index would be
/// a second generated artifact to keep in step with this one.
pub fn operation(key: OperationKey<'_>) -> Option<&'static Operation> {
    operations().find(|operation| key.matches(operation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_operation_is_found_by_its_symbol() {
        let operation = operation(OperationKey::id("zendesk-ticket-show"))
            .expect("the shipped catalog carries zendesk-ticket-show");
        assert_eq!(operation.provider, "zendesk");
        assert!(operation.flux.contains("op zendesk-ticket-show("));
    }

    #[test]
    fn an_unknown_key_is_none_rather_than_a_panic() {
        // A negative sentinel must not be a plausible vendor name. `salesforce` was used here, and
        // shipping the Salesforce connector (C-163) turned "definitely unknown" into "shipped" and broke
        // three tests across two crates at once — `AGENTS.md` had named Salesforce as a provider that
        // belongs here all along, so this was scheduled to happen. The sentinel below is not a company,
        // and each use is self-checking: the assertion *is* that the catalogue does not carry it, so this
        // cannot rot into a vacuous pass the way a stale hardcoded name can.
        const NO_SUCH_PROVIDER: &str = "no-such-vendor";

        assert!(operation(OperationKey::id("zendesk-ticket-obliterate")).is_none());
        assert!(provider(ProviderKey::id(NO_SUCH_PROVIDER)).is_none());
        assert!(operations_of(ProviderKey::id(NO_SUCH_PROVIDER)).is_empty());
    }

    /// Listing by provider is the whole point of the middle level, so it must agree with the flat
    /// listing rather than being a second, drifting source.
    #[test]
    fn listing_by_provider_partitions_the_catalog() {
        let listed: usize = providers()
            .iter()
            .map(|provider| operations_of(ProviderKey::id(provider.id)).len())
            .sum();
        assert_eq!(listed, operations().count());
        assert!(
            listed > 0,
            "an empty catalog would pass every other test here"
        );
    }

    /// Ids are what lookups key on, so a duplicate would make one of the two unreachable.
    #[test]
    fn operation_ids_are_unique_across_the_catalog() {
        let mut ids: Vec<&str> = operations().map(|operation| operation.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate operation id in the catalog");
    }

    #[test]
    fn providers_are_listed_in_a_stable_order() {
        let ids: Vec<&str> = providers().iter().map(|provider| provider.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }
}
