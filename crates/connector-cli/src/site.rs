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

use anyhow::Result;
use serde::Serialize;

use connector_spec::{
    AuthScheme, Connector, HttpMethod, Idempotency, JsonSchema, Operation, Param, Risk,
};

use crate::catalog::{self, OperationRendering};
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

/// An [`AuthScheme`], flattened to a fixed two-key shape.
///
/// The IR's own encoding is externally tagged — `"bearer"` for one variant and
/// `{"header": {"name": "…"}}` for another — which is a JSON shape that changes with the value. A
/// consumer would need a discriminated union to read it. Here the kind is always a string and the
/// name is always present, `null` when the variant carries none.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct SchemeEntry {
    /// `bearer`, `basic`, `header` or `query`.
    kind: &'static str,
    /// The header or query-parameter name, for the two variants that carry one.
    name: Option<String>,
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
    /// The HTTP method, uppercase.
    method: HttpMethod,
    /// The path template, relative to [`ProviderEntry::base_url`].
    path: String,
    /// Every named parameter, in request-position order, each carrying its JSON Schema verbatim.
    parameters: Vec<ParameterEntry>,
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
        let operation = connector.operation(&rendering.id).ok_or_else(|| {
            anyhow::anyhow!(
                "connector `{}` has no operation `{}` to describe",
                connector.id,
                rendering.id
            )
        })?;
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
        base_url: connector.base_url.clone(),
        api_version: connector.api_version.clone(),
        hosts,
        services,
        auth: provider_auth(connector),
        operation_count: operations.len(),
        operations,
    })
}

/// Serialize the whole catalogue.
///
/// Pretty-printed, and with a trailing newline: this is a **committed, reviewed** artifact like
/// every other generated file here, and a one-line JSON blob is not something anyone can read in a
/// diff. Two-space indentation is `serde_json`'s own default, so the fixed point costs nothing to
/// maintain.
pub fn document(providers: Vec<ProviderEntry>) -> Result<String> {
    let document = Document {
        schema_version: SCHEMA_VERSION,
        generator: crate::seam::generator(),
        providers,
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
        method: operation.method,
        path: operation.path.clone(),
        parameters: parameters(operation),
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
/// flux's plugin-protocol vocabulary — this repository does not get to rename it.
fn scheme_kind(scheme: &AuthScheme) -> &'static str {
    match scheme {
        AuthScheme::Bearer => "bearer",
        AuthScheme::Basic => "basic",
        AuthScheme::Header { .. } => "header",
        AuthScheme::Query { .. } => "query",
    }
}

/// The header or query-parameter name the scheme carries, if it carries one.
fn scheme_name(scheme: &AuthScheme) -> Option<String> {
    match scheme {
        AuthScheme::Bearer | AuthScheme::Basic => None,
        AuthScheme::Header { name } | AuthScheme::Query { name } => Some(name.clone()),
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
            quirks: Quirks::default(),
        }
    }

    fn connector() -> Connector {
        Connector {
            id: "acme".to_string(),
            authority: None,
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

    /// An absent value is `null`, never a missing key: a consumer types the document once and never
    /// tests for existence.
    #[test]
    fn optional_fields_are_null_rather_than_absent() {
        let document = rendered();
        let operation = &document["providers"][0]["operations"][0];
        assert_eq!(operation["body_schema"], Value::Null);
        assert_eq!(operation["response_schema"], Value::Null);

        let credential = &document["providers"][0]["auth"]["credentials"][0];
        assert_eq!(credential["user_suffix"], Value::Null);
        assert_eq!(
            credential["scheme"],
            json!({"kind": "header", "name": "X-Id"})
        );
    }

    /// The scheme is a fixed two-key object rather than the IR's externally tagged encoding, so a
    /// consumer reads `scheme.kind` without a discriminated union.
    #[test]
    fn a_scheme_without_a_name_still_carries_the_key() {
        let mut connector = connector();
        connector.auth = vec![AuthMethod::bearer("acme.token", vec!["ACME".to_string()])];
        connector.default_auth = vec![AuthRequirement::single("acme.token")];
        let entry = provider_entry(&connector, &renderings()).unwrap();
        let document: Value = serde_json::from_str(&document(vec![entry]).unwrap()).unwrap();
        assert_eq!(
            document["providers"][0]["auth"]["credentials"][0]["scheme"],
            json!({"kind": "bearer", "name": null})
        );
        assert_eq!(
            document["providers"][0]["auth"]["schemes"],
            json!(["bearer"])
        );
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
            },
            connector_spec::Service {
                name: "calendar".to_string(),
                description: "Calendar.".to_string(),
                base_url: None,
                api_version: Some("v3".to_string()),
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
