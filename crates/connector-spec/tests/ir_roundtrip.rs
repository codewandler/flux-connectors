//! Serde round-trip tests for the connector IR.
//!
//! These exercise the crate through its public API only — the IR is the contract both front-ends
//! produce and `connector-flux` consumes, so anything these tests cannot reach is not part of it.
//!
//! The fixture is deliberately babelforce-shaped: three declared auth methods where two apiKey
//! headers must travel **together** and are an **alternative** to OAuth2. That is the real spec
//! (`~/babelforce/projects/babelforce-api/.../manager.openapi.json`, document-level
//! `security: [{oauth2: [*]}, {accessId: [], accessToken: []}]`), not a hypothetical.

use std::collections::BTreeMap;

use connector_spec::{
    AuthMethod, AuthRequirement, AuthScheme, BodyEncoding, Connector, HttpMethod, Idempotency,
    OAuth2Spec, OAuthGrant, Operation, OperationDirection, Param, ParamSet, Provenance, Quirks,
    Risk, DEFAULT_SERVICE,
};
use serde_json::json;

/// `oauth2` OR (`accessId` AND `accessToken`) — babelforce's document-level `security`.
fn babelforce_default_auth() -> Vec<AuthRequirement> {
    vec![
        AuthRequirement::all(["babelforce.oauth2"]),
        AuthRequirement::all(["babelforce.access_id", "babelforce.access_token"]),
    ]
}

fn babelforce_auth_methods() -> Vec<AuthMethod> {
    vec![
        AuthMethod {
            name: "babelforce.oauth2".into(),
            scheme: AuthScheme::Bearer,
            env: vec!["BABELFORCE_ACCESS_TOKEN".into()],
            oauth2: Some(OAuth2Spec {
                endpoint: "babelforce.endpoint".into(),
                token_path: "/oauth/token".into(),
                grants: vec![OAuthGrant::Password, OAuthGrant::RefreshToken],
                ..OAuth2Spec::default()
            }),
            ..AuthMethod::default()
        },
        AuthMethod {
            name: "babelforce.access_id".into(),
            scheme: AuthScheme::Header {
                name: "X-Auth-Access-Id".into(),
                prefix: String::new(),
            },
            env: vec!["BABELFORCE_ACCESS_ID".into()],
            ..AuthMethod::default()
        },
        AuthMethod {
            name: "babelforce.access_token".into(),
            scheme: AuthScheme::Header {
                name: "X-Auth-Access-Token".into(),
                prefix: String::new(),
            },
            env: vec!["BABELFORCE_ACCESS_TOKEN".into()],
            ..AuthMethod::default()
        },
    ]
}

fn op(id: &str, auth: Option<Vec<AuthRequirement>>) -> Operation {
    Operation {
        id: id.into(),
        service: DEFAULT_SERVICE.into(),
        method: HttpMethod::Get,
        direction: OperationDirection::Read,
        path: "/v2/calls".into(),
        description: "List calls".into(),
        risk: Risk::Low,
        idempotency: Idempotency::Idempotent,
        semantic_effects: Vec::new(),
        repeatable_because: None,
        expose: true,
        auth,
        params: ParamSet::default(),
        response_schema: None,
        credential_response: Vec::new(),
        produces_credential: None,
        quirks: Quirks::default(),
    }
}

