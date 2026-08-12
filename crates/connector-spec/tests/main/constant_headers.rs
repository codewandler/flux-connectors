//! `const_headers`: the vendor-fixed request headers a connector declares, and the one thing they
//! may never carry.
//!
//! C-55. Every other field in this schema that could hold a secret holds a **reference** — a
//! credential name, an environment-variable key — that a host resolves at request time and that
//! nothing writes down. A constant header is different in kind: it is a literal, and it travels
//! verbatim into generated Flux, the capability manifest and the public catalogue. So the field is
//! exactly the shape of a back door to the `$auth` seam C-10 owns, and the tests below are what keep
//! it from being one.
//!
//! The distribution rule is here too, because it is a loader decision: a provider states a header
//! once and every operation's IR carries it, so no consumer has to resolve an inheritance to know
//! what a request sends.

use connector_spec::provider::load;
use connector_spec::Connector;

/// A connector with one operation, and whatever `const_headers` the case under test needs.
fn provider(const_headers: &str) -> String {
    format!(
        r#"
id = "vendor"
base_url = "https://api.vendor.example"

[[auth]]
name = "vendor.api_key"
scheme = {{ header = {{ name = "X-Api-Key" }} }}
env = ["VENDOR_API_KEY"]

{const_headers}

[[operations]]
id = "vendor-thing-list"
method = "GET"
direction = "read"
path = "/v1/things"
description = "List things."
risk = "low"
idempotency = "idempotent"
"#
    )
}

fn refusal(source: &str) -> String {
    load("providers/fixture.toml", source)
        .err()
        .map(|error| error.to_string())
        .unwrap_or_else(|| panic!("this must be refused:\n{source}"))
}

fn connector(source: &str) -> Connector {
    load("providers/fixture.toml", source)
        .unwrap_or_else(|error| panic!("the fixture must load: {error}"))
        .connector
}

/// **A constant header can never carry a credential.**
///
/// Four routes to one failure, and all four are refused: the header the seam owns, the header a
/// declared credential is injected into, a value that *is* a token, and a value that names the place
/// a token comes from. The last is the subtle one — nothing interpolates a constant header, so
/// `${VENDOR_API_KEY}` does not resolve to anything; it either reaches the vendor as those
/// characters, or it teaches the author to paste the real value in when it does not work.
#[test]
fn a_constant_header_may_not_carry_a_credential() {
    for header in [
        r#"[const_headers]
"Authorization" = "Bearer sk-live-1234567890""#,
        r#"[const_headers]
"Proxy-Authorization" = "Basic dXNlcjpwYXNz""#,
        r#"[const_headers]
"Cookie" = "session=abc123""#,
        r#"[const_headers]
"X-Api-Key" = "a-literal-key""#,
        r#"[const_headers]
"X-Vendor-Token" = "${VENDOR_API_KEY}""#,
        r#"[const_headers]
"X-Vendor-Token" = "VENDOR_API_KEY""#,
        r#"[const_headers]
"X-Vendor-Token" = "{{ vendor.api_key }}""#,
        r#"[const_headers]
"X-Vendor-Token" = "env:VENDOR_API_KEY""#,
        r#"[const_headers]
"X-Vendor-Token" = "vendor.api_key""#,
    ] {
        let source = provider(header);
        let rendered = refusal(&source);
        assert!(
            rendered.contains("credential") || rendered.contains("interpolates"),
            "the refusal must say why, got: {rendered}\nfor:\n{header}"
        );
    }
}

/// The rule holds at operation level too — the two levels are one mechanism, and a refusal that
/// covered only the provider table would leave the door open one line further down.
#[test]
fn an_operation_level_constant_header_may_not_carry_a_credential_either() {
    let source = provider("")
        + r#"
[operations.params.const_headers]
"Authorization" = "Bearer sk-live-1234567890"
"#;
    let rendered = refusal(&source);
    assert!(
        rendered.contains("vendor-thing-list") && rendered.contains("credential"),
        "the refusal must name the operation and the reason, got: {rendered}"
    );
}

/// A value is emitted verbatim into a header record, so a newline in one would append a header of
/// the author's choosing to every request the operation makes.
#[test]
fn a_constant_header_value_may_not_carry_a_line_break() {
    let source = provider(
        r#"[const_headers]
"Accept" = "application/json\r\nAuthorization: Bearer sk-live-1234567890""#,
    );
    let rendered = refusal(&source);
    assert!(
        rendered.contains("field value"),
        "the refusal must name what an HTTP field value may hold, got: {rendered}"
    );
}

