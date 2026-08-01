//! `site/catalog.json`: the whole catalogue as one static JSON document, for a website to read
//! (C-42).
//!
//! This is the **fourth backend over one IR**, after the Flux module, the connector manifest and
//! the `connector-catalog` crate. It re-derives nothing: [`crate::catalog`] already walks the IR
//! for the Rust catalogue, and the two share the credential and host walks so that a site and a
//! `cargo add` consumer cannot be told different things about the same operation.
//!
//! ```text
//!                                   ┌─► connectors/<p>.flux           (installable)
//! providers/*.toml ─► Connector IR ─┼─► connectors/<p>.connector.toml (manifest)
//!                                   ├─► crates/catalog/…              (Rust consumers)
//!                                   └─► site/catalog.json             (the website)
//! ```
//!
//! **The load-bearing point is that the site never hand-maintains catalogue data.** That is the
//! action-proxy failure this repository exists to correct, re-enacted in JavaScript. The document
//! is generated, committed, and drift-checked through [`crate::pipeline::plan`] exactly like every
//! other artifact, and `crates/connector-cli/tests/site_catalog.rs` fails when it is stale.
//!
//! # The published shape
//!
//! Specified in [`docs/designs/catalog-json.md`](../../../../docs/designs/catalog-json.md), because
//! a website is written against it. Three properties of it are worth stating here, since they are
//! what the code below is arranged to hold:
//!
//! - **Every key is always present.** An absent value is `null` or `[]`, never a missing key, so a
//!   consumer can type the document once and never test for existence. Nothing here uses
//!   `skip_serializing_if`.
//! - **Entities are objects, never tuples.** C-37's global address (`oip`, `pid`) lands as an added
//!   field on the provider and operation objects, and an added field is additive for every consumer
//!   that reads by name — no reshape, and no `schema_version` bump.
//! - **Deterministic.** Every value is a function of the IR, walked in the IR's own order, with no
//!   map iteration and no timestamp. `serde_json::Value` is backed by a `BTreeMap` unless
//!   `preserve_order` is enabled, so the vendor schemas carried verbatim serialize with sorted keys
//!   (`connector-spec`'s `tests/determinism.rs` is the tripwire for that feature being turned on).
//!
//! # Why one document rather than one per provider
//!
//! A website wants one fetch, and the explorer's cross-provider filters — by risk, by whether an
//! operation works — are queries over the whole catalogue. But a whole-catalogue file is not a
//! function of a `--provider` run: `build --provider zendesk` compiles one provider and would have
//! to drop the other two to write this honestly. So [`crate::pipeline::plan`] emits it **only for a
//! full run**, and a scoped build leaves the committed document untouched rather than truncating
//! it. That is the same reasoning `crates/catalog/src/generated.rs` records for keeping its
//! provider index by hand, reached from the other direction.

use std::collections::BTreeMap;

use anyhow::Result;
use serde::Serialize;

use connector_spec::{
    AuthScheme, ChannelBinding, Connector, EventDecl, HttpMethod, Idempotency, JsonSchema,
    ManualSetup, Operation, Param, Reply, Risk, Selector, Subscription, VerificationScheme,
};

use crate::catalog::{self, OperationRendering};
use crate::core_catalog::CoreCatalog;
use crate::inbound;
use crate::status::{self, Status};

/// The document's format version.
///
/// Bumped only when an existing field changes meaning or disappears. **Adding a field does not bump
/// it** — every consumer reads by name, so a new key is invisible to one that does not know it, and
/// C-37's `oip` is the case this rule is written for.
const SCHEMA_VERSION: u32 = 2;

/// The whole catalogue: every provider, every operation, and what does not work.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct Document {
    /// See [`SCHEMA_VERSION`].
    schema_version: u32,
    /// The generator identity, matching the header every other artifact carries.
    generator: String,
    /// Every provider, ordered by id — discovery's order, and the order `crates/catalog` publishes.
    providers: Vec<ProviderEntry>,
    /// Flux-owned built-ins and language nodes. `null` only in minimal test fixtures that do not
    /// carry the optional vendored snapshot.
    core: Option<CoreCatalog>,
}

/// One connector, and everything it publishes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderEntry {
    /// The connector id, e.g. `zendesk`. Names `connectors/<id>.flux`.
    id: String,
    /// The reverse-DNS authority the provider publishes under (`com.amazonaws`), or `null` when it
    /// declares none — which is every provider shipped today.
    authority: Option<String>,
    /// The vendor's display name.
    vendor: String,
    /// What the connector is for, in one line.
    description: String,
    /// **How this connector executes** (C-405): `http`, `socket`, `process`, `container`, `plugin`
    /// or `remote` — flux's runtime axis, mirrored.
    ///
    /// Always present and never null, unlike [`authority`](Self::authority) and
    /// [`api_version`](Self::api_version): the IR always names a runtime, so the published document
    /// always does too. A consumer refusing a locally-executing connector — a host serving more than
    /// one tenant must, because process, container and raw-socket execution consume the host's own
    /// identity and network position — reads this rather than deriving `http` from the fact that
    /// every connector shipped so far happens to be one.
    runtime: String,
    /// The API base URL, templating included. A service may override it; see
    /// [`ServiceEntry::base_url`].
    base_url: String,
    /// The vendor's API version, as the default for this provider's services. `null` when unstated.
    api_version: Option<String>,
    /// Every host this connector reaches, as its base URLs spell them: the union of its services'
    /// (C-49), in declaration order and deduplicated.
    ///
    /// The union rather than `base_url`'s own host, because a multi-service provider reaches a host
    /// per service — Google's Gmail is on `gmail.googleapis.com` while its Calendar and Drive are on
    /// `www.googleapis.com` — and a list built from the connector-level value alone would tell a
    /// reader this provider never calls a host it calls on three of its eight operations. Widening
    /// happens the other way and is what must not: a *service's* [`ServiceEntry::hosts`] stays its own
    /// and is never the union, because that one is an egress claim rather than a description.
    /// Identical to the old value for a single-surface provider, whose union is its one host.
    hosts: Vec<String>,
    /// The provider's API surfaces (C-49). Always at least one: a provider with a single surface
    /// publishes the reserved `default` service, so a consumer can group by service unconditionally
    /// rather than special-casing the providers that have not been split.
    services: Vec<ServiceEntry>,
    /// The credentials it declares and how they reach the wire.
    auth: ProviderAuth,
    /// How many operations it publishes — the number a provider list renders without walking
    /// `operations`.
    operation_count: usize,
    /// Every operation, in the order the provider declares them, which is also the order
    /// `connectors/<id>.flux` carries them.
    operations: Vec<OperationEntry>,
    /// The events this connector **receives** — the inbound half of its surface (C-83).
    ///
    /// An operation is flux calling the vendor; an event is the vendor calling flux. Both are
    /// members of a service and both are published here, in IR order, so a consumer can render what
    /// a connector listens for without reading a provider TOML. `[]` for the sixteen connectors that
    /// declare none.
    events: Vec<EventEntry>,
    /// The ingress surfaces this connector describes — the third member kind (C-82/C-83).
    ///
    /// A binding **declares**; it never installs, and it is emitted into no module at all. Nothing
    /// here is a URL, a schedule or a secret: the endpoint is the operator's deployment detail and
    /// every credential is a name the host resolves.
    channels: Vec<ChannelEntry>,
    /// **Every configuration field whose value comes from a closed set** (C-225). `[]` for the
    /// connectors that declare none, which is nearly all of them.
    ///
    /// An **additive** key, so no `SCHEMA_VERSION` bump: nothing existing changes type or meaning.
    /// And deliberately *not* the whole configuration surface — labels, help, `binds`, `format` and
    /// the derived level are C-87's, and that story carries a breaking change to the `auth.oauth2`
    /// flattening which this one must not drag in. What is published here is the part a closed set
    /// is worthless without: a product that cannot see the choices renders a text box, and the
    /// declaration has moved without the benefit.
    config_choices: Vec<ConfigChoicesEntry>,
}