fn babelforce() -> Connector {
    Connector {
        id: "babelforce".into(),
        authority: None,
        runtime: connector_spec::Runtime::Http,
        api_version: None,
        services: Vec::new(),
        vendor: "Babelforce".into(),
        base_url: "https://{tenant}.babelforce.com".into(),
        description: "Babelforce manager API".into(),
        auth: babelforce_auth_methods(),
        default_auth: babelforce_default_auth(),
        operations: vec![
            // ZERO — an explicitly unauthenticated endpoint (OpenAPI `security: []`).
            op("babelforce.health", Some(Vec::new())),
            // UNSET — inherits the connector default.
            op("babelforce.call.list", None),
            // ONE-OF-SEVERAL — either OAuth2 or the api-key pair.
            op("babelforce.call.show", Some(babelforce_default_auth())),
            // ALL-OF-SEVERAL — the two api-key headers, sent together.
            op(
                "babelforce.call.create",
                Some(vec![AuthRequirement::all([
                    "babelforce.access_id",
                    "babelforce.access_token",
                ])]),
            ),
        ],
        events: Vec::new(),
        channels: Vec::new(),
        config: Vec::new(),
        verify: None,
        graphs: Vec::new(),
        provenance: Provenance::default(),
    }
}

/// The headline of C-2: all three auth cardinalities survive a serde round-trip, and *unset* stays
/// distinguishable from an *explicit empty list*.
#[test]
fn auth_requirement_cardinalities_round_trip() {
    let connector = babelforce();

    let encoded = serde_json::to_string(&connector).expect("the IR must serialize");
    let decoded: Connector = serde_json::from_str(&encoded).expect("the IR must deserialize");
    assert_eq!(connector, decoded, "the IR must survive a serde round-trip");

    let by_id = |id: &str| -> Operation {
        decoded
            .operations
            .iter()
            .find(|o| o.id == id)
            .expect("operation present after round-trip")
            .clone()
    };

    // ZERO: an explicit empty alternatives list means "this operation needs no auth".
    let health = by_id("babelforce.health");
    assert_eq!(
        health.auth,
        Some(Vec::new()),
        "an explicit `security: []` must round-trip as Some(empty), never as None"
    );
    assert!(
        decoded.effective_auth(&health).is_empty(),
        "an explicitly unauthenticated operation must not inherit the connector default"
    );

    // UNSET: no declaration at all inherits the connector-level default.
    let list = by_id("babelforce.call.list");
    assert_eq!(
        list.auth, None,
        "an operation that declares nothing must round-trip as None, never as Some(empty)"
    );
    assert_eq!(
        decoded.effective_auth(&list),
        babelforce_default_auth().as_slice(),
        "an unset operation must inherit the connector-level default"
    );

    // ONE-OF-SEVERAL (OR): two alternatives, either of which authenticates the request.
    let show = by_id("babelforce.call.show");
    let alternatives = show.auth.as_ref().expect("declared");
    assert_eq!(alternatives.len(), 2, "two alternatives (OR)");
    assert!(alternatives[0].contains("babelforce.oauth2"));
    assert!(alternatives[1].contains("babelforce.access_id"));

    // ALL-OF-SEVERAL (AND): one mechanism naming two credentials that travel together.
    let create = by_id("babelforce.call.create");
    let alternatives = create.auth.as_ref().expect("declared");
    assert_eq!(alternatives.len(), 1, "one alternative");
    assert_eq!(
        alternatives[0].credentials().iter().collect::<Vec<_>>(),
        vec!["babelforce.access_id", "babelforce.access_token"],
        "both credentials must be preserved — a mechanism is a set, not a single credential"
    );
}

/// `unset` and `explicit empty` must differ on the wire, not merely in memory: the JSON encoding
/// omits the key entirely for unset and emits `[]` for "no auth". This is what makes the
/// distinction survive a trip through `connectors.lock` and a regenerated artifact.
#[test]
fn unset_and_explicit_empty_auth_differ_on_the_wire() {
    let unset = serde_json::to_value(op("x", None)).expect("serialize");
    let explicit = serde_json::to_value(op("x", Some(Vec::new()))).expect("serialize");

    assert!(
        unset.get("auth").is_none(),
        "unset auth must be omitted from the encoding, got {unset}"
    );
    assert_eq!(
        explicit.get("auth"),
        Some(&json!([])),
        "explicit no-auth must encode as an empty array"
    );
    assert_ne!(unset, explicit);
}

