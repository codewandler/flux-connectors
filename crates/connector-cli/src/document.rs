//! Lowering the IR to the canonical per-provider catalog document — C-536 (Decision 0022).
//!
//! One deterministic, committed JSON document per provider, at `catalog/<name>.catalog.json`:
//! the reviewed artifact of [the catalog-artifact design](../../../docs/designs/catalog-artifact.md).
//! It carries the complete published surface — provider and services metadata, every operation
//! with an explicit **request template**, the full auth surface including the complete OAuth2
//! declaration and its token-endpoint quirks, configuration, verify, events, channel bindings —
//! and it is the first artifact the four surfaces the op grammar cannot say ever reach:
//! service `roles`, `quirks.pagination`, `quirks.rate_limit` and the structural `error_envelope`.
//!
//! # The template vocabulary is closed and total
//!
//! A request template is built from **literal values**, `{var}` interpolation over caller
//! parameters and endpoint slots, and `{"$param": name}` splices — nothing else. It is the data
//! equivalent of what `connector-pack/src/request.rs` accepts from the emitted Flux body, and the
//! closure runs the same direction: anything that seven-node evaluator refuses as unclassifiable
//! configuration (C-193/C-232) has **no spelling here and is a build error**, never a silently
//! degraded document. The C-110 shape — a vendor's own brace-carrying syntax bound as a literal —
//! used to build successfully and fail at installation; it now fails the build that would have
//! committed it. `crates/connector-pack/tests/document_differential.rs` holds the template and the
//! Flux-derived request together, field for field; the whole-catalogue gate is C-538's.
//!
//! # No field for a registration value exists
//!
//! The OAuth2 object deliberately has **no field** for a client identifier or secret: those are
//! per-deployment registration values, published only as the operator-level configuration
//! requirement through the existing `binds = "oauth.*"` grammar. The schema closes the object
//! (`additionalProperties: false`), so a future document cannot carry one for a consumer to
//! mistakenly trust, and a provider file declaring a non-empty value is refused here rather than
//! emitted. (Exchange-side X-154 additionally ignores any such value; unrepresentable-plus-ignored
//! is the pairing.)
//!
//! # Determinism
//!
//! The document is a function of the IR alone, and the emitted key order is **alphabetical
//! throughout**, not the struct-declaration order: [`render`] round-trips the typed wire shape
//! through `serde_json::to_value` so the schema validates the exact value being written, and
//! `serde_json`'s map is BTree-backed (no `preserve_order`), so every object re-sorts on the way
//! through. Determinism comes from that sort plus `BTreeMap` everywhere a map is built by hand;
//! the text is `to_string_pretty` plus one trailing newline, exactly the `core_catalog.rs` shape.
//! Unchanged inputs reproduce every byte; `connectors.lock` hashes each document per provider.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

use connector_spec::{
    AuthMethod, AuthScheme, BodyEncoding, ChannelBinding, ConfigField, Connector, HttpMethod,
    Idempotency, ManualSetup, OAuthGrant, OAuthRedirect, Operation, OperationDirection, Param,
    Risk, Role, SemanticEffect, Service, SocketConnectSpec, Subscription, Tag, TokenEndpointQuirk,
    VerificationScheme, FREE_FORM_BODY,
};

use crate::seam;

/// The document's wire-contract version. Additive evolution does not bump it; anything a consumer
/// must act on does, and is refused by older readers — the `site.rs` rule, restated for the
/// canonical artifact.
pub const SCHEMA_VERSION: u32 = 1;

/// The schema's `$id`, and the `$schema` every document names — the same repository-addressed form
/// `schema/provider-toml.schema.json` uses.
pub const SCHEMA_ID: &str =
    "https://github.com/codewandler/flux-connectors/catalog/connector-document.schema.json";

// ---------------------------------------------------------------------------------------------
// The wire shape. Declaration order here does NOT survive into the text: `render` round-trips
// through `serde_json::to_value` for schema validation, whose BTree-backed map re-sorts every
// object, so the emitted key order is alphabetical. Nothing may depend on field order.
// ---------------------------------------------------------------------------------------------

#[derive(Serialize)]
struct Document<'a> {
    #[serde(rename = "$schema")]
    schema: &'static str,
    schema_version: u32,
    /// The generator identity, as the manifests carry it. Input hashes are deliberately absent:
    /// they live in `connectors.lock`, which also hashes this document — embedding `toml_sha256`
    /// here would make a comment-only provider edit rewrite the document, and the lockfile exists
    /// so that such an edit moves the lockfile and nothing else.
    generator: String,
    connector: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    vendor: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    /// Never skipped, for the manifest's reason: a host must not infer `http` from an absence.
    runtime: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    authority: Option<&'a str>,
    /// The bounded read a settings page invokes — an operation id.
    #[serde(skip_serializing_if = "Option::is_none")]
    verify: Option<&'a str>,
    services: Vec<DocService<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    auth: Vec<DocAuth<'a>>,
    /// Connector-level alternatives, flattened to the OR-of-AND-groups shape `catalog.json`
    /// already publishes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    default_auth: Vec<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    config: Vec<DocConfig<'a>>,
    operations: Vec<DocOperation<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    events: Vec<DocEvent<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    channels: Vec<DocChannel<'a>>,
}

/// One service, with its base URL and API version **resolved** (the connector's as defaults) and
/// its template preserved — the `{variable}` spelling is what the request template's `{base}`
/// slot substitutes into.
#[derive(Serialize)]
struct DocService<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    gid: Option<String>,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    base_url: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_version: Option<&'a str>,
    #[serde(skip_serializing_if = "is_false")]
    legacy: bool,
    /// The first artifact a declared service role reaches.
    #[serde(skip_serializing_if = "<[Role]>::is_empty")]
    roles: &'a [Role],
    #[serde(skip_serializing_if = "<[Tag]>::is_empty")]
    tags: &'a [Tag],
}

/// One credential declaration — scheme, acquisition and placement facts, never a value.
#[derive(Serialize)]
struct DocAuth<'a> {
    name: &'a str,
    scheme: DocScheme<'a>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    env: &'a [String],
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    user_env: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    user_suffix: Option<&'a str>,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    /// Whose authority the credential exercises — stated even when unstated, so a consumer never
    /// has to know the IR's default.
    subject: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    hazard: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    oauth2: Option<DocOAuth2<'a>>,
    /// The measured token-endpoint departures (C-440) — their first structured artifact beyond
    /// the TOML manifest.
    #[serde(skip_serializing_if = "<[TokenEndpointQuirk]>::is_empty")]
    token_endpoint_quirks: &'a [TokenEndpointQuirk],
}

/// The placement scheme, flattened to `{kind, name, prefix}` so a consumer needs no discriminated
/// union — `site.rs`'s `SchemeEntry` shape.
#[derive(Serialize)]
struct DocScheme<'a> {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<&'a str>,
}

/// The complete OAuth2 declaration — grants, paths, scopes, redirect — **with no field for a
/// registration value**. See the module documentation.
#[derive(Serialize)]
struct DocOAuth2<'a> {
    #[serde(skip_serializing_if = "str::is_empty")]
    endpoint: &'a str,
    /// The declared service the token exchange resolves against when it is not `endpoint` (C-556) —
    /// a name, never a URL, so a consumer joins `token_path` onto this service's base URL when set
    /// and onto `endpoint`'s otherwise. Skipped when absent, so no single-host document moves.
    #[serde(skip_serializing_if = "str::is_empty")]
    token_endpoint: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    authorize_path: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    token_path: &'a str,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    scopes: &'a [String],
    #[serde(skip_serializing_if = "<[OAuthGrant]>::is_empty")]
    grants: &'a [OAuthGrant],
    #[serde(skip_serializing_if = "Option::is_none")]
    redirect: Option<&'a OAuthRedirect>,
    /// Whether this is a public PKCE client that uses no client secret (C-556). Skipped when
    /// `false` — a confidential client — so no already-published document moves.
    #[serde(skip_serializing_if = "is_false")]
    public_client: bool,
}