/// One configuration field that permits a closed set of values, and the set (C-225).
///
/// Addressed by `(service, kind, name)` — the same address `connector-pack`'s configuration port
/// stores a value under — so a consumer joins on it rather than re-parsing a `binds` string it
/// cannot see yet.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ConfigChoicesEntry {
    /// The [`ServiceEntry::name`] this field configures — written out, `default` included.
    service: String,
    /// The declared field name, e.g. `host`. The key a host stores the collected value under.
    field: String,
    /// The form label, e.g. `New Relic API host`.
    label: String,
    /// Where the value goes: `endpoint`, `path`, `query`, `header`, `username` or `oauth`.
    kind: &'static str,
    /// The name within `kind` — the base-URL `{variable}`, or the pinned wire name.
    name: String,
    /// The permitted values, in the vendor's own order.
    choices: Vec<ChoiceEntry>,
}

/// One permitted value, and the text a renderer shows for it.
///
/// The label is the whole reason this is an object rather than a string: a dropdown of hostnames is
/// a dropdown nobody can answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ChoiceEntry {
    value: String,
    label: String,
}

/// One event a vendor sends, with the vendor's own spelling and its schema intact.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct EventEntry {
    /// The event name, in the vendor's spelling — `app_mention`, `issues.opened`. Never respelled.
    name: String,
    /// The [`ServiceEntry::name`] it belongs to — exactly one, like every other member.
    service: String,
    /// The rendered address, `com.slack.api:v1#app_mention`, or `null` when the connector declares
    /// no authority or no API version. Events, operations and bindings share one namespace per
    /// service, so they share one address form and the `#` fragment carries no kind tag.
    oip: Option<String>,
    /// What the event means, in one line.
    description: String,
    /// Whether a product should offer this event **on** when a user connects. Slack's `message` is
    /// the case for `false`: it fires for every human message in every channel the app is in.
    default: bool,
    /// An optional grouping label, so a long event list renders as sections. Empty when unset.
    group: String,
    /// Field equalities that narrow a coarse vendor event into this one — GitHub's one `issues`
    /// event with `{"action": {"const": "opened"}}`. `{}` when the discriminator alone identifies it.
    when: BTreeMap<String, JsonSchema>,
    /// The vendor's JSON Schema for the payload, carried verbatim, or `null` when it publishes none.
    schema: Option<JsonSchema>,
}

/// One ingress surface: a transport, the events it carries, how a delivery is proven, and what
/// answers it.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ChannelEntry {
    /// The binding name, e.g. `events-api`.
    name: String,
    /// The [`ServiceEntry::name`] it belongs to.
    service: String,
    /// The rendered address, or `null` when the connector publishes none.
    oip: Option<String>,
    /// What the binding is for, in one line.
    description: String,
    /// `webhook`, `socket` or `poll` — flux owns the transport, the connector owns the binding.
    transport: &'static str,
    /// The [`EventEntry::name`]s this binding carries, all from the same service.
    events: Vec<String>,
    /// How a delivery proves it came from the vendor. **Always present**, and always naming its
    /// kind — see [`crate::inbound`] for why an omitted key would have been the wrong encoding.
    verification: VerificationEntry,
    /// Where to read *which event this is*, when the transport carries it out of band.
    discriminator: Option<SelectorEntry>,
    /// Where to read the vendor's redelivery id, so a flow can dedupe at-least-once delivery.
    delivery_id: Option<SelectorEntry>,
    /// Flow symbol → dotted path into the vendor's envelope, in the grammar `Param::wire` already
    /// uses. `{}` when the binding maps nothing.
    payload: BTreeMap<String, String>,
    /// The operation that answers an event on this binding, or `null` for a fire-and-forget one.
    reply: Option<ReplyEntry>,
    /// The cursor operation a `poll` binding calls — required for that transport, `null` otherwise.
    cursor: Option<String>,
    /// The **suggested** poll interval. Advisory: this repository runs nothing, and flux's cron
    /// drops ticks across a restart.
    interval: Option<String>,
    /// How the binding is registered with the vendor through its own API, or `null` when it has none.
    subscription: Option<SubscriptionEntry>,
    /// What a human does in the vendor's dashboard, when there is no subscription API.
    setup: Option<SetupEntry>,
}

/// A binding's verification, published **totally**: a kind, the boolean it implies, and the HMAC
/// parameters when there are any.
///
/// `verified` is `kind != "none"` restated, exactly as `status.works` restates `issues.is_empty()`.
/// It is what lets a consumer tell a deliberately-unverifiable surface from a verified one without
/// inspecting the absence of a field — the C-82 invariant that silence is never a verification
/// answer, carried through to the artifact.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct VerificationEntry {
    /// `hmac`, `none` or `connection`. A stable machine token.
    kind: &'static str,
    /// Whether a delivery here can be attributed to the vendor at all. `false` for `none` only.
    verified: bool,
    /// The HMAC parameters, or `null` for the two kinds that carry none.
    hmac: Option<HmacEntry>,
}

/// The parameters of one HMAC signature scheme, in the vendor's own terms.
///
/// Nothing here verifies anything: the comparison runs host-side over the raw request bytes. This
/// only declares what that comparison uses — and [`secret`](Self::secret) is a **credential name**,
/// never a value. `site_catalog.rs::no_credential_value_reaches_the_document` builds with the
/// signing secret's variable set to a sentinel and asserts it appears nowhere.
///
/// **Every field of `HmacSpec` must appear here**, and restating them is why that needs a test
/// rather than a comment: a field the IR gains and this struct does not is dropped from the published
/// catalogue while both halves still compile. `HmacSpec` cannot be flattened in instead — this
/// document publishes every key always (`docs/designs/catalog-json.md`), and the IR skips its `None`
/// fields so that a provider TOML need not spell out absences. So the field set is held to the IR's
/// by `inbound_artifacts.rs::neither_projection_can_lose_a_field_hmac_spec_declares`, which derives
/// the authoritative list from `HmacSpec`'s own `Deserialize` impl (C-151).
#[derive(Debug, Clone, PartialEq, Serialize)]
struct HmacEntry {
    /// `sha1` or `sha256`.
    algorithm: &'static str,
    /// `hex` or `base64` — how the digest is spelled in the header.
    encoding: &'static str,
    /// The header carrying the signature, e.g. `X-Slack-Signature`.
    header: String,
    /// A literal prefix the header value carries before the digest (`v0=`), or `null`.
    prefix: Option<String>,
    /// The string that is signed, as a template over `{body}`, `{sorted_form}`, `{timestamp}` and
    /// `{url}` — `connector_spec::inbound::SIGNED_PLACEHOLDERS`, which is the closed set a consumer
    /// must be able to fill. `{sorted_form}` and `{url}` are the two a consumer cannot fill by
    /// splicing something it was handed: see the IR's `HmacSpec::signed` for what each derives.
    signed: String,
    /// Where the `{timestamp}` is read from. Present exactly when `signed` interpolates one.
    timestamp: Option<SelectorEntry>,
    /// How that timestamp is **spelled** — `unix_seconds` or `rfc3339` — and `null` for a scheme
    /// that signs none. A separate axis from the selector, which says only where the value is read
    /// from; the IR's default is resolved here so that a consumer reads the answer instead of
    /// having to know it (`crate::inbound::timestamp_format_of`).
    timestamp_format: Option<&'static str>,
    /// The **name** of the credential holding the shared secret. Resolve it against
    /// `provider.auth.credentials[].name`, where it is declared with `scheme: "signing"`.
    secret: String,
    /// How old a signed request may be (`5m`), or `null` for an untimestamped scheme.
    tolerance: Option<String>,
}