/// The auth scheme vocabulary is `flux_plugin_protocol::AuthScheme`'s, verbatim. This crate cannot
/// depend on that crate (flux-connectors depends on `flux-lang` from crates.io and nothing else of
/// flux's), so the wire form is pinned here instead: if either side drifts, this fails.
#[test]
fn auth_scheme_matches_the_flux_plugin_protocol_vocabulary() {
    let cases = [
        (AuthScheme::Bearer, json!("bearer")),
        (AuthScheme::Basic, json!("basic")),
        (
            AuthScheme::Header {
                name: "PRIVATE-TOKEN".into(),
                prefix: String::new(),
            },
            json!({"header": {"name": "PRIVATE-TOKEN"}}),
        ),
        (
            AuthScheme::Query {
                name: "api_key".into(),
            },
            json!({"query": {"name": "api_key"}}),
        ),
    ];

    for (scheme, wire) in cases {
        assert_eq!(
            serde_json::to_value(&scheme).expect("serialize"),
            wire,
            "scheme encoding must match flux's"
        );
        let back: AuthScheme = serde_json::from_value(wire).expect("deserialize");
        assert_eq!(back, scheme);
    }

    assert_eq!(
        AuthScheme::default(),
        AuthScheme::Bearer,
        "the default must stay Bearer, as it is in flux"
    );
}

/// A mechanism is a *set of credentials*: duplicates collapse and the serialized order is a
/// function of the set's value, not of the order the author happened to write it in.
#[test]
fn auth_requirement_is_a_set() {
    let written_one_way = AuthRequirement::all(["b.access_token", "b.access_id"]);
    let written_the_other = AuthRequirement::all(["b.access_id", "b.access_token", "b.access_id"]);

    assert_eq!(written_one_way, written_the_other);
    assert_eq!(written_one_way.len(), 2, "the duplicate must collapse");
    assert_eq!(
        serde_json::to_string(&written_one_way).expect("serialize"),
        serde_json::to_string(&written_the_other).expect("serialize"),
        "two equal requirement sets must encode identically"
    );
}

/// Every parameter and the response carry a real JSON Schema, and it survives the round-trip
/// intact — types must not collapse to `string` the way action-proxy's YAML did.
#[test]
fn parameter_and_response_schemas_survive_the_round_trip() {
    let mut connector = babelforce();
    let operation = &mut connector.operations[1];
    operation.params = ParamSet {
        path: vec![Param {
            name: "call_id".into(),
            wire: None,
            description: "The call id".into(),
            required: true,
            schema: json!({"type": "string", "format": "uuid"}),
        }],
        query: vec![Param {
            name: "limit".into(),
            wire: None,
            description: "Page size".into(),
            required: false,
            schema: json!({"type": "integer", "minimum": 1, "maximum": 200}),
        }],
        header: vec![Param {
            name: "X-Request-Id".into(),
            wire: None,
            description: String::new(),
            required: false,
            schema: json!({"type": "string"}),
        }],
        body: vec![Param {
            name: "notes".into(),
            wire: None,
            description: String::new(),
            required: false,
            schema: json!({"type": "array", "items": {"type": "string"}}),
        }],
        body_schema: None,
        const_headers: BTreeMap::from([("Accept".into(), "application/json".into())]),
        body_encoding: BodyEncoding::default(),
    };
    operation.response_schema = Some(json!({
        "type": "object",
        "properties": {"items": {"type": "array", "items": {"type": "object"}}},
        "required": ["items"],
    }));

    let encoded = serde_json::to_string(&connector).expect("serialize");
    let decoded: Connector = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(connector, decoded);

    let decoded_op = &decoded.operations[1];
    assert_eq!(
        decoded_op.params.query[0].schema["maximum"],
        json!(200),
        "a numeric bound must not degrade into a string"
    );
    assert_eq!(
        decoded_op.response_schema.as_ref().expect("present")["required"],
        json!(["items"]),
        "the response schema must travel intact"
    );
}