/// One configuration field: the declaration plus the level derived from what it binds — the same
/// pair the manifest publishes, spelled explicitly rather than flattened so every key is present.
#[derive(Serialize)]
struct DocConfig<'a> {
    name: &'a str,
    service: &'a str,
    label: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    help: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    example: Option<&'a str>,
    format: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    choices: Vec<DocChoice<'a>>,
    required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<&'a str>,
    approval: &'static str,
    secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    docs_url: Option<&'a str>,
    binds: &'a str,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    also_binds: &'a [String],
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    also_services: &'a [String],
    level: connector_spec::Level,
}

#[derive(Serialize)]
struct DocChoice<'a> {
    value: &'a str,
    label: &'a str,
}

#[derive(Serialize)]
struct DocOperation<'a> {
    id: &'a str,
    service: &'a str,
    direction: &'a OperationDirection,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    risk: &'a Risk,
    idempotency: &'a Idempotency,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeatability_condition: Option<&'a str>,
    /// Explicit even when empty: "none declared" is an answer, not an absence.
    semantic_effects: &'a [SemanticEffect],
    expose: bool,
    /// The **effective** requirement — the inheritance rule already resolved, so a consumer never
    /// re-implements it.
    auth: Vec<Vec<&'a str>>,
    request: DocRequest<'a>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    params: Vec<DocParam<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_schema: Option<Value>,
    /// Where each endpoint-configuration variable lands on the request (C-214's vocabulary:
    /// `origin`, `host`, `path`, `query`, `header`). A variable may land in more than one
    /// position (C-229), so the value is the sorted set of them.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    endpoint: BTreeMap<String, BTreeSet<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quirks: Option<DocQuirks<'a>>,
}

/// **The request template** — the data equivalent of the emitted Flux body.
#[derive(Serialize)]
struct DocRequest<'a> {
    method: &'a HttpMethod,
    /// `{base}` plus the vendor path, each placeholder renamed to the caller-facing parameter
    /// name or the endpoint slot it interpolates.
    url: String,
    /// Constant headers as literals, caller headers as `$param` splices, operator-pinned headers
    /// as `{slot}` literals. The media type is derived from the body encoding, never authored.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    headers: BTreeMap<String, Value>,
    /// The structured query record (C-30), in its wire order: null values are omitted at
    /// evaluation time and every key and value is RFC 3986-encoded exactly once.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    query: Vec<DocQueryEntry<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<DocBody>,
}

#[derive(Serialize)]
struct DocQueryEntry<'a> {
    name: &'a str,
    value: Value,
}

#[derive(Serialize)]
struct DocBody {
    encoding: &'static str,
    /// A JSON body: the nested wire tree, `$param` splices at caller leaves, pinned `const`
    /// values as literals.
    #[serde(skip_serializing_if = "Option::is_none")]
    template: Option<Value>,
    /// A `form` body: the pairs in emitted order — every always-sent pair first (required or
    /// `const`), then the truthiness-guarded optional ones (C-144's semantics).
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<Vec<DocFormField>>,
}

#[derive(Serialize)]
struct DocFormField {
    name: String,
    value: Value,
    /// Whether the pair always travels; a guarded pair is sent only when its value is truthy.
    required: bool,
}

#[derive(Serialize)]
struct DocParam<'a> {
    name: &'a str,
    position: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    wire: Option<&'a str>,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    required: bool,
    schema: &'a Value,
}

#[derive(Serialize)]
struct DocQuirks<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pagination: Option<&'a connector_spec::Pagination>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit: Option<&'a connector_spec::RateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_envelope: Option<&'a connector_spec::ErrorEnvelope>,
}

#[derive(Serialize)]
struct DocEvent<'a> {
    name: &'a str,
    service: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    oip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wire_value: Option<&'a str>,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    default: bool,
    #[serde(skip_serializing_if = "str::is_empty")]
    group: &'a str,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    when: &'a BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<&'a Value>,
}

#[derive(Serialize)]
struct DocChannel<'a> {
    name: &'a str,
    service: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    oip: Option<String>,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    transport: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    connect: Option<&'a SocketConnectSpec>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    events: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interval: Option<&'a str>,
    verification: DocVerification<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discriminator: Option<DocSelector<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_id: Option<DocSelector<'a>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    payload: &'a BTreeMap<String, String>,
    #[serde(skip_serializing_if = "is_false")]
    payload_root: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply: Option<DocReply<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subscription: Option<&'a Subscription>,
    #[serde(skip_serializing_if = "Option::is_none")]
    setup: Option<&'a ManualSetup>,
}

#[derive(Serialize)]
struct DocVerification<'a> {
    kind: &'static str,
    verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    hmac: Option<DocHmac<'a>>,
}

#[derive(Serialize)]
struct DocHmac<'a> {
    algorithm: &'static str,
    encoding: &'static str,
    header: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<&'a str>,
    signed: &'a str,
    secret: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tolerance: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp_format: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<DocSelector<'a>>,
}

#[derive(Serialize)]
struct DocSelector<'a> {
    source: &'static str,
    name: &'a str,
}

#[derive(Serialize)]
struct DocReply<'a> {
    operation: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    oip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<&'a str>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    bind: &'a BTreeMap<String, String>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

// ---------------------------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------------------------

/// Render one connector's canonical document, validated against [`schema`] before it is returned.
///
/// # Errors
///
/// Every construct the closed template vocabulary cannot say — an unclassifiable brace-carrying
/// literal, a declared OAuth2 registration value, a graph (deferred by the design), a body path
/// the wire grammar refuses — is an error naming the operation and the construct. A loud
/// compile-time refusal is better than plausible but incorrect data.
pub fn render(connector: &Connector) -> Result<String> {
    let document = lower(connector)?;
    let value = serde_json::to_value(&document).context("cannot serialize a catalog document")?;
    if let Err(error) = validator().validate(&value) {
        bail!(
            "connector `{}`'s catalog document does not validate against {SCHEMA_ID}: {error}",
            connector.id
        );
    }
    Ok(format!("{}\n", serde_json::to_string_pretty(&value)?))
}

/// The committed text of the document schema itself — one more planned artifact, so drift in the
/// schema is caught exactly as drift in a document is.
pub fn schema_text() -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(schema()).expect("the schema serializes")
    )
}

fn lower<'a>(connector: &'a Connector) -> Result<Document<'a>> {
    // The design defers the graph lowering ("declarative plan or stay unpublished"); nothing
    // shipped declares one, and dropping a declared surface silently is the one thing this
    // artifact exists to end.
    if !connector.graphs.is_empty() {
        bail!(
            "connector `{}` declares a graph, which the catalog document cannot say yet — the \
             lowering is deferred by docs/designs/catalog-artifact.md's open questions",
            connector.id
        );
    }

    let services = connector
        .service_names()
        .into_iter()
        .map(|name| {
            let declared: Option<&Service> = connector
                .services
                .iter()
                .find(|service| service.name == name);
            DocService {
                name,
                gid: connector.gid_of(name).map(|gid| gid.to_string()),
                description: declared.map(|s| s.description.as_str()).unwrap_or(""),
                base_url: connector.base_url_of(name),
                api_version: connector.api_version_of(name),
                legacy: declared.is_some_and(|s| s.legacy),
                roles: declared.map(|s| s.roles.as_slice()).unwrap_or(&[]),
                tags: declared.map(|s| s.tags.as_slice()).unwrap_or(&[]),
            }
        })
        .collect();

    let auth = connector
        .auth
        .iter()
        .map(|method| doc_auth(connector, method))
        .collect::<Result<Vec<_>>>()?;

    let config = connector
        .config
        .iter()
        .map(doc_config)
        .collect::<Result<Vec<_>>>()?;

    let operations = connector
        .operations
        .iter()
        .map(|operation| doc_operation(connector, operation))
        .collect::<Result<Vec<_>>>()?;

    let events = connector
        .events
        .iter()
        .map(|event| DocEvent {
            name: &event.name,
            service: &event.service,
            oip: member_oip(connector, &event.service, &event.name),
            wire_value: event.wire_value.as_deref(),
            description: &event.description,
            default: event.default,
            group: &event.group,
            when: &event.when,
            schema: event.schema.as_ref(),
        })
        .collect();

    let channels = connector
        .channels
        .iter()
        .map(|channel| doc_channel(connector, channel))
        .collect();

    Ok(Document {
        schema: SCHEMA_ID,
        schema_version: SCHEMA_VERSION,
        generator: seam::generator(),
        connector: &connector.id,
        vendor: &connector.vendor,
        description: &connector.description,
        runtime: connector.runtime.word(),
        authority: connector.authority.as_deref(),
        verify: connector.verify.as_deref(),
        services,
        auth,
        default_auth: requirements(&connector.default_auth),
        config,
        operations,
        events,
        channels,
    })
}