/// A name that is not an HTTP token could never be built into a request at all.
#[test]
fn a_constant_header_name_must_be_an_http_field_name() {
    for name in ["Notion Version", "Notion:Version", ""] {
        let source = provider(&format!(
            "[const_headers]\n{name:?} = \"2022-06-28\"\n",
            name = name
        ));
        let rendered = refusal(&source);
        assert!(
            rendered.contains("HTTP field name"),
            "`{name}` must be refused as a field name, got: {rendered}"
        );
    }
}

/// An empty value is a header the vendor did not ask for.
#[test]
fn a_constant_header_must_state_a_value() {
    let source = provider(
        r#"[const_headers]
"Notion-Version" = "  ""#,
    );
    assert!(refusal(&source).contains("empty value"));
}

/// **Declared once, carried by every operation.** The IR is the normalized form, so an operation
/// states every header it sends and nothing downstream re-derives an inheritance.
#[test]
fn a_provider_level_constant_header_is_distributed_onto_every_operation() {
    let source = r#"
id = "vendor"
base_url = "https://api.vendor.example"

[const_headers]
"Notion-Version" = "2022-06-28"

[[operations]]
id = "vendor-thing-list"
method = "GET"
direction = "read"
path = "/v1/things"
description = "List things."
risk = "low"
idempotency = "idempotent"

[[operations]]
id = "vendor-thing-create"
method = "POST"
direction = "write"
path = "/v1/things"
description = "Create a thing."
risk = "medium"
idempotency = "non_idempotent"
"#;

    let connector = connector(source);
    for operation in &connector.operations {
        assert_eq!(
            operation.params.const_headers.get("Notion-Version"),
            Some(&"2022-06-28".to_string()),
            "operation `{}` does not carry the provider's constant header",
            operation.id
        );
    }
}

/// An operation may pin a version the rest of the provider does not — and because HTTP field names
/// are case-insensitive, the two spellings are one header rather than two.
#[test]
fn an_operations_own_constant_header_replaces_the_providers() {
    let source = r#"
id = "vendor"
base_url = "https://api.vendor.example"

[const_headers]
"Notion-Version" = "2022-06-28"

[[operations]]
id = "vendor-thing-list"
method = "GET"
direction = "read"
path = "/v1/things"
description = "List things."
risk = "low"
idempotency = "idempotent"

[operations.params.const_headers]
"notion-version" = "2025-09-03"
"#;

    let headers = &connector(source).operations[0].params.const_headers;
    assert_eq!(
        headers.len(),
        1,
        "one header, not two spellings of it: {headers:?}"
    );
    assert_eq!(
        headers.get("notion-version"),
        Some(&"2025-09-03".to_string())
    );
}

/// A provider-level header is one declaration, so a problem with it is one problem — not one per
/// operation that inherited it.
#[test]
fn a_provider_level_refusal_is_reported_once() {
    let source = r#"
id = "vendor"
base_url = "https://api.vendor.example"

[const_headers]
"Authorization" = "Bearer sk-live-1234567890"

[[operations]]
id = "vendor-thing-list"
method = "GET"
direction = "read"
path = "/v1/things"
description = "List things."
risk = "low"
idempotency = "idempotent"

[[operations]]
id = "vendor-thing-create"
method = "POST"
direction = "write"
path = "/v1/things"
description = "Create a thing."
risk = "medium"
idempotency = "non_idempotent"
"#;

    let rendered = refusal(source);
    assert_eq!(
        rendered.matches("carries a credential").count(),
        1,
        "one declaration, one problem:\n{rendered}"
    );
}

/// **An operation that declares none encodes exactly as it did before the field existed.**
///
/// `connectors.lock` hashes this encoding, so a field that serialized as an empty map would move
/// every `ir_sha256` in the repository and churn the lockfile for providers nobody edited.
#[test]
fn an_operation_without_constant_headers_encodes_as_it_always_did() {
    let source = provider("");
    let operation = &connector(&source).operations[0];

    let encoded = serde_json::to_string(operation).expect("an operation encodes");
    assert!(
        !encoded.contains("const_headers"),
        "an empty `const_headers` must not reach the encoding: {encoded}"
    );
}

/// And one that declares some round-trips, because the lockfile and the catalogue both read the IR
/// back.
#[test]
fn a_constant_header_survives_the_ir_round_trip() {
    let source = provider(
        r#"[const_headers]
"Notion-Version" = "2022-06-28""#,
    );
    let operation = connector(&source).operations[0].clone();

    let encoded = serde_json::to_string(&operation).expect("an operation encodes");
    let decoded: connector_spec::Operation =
        serde_json::from_str(&encoded).expect("and decodes back");
    assert_eq!(decoded, operation);
}