/// `risk`, `idempotency` and `description` map onto the metadata a Flux composite op declares, and
/// use flux's own vocabulary (`flux_spec::Risk` / `flux_spec::Idempotency`).
#[test]
fn operation_metadata_uses_the_flux_vocabulary() {
    assert_eq!(serde_json::to_value(Risk::Low).unwrap(), json!("low"));
    assert_eq!(
        serde_json::to_value(Risk::Destructive).unwrap(),
        json!("destructive")
    );
    assert_eq!(
        serde_json::to_value(Idempotency::NonIdempotent).unwrap(),
        json!("non_idempotent")
    );
    assert_eq!(
        serde_json::to_value(Idempotency::Conditional).unwrap(),
        json!("conditional")
    );
    assert_eq!(
        serde_json::to_value(HttpMethod::Delete).unwrap(),
        json!("DELETE")
    );

    let operation = op("babelforce.call.hangup", None);
    let decoded: Operation =
        serde_json::from_str(&serde_json::to_string(&operation).unwrap()).unwrap();
    assert_eq!(decoded.description, "List calls");
    assert_eq!(decoded.risk, Risk::Low);
    assert_eq!(decoded.idempotency, Idempotency::Idempotent);
}

/// `body_encoding` is a **closed** set whose default is invisible, and every one of its three
/// self-descriptions agrees — C-144.
///
/// The three are the serde tag (what an author writes), [`BodyEncoding::tag`] (what an error message
/// names) and [`BodyEncoding::media_type`] (what the vendor sees). A variant added with a mismatched
/// `tag` would produce a refusal naming a key nobody can find in their file, and one with the wrong
/// media type would send a body under a header that contradicts it.
///
/// The invisible default is the compatibility guarantee: `json` must not appear in any serialization,
/// or every committed manifest, lockfile hash and catalogue entry would move.
#[test]
fn body_encoding_is_closed_and_its_default_is_invisible() {
    for encoding in [BodyEncoding::Json, BodyEncoding::Form] {
        assert_eq!(
            serde_json::to_value(encoding).unwrap(),
            json!(encoding.tag()),
            "`tag()` must be the spelling a provider file carries"
        );
        assert_eq!(
            serde_json::from_value::<BodyEncoding>(json!(encoding.tag())).unwrap(),
            encoding
        );
    }
    assert_eq!(BodyEncoding::default(), BodyEncoding::Json);
    assert_eq!(BodyEncoding::Json.media_type(), "application/json");
    assert_eq!(
        BodyEncoding::Form.media_type(),
        "application/x-www-form-urlencoded"
    );
    assert!(serde_json::from_value::<BodyEncoding>(json!("multipart")).is_err());

    // The default encodes as nothing at all, which is what keeps every shipped artifact byte-identical.
    let mut operation = op("acme.thing.create", None);
    operation.params = ParamSet {
        body: vec![Param {
            name: "subject".into(),
            wire: None,
            description: String::new(),
            required: true,
            schema: json!({"type": "string"}),
        }],
        ..ParamSet::default()
    };
    let json = serde_json::to_string(&operation).unwrap();
    assert!(
        !json.contains("body_encoding"),
        "a defaulted encoding must not appear on the wire: {json}"
    );

    operation.params.body_encoding = BodyEncoding::Form;
    let json = serde_json::to_string(&operation).unwrap();
    assert!(json.contains(r#""body_encoding":"form""#), "{json}");
    let decoded: Operation = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.params.body_encoding, BodyEncoding::Form);
}

/// **Landing `expose` must not move a single existing `ir_sha256`** (C-413).
///
/// `Operation::expose` defaults to `true` and carries `skip_serializing_if`, and its doc comment
/// claims that keeps every operation that says nothing hashing exactly as it did before the field
/// existed. That claim has to be a test: `Connector::hash_domain` feeds `LockEntry::ir_sha256`, so a
/// field serializing as `"expose": true` on all 299 shipped operations would move every hash in the
/// repository, churn `connectors.lock` for providers nobody edited, and regenerate all 557 committed
/// artifacts — which is precisely what "no shipped artifact moves" denies. Deleting the
/// `skip_serializing_if` must turn something red, and this is it.
///
/// The assertion is **byte equality against the pre-field encoding**, not merely the absence of the
/// substring. A field that elided its key while perturbing anything else in the encoding would still
/// have moved every hash, and a `contains` check would have shrugged.
#[test]
fn an_exposed_operation_serializes_exactly_as_it_did_before_the_field_existed() {
    let operation = op("acme.thing.list", None);
    assert!(
        operation.expose,
        "the field must default to exposed; a default that hides is a decision made by silence"
    );

    // The required direction joins the stable encoding; `expose` remains omitted at its default.
    let expected = json!({
        "id": "acme.thing.list",
        "method": "GET",
        "direction": "read",
        "path": "/v2/calls",
        "description": "List calls",
        "risk": "low",
        "idempotency": "idempotent",
    });
    assert_eq!(
        serde_json::to_value(&operation).unwrap(),
        expected,
        "an operation silent on `expose` must encode exactly as it did before the field existed, \
         or landing C-413 moved every `ir_sha256` in the repository"
    );

    // The converse, so the assertion above cannot be satisfied by dropping the field entirely.
    let mut unexposed = operation.clone();
    unexposed.expose = false;
    let json = serde_json::to_string(&unexposed).unwrap();
    assert!(json.contains(r#""expose":false"#), "{json}");
    let decoded: Operation = serde_json::from_str(&json).unwrap();
    assert!(!decoded.expose, "the field must survive the round trip");
}

/// **An operation's exposure is part of what the connector means**, so changing it has to be a change
/// `diff` and the lockfile can both see.
///
/// The elision above is only safe because it is *lossless*: `true` is the default, so its absence
/// encodes it exactly. `false` is not the default and must reach the hash domain, or an author could
/// unexpose an operation — a real change to what a model is handed — with `connectors.lock` reporting
/// nothing happened.
#[test]
fn an_unexposed_operation_reaches_the_hash_domain() {
    let mut connector = babelforce();
    connector.operations = vec![op("acme.thing.list", None)];

    let exposed = connector.hash_domain().expect("the hash domain encodes");
    assert!(
        !exposed.contains("expose"),
        "a defaulted `expose` reached the hash domain: {exposed}"
    );

    connector.operations[0].expose = false;
    let unexposed = connector.hash_domain().expect("the hash domain encodes");
    assert!(
        unexposed.contains("expose"),
        "an operation withheld from every model must be visible in the hash domain — otherwise \
         unexposing one is a change to what a host serves that no artifact records: {unexposed}"
    );
    assert_ne!(
        exposed, unexposed,
        "exposing and unexposing an operation must not hash alike"
    );
}

/// The IR must be expressive enough that a **hand-authored** provider TOML defines a complete
/// operation with no vendor spec anywhere in sight — the "two front-ends, one IR" requirement.
/// (Parsing `providers/*.toml` and its error surface is C-3; this only proves the shape fits.)
#[test]
fn a_hand_authored_toml_defines_a_complete_operation() {
    let source = r#"
id = "ollama"
vendor = "Ollama"
base_url = "http://localhost:11434"
description = "Local Ollama server — no vendor OpenAPI document exists"

[[auth]]
name = "ollama.api_key"
scheme = "bearer"
env = ["OLLAMA_API_KEY"]

[[operations]]
id = "ollama.generate"
method = "POST"
direction = "write"
path = "/api/generate"
description = "Generate a completion"
risk = "low"
idempotency = "non_idempotent"
auth = []

[[operations.params.body]]
name = "model"
required = true
schema = { type = "string" }

[[operations.params.body]]
name = "prompt"
required = true
schema = { type = "string" }

[operations.response_schema]
type = "object"
"#;

    let connector: Connector = toml::from_str(source).expect("a hand-authored TOML must load");

    assert_eq!(connector.id, "ollama");
    assert_eq!(connector.auth.len(), 1);
    assert_eq!(connector.auth[0].scheme, AuthScheme::Bearer);
    assert!(
        connector.default_auth.is_empty(),
        "an omitted connector default is an empty alternatives list"
    );

    let operation = &connector.operations[0];
    assert_eq!(operation.method, HttpMethod::Post);
    assert_eq!(operation.idempotency, Idempotency::NonIdempotent);
    assert_eq!(operation.params.body.len(), 2);
    assert_eq!(operation.params.body[0].schema["type"], json!("string"));
    assert_eq!(
        operation.auth,
        Some(Vec::new()),
        "`auth = []` in TOML must mean explicit no-auth, not unset"
    );
    assert!(operation.response_schema.is_some());

    // And the whole thing survives a JSON round-trip identically.
    let encoded = serde_json::to_string(&connector).expect("serialize");
    let decoded: Connector = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(connector, decoded);
}

/// Credential lookup: an operation's mechanism names credentials, and the connector resolves each
/// to the declaration that says how to inject it. C-10 walks exactly this path.
#[test]
fn credentials_resolve_to_declared_auth_methods() {
    let connector = babelforce();
    let create = &connector.operations[3];

    let resolved: Vec<&AuthScheme> = connector
        .effective_auth(create)
        .iter()
        .flat_map(|mechanism| mechanism.iter())
        .map(|credential| {
            &connector
                .auth_method(credential)
                .expect("every named credential must be declared")
                .scheme
        })
        .collect();

    assert_eq!(
        resolved,
        vec![
            &AuthScheme::Header {
                name: "X-Auth-Access-Id".into(),
                prefix: String::new(),
            },
            &AuthScheme::Header {
                name: "X-Auth-Access-Token".into(),
                prefix: String::new(),
            },
        ],
        "the AND case must resolve to two header injections on one request"
    );
    assert!(connector.auth_method("nope").is_none());
}

/// Quirks and provenance are part of the IR contract and round-trip like everything else.
#[test]
fn quirks_and_provenance_round_trip() {
    use connector_spec::{ErrorEnvelope, Pagination, RateLimit};

    let mut connector = babelforce();
    connector.provenance = Provenance {
        source_url: Some("https://example.test/manager.openapi.json".into()),
        upstream_version: Some("2024-05-06".into()),
        fetched_at: Some("2026-07-30T09:00:00Z".into()),
        spec_sha256: Some("a".repeat(64)),
        specs: vec![connector_spec::SpecSource {
            path: "specs/babelforce/manager-2026-07-10.openapi.yaml".into(),
            sha256: Some("a".repeat(64)),
            ..Default::default()
        }],
        operation_specs: [(
            "babelforce-call-list".into(),
            connector_spec::OperationSpecSource {
                operation_id: "listCalls".into(),
                source_url: Some("https://example.test/manager.openapi.json".into()),
                upstream_version: "2024-05-06".into(),
                sha256: "a".repeat(64),
            },
        )]
        .into(),
        toml_sha256: Some("b".repeat(64)),
    };
    connector.operations[1].quirks = Quirks {
        pagination: Some(Pagination::Cursor {
            cursor_param: "cursor".into(),
            next_cursor_pointer: "/meta/next_cursor".into(),
            max_pages: 20,
        }),
        rate_limit: Some(RateLimit {
            requests: 100,
            per_seconds: 60,
            bucket: Some("babelforce.calls".into()),
        }),
        error_envelope: Some(ErrorEnvelope {
            message_pointer: "/error/message".into(),
            code_pointer: Some("/error/code".into()),
        }),
    };

    let encoded = serde_json::to_string(&connector).expect("serialize");
    let decoded: Connector = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(connector, decoded);
    assert!(!decoded.operations[1].quirks.is_empty());
    assert!(decoded.operations[0].quirks.is_empty());
}