/// The OR-of-AND-groups flattening `catalog.json` already publishes.
fn requirements(declared: &[connector_spec::AuthRequirement]) -> Vec<Vec<&str>> {
    declared
        .iter()
        .map(|requirement| requirement.iter().map(String::as_str).collect())
        .collect()
}

fn doc_auth<'a>(connector: &Connector, method: &'a AuthMethod) -> Result<DocAuth<'a>> {
    let scheme = match &method.scheme {
        AuthScheme::Bearer => DocScheme {
            kind: "bearer",
            name: None,
            prefix: None,
        },
        AuthScheme::Basic => DocScheme {
            kind: "basic",
            name: None,
            prefix: None,
        },
        AuthScheme::Header { name, prefix } => DocScheme {
            kind: "header",
            name: Some(name),
            prefix: (!prefix.is_empty()).then_some(prefix.as_str()),
        },
        AuthScheme::Query { name } => DocScheme {
            kind: "query",
            name: Some(name),
            prefix: None,
        },
        AuthScheme::Signing => DocScheme {
            kind: "signing",
            name: None,
            prefix: None,
        },
    };

    let oauth2 = match &method.oauth2 {
        None => None,
        Some(spec) => {
            // Registration identity is not vendor truth. The document has no field for it, so a
            // declared value would vanish silently — which is worse than a refusal, because the
            // author plainly expected it to matter.
            if !spec.client_id.is_empty() {
                bail!(
                    "connector `{}` declares an OAuth2 client identifier value on `{}`; a \
                     registration value is per-deployment and the catalog document has no field \
                     for one — publish the requirement through a `[[config]]` field binding \
                     `oauth.client_id` instead (C-536)",
                    connector.id,
                    method.name
                );
            }
            Some(DocOAuth2 {
                endpoint: &spec.endpoint,
                token_endpoint: &spec.token_endpoint,
                authorize_path: &spec.authorize_path,
                token_path: &spec.token_path,
                scopes: &spec.scopes,
                grants: &spec.grants,
                redirect: spec.redirect.as_ref(),
                public_client: spec.public_client,
            })
        }
    };

    Ok(DocAuth {
        name: &method.name,
        scheme,
        env: &method.env,
        user_env: &method.user_env,
        user_suffix: method.user_suffix.as_deref(),
        description: &method.description,
        subject: method.subject.word(),
        hazard: method.hazard.as_ref().map(|hazard| hazard.word()),
        oauth2,
        token_endpoint_quirks: &method.quirks.token_endpoint,
    })
}

fn doc_config(field: &ConfigField) -> Result<DocConfig<'_>> {
    let level = field.level().ok_or_else(|| {
        anyhow!(
            "configuration field {:?} has invalid binding {:?}",
            field.name,
            field.binds
        )
    })?;
    Ok(DocConfig {
        name: &field.name,
        service: &field.service,
        label: &field.label,
        help: &field.help,
        example: field.example.as_deref(),
        format: field.format.word(),
        choices: field
            .choices
            .iter()
            .map(|choice| DocChoice {
                value: &choice.value,
                label: &choice.label,
            })
            .collect(),
        required: field.required,
        default: field.default.as_deref(),
        approval: field.approval.word(),
        secret: field.secret,
        docs_url: field.docs_url.as_deref(),
        binds: &field.binds,
        also_binds: &field.also_binds,
        also_services: &field.also_services,
        level,
    })
}

fn doc_channel<'a>(connector: &Connector, channel: &'a ChannelBinding) -> DocChannel<'a> {
    let verification = crate::inbound::verification_of(channel);
    DocChannel {
        name: &channel.name,
        service: &channel.service,
        oip: member_oip(connector, &channel.service, &channel.name),
        description: &channel.description,
        transport: crate::inbound::transport_token(channel.transport),
        connect: channel.connect.as_ref(),
        events: channel.events.iter().map(String::as_str).collect(),
        cursor: channel.cursor.as_deref(),
        interval: channel.interval.as_deref(),
        verification: DocVerification {
            kind: verification.kind(),
            verified: verification.verified(),
            hmac: match &channel.verification {
                Some(VerificationScheme::Hmac(spec)) => Some(DocHmac {
                    algorithm: crate::inbound::digest_token(spec.algorithm),
                    encoding: crate::inbound::encoding_token(spec.encoding),
                    header: &spec.header,
                    prefix: spec.prefix.as_deref(),
                    signed: &spec.signed,
                    secret: &spec.secret,
                    tolerance: spec.tolerance.as_deref(),
                    timestamp_format: crate::inbound::timestamp_format_of(spec),
                    timestamp: spec.timestamp.as_ref().map(doc_selector),
                }),
                Some(VerificationScheme::None) | None => None,
            },
        },
        discriminator: channel.discriminator.as_ref().map(doc_selector),
        delivery_id: channel.delivery_id.as_ref().map(doc_selector),
        payload: &channel.payload,
        payload_root: channel.payload_root,
        reply: channel.reply.as_ref().map(|reply| DocReply {
            operation: &reply.operation,
            oip: member_oip(connector, &channel.service, &reply.operation),
            result: reply.result.as_deref(),
            bind: &reply.bind,
        }),
        subscription: channel.subscription.as_ref(),
        setup: channel.setup.as_ref(),
    }
}

fn doc_selector(selector: &connector_spec::Selector) -> DocSelector<'_> {
    DocSelector {
        source: crate::inbound::source_token(selector.source),
        name: &selector.name,
    }
}

fn member_oip(connector: &Connector, service: &str, name: &str) -> Option<String> {
    connector
        .oip_of_member(service, name)
        .map(|oip| oip.to_string())
}

// ---------------------------------------------------------------------------------------------
// The operation: params, request template, endpoint slots
// ---------------------------------------------------------------------------------------------

/// The vendor spelling: [`Param::wire`] when declared, the caller-facing name otherwise — the
/// same rule `connector-flux`'s emitter reads every position through.
fn wire_name(param: &Param) -> &str {
    param.wire.as_deref().unwrap_or(&param.name)
}

/// The value a body field is pinned to, when its schema pins one — JSON Schema's `const` read as
/// the declaration it already is (`connector-flux/src/op.rs::constant`).
fn constant(param: &Param) -> Option<&Value> {
    param.schema.get("const")
}

fn doc_operation<'a>(
    connector: &'a Connector,
    operation: &'a Operation,
) -> Result<DocOperation<'a>> {
    let pins = pins_of(connector, operation);

    let params = doc_params(operation);
    let request = request_template(operation, &pins)?;
    let endpoint = endpoint_slots(connector, operation, &pins)?;

    let quirks = (!operation.quirks.is_empty()).then_some(DocQuirks {
        pagination: operation.quirks.pagination.as_ref(),
        rate_limit: operation.quirks.rate_limit.as_ref(),
        error_envelope: operation.quirks.error_envelope.as_ref(),
    });

    Ok(DocOperation {
        id: &operation.id,
        service: &operation.service,
        direction: &operation.direction,
        description: &operation.description,
        risk: &operation.risk,
        idempotency: &operation.idempotency,
        repeatability_condition: operation.repeatability_condition(),
        semantic_effects: &operation.semantic_effects,
        expose: operation.expose,
        auth: requirements(connector.effective_auth(operation)),
        request,
        params,
        // The *effective* schema, so a credential-minting operation's document promises the
        // handle, never the secret (C-136/C-430).
        response_schema: operation.effective_response_schema(),
        endpoint,
        quirks,
    })
}

