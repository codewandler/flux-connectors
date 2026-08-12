//! Determinism tests for the connector IR.
//!
//! `connectors.lock` (C-7) hashes the serialized IR and `flux-connectors check` fails on a
//! mismatch, so a single leaked iteration order would turn into phantom drift on every build for
//! every provider. These tests are the guard: they assert that the encoding is a pure function of
//! the IR's *value*, not of the order any collection happened to be built in.

use connector_spec::{
    AuthMethod, AuthRequirement, AuthScheme, Connector, HttpMethod, Idempotency, Operation,
    OperationDirection, Param, ParamSet, Provenance, Quirks, Risk, DEFAULT_SERVICE,
};
use serde_json::json;

/// Builds the same connector twice over, taking every collection in a different order the second
/// time. The two values must encode to identical bytes.
fn connector(reversed: bool) -> Connector {
    let credentials: Vec<&str> = if reversed {
        vec!["b.access_token", "b.access_id"]
    } else {
        vec!["b.access_id", "b.access_token"]
    };

    let schema = if reversed {
        json!({"minimum": 1, "format": "int32", "type": "integer"})
    } else {
        json!({"type": "integer", "format": "int32", "minimum": 1})
    };

    Connector {
        id: "b".into(),
        authority: None,
        runtime: connector_spec::Runtime::Http,
        api_version: None,
        services: Vec::new(),
        vendor: "Babelforce".into(),
        base_url: "https://{tenant}.babelforce.com".into(),
        description: String::new(),
        auth: vec![
            AuthMethod {
                name: "b.access_id".into(),
                scheme: AuthScheme::Header {
                    name: "X-Auth-Access-Id".into(),
                    prefix: String::new(),
                },
                env: vec!["B_ACCESS_ID".into()],
                ..AuthMethod::default()
            },
            AuthMethod {
                name: "b.access_token".into(),
                scheme: AuthScheme::Header {
                    name: "X-Auth-Access-Token".into(),
                    prefix: String::new(),
                },
                env: vec!["B_ACCESS_TOKEN".into()],
                ..AuthMethod::default()
            },
        ],
        default_auth: vec![AuthRequirement::all(credentials.iter().copied())],
        operations: vec![Operation {
            id: "b.call.list".into(),
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
            auth: None,
            params: ParamSet {
                query: vec![Param {
                    name: "limit".into(),
                    wire: None,
                    description: String::new(),
                    required: false,
                    schema: schema.clone(),
                }],
                ..ParamSet::default()
            },
            response_schema: Some(schema),
            credential_response: Vec::new(),
            produces_credential: None,
            quirks: Quirks::default(),
        }],
        events: Vec::new(),
        channels: Vec::new(),
        config: Vec::new(),
        verify: None,
        graphs: Vec::new(),
        provenance: Provenance::default(),
    }
}

/// The acceptance criterion, stated directly: identical inputs produce byte-identical output.
#[test]
fn identical_inputs_serialize_to_identical_bytes() {
    let straight = connector(false).canonical_json().expect("serialize");
    let shuffled = connector(true).canonical_json().expect("serialize");

    assert_eq!(
        straight, shuffled,
        "two connectors with the same value must encode to the same bytes regardless of the order \
         their collections were built in"
    );
}

/// Repeating the encoding must not change it either — the failure mode a `HashMap` field would
/// produce is a *different* order on each run within one process.
#[test]
fn repeated_serialization_is_stable() {
    let value = connector(false);
    let first = value.canonical_json().expect("serialize");
    for _ in 0..64 {
        assert_eq!(
            first,
            value.canonical_json().expect("serialize"),
            "the encoding must not vary between runs"
        );
    }
}

/// A decoded connector re-encodes to exactly the bytes it was decoded from, so a round trip
/// through `connectors.lock` or a regenerated artifact cannot perturb the hash.
#[test]
fn decode_then_encode_is_a_fixed_point() {
    let encoded = connector(false).canonical_json().expect("serialize");
    let decoded: Connector = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(encoded, decoded.canonical_json().expect("serialize"));
}

/// `serde_json`'s object map must stay key-sorted (`BTreeMap`), which is what makes an arbitrary
/// JSON Schema value deterministic without this crate canonicalizing it by hand. Enabling
/// `serde_json/preserve_order` anywhere in the workspace would silently swap that for insertion
/// order — feature unification would apply it here too, and every schema's key order would start
/// tracking how the vendor document happened to be parsed. This is the tripwire for that.
#[test]
fn serde_json_object_keys_stay_sorted() {
    let value: serde_json::Value =
        serde_json::from_str(r#"{"type":"integer","minimum":1,"format":"int32"}"#).expect("parse");
    assert_eq!(
        serde_json::to_string(&value).expect("serialize"),
        r#"{"format":"int32","minimum":1,"type":"integer"}"#,
        "serde_json must serialize object keys in sorted order — see this test's doc comment"
    );
}

/// A mechanism's credentials are a set, so its encoding must depend on the set's members alone.
#[test]
fn requirement_encoding_ignores_authoring_order() {
    let a = AuthRequirement::all(["z.credential", "a.credential", "m.credential"]);
    let b = AuthRequirement::all(["m.credential", "z.credential", "a.credential"]);
    assert_eq!(a, b);
    assert_eq!(
        serde_json::to_string(&a).expect("serialize"),
        serde_json::to_string(&b).expect("serialize"),
    );
}