/// Where on an inbound request one named value is read from.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct SelectorEntry {
    /// `header` or `body`.
    source: &'static str,
    /// The header name, or the dotted body path.
    name: String,
}

/// The outbound half of a binding: which operation answers, and how its parameters are filled.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ReplyEntry {
    /// The `OperationEntry::id` that answers — always an operation this same connector publishes.
    operation: String,
    /// **The reply as a rendered oip**, `com.slack.api:v1#slack-chat-post-message`, or `null` when
    /// the connector declares no authority or no API version.
    ///
    /// The id above is what a host resolves locally; this is the address that survives leaving the
    /// repository, and it is the form the design writes a reply in.
    oip: Option<String>,
    /// The reply parameter carrying the **journey's own output** — the one field no path into the
    /// triggering event can reach. `null` when every parameter comes off the payload.
    result: Option<String>,
    /// Reply parameter name → [`ChannelEntry::payload`] key. `{}` when nothing is bound.
    bind: BTreeMap<String, String>,
}

/// How a webhook is registered with the vendor through its own API.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct SubscriptionEntry {
    /// The operation that registers the endpoint.
    subscribe: String,
    /// The operation that removes it, or `null` when the vendor offers no removal — worth knowing,
    /// because it means a disconnect leaves the vendor posting to a dead URL.
    unsubscribe: Option<String>,
    /// The operation that lists existing registrations, for reconciling duplicates.
    list: Option<String>,
    /// Which parameter of `subscribe` receives the product's public callback URL. The URL itself is
    /// never here.
    callback_param: String,
}

/// What a human does in the vendor's dashboard, for vendors with no subscription API.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct SetupEntry {
    /// The vendor's own page, so a UI links out rather than restating it.
    docs_url: Option<String>,
    /// The steps, in order, each one thing a person does.
    steps: Vec<String>,
}

/// One API surface of a provider: the unit a consumer addresses, versions and installs (C-49).
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ServiceEntry {
    /// The service name, e.g. `s3`, or `default` for a provider with a single surface.
    name: String,
    /// What the service is for, in one line. Empty for the implicit `default` service, which has no
    /// declaration to carry one.
    description: String,
    /// The base URL calls to this service reach — its own override, else the provider's.
    base_url: String,
    /// The hosts those calls reach, as the base URL spells them. A service's egress surface is its
    /// own and is never widened to the union of the provider's.
    hosts: Vec<String>,
    /// The vendor's API version for this service. `null` when neither it nor the provider states one.
    api_version: Option<String>,
    /// The service's rendered address — `com.amazonaws/s3:2006-03-01` — or `null` when the provider
    /// declares no authority or no version. `default` is elided from it, never spelled out.
    gid: Option<String>,
    /// How many operations belong to it. The per-service counts sum to
    /// [`ProviderEntry::operation_count`], because the services partition the operation set.
    operation_count: usize,
}

/// A connector's credentials: what it declares, and what it requires by default.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ProviderAuth {
    /// The distinct scheme kinds in play (`bearer`, `basic`, `header`, `query`), in declaration
    /// order — what a provider list renders as "auth scheme" without unfolding every credential.
    /// Empty when the connector declares none.
    schemes: Vec<&'static str>,
    /// Every declared credential.
    credentials: Vec<CredentialEntry>,
    /// The connector-wide default requirement, as alternatives (OR) of mechanisms (AND).
    default: Vec<Vec<String>>,
}

/// One declared credential — **a reference, never a value**.
///
/// `env` and `user_env` name environment variables; nothing here resolves one, and nothing in this
/// module reads the process environment at all.
/// `crates/connector-cli/tests/site_catalog.rs::no_credential_value_reaches_the_document` runs a
/// real build with a credential variable set to a sentinel and asserts the sentinel is nowhere in
/// the output.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct CredentialEntry {
    /// The credential name an operation's requirement references, e.g. `zendesk.api_token`.
    name: String,
    /// How the resolved secret is injected.
    scheme: SchemeEntry,
    /// What the credential is, for the prompt that asks an operator to supply it.
    description: String,
    /// Environment variable **names** to resolve the secret from, tried in order.
    env: Vec<String>,
    /// For `basic`: environment variable **names** holding the username half.
    user_env: Vec<String>,
    /// For `basic`: a literal appended to the resolved user value — Zendesk's `/token` marker,
    /// which is public API syntax and not a credential. `null` when there is none.
    user_suffix: Option<String>,
    /// Whether the host runs OAuth2 token grants for this credential.
    oauth2: bool,
}

/// An [`AuthScheme`], flattened to a fixed three-key shape.
///
/// The IR's own encoding is externally tagged — `"bearer"` for one variant and
/// `{"header": {"name": "…"}}` for another — which is a JSON shape that changes with the value. A
/// consumer would need a discriminated union to read it. Here the kind is always a string, the name
/// is always present (`null` when the variant carries none), and the prefix is always a string.
///
/// **`prefix` is published rather than dropped, and that is the point of C-184.** Without it, Okta's
/// `Authorization: SSWS <token>` and LaunchDarkly's raw `Authorization: <token>` flatten to the same
/// two keys, so a consumer reading this document would build one of them wrong while believing the
/// catalogue had described it. It is a literal scheme word — never any part of a credential.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct SchemeEntry {
    /// `bearer`, `basic`, `header` or `query`.
    kind: &'static str,
    /// The header or query-parameter name, for the two variants that carry one.
    name: Option<String>,
    /// The literal text in front of the credential in a header value, **trailing space included**.
    /// Empty for a raw-value header and for every non-header scheme. The two presets spell theirs
    /// out here rather than leaving a consumer to know them: `bearer` is `"Bearer "`.
    prefix: String,
}