/// The caller surface: every declared parameter with its structural position. A `const`-pinned
/// body field is sent but never declared — it lives in the body template, exactly as the emitted
/// declaration omits it — and a free-form body is one parameter under [`FREE_FORM_BODY`].
fn doc_params(operation: &Operation) -> Vec<DocParam<'_>> {
    let set = &operation.params;
    let mut params = Vec::new();
    for (position, group) in [
        ("path", &set.path),
        ("query", &set.query),
        ("header", &set.header),
        ("body", &set.body),
    ] {
        for param in group {
            if position == "body" && constant(param).is_some() {
                continue;
            }
            params.push(DocParam {
                name: &param.name,
                position,
                wire: param.wire.as_deref(),
                description: &param.description,
                required: param.required,
                schema: &param.schema,
            });
        }
    }
    if let Some(schema) = &set.body_schema {
        params.push(DocParam {
            name: FREE_FORM_BODY,
            position: "body",
            wire: None,
            description: "",
            required: true,
            schema,
        });
    }
    params
}

/// An operator-pinned value that applies to this operation, in the emitter's own selection order
/// (`connector-flux/src/op.rs::bind_pins`): a path pin only where the path carries its
/// placeholder, a query or header pin on every operation of the service.
struct DocPin<'a> {
    position: connector_spec::Position,
    name: &'a str,
    variable: String,
}

fn pins_of<'a>(connector: &'a Connector, operation: &'a Operation) -> Vec<DocPin<'a>> {
    connector
        .config_of(&operation.service)
        .flat_map(|field| field.pins())
        .filter(|pin| match pin.position {
            connector_spec::Position::Path => {
                connector_spec::config::template_variables(&operation.path).contains(&pin.name)
            }
            connector_spec::Position::Query | connector_spec::Position::Header => true,
        })
        .map(|pin| DocPin {
            position: pin.position,
            name: pin.name,
            variable: pin.variable.into_owned(),
        })
        .collect()
}

/// `{"$param": name}` — the splice form of the closed vocabulary.
fn param_splice(name: &str) -> Value {
    json!({ "$param": name })
}

fn request_template<'a>(operation: &'a Operation, pins: &[DocPin<'a>]) -> Result<DocRequest<'a>> {
    let set = &operation.params;
    let url = format!("{{base}}{}", path_template(operation, pins)?);

    // The structured query record, in the emitted record's own order: BTreeMap over wire names.
    let mut query: BTreeMap<&str, Value> = BTreeMap::new();
    for param in &set.query {
        let wire = wire_name(param);
        if query.insert(wire, param_splice(&param.name)).is_some() {
            bail!(
                "operation `{}` declares the query parameter `{wire}` twice",
                operation.id
            );
        }
    }
    for pin in pins {
        if pin.position != connector_spec::Position::Query {
            continue;
        }
        if query
            .insert(pin.name, Value::String(format!("{{{}}}", pin.variable)))
            .is_some()
        {
            bail!(
                "operation `{}`'s query pin `{}` collides with a declared parameter",
                operation.id,
                pin.name
            );
        }
    }

    // Headers: the derived media type, the vendor's constants, the caller's, the operator's.
    let has_body = !set.body.is_empty() || set.body_schema.is_some();
    let mut headers: BTreeMap<String, Value> = BTreeMap::new();
    if has_body {
        headers.insert(
            "content-type".to_string(),
            Value::String(set.body_encoding.media_type().to_string()),
        );
    }
    for param in &set.header {
        headers.insert(wire_name(param).to_string(), param_splice(&param.name));
    }
    for (name, value) in &set.const_headers {
        refuse_braced_literal(operation, &format!("constant header `{name}`"), value)?;
        headers.insert(name.clone(), Value::String(value.clone()));
    }
    for pin in pins {
        if pin.position == connector_spec::Position::Header {
            headers.insert(
                pin.name.to_string(),
                Value::String(format!("{{{}}}", pin.variable)),
            );
        }
    }

    let body = body_template(operation)?;

    Ok(DocRequest {
        method: &operation.method,
        url,
        headers,
        query: query
            .into_iter()
            .map(|(name, value)| DocQueryEntry { name, value })
            .collect(),
        body,
    })
}

/// The vendor path with each `{wire}` renamed to the caller-facing parameter name or the pin's
/// slot variable — `connector-flux`'s `path_template`, writing template names instead of Flux
/// symbols. Both directions of mismatch are refused for the emitter's reasons.
fn path_template(operation: &Operation, pins: &[DocPin<'_>]) -> Result<String> {
    let path_params = &operation.params.path;
    let mut out = String::new();
    let mut used = vec![false; path_params.len()];
    let mut rest = operation.path.as_str();

    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            bail!(
                "operation `{}`'s path {:?} has an unterminated placeholder",
                operation.id,
                operation.path
            );
        };
        let wire = &after[..close];
        let name = match path_params
            .iter()
            .position(|param| wire_name(param) == wire)
        {
            Some(index) => {
                used[index] = true;
                path_params[index].name.as_str()
            }
            None => pins
                .iter()
                .find(|pin| pin.position == connector_spec::Position::Path && pin.name == wire)
                .map(|pin| pin.variable.as_str())
                .ok_or_else(|| {
                    anyhow!(
                        "operation `{}`'s path names `{{{wire}}}`, which neither a path \
                         parameter nor an operator pin declares",
                        operation.id
                    )
                })?,
        };
        out.push('{');
        out.push_str(name);
        out.push('}');
        rest = &after[close + 1..];
    }
    out.push_str(rest);

    if let Some(index) = used.iter().position(|used| !used) {
        bail!(
            "operation `{}` declares the path parameter `{}`, which its path never places",
            operation.id,
            path_params[index].name
        );
    }
    if !out.starts_with('/') {
        out.insert(0, '/');
    }
    Ok(out)
}

/// The body template, or `None` for a request that sends none.
fn body_template(operation: &Operation) -> Result<Option<DocBody>> {
    let set = &operation.params;

    if set.body_schema.is_some() {
        return Ok(Some(DocBody {
            encoding: "json",
            template: Some(param_splice(FREE_FORM_BODY)),
            fields: None,
        }));
    }
    if set.body.is_empty() {
        return Ok(None);
    }

    // Exhaustive on purpose: a third `BodyEncoding` variant must fail to compile here rather
    // than fall through to a spelling it never meant — the module's refuse-loudly rule, applied
    // at the type level.
    match set.body_encoding {
        BodyEncoding::Json => {
            let tree = body_tree(operation)?;
            Ok(Some(DocBody {
                encoding: "json",
                template: Some(tree),
                fields: None,
            }))
        }
        BodyEncoding::Form => {
            // The emitted pair order: every always-sent pair first, then the guarded ones, each
            // group in declaration order (`connector-flux/src/op.rs::form_payload`).
            let mut fields = Vec::new();
            let (always, guarded): (Vec<&Param>, Vec<&Param>) = set
                .body
                .iter()
                .partition(|param| param.required || constant(param).is_some());
            for (group, required) in [(always, true), (guarded, false)] {
                for param in group {
                    let value = match constant(param) {
                        Some(value) => {
                            refuse_braced_constant(operation, param, value)?;
                            value.clone()
                        }
                        None => param_splice(&param.name),
                    };
                    fields.push(DocFormField {
                        name: wire_name(param).to_string(),
                        value,
                        required,
                    });
                }
            }
            Ok(Some(DocBody {
                encoding: "form",
                template: None,
                fields: Some(fields),
            }))
        }
    }
}

/// One step of a parsed [`Param::wire`] path — the emitter's grammar (`key` or `key[0]`, joined
/// by dots), refused where it refuses.
enum Step<'a> {
    Key(&'a str),
    Index(usize),
}