/// One operation: the metadata, the typed parameters, the generated Flux, and whether it works.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct OperationEntry {
    /// The Flux symbol the operation is declared and called by. Unique across the catalogue.
    id: String,
    /// The [`ProviderEntry::id`] it belongs to.
    provider: String,
    /// The [`ServiceEntry::name`] it belongs to — exactly one, and `default` for a provider with a
    /// single API surface. This is the grouping a consumer wants once a provider is more than one
    /// API (C-49).
    service: String,
    /// What it does, in one line — the same text a model sees as the tool description.
    description: String,
    /// How much damage it can do. Serialized in flux's own vocabulary (`low`…`destructive`).
    risk: Risk,
    /// Whether repeating it is safe (`idempotent`, `non_idempotent`, `conditional`).
    idempotency: Idempotency,
    /// **The condition under which repeating this write is safe** — `null` for every operation that
    /// does not declare `idempotency = "conditional"`, which is almost all of them.
    ///
    /// This is the artifact half of C-186, and it is what makes `conditional` mean anything to a
    /// consumer. flux reserves that value for a mutation which is "genuinely safe to repeat under
    /// **stated** conditions" (`flux_spec::coherence`, I3) — and before this field the conditions
    /// were stated nowhere a machine or a reviewer could reach. Six shipped operations, three of
    /// them Stripe money movements, published `conditional` and no condition at all.
    ///
    /// Published rather than merely stored, for the same reason the rest of this document is: the
    /// claim travels to consumers, so the evidence for it has to travel with it.
    repeatable_because: Option<String>,
    /// The HTTP method, uppercase.
    method: HttpMethod,
    /// The path template, relative to [`ProviderEntry::base_url`].
    path: String,
    /// Every named parameter, in request-position order, each carrying its JSON Schema verbatim.
    parameters: Vec<ParameterEntry>,
    /// **One JSON Schema for everything the operation receives**, composed from the parameters
    /// above and [`Self::body_schema`] — [`Operation::input_schema`].
    ///
    /// Both are published, and they answer different questions. `parameters` is the *authoring*
    /// view: it keeps each parameter's request position and wire spelling, which a form renderer
    /// and a request builder need. This is the *calling* view, the single object a caller passes and
    /// the thing `ToolSpec.input_schema` requires — which is exactly why it is composed once here
    /// rather than by each consumer, who would each disagree at the corners (C-125).
    ///
    /// Never `null`: an operation that takes nothing composes an empty object schema, because
    /// "takes nothing" is a derived answer. Absence is reserved for the output side, where "we do
    /// not know" genuinely is the state of things.
    input_schema: JsonSchema,
    /// The schema of a free-form body, when the body *is* a schema rather than assembled from
    /// named fields. Mutually exclusive with body parameters; `null` in the common case.
    body_schema: Option<JsonSchema>,
    /// The JSON Schema of a successful response, when the vendor publishes one.
    response_schema: Option<JsonSchema>,
    /// The credentials required, as alternatives (OR) of mechanisms (AND) — the IR's own shape.
    /// `[["a", "b"]]` is one mechanism needing both, not two ways to authenticate.
    credentials: Vec<Vec<String>>,
    /// The hosts a call reaches.
    hosts: Vec<String>,
    /// The generated Flux, verbatim: exactly the `op` declaration `connectors/<provider>.flux`
    /// carries for this operation.
    flux: String,
    /// Whether it currently works, and if not, why. See [`crate::status`].
    status: Status,
}

/// One request parameter, with the vendor's schema intact.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ParameterEntry {
    /// The caller-facing name: what the generated op declares and what a model passes.
    name: String,
    /// Where it travels on the request: `path`, `query`, `header` or `body`.
    #[serde(rename = "in")]
    position: &'static str,
    /// The spelling the vendor sees when it differs — a body field's dotted JSON path, or a plain
    /// alias for a path/query/header parameter. `null` when the two names agree.
    wire: Option<String>,
    /// Human-readable description, surfaced to a model as part of the op's tool contract.
    description: String,
    /// Whether the vendor requires it.
    required: bool,
    /// The JSON Schema, carried verbatim — constraint keywords included. Flux's declaration
    /// narrows this to `Any`/`Bool`/`Number`/`String`/`List<T>`; nothing is lost here.
    schema: JsonSchema,
}

/// Compile one connector into its entry in the document.
///
/// `renderings` is passed in rather than recomputed, for the reason [`crate::catalog::render`]
/// records: the Flux is emitted **once** per operation and fed to every backend, so "the JSON
/// carries the text the module carries" is arithmetic rather than a coincidence a future edit could
/// break.
pub fn provider_entry(
    connector: &Connector,
    renderings: &[OperationRendering],
) -> Result<ProviderEntry> {
    let mut operations = Vec::with_capacity(renderings.len());
    for rendering in renderings {
        let operation = catalog::operation_for(connector, &rendering.id)?;
        // The host the call actually reaches, which is the operation's **service**'s — not the
        // provider's, which for a multi-service provider is a different host entirely.
        let host = catalog::host_of(connector.base_url_of(&operation.service))?.to_string();
        operations.push(operation_entry(connector, operation, rendering, host));
    }

    let mut services = Vec::new();
    // The provider's own host list is assembled here rather than from `connector.base_url`, so that it
    // is the union of what its services reach — see [`ProviderEntry::hosts`].
    let mut hosts: Vec<String> = Vec::new();
    for name in connector.service_names() {
        let base_url = connector.base_url_of(name);
        let host = catalog::host_of(base_url)?.to_string();
        if !hosts.contains(&host) {
            hosts.push(host);
        }
        services.push(ServiceEntry {
            name: name.to_owned(),
            description: connector
                .service(name)
                .map(|service| service.description.clone())
                .unwrap_or_default(),
            base_url: base_url.to_owned(),
            // This one is the service's own and is deliberately *not* the union above: it is an
            // egress claim about one installable unit.
            hosts: vec![catalog::host_of(base_url)?.to_string()],
            api_version: connector.api_version_of(name).map(str::to_owned),
            gid: connector.gid_of(name).map(|gid| gid.to_string()),
            operation_count: connector.operations_of(name).count(),
        });
    }

    Ok(ProviderEntry {
        id: connector.id.clone(),
        authority: connector.authority.clone(),
        vendor: connector.vendor.clone(),
        description: connector.description.clone(),
        runtime: connector.runtime.word().to_owned(),
        base_url: connector.base_url.clone(),
        api_version: connector.api_version.clone(),
        hosts,
        services,
        auth: provider_auth(connector),
        operation_count: operations.len(),
        operations,
        events: connector
            .events
            .iter()
            .map(|event| event_entry(connector, event))
            .collect(),
        channels: connector
            .channels
            .iter()
            .map(|channel| channel_entry(connector, channel))
            .collect(),
        config_choices: connector
            .config
            .iter()
            .filter_map(config_choices_entry)
            .collect(),
    })
}

/// One configuration field's closed set, or `None` when the field is open (C-225).
fn config_choices_entry(field: &connector_spec::ConfigField) -> Option<ConfigChoicesEntry> {
    let binding = field.binding().filter(|_| field.is_closed())?;
    Some(ConfigChoicesEntry {
        service: field.service.clone(),
        field: field.name.clone(),
        label: field.label.clone(),
        kind: binding.kind(),
        name: binding.target().to_owned(),
        choices: field
            .choices
            .iter()
            .map(|choice| ChoiceEntry {
                value: choice.value.clone(),
                label: choice.label.clone(),
            })
            .collect(),
    })
}