fn wire_steps<'a>(operation: &Operation, param: &Param, wire: &'a str) -> Result<Vec<Step<'a>>> {
    let refuse = |reason: String| {
        anyhow!(
            "operation `{}` body field `{}` has wire path {wire:?} the template cannot say: {reason}",
            operation.id,
            param.name
        )
    };
    let mut steps = Vec::new();
    for segment in wire.split('.') {
        let (key, brackets) = match segment.find('[') {
            Some(at) => segment.split_at(at),
            None => (segment, ""),
        };
        if key.is_empty() {
            return Err(refuse("a segment is empty".to_string()));
        }
        if key.contains(']') {
            return Err(refuse("it closes a bracket it never opened".to_string()));
        }
        if key.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(refuse(format!(
                "a bare numeric segment is ambiguous; spell an index as `[{key}]`"
            )));
        }
        steps.push(Step::Key(key));

        if brackets.is_empty() {
            continue;
        }
        let inner = &brackets[1..];
        let Some(close) = inner.find(']') else {
            return Err(refuse("its `[` is never closed".to_string()));
        };
        let (digits, after) = inner.split_at(close);
        if !after[1..].is_empty() {
            return Err(refuse("text follows the closing `]`".to_string()));
        }
        if digits.is_empty()
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
            || (digits.len() > 1 && digits.starts_with('0'))
        {
            return Err(refuse(format!("`[{digits}]` is not a decimal index")));
        }
        let index = digits
            .parse::<usize>()
            .map_err(|_| refuse("the index does not fit this machine".to_string()))?;
        steps.push(Step::Index(index));
    }
    Ok(steps)
}

/// The JSON body tree: each field's value at the wire path it declares, `$param` splices at
/// caller leaves and pinned `const` values as literals — `connector-flux`'s `body_tree`, lowered
/// to data.
fn body_tree(operation: &Operation) -> Result<Value> {
    enum Node {
        Leaf(Value),
        Branch(BTreeMap<String, Node>),
        Elements(BTreeMap<usize, Node>),
    }

    impl Node {
        fn empty_for(step: &Step<'_>) -> Node {
            match step {
                Step::Key(_) => Node::Branch(BTreeMap::new()),
                Step::Index(_) => Node::Elements(BTreeMap::new()),
            }
        }

        fn insert(&mut self, steps: &[Step<'_>], leaf: Value) -> std::result::Result<(), ()> {
            let (head, rest) = steps.split_first().expect("a wire path has a step");
            let slot = match (&mut *self, head) {
                (Node::Branch(children), Step::Key(key)) => {
                    if rest.is_empty() {
                        if children.contains_key(*key) {
                            return Err(());
                        }
                        children.insert((*key).to_string(), Node::Leaf(leaf));
                        return Ok(());
                    }
                    children
                        .entry((*key).to_string())
                        .or_insert_with(|| Node::empty_for(&rest[0]))
                }
                (Node::Elements(items), Step::Index(index)) => {
                    if rest.is_empty() {
                        if items.contains_key(index) {
                            return Err(());
                        }
                        items.insert(*index, Node::Leaf(leaf));
                        return Ok(());
                    }
                    items
                        .entry(*index)
                        .or_insert_with(|| Node::empty_for(&rest[0]))
                }
                _ => return Err(()),
            };
            slot.insert(rest, leaf)
        }

        fn dense(&self) -> bool {
            match self {
                Node::Leaf(_) => true,
                Node::Branch(children) => children.values().all(Node::dense),
                Node::Elements(items) => {
                    items
                        .keys()
                        .enumerate()
                        .all(|(position, index)| position == *index)
                        && items.values().all(Node::dense)
                }
            }
        }

        fn into_value(self) -> Value {
            match self {
                Node::Leaf(value) => value,
                Node::Branch(children) => Value::Object(
                    children
                        .into_iter()
                        .map(|(key, child)| (key, child.into_value()))
                        .collect(),
                ),
                Node::Elements(items) => {
                    Value::Array(items.into_values().map(Node::into_value).collect())
                }
            }
        }
    }

    let mut root = Node::Branch(BTreeMap::new());
    for param in &operation.params.body {
        // A dotted or bracketed name with no `wire` cannot be read either way — the emitter's
        // `NestedBodyField` refusal, restated for the document.
        if param.wire.is_none() && param.name.contains(['.', '[', ']']) {
            bail!(
                "operation `{}` body field `{}` is dotted or bracketed with no `wire` path; \
                 whether it is a path or a literal key cannot be guessed",
                operation.id,
                param.name
            );
        }
        let wire = wire_name(param);
        let steps = wire_steps(operation, param, wire)?;
        let leaf = match constant(param) {
            Some(value) => {
                refuse_braced_constant(operation, param, value)?;
                value.clone()
            }
            None => param_splice(&param.name),
        };
        if root.insert(&steps, leaf).is_err() {
            bail!(
                "operation `{}` body field `{}` contests the wire path {wire:?} another field \
                 already occupies",
                operation.id,
                param.name
            );
        }
    }
    if !root.dense() {
        bail!(
            "operation `{}` declares a body array with a hole in its indices",
            operation.id
        );
    }
    Ok(root.into_value())
}

// ---------------------------------------------------------------------------------------------
// The closed vocabulary, enforced
// ---------------------------------------------------------------------------------------------

/// Whether `literal` carries a `{…}` the template's evaluation would read as a placeholder —
/// the same brace grammar `connector-pack`'s `scan_template` answers, `{{` escapes included.
fn carries_placeholder(literal: &str) -> bool {
    let mut rest = literal;
    while let Some(open) = rest.find('{') {
        let at_brace = &rest[open..];
        let (open_token, close_token) = if at_brace.starts_with("{{") {
            ("{{", "}}")
        } else {
            ("{", "}")
        };
        let inner = &at_brace[open_token.len()..];
        let Some(close) = inner.find(close_token) else {
            return false;
        };
        if open_token == "{" {
            return true;
        }
        rest = &inner[close + close_token.len()..];
    }
    false
}

/// **Refuse a literal the closed template vocabulary cannot say** — a brace-carrying string that
/// evaluation would fill with configuration, inside a value that is the vendor's own syntax. This
/// is `connector-pack`'s C-193/C-232 refusal moved to the build: the C-110 shape becomes a build
/// error here, never a committed document that installs nothing.
fn refuse_braced_literal(operation: &Operation, context: &str, literal: &str) -> Result<()> {
    if carries_placeholder(literal) {
        bail!(
            "operation `{}`'s {context} binds the string literal {literal:?}, whose `{{…}}` has \
             no spelling in the closed request-template vocabulary (C-536) — the same construct \
             `connector-pack` refuses as unclassifiable configuration (C-193/C-232), refused at \
             build time rather than committed as a degraded document",
            operation.id
        );
    }
    Ok(())
}

/// A pinned `const` value is a literal in the template, so every string inside it is held to the
/// closed vocabulary — and an object spelling `$param` would be misread as a splice, so it is
/// refused outright.
fn refuse_braced_constant(operation: &Operation, param: &Param, value: &Value) -> Result<()> {
    fn walk(operation: &Operation, param: &Param, value: &Value) -> Result<()> {
        match value {
            Value::String(text) => refuse_braced_literal(
                operation,
                &format!("constant body field `{}`", param.name),
                text,
            ),
            Value::Array(items) => items
                .iter()
                .try_for_each(|item| walk(operation, param, item)),
            Value::Object(fields) => {
                if fields.contains_key("$param") {
                    bail!(
                        "operation `{}`'s constant body field `{}` spells `$param`, which the \
                         template reserves for caller splices",
                        operation.id,
                        param.name
                    );
                }
                fields
                    .values()
                    .try_for_each(|item| walk(operation, param, item))
            }
            _ => Ok(()),
        }
    }
    walk(operation, param, value)
}

// ---------------------------------------------------------------------------------------------
// Endpoint slots
// ---------------------------------------------------------------------------------------------

/// The sentinel `connector-pack` reads templates positionally through; a value never passes
/// through a marked string, so nothing a tenant configures can forge one.
const MARK: char = '\u{1}';

fn mark_placeholders(template: &str) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let at_brace = &rest[open..];
        let (open_token, close_token) = if at_brace.starts_with("{{") {
            ("{{", "}}")
        } else {
            ("{", "}")
        };
        let inner = &at_brace[open_token.len()..];
        let Some(close) = inner.find(close_token) else {
            out.push_str(at_brace);
            return out;
        };
        if open_token == "{{" {
            out.push_str("{{");
            rest = inner;
            continue;
        }
        out.push(MARK);
        out.push_str(inner[..close].trim());
        out.push(MARK);
        rest = &inner[close + close_token.len()..];
    }
    out.push_str(rest);
    out
}