/// One declared event, in IR order.
fn event_entry(connector: &Connector, event: &EventDecl) -> EventEntry {
    EventEntry {
        name: event.name.clone(),
        service: event.service.clone(),
        oip: member_oip(connector, &event.service, &event.name),
        description: event.description.clone(),
        default: event.default,
        group: event.group.clone(),
        when: event.when.clone(),
        schema: event.schema.clone(),
    }
}

/// One channel binding, in IR order.
fn channel_entry(connector: &Connector, channel: &ChannelBinding) -> ChannelEntry {
    ChannelEntry {
        name: channel.name.clone(),
        service: channel.service.clone(),
        oip: member_oip(connector, &channel.service, &channel.name),
        description: channel.description.clone(),
        transport: inbound::transport_token(channel.transport),
        events: channel.events.clone(),
        verification: verification_entry(channel),
        discriminator: channel.discriminator.as_ref().map(selector_entry),
        delivery_id: channel.delivery_id.as_ref().map(selector_entry),
        payload: channel.payload.clone(),
        reply: channel
            .reply
            .as_ref()
            .map(|reply| reply_entry(connector, channel, reply)),
        cursor: channel.cursor.clone(),
        interval: channel.interval.clone(),
        subscription: channel.subscription.as_ref().map(subscription_entry),
        setup: channel.setup.as_ref().map(setup_entry),
    }
}

/// A member's rendered address, or `None` when the connector publishes none.
///
/// One helper for all three member kinds, because they share one namespace per service and
/// therefore one address form — `Connector::oip_of_member` is the IR's own statement of that.
fn member_oip(connector: &Connector, service: &str, name: &str) -> Option<String> {
    connector
        .oip_of_member(service, name)
        .map(|oip| oip.to_string())
}

/// The total verification projection — the classification comes from [`crate::inbound`], which the
/// manifest reads too, so the two artifacts cannot disagree about whether a surface is verified.
fn verification_entry(channel: &ChannelBinding) -> VerificationEntry {
    let kind = inbound::verification_of(channel);
    let hmac = match &channel.verification {
        Some(VerificationScheme::Hmac(spec)) => Some(HmacEntry {
            algorithm: inbound::digest_token(spec.algorithm),
            encoding: inbound::encoding_token(spec.encoding),
            header: spec.header.clone(),
            prefix: spec.prefix.clone(),
            signed: spec.signed.clone(),
            timestamp: spec.timestamp.as_ref().map(selector_entry),
            timestamp_format: inbound::timestamp_format_of(spec),
            // A credential **name**. Nothing in this module reads the process environment.
            secret: spec.secret.clone(),
            tolerance: spec.tolerance.clone(),
        }),
        Some(VerificationScheme::None) | None => None,
    };
    VerificationEntry {
        kind: kind.kind(),
        verified: kind.verified(),
        hmac,
    }
}

fn selector_entry(selector: &Selector) -> SelectorEntry {
    SelectorEntry {
        source: inbound::source_token(selector.source),
        name: selector.name.clone(),
    }
}

fn reply_entry(connector: &Connector, channel: &ChannelBinding, reply: &Reply) -> ReplyEntry {
    ReplyEntry {
        operation: reply.operation.clone(),
        // The reply's own service is the binding's: a binding answers with an operation of the same
        // connector and the same service, which is what the loader enforces.
        oip: member_oip(connector, &channel.service, &reply.operation),
        result: reply.result.clone(),
        bind: reply.bind.clone(),
    }
}

fn subscription_entry(subscription: &Subscription) -> SubscriptionEntry {
    SubscriptionEntry {
        subscribe: subscription.subscribe.clone(),
        unsubscribe: subscription.unsubscribe.clone(),
        list: subscription.list.clone(),
        callback_param: subscription.callback_param.clone(),
    }
}

fn setup_entry(setup: &ManualSetup) -> SetupEntry {
    SetupEntry {
        docs_url: setup.docs_url.clone(),
        steps: setup.steps.clone(),
    }
}

/// Serialize the whole catalogue.
///
/// Pretty-printed, and with a trailing newline: this is a **committed, reviewed** artifact like
/// every other generated file here, and a one-line JSON blob is not something anyone can read in a
/// diff. Two-space indentation is `serde_json`'s own default, so the fixed point costs nothing to
/// maintain.
pub fn document(providers: Vec<ProviderEntry>) -> Result<String> {
    document_with_core(providers, None)
}

/// Serialize the public catalogue with the independently owned Flux core projection.
pub fn document_with_core(
    providers: Vec<ProviderEntry>,
    core: Option<CoreCatalog>,
) -> Result<String> {
    let document = Document {
        schema_version: SCHEMA_VERSION,
        generator: crate::seam::generator(),
        providers,
        core,
    };
    Ok(format!("{}\n", serde_json::to_string_pretty(&document)?))
}

fn operation_entry(
    connector: &Connector,
    operation: &Operation,
    rendering: &OperationRendering,
    host: String,
) -> OperationEntry {
    OperationEntry {
        id: operation.id.clone(),
        provider: connector.id.clone(),
        service: operation.service.clone(),
        description: operation.description.clone(),
        risk: operation.risk,
        idempotency: operation.idempotency,
        // The trimmed reading, not the raw field: an author's stray whitespace is not part of the
        // claim, and a reason too short to be one never reaches here because the loader refused the
        // provider file that stated it.
        repeatable_because: operation.repeatability_condition().map(str::to_owned),
        method: operation.method,
        path: operation.path.clone(),
        parameters: parameters(operation),
        input_schema: operation.input_schema(),
        body_schema: operation.params.body_schema.clone(),
        response_schema: operation.response_schema.clone(),
        credentials: catalog::credential_mechanisms(connector, operation)
            .into_iter()
            .map(|mechanism| mechanism.into_iter().map(str::to_string).collect())
            .collect(),
        hosts: vec![host],
        flux: rendering.source.clone(),
        status: status::of(connector, operation),
    }
}

/// Every named parameter, flattened into one list that carries its own position.
///
/// A flat list rather than four keyed groups: the position is a *property* of a parameter, and a
/// site renders both an ordered signature and a per-position grouping from a flat list, while
/// grouped keys force the ordered view to be reassembled. The order is the IR's own — path, query,
/// header, body — which is also the order the Flux declaration takes its arguments in.
///
/// `body_schema` is deliberately not here: it is a schema, not a parameter, and it lives on the
/// operation beside this list.
fn parameters(operation: &Operation) -> Vec<ParameterEntry> {
    let groups = [
        ("path", &operation.params.path),
        ("query", &operation.params.query),
        ("header", &operation.params.header),
        ("body", &operation.params.body),
    ];
    groups
        .into_iter()
        .flat_map(|(position, params)| {
            params
                .iter()
                .map(move |param| parameter_entry(position, param))
        })
        .collect()
}

fn parameter_entry(position: &'static str, param: &Param) -> ParameterEntry {
    ParameterEntry {
        name: param.name.clone(),
        position,
        wire: param.wire.clone(),
        description: param.description.clone(),
        required: param.required,
        schema: param.schema.clone(),
    }
}

fn provider_auth(connector: &Connector) -> ProviderAuth {
    let mut schemes: Vec<&'static str> = Vec::new();
    for method in &connector.auth {
        let kind = scheme_kind(&method.scheme);
        if !schemes.contains(&kind) {
            schemes.push(kind);
        }
    }

    ProviderAuth {
        schemes,
        credentials: connector
            .auth
            .iter()
            .map(|method| CredentialEntry {
                name: method.name.clone(),
                scheme: SchemeEntry {
                    kind: scheme_kind(&method.scheme),
                    name: scheme_name(&method.scheme),
                    prefix: scheme_prefix(&method.scheme),
                },
                description: method.description.clone(),
                env: method.env.clone(),
                user_env: method.user_env.clone(),
                user_suffix: method.user_suffix.clone(),
                oauth2: method.oauth2.is_some(),
            })
            .collect(),
        default: connector
            .default_auth
            .iter()
            .map(|mechanism| mechanism.iter().cloned().collect())
            .collect(),
    }
}

/// The scheme's kind token.
///
/// An exhaustive match, deliberately, and for the reason [`crate::catalog`] gives for its own: a
/// variant added to the IR is a compile error here rather than a silent gap in the published
/// document. The spellings are the IR's own `rename_all = "snake_case"` encoding, which is in turn
/// flux's plugin-protocol vocabulary — this repository does not get to rename it, with the one
/// exception [`AuthScheme::Signing`] documents.
fn scheme_kind(scheme: &AuthScheme) -> &'static str {
    match scheme {
        AuthScheme::Bearer => "bearer",
        AuthScheme::Basic => "basic",
        AuthScheme::Header { .. } => "header",
        AuthScheme::Query { .. } => "query",
        AuthScheme::Signing => "signing",
    }
}

/// The header or query-parameter name the scheme carries, if it carries one.
fn scheme_name(scheme: &AuthScheme) -> Option<String> {
    match scheme {
        // `Signing` is here rather than folded in with the two above only to make the reason
        // explicit: it carries no name because it is never placed on a request at all. The header a
        // signature *arrives* in belongs to the channel binding that verifies it, not to the
        // credential — the same secret can verify two bindings that spell their header differently.
        AuthScheme::Bearer | AuthScheme::Basic | AuthScheme::Signing => None,
        AuthScheme::Header { name, .. } | AuthScheme::Query { name } => Some(name.clone()),
    }
}