/// Each `\u{1}name\u{1}` in `marked`, as `(byte offset, name)`.
fn marked_placeholders(marked: &str) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let mut rest = marked;
    let mut consumed = 0;
    while let Some(open) = rest.find(MARK) {
        let after = &rest[open + MARK.len_utf8()..];
        let Some(close) = after.find(MARK) else {
            break;
        };
        found.push((consumed + open, &after[..close]));
        consumed += open + close + 2 * MARK.len_utf8();
        rest = &after[close + MARK.len_utf8()..];
    }
    found
}

/// `{origin}` followed only by a connector-declared path — the operator-approved origin base
/// (C-508), the one URL template that intentionally contains no `://`.
fn origin_template(literal: &str) -> Option<&str> {
    let rest = literal.strip_prefix('{')?;
    let close = rest.find('}')?;
    let variable = &rest[..close];
    (!variable.is_empty() && rest[close + 1..].starts_with('/')).then_some(variable)
}

/// **Where each endpoint-configuration variable lands** — read off the IR the way
/// `connector-pack`'s `endpoint_slots` reads it off the emitted body, with the same closure: a
/// base URL whose braces are classifiable as neither a templated URL nor an origin base is a
/// build error.
fn endpoint_slots(
    connector: &Connector,
    operation: &Operation,
    pins: &[DocPin<'_>],
) -> Result<BTreeMap<String, BTreeSet<&'static str>>> {
    let mut slots: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();

    let base = connector
        .base_url_of(&operation.service)
        .trim_end_matches('/');
    if carries_placeholder(base) {
        let marked = mark_placeholders(base);
        if let Some((_, after_scheme)) = marked.split_once("://") {
            let authority_end = after_scheme
                .find(['/', '?', '#'])
                .unwrap_or(after_scheme.len());
            let query_start = after_scheme.find(['?', '#']).unwrap_or(after_scheme.len());
            for (offset, name) in marked_placeholders(after_scheme) {
                let slot = if offset < authority_end {
                    "host"
                } else if offset < query_start {
                    "path"
                } else {
                    "query"
                };
                slots.entry(name.to_string()).or_default().insert(slot);
            }
        } else if let Some(variable) = origin_template(base) {
            slots
                .entry(variable.to_string())
                .or_default()
                .insert("origin");
            if let Some((_, other)) = marked_placeholders(&marked)
                .into_iter()
                .find(|(_, name)| *name != variable)
            {
                bail!(
                    "operation `{}`'s base URL {base:?} places `{{{other}}}` outside the origin \
                     slot, which the request template cannot say (C-536)",
                    operation.id
                );
            }
        } else {
            bail!(
                "operation `{}`'s base URL {base:?} carries a `{{…}}` that is neither a templated \
                 URL nor an operator-approved origin base — the template vocabulary is closed \
                 (C-536)",
                operation.id
            );
        }
    }

    for pin in pins {
        let slot = match pin.position {
            connector_spec::Position::Path => "path",
            connector_spec::Position::Query => "query",
            connector_spec::Position::Header => "header",
        };
        slots.entry(pin.variable.clone()).or_default().insert(slot);
    }

    Ok(slots)
}

// ---------------------------------------------------------------------------------------------
// The schema
// ---------------------------------------------------------------------------------------------

fn validator() -> &'static jsonschema::Validator {
    static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
    VALIDATOR
        .get_or_init(|| jsonschema::validator_for(schema()).expect("the document schema compiles"))
}