/// The literal text in front of the credential in a header value.
///
/// The two presets are resolved to the strings they stand for rather than reported as empty: a
/// consumer building `Authorization` for a `bearer` credential needs `"Bearer "`, and making it
/// derive that from `kind` would be asking it to re-implement the one mapping this document exists
/// to publish. `crate::catalog::placement` lowers the same three variants to the same three strings,
/// and `a_published_prefix_matches_the_placement_the_catalogue_emits` holds them together.
fn scheme_prefix(scheme: &AuthScheme) -> String {
    match scheme {
        AuthScheme::Bearer => "Bearer ".to_string(),
        AuthScheme::Basic => "Basic ".to_string(),
        AuthScheme::Header { prefix, .. } => prefix.clone(),
        // Neither is a header value, so neither has a prefix. Query placement has no prefix axis at
        // all — the committed catalogue has zero query placements, and C-184 recorded the gap
        // rather than building for it.
        AuthScheme::Query { .. } | AuthScheme::Signing => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use connector_spec::{AuthMethod, AuthRequirement, ParamSet, Quirks};
    use serde_json::{json, Value};

    fn operation() -> Operation {
        Operation {
            id: "acme-thing-list".to_string(),
            service: connector_spec::DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Get,
            path: "/v2/things".to_string(),
            description: "List things".to_string(),
            risk: Risk::Destructive,
            idempotency: Idempotency::NonIdempotent,
            repeatable_because: None,
            expose: true,
            auth: None,
            params: ParamSet {
                query: vec![Param {
                    name: "req_id".to_string(),
                    wire: Some("requester_id".to_string()),
                    description: "Filter by requester".to_string(),
                    required: false,
                    schema: json!({"type": "string", "format": "uuid"}),
                }],
                ..ParamSet::default()
            },
            response_schema: None,
            credential_response: Vec::new(),
            quirks: Quirks::default(),
        }
    }

    fn connector() -> Connector {
        Connector {
            id: "acme".to_string(),
            authority: None,
            runtime: connector_spec::Runtime::Http,
            api_version: None,
            services: Vec::new(),
            vendor: "Acme".to_string(),
            base_url: "https://{tenant}.acme.example/api".to_string(),
            description: "Acme".to_string(),
            auth: vec![
                AuthMethod::header("acme.access_id", "X-Id", vec!["ACME_ID".to_string()]),
                AuthMethod::header(
                    "acme.access_token",
                    "X-Token",
                    vec!["ACME_TOKEN".to_string()],
                ),
            ],
            default_auth: vec![AuthRequirement::all([
                "acme.access_id",
                "acme.access_token",
            ])],
            operations: vec![operation()],
            events: Vec::new(),
            channels: Vec::new(),
            config: Vec::new(),
            verify: None,
            graphs: Vec::new(),
            provenance: Default::default(),
        }
    }

    fn renderings() -> Vec<OperationRendering> {
        vec![OperationRendering {
            id: "acme-thing-list".to_string(),
            source: "op acme-thing-list -> Any\n".to_string(),
        }]
    }

    fn rendered() -> Value {
        let entry = provider_entry(&connector(), &renderings()).unwrap();
        serde_json::from_str(&document(vec![entry]).unwrap()).unwrap()
    }

    #[test]
    fn serialization_is_deterministic() {
        let first = document(vec![provider_entry(&connector(), &renderings()).unwrap()]).unwrap();
        let second = document(vec![provider_entry(&connector(), &renderings()).unwrap()]).unwrap();
        assert_eq!(first, second);
        assert!(first.ends_with("}\n"), "the artifact ends with a newline");
    }

    /// The vocabulary is flux's own, taken straight from the IR's encoding rather than restated —
    /// a second spelling of `non_idempotent` is a second thing to keep in step.
    #[test]
    fn risk_and_idempotency_use_the_flux_vocabulary() {
        let document = rendered();
        let operation = &document["providers"][0]["operations"][0];
        assert_eq!(operation["risk"], json!("destructive"));
        assert_eq!(operation["idempotency"], json!("non_idempotent"));
        assert_eq!(operation["method"], json!("GET"));
    }

    /// **The AND/OR shape survives.** Two credentials on one request must not read as two ways to
    /// authenticate — the same property `crates/catalog` holds, from the same walk.
    #[test]
    fn an_and_group_stays_one_mechanism() {
        let document = rendered();
        assert_eq!(
            document["providers"][0]["operations"][0]["credentials"],
            json!([["acme.access_id", "acme.access_token"]])
        );
    }

    /// A parameter keeps its vendor schema **and** its wire spelling. Dropping either is how a
    /// typed catalogue quietly becomes a stringly-typed one.
    #[test]
    fn a_parameter_keeps_its_schema_its_position_and_its_wire_name() {
        let document = rendered();
        let parameter = &document["providers"][0]["operations"][0]["parameters"][0];
        assert_eq!(parameter["name"], json!("req_id"));
        assert_eq!(parameter["in"], json!("query"));
        assert_eq!(parameter["wire"], json!("requester_id"));
        assert_eq!(
            parameter["schema"],
            json!({"type": "string", "format": "uuid"})
        );
    }

    /// **The composed input schema is published beside the parameters, not instead of them.**
    ///
    /// It is the IR's composition verbatim — the point of C-125 is that there is one derivation, so
    /// this projection must not become a second one. Keyed by the caller-facing `req_id` rather than
    /// the wire `requester_id`: the vendor's spelling stays in the parameter entry, where a request
    /// builder reads it, and a caller passing `requester_id` would be passing an argument the
    /// operation does not have.
    #[test]
    fn an_operation_publishes_one_composed_input_schema() {
        let document = rendered();
        let operation = &document["providers"][0]["operations"][0];

        assert_eq!(
            operation["input_schema"],
            json!({
                "type": "object",
                "properties": {"req_id": {"type": "string", "format": "uuid"}},
                "required": [],
            })
        );
        assert_eq!(operation["input_schema"], self::operation().input_schema());
    }

    /// An operation that takes nothing publishes an **empty object schema**, not `null`. The
    /// asymmetry with `response_schema` next door is deliberate: "takes nothing" is derived, while
    /// "returns we-do-not-know" is not, and a permissive placeholder there would be
    /// indistinguishable from a real declaration.
    #[test]
    fn an_operation_that_takes_nothing_still_publishes_a_schema() {
        let mut connector = connector();
        connector.operations[0].params = ParamSet::default();
        let entry = provider_entry(&connector, &renderings()).unwrap();
        let document: Value = serde_json::from_str(&self::document(vec![entry]).unwrap()).unwrap();

        assert_eq!(
            document["providers"][0]["operations"][0]["input_schema"],
            json!({"type": "object", "properties": {}, "required": []})
        );
    }

    /// An absent value is `null` or `[]`, never a missing key: a consumer types the document once
    /// and never tests for existence.
    ///
    /// # Derived, not listed (C-206)
    ///
    /// The predecessor named three fields — `body_schema`, `response_schema`, `user_suffix` — and so
    /// it passed while a *fourth* field grew a `skip_serializing_if` and dropped its key from every
    /// shipped operation. A list covers what someone remembered and stops covering the class the
    /// moment a field is added, which is the same hand-maintained truth this repository exists to
    /// correct.
    ///
    /// The rule instead: render the document **twice** from the same emitter, once with every
    /// optional value absent and once with each of them present, and require that two objects at the
    /// same position carry the same keys. A conditionally-encoded key is exactly a key that appears
    /// in one rendering and not the other, and it fails here whichever direction it is missing in,
    /// for any field, without this test knowing one by name.
    ///
    /// The fixture below is the guard's one maintained input, and it is a much better thing to
    /// maintain than a field list: a new optional that nobody exercises here weakens the guard,
    /// while a new optional that nobody *lists* used to defeat it outright.
    #[test]
    fn optional_fields_are_null_rather_than_absent() {
        let sparse = rendered();
        let rich = rendered_with_optionals_present();

        // The rule, first, so that it is the derived check that reports a conditional key rather
        // than one of the sanity lines below happening to notice.
        let mut visited = std::collections::BTreeSet::new();
        same_published_shape(&sparse, &rich, "$", &mut visited);

        // Anti-vacuity, and named as positions rather than as a count: a walk that stopped early
        // would assert nothing and pass. `status` is the one the enumerated predecessor never
        // reached, so it is the one worth naming.
        for reached in [
            "$.providers[0]",
            "$.providers[0].auth.credentials[0]",
            "$.providers[0].operations[0]",
            "$.providers[0].operations[0].status",
        ] {
            assert!(
                visited.contains(reached),
                "the shape walk never reached `{reached}`, so it proves nothing about it"
            );
        }

        // And the sparse rendering really is the empty case, so the comparison above had something
        // to compare. These are examples of the class, not the guard for it.
        let operation = &sparse["providers"][0]["operations"][0];
        assert_eq!(operation["body_schema"], Value::Null);
        assert_eq!(operation["response_schema"], Value::Null);
        assert_eq!(operation["status"]["notes"], json!([]));
        assert_eq!(
            sparse["providers"][0]["auth"]["credentials"][0]["user_suffix"],
            Value::Null
        );
    }

    /// Positions whose value is **somebody else's JSON**, carried verbatim.
    ///
    /// A vendor schema, an event matcher, a binding map, and the independently owned core catalogue.
    /// Their keys are the vendor's or another emitter's and differ between two operations
    /// legitimately, so the shape walk stops at them.
    ///
    /// This is a list of *opaque positions*, not of optional fields, and the difference is the whole
    /// point: an opaque position is a property of the format and does not drift, while an optional
    /// field is precisely the thing that does.
    const OPAQUE: &[&str] = &[
        "schema",
        "schemas",
        "input_schema",
        "body_schema",
        "response_schema",
        "when",
        "payload",
        "bind",
        "tool_spec",
        "core",
    ];

    /// Assert that two renderings publish the same keys everywhere, recording each position reached.
    ///
    /// Arrays are walked pairwise and a length difference is fine — one rendering may declare more
    /// operations, or more notes, than the other. It is the *keys* of the objects inside that must
    /// agree.
    fn same_published_shape(
        sparse: &Value,
        rich: &Value,
        path: &str,
        visited: &mut std::collections::BTreeSet<String>,
    ) {
        match (sparse, rich) {
            (Value::Object(left), Value::Object(right)) => {
                assert_eq!(
                    left.keys().collect::<Vec<_>>(),
                    right.keys().collect::<Vec<_>>(),
                    "`{path}` publishes a different set of keys depending on its content, so a \
                     consumer has to test for existence"
                );
                visited.insert(path.to_string());
                for (key, value) in left {
                    if OPAQUE.contains(&key.as_str()) {
                        continue;
                    }
                    same_published_shape(value, &right[key], &format!("{path}.{key}"), visited);
                }
            }
            (Value::Array(left), Value::Array(right)) => {
                for (index, value) in left.iter().enumerate() {
                    let Some(counterpart) = right.get(index) else {
                        break;
                    };
                    same_published_shape(value, counterpart, &format!("{path}[{index}]"), visited);
                }
            }
            _ => {}
        }
    }

    /// The same connector as [`rendered`], with each optional value it can carry supplied.
    ///
    /// Every mutation here exists to make one key non-empty in the rendering, so that the walk above
    /// has something to compare the empty case against. `auth = Some(vec![])` is C-206's: it is what
    /// puts an entry in `status.notes`, and it is the case the old enumerated guard could not see.
    fn rendered_with_optionals_present() -> Value {
        let mut connector = connector();
        connector.authority = Some("com.acme.api".to_string());
        connector.api_version = Some("v2".to_string());
        connector.operations[0].auth = Some(vec![]);
        connector.operations[0].response_schema = Some(json!({"type": "object"}));
        connector.operations[0].params.body_schema = Some(json!({"type": "object"}));
        connector.auth[0].user_suffix = Some("/token".to_string());

        let entry = provider_entry(&connector, &renderings()).unwrap();
        serde_json::from_str(&document(vec![entry]).unwrap()).unwrap()
    }

    /// The scheme is a fixed three-key object rather than the IR's externally tagged encoding, so a
    /// consumer reads `scheme.kind` without a discriminated union.
    ///
    /// `bearer` publishes the prefix it stands for rather than an empty string: the mapping from
    /// `kind` to wire syntax is exactly what this document exists to spare a consumer, and a
    /// consumer that had to hard-code `"Bearer "` would be re-deriving it (C-184).
    #[test]
    fn a_scheme_without_a_name_still_carries_the_key() {
        let mut connector = connector();
        connector.auth = vec![AuthMethod::bearer("acme.token", vec!["ACME".to_string()])];
        connector.default_auth = vec![AuthRequirement::single("acme.token")];
        let entry = provider_entry(&connector, &renderings()).unwrap();
        let document: Value = serde_json::from_str(&document(vec![entry]).unwrap()).unwrap();
        assert_eq!(
            document["providers"][0]["auth"]["credentials"][0]["scheme"],
            json!({"kind": "bearer", "name": null, "prefix": "Bearer "})
        );
        assert_eq!(
            document["providers"][0]["auth"]["schemes"],
            json!(["bearer"])
        );
    }

    /// **A declared scheme word reaches the published document (C-184).** Without this key, Okta's
    /// `Authorization: SSWS <token>` and LaunchDarkly's raw `Authorization: <token>` are the same two
    /// keys, and a consumer building the first from the catalogue would build the second — a request
    /// the vendor rejects, from a document that looked complete.
    #[test]
    fn a_declared_header_prefix_is_published() {
        let mut connector = connector();
        connector.auth = vec![AuthMethod::prefixed_header(
            "acme.token",
            "Authorization",
            "SSWS ",
            vec!["ACME".to_string()],
        )];
        connector.default_auth = vec![AuthRequirement::single("acme.token")];
        let entry = provider_entry(&connector, &renderings()).unwrap();
        let document: Value = serde_json::from_str(&document(vec![entry]).unwrap()).unwrap();
        assert_eq!(
            document["providers"][0]["auth"]["credentials"][0]["scheme"],
            json!({"kind": "header", "name": "Authorization", "prefix": "SSWS "})
        );
    }

    /// The published prefix and the prefix the embedded Rust catalogue emits are the same string, for
    /// all three header-bearing schemes. They are produced by two functions in two modules, and a
    /// consumer that trusted the document while the pack sent something else would be debugging a
    /// 401 against a catalogue that agreed with itself everywhere it could see.
    #[test]
    fn a_published_prefix_matches_the_placement_the_catalogue_emits() {
        for scheme in [
            AuthScheme::Bearer,
            AuthScheme::Basic,
            AuthScheme::Header {
                name: "Authorization".to_string(),
                prefix: "Token token=".to_string(),
            },
            AuthScheme::Header {
                name: "X-Figma-Token".to_string(),
                prefix: String::new(),
            },
        ] {
            let published = scheme_prefix(&scheme);
            let emitted = crate::catalog::placement(&scheme);
            assert!(
                emitted.contains(&format!("prefix: {:?}", published)),
                "site publishes {published:?} but the catalogue emits {emitted}"
            );
        }
    }

    /// The Flux is the text the module carries, not a re-emission of it.
    #[test]
    fn the_operation_carries_the_rendering_it_was_given() {
        let document = rendered();
        assert_eq!(
            document["providers"][0]["operations"][0]["flux"],
            json!("op acme-thing-list -> Any\n")
        );
    }

    /// A rendering for an operation the connector does not declare would publish an entry nothing
    /// regenerates. It is a failure, not an entry that gets skipped — the same refusal
    /// `crate::catalog::render` makes.
    #[test]
    fn a_rendering_without_an_operation_is_refused() {
        let stray = vec![OperationRendering {
            id: "acme-thing-vanished".to_string(),
            source: String::new(),
        }];
        provider_entry(&connector(), &stray).expect_err("a stray rendering must not be described");
    }

    /// The host keeps its templating and drops the path — the same answer `crates/catalog` gives,
    /// because it is the same function.
    #[test]
    fn the_host_agrees_with_the_rust_catalog() {
        let document = rendered();
        assert_eq!(
            document["providers"][0]["hosts"],
            json!(["{tenant}.acme.example"])
        );
        assert_eq!(
            document["providers"][0]["operations"][0]["hosts"],
            json!(["{tenant}.acme.example"])
        );
    }

    /// **A provider's published host list is the union of its services'** (C-49), while each service's
    /// stays its own.
    ///
    /// Google is the shipped case: Gmail on `gmail.googleapis.com`, Calendar and Drive on
    /// `www.googleapis.com`. Publishing only the connector-level host would tell a reader the provider
    /// never calls a host three of its operations call, and publishing the union *per service* would
    /// widen an egress claim — so the two fields differ on purpose and both directions are asserted
    /// here.
    #[test]
    fn a_multi_service_provider_publishes_the_union_of_its_services_hosts() {
        let mut connector = connector();
        connector.base_url = "https://www.acme.example".to_string();
        connector.services = vec![
            connector_spec::Service {
                name: "mail".to_string(),
                description: "Mail.".to_string(),
                base_url: Some("https://mail.acme.example".to_string()),
                api_version: Some("v1".to_string()),
                roles: Vec::new(),
            },
            connector_spec::Service {
                name: "calendar".to_string(),
                description: "Calendar.".to_string(),
                base_url: None,
                api_version: Some("v3".to_string()),
                roles: Vec::new(),
            },
        ];
        connector.operations[0].service = "mail".to_string();

        let entry = provider_entry(&connector, &renderings()).expect("the connector describes");
        let document = serde_json::to_value(&entry).expect("the entry serializes");

        assert_eq!(
            document["hosts"],
            json!(["mail.acme.example", "www.acme.example"]),
            "the provider must publish every host it reaches, in service declaration order"
        );
        assert_eq!(
            document["services"][0]["hosts"],
            json!(["mail.acme.example"])
        );
        assert_eq!(
            document["services"][1]["hosts"],
            json!(["www.acme.example"])
        );
        assert_eq!(
            document["operations"][0]["hosts"],
            json!(["mail.acme.example"]),
            "an operation reaches its own service's host"
        );
    }

    /// And a single-surface provider publishes exactly the one host it always did — the union of one.
    #[test]
    fn a_single_service_provider_publishes_exactly_its_own_host() {
        let entry = provider_entry(&connector(), &renderings()).expect("the connector describes");
        let document = serde_json::to_value(&entry).expect("the entry serializes");
        assert_eq!(document["hosts"], json!(["{tenant}.acme.example"]));
    }

    #[test]
    fn the_document_names_its_version_without_publishing_internal_references() {
        let document = rendered();
        assert_eq!(document["schema_version"], json!(SCHEMA_VERSION));
        assert!(document.get("documentation").is_none());
        assert!(
            document["providers"][0]["operations"][0]["status"]["issues"][0]
                .get("story")
                .is_none()
        );
        assert_eq!(document["providers"][0]["operation_count"], json!(1));
    }
}