/// The versioned JSON Schema every document validates against — committed beside the documents
/// and enforced by [`render`], the way `core_catalog.rs` validates `web/public/v1/**`.
pub fn schema() -> &'static Value {
    static SCHEMA: OnceLock<Value> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": SCHEMA_ID,
            "title": "flux-connectors canonical catalog document",
            "description": "One canonical, deterministic catalog document per provider (Decision 0022, C-536): the complete published connector surface, including an explicit request template per operation. The template vocabulary is closed and total — literal values, `{var}` interpolation over caller parameters and endpoint slots, and `$param` splices — and the oauth2 object deliberately has no field for a registration value: the client identifier and secret are per-deployment, published only as operator-level configuration requirements through the `binds` grammar.",
            "type": "object",
            "properties": {
                "$schema": { "const": SCHEMA_ID },
                "schema_version": { "const": SCHEMA_VERSION },
                "generator": { "type": "string", "minLength": 1 },
                "connector": { "type": "string", "minLength": 1 },
                "vendor": { "type": "string" },
                "description": { "type": "string" },
                "runtime": { "enum": ["http", "socket", "process", "container", "plugin", "remote"] },
                "authority": { "type": "string", "minLength": 1 },
                "verify": { "type": "string", "minLength": 1 },
                "services": { "type": "array", "minItems": 1, "items": { "$ref": "#/$defs/service" } },
                "auth": { "type": "array", "items": { "$ref": "#/$defs/auth" } },
                "default_auth": { "$ref": "#/$defs/requirements" },
                "config": { "type": "array", "items": { "$ref": "#/$defs/config_field" } },
                "operations": { "type": "array", "items": { "$ref": "#/$defs/operation" } },
                "events": { "type": "array", "items": { "$ref": "#/$defs/event" } },
                "channels": { "type": "array", "items": { "$ref": "#/$defs/channel" } }
            },
            "required": ["$schema", "schema_version", "generator", "connector", "runtime", "services", "operations"],
            "additionalProperties": false,
            "$defs": {
                "json_schema": {
                    "description": "A JSON Schema value, carried verbatim from the vendor's declaration.",
                    "type": ["object", "boolean"]
                },
                "requirements": {
                    "description": "Auth alternatives: OR of AND-groups of declared credential names.",
                    "type": "array",
                    "items": { "type": "array", "items": { "type": "string", "minLength": 1 } }
                },
                "service": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1 },
                        "gid": { "type": "string" },
                        "description": { "type": "string" },
                        "base_url": { "type": "string", "minLength": 1 },
                        "api_version": { "type": "string" },
                        "legacy": { "type": "boolean" },
                        "roles": { "type": "array", "items": { "enum": ["llm_catalogue"] } },
                        "tags": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["name", "base_url"],
                    "additionalProperties": false
                },
                "auth": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1 },
                        "scheme": { "$ref": "#/$defs/scheme" },
                        "env": { "type": "array", "items": { "type": "string" } },
                        "user_env": { "type": "array", "items": { "type": "string" } },
                        "user_suffix": { "type": "string" },
                        "description": { "type": "string" },
                        "subject": { "enum": ["unstated", "app", "user"] },
                        "hazard": { "type": "string" },
                        "oauth2": { "$ref": "#/$defs/oauth2" },
                        "token_endpoint_quirks": { "type": "array", "items": { "$ref": "#/$defs/token_endpoint_quirk" } }
                    },
                    "required": ["name", "scheme", "subject"],
                    "additionalProperties": false
                },
                "scheme": {
                    "type": "object",
                    "properties": {
                        "kind": { "enum": ["bearer", "basic", "header", "query", "signing"] },
                        "name": { "type": "string" },
                        "prefix": { "type": "string" }
                    },
                    "required": ["kind"],
                    "additionalProperties": false
                },
                "oauth2": {
                    "description": "The complete OAuth2 declaration. Deliberately, no field exists for a registration value: the client identifier and secret are per-deployment, published only as operator-level configuration requirements through the `binds` grammar (C-536).",
                    "type": "object",
                    "properties": {
                        "endpoint": { "type": "string" },
                        "token_endpoint": { "type": "string" },
                        "authorize_path": { "type": "string" },
                        "token_path": { "type": "string" },
                        "scopes": { "type": "array", "items": { "type": "string" } },
                        "grants": { "type": "array", "items": { "enum": ["authorization_code", "password", "refresh_token", "client_credentials"] } },
                        "public_client": { "type": "boolean" },
                        "redirect": {
                            "type": "object",
                            "properties": {
                                "port": { "type": "integer", "minimum": 1, "maximum": 65535 },
                                "path": { "type": "string" }
                            },
                            "required": ["port", "path"],
                            "additionalProperties": false
                        }
                    },
                    "additionalProperties": false
                },
                "token_endpoint_quirk": {
                    "type": "object",
                    "properties": {
                        "grant": { "type": "string" },
                        "behaviour": { "type": "string" },
                        "attribution": { "type": "string" },
                        "measured": { "type": "string" }
                    },
                    "required": ["grant", "behaviour", "attribution", "measured"],
                    "additionalProperties": false
                },
                "config_field": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1 },
                        "service": { "type": "string", "minLength": 1 },
                        "label": { "type": "string", "minLength": 1 },
                        "help": { "type": "string" },
                        "example": { "type": "string" },
                        "format": { "enum": ["text", "subdomain", "hostname", "url", "origin", "email", "token"] },
                        "choices": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "value": { "type": "string" },
                                    "label": { "type": "string" }
                                },
                                "required": ["value", "label"],
                                "additionalProperties": false
                            }
                        },
                        "required": { "type": "boolean" },
                        "default": { "type": "string" },
                        "approval": { "enum": ["none", "operator"] },
                        "secret": { "type": "boolean" },
                        "docs_url": { "type": "string" },
                        "binds": { "type": "string", "minLength": 1 },
                        "also_binds": { "type": "array", "items": { "type": "string" } },
                        "also_services": { "type": "array", "items": { "type": "string" } },
                        "level": { "enum": ["operator", "connection"] }
                    },
                    "required": ["name", "service", "label", "format", "required", "approval", "secret", "binds", "level"],
                    "additionalProperties": false
                },
                "operation": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "minLength": 1 },
                        "service": { "type": "string", "minLength": 1 },
                        "direction": { "enum": ["read", "write"] },
                        "description": { "type": "string" },
                        "risk": { "enum": ["low", "medium", "high", "destructive"] },
                        "idempotency": { "enum": ["idempotent", "non_idempotent", "conditional"] },
                        "repeatability_condition": { "type": "string" },
                        "semantic_effects": { "type": "array", "items": { "type": "string" } },
                        "expose": { "type": "boolean" },
                        "auth": { "$ref": "#/$defs/requirements" },
                        "request": { "$ref": "#/$defs/request" },
                        "params": { "type": "array", "items": { "$ref": "#/$defs/param" } },
                        "response_schema": { "$ref": "#/$defs/json_schema" },
                        "endpoint": {
                            "type": "object",
                            "additionalProperties": {
                                "type": "array",
                                "minItems": 1,
                                "items": { "enum": ["origin", "host", "path", "query", "header"] }
                            }
                        },
                        "quirks": { "$ref": "#/$defs/quirks" }
                    },
                    "required": ["id", "service", "direction", "risk", "idempotency", "semantic_effects", "expose", "auth", "request"],
                    "additionalProperties": false
                },
                "request": {
                    "type": "object",
                    "properties": {
                        "method": { "enum": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] },
                        "url": { "type": "string", "pattern": "^\\{base\\}/" },
                        "headers": { "type": "object", "additionalProperties": { "$ref": "#/$defs/value_template" } },
                        "query": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string", "minLength": 1 },
                                    "value": { "$ref": "#/$defs/value_template" }
                                },
                                "required": ["name", "value"],
                                "additionalProperties": false
                            }
                        },
                        "body": { "$ref": "#/$defs/body" }
                    },
                    "required": ["method", "url"],
                    "additionalProperties": false
                },
                "param_splice": {
                    "description": "The whole value of the named caller parameter.",
                    "type": "object",
                    "properties": { "$param": { "type": "string", "minLength": 1 } },
                    "required": ["$param"],
                    "additionalProperties": false
                },
                "value_template": {
                    "description": "A literal (which may interpolate `{endpoint-slot}` placeholders) or a `$param` splice — the closed vocabulary, in value position.",
                    "oneOf": [
                        { "type": "string" },
                        { "$ref": "#/$defs/param_splice" }
                    ]
                },
                "body_template": {
                    "description": "A JSON body: literals, nested objects and arrays, `$param` splices at caller leaves. Nothing else has a spelling.",
                    "oneOf": [
                        { "$ref": "#/$defs/param_splice" },
                        {
                            "type": "object",
                            "not": { "required": ["$param"] },
                            "additionalProperties": { "$ref": "#/$defs/body_template" }
                        },
                        { "type": "array", "items": { "$ref": "#/$defs/body_template" } },
                        { "type": ["string", "number", "boolean", "null"] }
                    ]
                },
                "body": {
                    "oneOf": [
                        {
                            "type": "object",
                            "properties": {
                                "encoding": { "const": "json" },
                                "template": { "$ref": "#/$defs/body_template" }
                            },
                            "required": ["encoding", "template"],
                            "additionalProperties": false
                        },
                        {
                            "type": "object",
                            "properties": {
                                "encoding": { "const": "form" },
                                "fields": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "name": { "type": "string", "minLength": 1 },
                                            "value": { "$ref": "#/$defs/value_template" },
                                            "required": { "type": "boolean" }
                                        },
                                        "required": ["name", "value", "required"],
                                        "additionalProperties": false
                                    }
                                }
                            },
                            "required": ["encoding", "fields"],
                            "additionalProperties": false
                        }
                    ]
                },
                "param": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1 },
                        "position": { "enum": ["path", "query", "header", "body"] },
                        "wire": { "type": "string" },
                        "description": { "type": "string" },
                        "required": { "type": "boolean" },
                        "schema": { "$ref": "#/$defs/json_schema" }
                    },
                    "required": ["name", "position", "required", "schema"],
                    "additionalProperties": false
                },
                "quirks": {
                    "type": "object",
                    "properties": {
                        "pagination": {
                            "oneOf": [
                                {
                                    "type": "object",
                                    "properties": {
                                        "page": {
                                            "type": "object",
                                            "properties": {
                                                "page_param": { "type": "string" },
                                                "size_param": { "type": "string" },
                                                "page_size": { "type": "integer" },
                                                "max_pages": { "type": "integer" }
                                            },
                                            "required": ["page_param", "max_pages"],
                                            "additionalProperties": false
                                        }
                                    },
                                    "required": ["page"],
                                    "additionalProperties": false
                                },
                                {
                                    "type": "object",
                                    "properties": {
                                        "cursor": {
                                            "type": "object",
                                            "properties": {
                                                "cursor_param": { "type": "string" },
                                                "next_cursor_pointer": { "type": "string" },
                                                "max_pages": { "type": "integer" }
                                            },
                                            "required": ["cursor_param", "next_cursor_pointer", "max_pages"],
                                            "additionalProperties": false
                                        }
                                    },
                                    "required": ["cursor"],
                                    "additionalProperties": false
                                }
                            ]
                        },
                        "rate_limit": {
                            "type": "object",
                            "properties": {
                                "requests": { "type": "integer" },
                                "per_seconds": { "type": "integer" },
                                "bucket": { "type": "string" }
                            },
                            "required": ["requests", "per_seconds"],
                            "additionalProperties": false
                        },
                        "error_envelope": {
                            "type": "object",
                            "properties": {
                                "message_pointer": { "type": "string" },
                                "code_pointer": { "type": "string" }
                            },
                            "required": ["message_pointer"],
                            "additionalProperties": false
                        }
                    },
                    "additionalProperties": false
                },
                "event": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1 },
                        "service": { "type": "string", "minLength": 1 },
                        "oip": { "type": "string" },
                        "wire_value": { "type": "string" },
                        "description": { "type": "string" },
                        "default": { "type": "boolean" },
                        "group": { "type": "string" },
                        "when": { "type": "object", "additionalProperties": { "$ref": "#/$defs/json_schema" } },
                        "schema": { "$ref": "#/$defs/json_schema" }
                    },
                    "required": ["name", "service", "default"],
                    "additionalProperties": false
                },
                "channel": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1 },
                        "service": { "type": "string", "minLength": 1 },
                        "oip": { "type": "string" },
                        "description": { "type": "string" },
                        "transport": { "enum": ["webhook", "socket", "poll"] },
                        "connect": { "type": "object" },
                        "events": { "type": "array", "items": { "type": "string" } },
                        "cursor": { "type": "string" },
                        "interval": { "type": "string" },
                        "verification": {
                            "type": "object",
                            "properties": {
                                "kind": { "type": "string" },
                                "verified": { "type": "boolean" },
                                "hmac": { "type": "object" }
                            },
                            "required": ["kind", "verified"],
                            "additionalProperties": false
                        },
                        "discriminator": { "$ref": "#/$defs/selector" },
                        "delivery_id": { "$ref": "#/$defs/selector" },
                        "payload": { "type": "object", "additionalProperties": { "type": "string" } },
                        "payload_root": { "type": "boolean" },
                        "reply": {
                            "type": "object",
                            "properties": {
                                "operation": { "type": "string" },
                                "oip": { "type": "string" },
                                "result": { "type": "string" },
                                "bind": { "type": "object", "additionalProperties": { "type": "string" } }
                            },
                            "required": ["operation"],
                            "additionalProperties": false
                        },
                        "subscription": { "type": "object" },
                        "setup": { "type": "object" }
                    },
                    "required": ["name", "service", "transport", "verification"],
                    "additionalProperties": false
                },
                "selector": {
                    "type": "object",
                    "properties": {
                        "source": { "enum": ["header", "body"] },
                        "name": { "type": "string" }
                    },
                    "required": ["source", "name"],
                    "additionalProperties": false
                }
            }
        })
    })
}

// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete hand-authored connector the loader accepts, parameterised on one operation body.
    fn connector_with(operations: &str) -> Connector {
        let definition = format!(
            r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"
default_auth = [{{ credentials = ["acme.token"] }}]

[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["ACME_TOKEN"]

{operations}
"#
        );
        connector_spec::provider::load_with_spec("providers/acme.toml", &definition, &[])
            .expect("the fixture loads")
            .connector
    }

    /// **The closed-template refusal** (failing-first for C-536's third acceptance item): a
    /// construct `connector-pack`'s evaluator refuses today — a vendor's own brace-carrying
    /// syntax pinned as a body literal, the C-110 shape — is a **build error** in document
    /// lowering, never a silently degraded document.
    #[test]
    fn an_unclassifiable_brace_literal_is_a_build_error_never_a_degraded_document() {
        let connector = connector_with(
            r#"
[[operations]]
id = "acme-graph-query"
method = "POST"
direction = "read"
path = "/graphql"
description = "The C-110 shape: a pinned query document whose braces are vendor syntax"
risk = "low"
idempotency = "idempotent"

[[operations.params.body]]
name = "query"
required = true
schema = { type = "string", const = "query { viewer { login } }" }
"#,
        );

        let error = render(&connector).expect_err("a C-110-shaped literal must not lower");
        let message = format!("{error:#}");
        assert!(
            message.contains("acme-graph-query") && message.contains("closed"),
            "the refusal names the operation and the closed vocabulary: {message}"
        );
    }

    /// The same closure, for a constant header whose value carries a brace.
    #[test]
    fn a_braced_constant_header_is_refused() {
        let connector = connector_with(
            r#"
[[operations]]
id = "acme-thing-get"
method = "GET"
direction = "read"
path = "/things"
description = "Get things"
risk = "low"
idempotency = "idempotent"

[operations.params.const_headers]
"X-Weird" = "{templated}"
"#,
        );

        let error = render(&connector).expect_err("a braced constant header must not lower");
        assert!(
            format!("{error:#}").contains("X-Weird"),
            "the refusal names the header: {error:#}"
        );
    }

    /// A provider file declaring a non-empty OAuth2 registration value is a build error, not
    /// emitted data — the document has no field it could survive into.
    #[test]
    fn a_declared_registration_value_is_a_build_error() {
        let connector = connector_with(
            r#"
[[operations]]
id = "acme-thing-get"
method = "GET"
direction = "read"
path = "/things"
description = "Get things"
risk = "low"
idempotency = "idempotent"

[[auth]]
name = "acme.oauth_token"
scheme = "bearer"
description = "OAuth2 access token"

[auth.oauth2]
token_path = "/oauth/token"
grants = ["authorization_code"]
client_id = "registered-elsewhere"
"#,
        );

        let error = render(&connector).expect_err("a registration value must not lower");
        let message = format!("{error:#}");
        assert!(
            message.contains("oauth.client_id"),
            "the refusal points at the `binds` grammar: {message}"
        );
    }

    /// A graph has no document lowering yet (deferred by the design's open questions); dropping
    /// a declared surface silently is the failure this artifact exists to end, so it refuses.
    #[test]
    fn a_declared_graph_is_refused_rather_than_dropped() {
        let mut connector = connector_with(
            r#"
[[operations]]
id = "acme-thing-get"
method = "GET"
direction = "read"
path = "/things"
description = "Get things"
risk = "low"
idempotency = "idempotent"
"#,
        );
        connector.graphs.push(connector_spec::Graph {
            name: "flow".to_string(),
            service: connector_spec::DEFAULT_SERVICE.to_string(),
            description: String::new(),
            inputs: Vec::new(),
            output: None,
            expose: true,
            nodes: Vec::new(),
            edges: Vec::new(),
        });

        let error = render(&connector).expect_err("a graph must not vanish silently");
        assert!(
            format!("{error:#}").contains("graph"),
            "the refusal names the surface: {error:#}"
        );
    }

    /// A `form` body is representable: encoding, media type, pair order (always-sent pairs
    /// first, guarded ones after — `form_payload`'s partition), and a pinned `const` as a
    /// literal. Proven against a fixture because no shipped provider declares the encoding yet
    /// (C-144 landed the axis; `grep -rn 'body_encoding' providers/ | grep -v '#'` matches
    /// nothing) — the axis must not become sayable-in-Flux but unsayable-in-the-document.
    #[test]
    fn a_form_body_is_spelled_structurally() {
        let connector = connector_with(
            r#"
[[operations]]
id = "acme-message-send"
method = "POST"
direction = "write"
path = "/messages"
description = "Send a message"
risk = "medium"
idempotency = "non_idempotent"

[operations.params]
body_encoding = "form"

[[operations.params.body]]
name = "to"
required = true
schema = { type = "string" }

[[operations.params.body]]
name = "channel"
schema = { type = "string", const = "sms" }

[[operations.params.body]]
name = "note"
schema = { type = "string" }
"#,
        );

        let rendered = render(&connector).expect("the form fixture renders");
        let document: Value = serde_json::from_str(&rendered).expect("JSON");
        let request = &document["operations"][0]["request"];
        assert_eq!(
            request["headers"]["content-type"], "application/x-www-form-urlencoded",
            "the media type follows the declared encoding"
        );
        assert_eq!(request["body"]["encoding"], "form");
        assert_eq!(
            request["body"]["fields"],
            json!([
                { "name": "to", "value": { "$param": "to" }, "required": true },
                { "name": "channel", "value": "sms", "required": true },
                { "name": "note", "value": { "$param": "note" }, "required": false },
            ]),
            "always-sent pairs precede guarded ones, each group in declaration order"
        );
    }

    /// The rendered document is a fixed point: rendering twice yields identical bytes, and the
    /// text validates against the published schema (render already enforces it; this pins it).
    #[test]
    fn rendering_is_deterministic_and_validates() {
        let connector = connector_with(
            r#"
[[operations]]
id = "acme-thing-get"
method = "GET"
direction = "read"
path = "/things/{thing_id}"
description = "Get one thing"
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "thing_id"
required = true
schema = { type = "string" }

[[operations.params.query]]
name = "expand"
schema = { type = "string" }
"#,
        );

        let first = render(&connector).expect("the fixture renders");
        let second = render(&connector).expect("the fixture renders again");
        assert_eq!(first, second, "equal inputs must produce identical bytes");

        let document: Value = serde_json::from_str(&first).expect("the document is JSON");
        assert_eq!(document["schema_version"], SCHEMA_VERSION);
        let operation = &document["operations"][0];
        assert_eq!(operation["request"]["url"], "{base}/things/{thing_id}");
        assert_eq!(
            operation["request"]["query"][0]["value"],
            json!({ "$param": "expand" })
        );
    }
}
