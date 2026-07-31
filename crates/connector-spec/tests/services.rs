//! A provider's **services**: the middle addressing level, as `providers/*.toml` declares it and as
//! the IR encodes it (C-49, `docs/designs/provider-services.md`).
//!
//! Every assertion here goes through `provider::load` and the connector's own canonical encoding,
//! never through field access. That is deliberate on two counts:
//!
//! - the *file* is the surface an author writes, and "unset means `default`" is a statement about
//!   the file, not about a Rust struct;
//! - the encoding is what `connectors.lock` hashes, so "a `default`-only connector encodes exactly as
//!   it did before services existed" is a claim about bytes and has to be tested as one.
//!
//! `service_partition.rs` covers the partition invariant and the address round-trip, which do need
//! the accessors.

use std::path::{Path, PathBuf};

use connector_spec::{provider, Connector};

/// A two-service provider, AWS-shaped: one authority, two API surfaces, one date each.
const AWS: &str = r#"
id = "aws"
vendor = "Amazon Web Services"
authority = "com.amazonaws"
base_url = "https://amazonaws.com"
api_version = "2010-05-08"
description = "Amazon Web Services."

[[services]]
name = "s3"
description = "Object storage."
base_url = "https://s3.amazonaws.com"
api_version = "2006-03-01"

[[services]]
name = "bedrock-runtime"
description = "Model inference."
base_url = "https://bedrock-runtime.us-east-1.amazonaws.com"
api_version = "2023-09-30"

[[operations]]
id = "aws-s3-object-get"
service = "s3"
method = "GET"
path = "/{bucket}/{key}"
description = "Fetch one object."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "bucket"
required = true
schema = { type = "string" }

[[operations.params.path]]
name = "key"
required = true
schema = { type = "string" }

[[operations]]
id = "aws-bedrock-model-invoke"
service = "bedrock-runtime"
method = "POST"
path = "/model/{model_id}/invoke"
description = "Invoke a model."
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.path]]
name = "model_id"
required = true
schema = { type = "string" }
"#;

/// A single-service provider: no `[[services]]`, no `service` on the operation. The shape all six
/// shipped providers have.
const SINGLE: &str = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"

[[operations]]
id = "acme-thing-get"
method = "GET"
path = "/v1/things/{thing_id}"
description = "Fetch one thing."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "thing_id"
required = true
schema = { type = "integer" }
"#;

fn load(source: &str) -> Connector {
    provider::load("providers/fixture.toml", source)
        .unwrap_or_else(|error| panic!("the fixture must load:\n{error}"))
        .connector
}

fn refuse(source: &str) -> String {
    let error = provider::load("providers/fixture.toml", source)
        .err()
        .unwrap_or_else(|| panic!("this definition must not load"));
    format!("{error}")
}

fn json(source: &str) -> String {
    load(source)
        .canonical_json()
        .expect("the IR encodes to canonical JSON")
}

/// A service is an IR level with its own fields, and each operation names exactly one.
#[test]
fn a_declared_service_reaches_the_ir() {
    let encoded = json(AWS);

    assert!(
        encoded.contains(r#""authority":"com.amazonaws""#),
        "the provider's authority must reach the IR:\n{encoded}"
    );
    assert!(
        encoded.contains(
            r#""services":[{"name":"s3","description":"Object storage.","base_url":"https://s3.amazonaws.com","api_version":"2006-03-01"}"#
        ),
        "a service carries its name, description, base URL override and API version:\n{encoded}"
    );
    assert!(
        encoded.contains(r#""name":"bedrock-runtime""#)
            && encoded.contains(r#""api_version":"2023-09-30""#),
        "each service versions itself:\n{encoded}"
    );
    assert!(
        encoded.contains(r#""service":"s3""#) && encoded.contains(r#""service":"bedrock-runtime""#),
        "each operation names the service it belongs to:\n{encoded}"
    );
}

/// **The byte-identity property, at the IR level.** A provider that declares no service encodes
/// exactly as it did before services existed: no `services`, no `authority`, no `api_version`, and no
/// `service` on any operation. Everything downstream of the encoding — the lockfile hash, and
/// therefore every artifact keyed by it — inherits that.
#[test]
fn a_single_service_provider_encodes_no_service_at_all() {
    let encoded = json(SINGLE);

    for absent in ["services", "authority", "api_version", "service"] {
        assert!(
            !encoded.contains(&format!("\"{absent}\"")),
            "a `default`-only provider must not encode `{absent}`:\n{encoded}"
        );
    }
}

/// `default` is the name of the implicit service, so stating it and omitting it are one meaning —
/// and one meaning gets one encoding. This is the same objection `AuthRequirement` records against
/// an empty mechanism inside a non-empty alternatives list.
#[test]
fn stating_the_default_service_encodes_exactly_as_omitting_it() {
    let stated = SINGLE.replace(
        r#"id = "acme-thing-get""#,
        "id = \"acme-thing-get\"\nservice = \"default\"",
    );
    // The hash domain rather than the full encoding: the two files differ in their bytes, so their
    // `provenance.toml_sha256` differs by construction. What must agree is the *compiled meaning*,
    // which is exactly what the hash domain is.
    let domain = |source: &str| load(source).hash_domain().expect("the IR hashes");
    assert_eq!(domain(&stated), domain(SINGLE));
}

/// The reserved name may not be redeclared: a `[[services]]` entry called `default` would be a
/// second, contradictable definition of the implicit service.
#[test]
fn declaring_the_reserved_default_service_is_refused() {
    let source = SINGLE.replace(
        "[[operations]]",
        "[[services]]\nname = \"default\"\n\n[[operations]]",
    );
    let error = refuse(&source);
    assert!(error.contains("default"), "{error}");
    assert!(error.contains("reserved"), "{error}");
}

/// An operation naming a service nothing declares is a loud error that lists the services that do
/// exist — C-3's treatment of a duplicate op id, applied one level up.
#[test]
fn an_operation_naming_an_undeclared_service_is_refused_and_the_error_lists_what_exists() {
    let error = refuse(&AWS.replace(r#"service = "s3""#, r#"service = "s4""#));
    assert!(error.contains("\"s4\""), "{error}");
    assert!(error.contains("s3"), "the error must list s3:\n{error}");
    assert!(
        error.contains("bedrock-runtime"),
        "the error must list bedrock-runtime:\n{error}"
    );
}

/// In a provider that declares named services there is no implicit `default` to fall into. An
/// operation that omits `service` would otherwise land in a service nothing declares, and would emit
/// an `aws-default.flux` nobody asked for.
#[test]
fn omitting_the_service_in_a_multi_service_provider_is_refused() {
    let error = refuse(&AWS.replace("service = \"s3\"\n", ""));
    assert!(error.contains("default"), "{error}");
    assert!(error.contains("s3"), "{error}");
    assert!(error.contains("bedrock-runtime"), "{error}");
}

#[test]
fn a_duplicate_service_declaration_is_refused() {
    let error = refuse(&AWS.replace(r#"name = "bedrock-runtime""#, r#"name = "s3""#));
    assert!(error.contains("s3"), "{error}");
    assert!(
        error.contains("more than once"),
        "the duplicate must be named as one:\n{error}"
    );
}

#[test]
fn an_empty_service_name_is_refused() {
    let error = refuse(&AWS.replace(r#"name = "s3""#, r#"name = """#));
    assert!(error.contains("must not be empty"), "{error}");
}

/// **A service name is not free text, and the reason is not tidiness.** It is the middle segment of
/// the service's address *and* part of the emitted `<provider>-<service>.flux`, so an unvalidated one
/// both publishes an address that does not parse back and lets a provider file choose where a build
/// writes. `..` and `/` are the two that matter; the case and space cases are the same rule.
#[test]
fn an_unspellable_service_name_is_refused() {
    for name in [
        "../../../../outside/pwned",
        "a/b",
        "My Service",
        "S3",
        "  ",
        "s3.",
        "s3:v1",
        "s3#get",
    ] {
        let error = refuse(&AWS.replace(r#"name = "s3""#, &format!("name = {name:?}")));
        assert!(
            error.contains("service name"),
            "service name {name:?} must be refused by the loader, got:\n{error}"
        );
    }
}

/// An authority is validated for the same reason: `Connector::gid_of` renders whatever the loader
/// accepted, and `com.acme/s3` renders `com.acme/s3:v2` — a string that reparses as a *different*
/// address, which is precisely the masquerade the address module claims is impossible.
#[test]
fn an_unspellable_authority_is_refused() {
    for authority in [
        "com.acme/s3",
        "",
        "acme",
        "Com.ACME",
        "com..acme",
        "com.acme:1",
        "com.acme#x",
    ] {
        let error = refuse(&AWS.replace(
            r#"authority = "com.amazonaws""#,
            &format!("authority = {authority:?}"),
        ));
        assert!(
            error.contains("authority"),
            "authority {authority:?} must be refused by the loader, got:\n{error}"
        );
    }
}

/// A version carrying one of the scheme's separators would move the boundary a parser reads the
/// address at, so it is refused at both levels that can declare one.
#[test]
fn an_api_version_carrying_a_separator_is_refused() {
    for version in ["", "v2/beta", "v2:1", "v2#x"] {
        let connector_level = refuse(&AWS.replace(
            r#"api_version = "2010-05-08""#,
            &format!("api_version = {version:?}"),
        ));
        assert!(
            connector_level.contains("api_version"),
            "connector `api_version` {version:?} must be refused, got:\n{connector_level}"
        );

        let service_level = refuse(&AWS.replace(
            r#"api_version = "2006-03-01""#,
            &format!("api_version = {version:?}"),
        ));
        assert!(
            service_level.contains("api_version"),
            "service `api_version` {version:?} must be refused, got:\n{service_level}"
        );
    }
}

/// Service fields are part of a connector's **compiled meaning**, like C-37's addresses and unlike
/// C-7's provenance: move one and the generated module moves, so the hash must move with it.
#[test]
fn every_service_field_is_inside_the_hash_domain() {
    let base = load(AWS).ir_sha256().expect("the IR hashes");

    let moved = [
        ("an authority", AWS.replace("com.amazonaws", "com.aws")),
        (
            "a connector api_version",
            AWS.replace("2010-05-08", "2011-06-15"),
        ),
        (
            "a service api_version",
            AWS.replace("2006-03-01", "2006-03-02"),
        ),
        (
            "a service base_url",
            AWS.replace("https://s3.amazonaws.com", "https://s3.eu.amazonaws.com"),
        ),
        (
            "a service description",
            AWS.replace("Object storage.", "Objects."),
        ),
        (
            "a service name",
            AWS.replace("\"s3\"", "\"simple-storage\""),
        ),
    ];
    for (what, source) in moved {
        assert_ne!(
            base,
            load(&source).ir_sha256().expect("the IR hashes"),
            "changing {what} left the IR hash where it was"
        );
    }
}

/// And a connector that declares none of them hashes what it always hashed. This is the other half
/// of the byte-identity claim: `connectors.lock` must not churn for a provider nobody edited.
#[test]
fn a_default_only_connector_hashes_no_service_fields() {
    let domain = load(SINGLE).hash_domain().expect("the IR hashes");
    for absent in ["services", "authority", "api_version", "service"] {
        assert!(
            !domain.contains(&format!("\"{absent}\"")),
            "the hash domain of a `default`-only connector must not name `{absent}`:\n{domain}"
        );
    }
}

/// **What every shipped provider must hold now that one of them declares services.**
///
/// C-49 pinned that *no* shipped provider declared any (`every_shipped_provider_is_single_service`).
/// C-69 ships `google` — gmail, calendar and drive under one vendor — so that pin is now false by
/// design, and deleting it without a replacement would leave the shipped catalogue unchecked at
/// exactly the level this story exercises. What is asserted instead is the pair of claims the old test
/// was standing in for, one per shape:
///
/// - **A declared service name is spellable**, and that is a safety property rather than a tidiness
///   one: the name reaches the emitted `<provider>-<service>.flux` — so an unvalidated one lets a
///   provider file choose where a build writes — and it is the middle segment of every address the
///   service publishes. The loader enforces it (`an_unspellable_service_name_is_refused` above, over
///   the hostile spellings); this is the claim over what ships. It also refuses the reserved name and
///   requires that a declared service actually own operations, so no provider can ship a service that
///   emits an empty module.
/// - **A single-service provider still declares nothing at all**, which is the byte-identity property
///   the reshape rests on: eleven of the twelve shipped providers encode no `services`, no `service`,
///   and therefore hash exactly what they hashed before services existed.
///
/// Derived from the directory rather than a hard-coded list, and it fails when *no* provider declares
/// services: without a multi-service one shipping, the first half of this test would pass vacuously
/// and the emitted-per-service path would be covered by fixtures only.
#[test]
fn every_shipped_service_is_spellable_and_a_single_service_provider_declares_none() {
    let mut multi_service = Vec::new();

    for path in shipped() {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let connector = provider::load(&format!("providers/{name}"), &source)
            .unwrap_or_else(|error| panic!("providers/{name} does not load: {error}"))
            .connector;

        if connector.is_default_only() {
            let encoded = connector.canonical_json().expect("the IR encodes");
            let document: serde_json::Value =
                serde_json::from_str(&encoded).expect("the canonical encoding is JSON");
            if let Some(position) = service_key_outside_a_schema(&document, "$") {
                panic!(
                    "providers/{name} has one API surface, so it must encode no service key — \
                     otherwise its lockfile entry and every artifact keyed by it churn for a \
                     provider nobody edited. Found one at {position}:\n{encoded}"
                );
            }
            continue;
        }
        multi_service.push(name.clone());

        for service in connector.service_names() {
            if let Err(reason) = connector_spec::address::validate_service_name(service) {
                panic!(
                    "providers/{name} declares service {service:?}, which the address grammar \
                     refuses: {reason}. The name reaches `connectors/*-{service}.flux`"
                );
            }
            assert_ne!(
                service,
                connector_spec::DEFAULT_SERVICE,
                "providers/{name} declares the reserved service name"
            );
            assert!(
                connector.operations_of(service).next().is_some(),
                "providers/{name} declares service {service:?} with no operation in it, so a build \
                 emits an empty module and manifest for it"
            );
        }

        for operation in &connector.operations {
            assert_ne!(
                operation.service,
                connector_spec::DEFAULT_SERVICE,
                "providers/{name} declares named services, so operation `{}` cannot fall into the \
                 reserved one",
                operation.id
            );
        }
    }

    assert!(
        !multi_service.is_empty(),
        "no shipped provider declares services, so the spelling and per-service claims above are \
         vacuous and only fixtures cover the emitted-per-service path — C-69's `google` is the one \
         that keeps them honest"
    );
}

/// **The relaxation above is exactly one word wide, and this is the proof.**
///
/// [`service_key_outside_a_schema`] replaced a substring scan, so the question a reviewer must be
/// able to answer is whether it still bites. It does: an IR-shaped `service` key is found at every
/// depth the encoding can put one — top level, inside an operation, inside a config field, and
/// inside an array element — and the *only* thing newly tolerated is the same word appearing as a
/// vendor's own property name inside a schema.
#[test]
fn the_service_key_walk_still_finds_an_ir_service_key_at_every_depth() {
    for (case, document) in [
        (
            "top level",
            serde_json::json!({ "id": "x", "services": [{ "name": "s3" }] }),
        ),
        (
            "on an operation",
            serde_json::json!({ "operations": [{ "id": "x-get", "service": "s3" }] }),
        ),
        (
            "on a config field",
            serde_json::json!({ "config": [{ "name": "token", "service": "s3" }] }),
        ),
        (
            "under an unrelated nesting the walk has never been told about",
            serde_json::json!({ "a": { "b": [{ "c": { "service": "s3" } }] } }),
        ),
    ] {
        assert!(
            service_key_outside_a_schema(&document, "$").is_some(),
            "the walk missed an IR service key {case}; it would no longer guard the encoding"
        );
    }

    // The one tolerated shape, and the reason the walk exists: a vendor whose own domain noun is
    // "service". PagerDuty's `GET /services` answers `{"services": [...]}` and its incidents carry a
    // `service` reference — vendor data inside a schema, at any depth, in any of the three
    // schema-bearing keys.
    for (case, document) in [
        (
            "a response schema's own property",
            serde_json::json!({
                "operations": [{
                    "id": "pagerduty-service-list",
                    "response_schema": {
                        "type": "object",
                        "properties": { "services": { "type": "array" } }
                    }
                }]
            }),
        ),
        (
            "nested deep inside a response schema",
            serde_json::json!({
                "operations": [{
                    "response_schema": {
                        "properties": {
                            "incidents": {
                                "items": { "properties": { "service": { "type": "object" } } }
                            }
                        }
                    }
                }]
            }),
        ),
        (
            "a parameter's schema and a free-form body schema",
            serde_json::json!({
                "operations": [{
                    "params": {
                        "query": [{ "name": "q", "schema": { "properties": { "service": {} } } }],
                        "body_schema": { "properties": { "services": {} } }
                    }
                }]
            }),
        ),
    ] {
        assert_eq!(
            service_key_outside_a_schema(&document, "$"),
            None,
            "the walk flagged {case}, which is the vendor's word and not this repository's field"
        );
    }
}

/// The JSON keys whose values are **vendor JSON Schema**, not this repository's IR structure.
///
/// A schema describes what a vendor sends and receives, so every key inside one is the vendor's
/// word rather than ours. `service`/`services` are ordinary English, and a vendor is entitled to
/// both.
const SCHEMA_KEYS: [&str; 3] = ["schema", "response_schema", "body_schema"];

/// Finds a `service` or `services` key in the encoded IR, ignoring anything inside a JSON Schema,
/// and returns the path to the first one.
///
/// # Why this is a walk and not `encoded.contains("\"service\"")`
///
/// A substring scan was the original spelling, and C-162 measured what it actually asserts. PagerDuty
/// is an incident-response vendor whose own domain noun is *service*: `GET /services` answers
/// `{"services": [...]}`, and every incident carries a `service` reference. Those spellings are
/// vendor data sitting inside `response_schema`, and describing them accurately is the connector's
/// whole job — renaming them to satisfy a scan would make the declared response shape wrong about
/// what PagerDuty sends.
///
/// So the scan was reporting a **collision of words**, not the property it exists to protect. That
/// property is about the *IR's own* fields — `Connector::services`, and the `service` on an
/// operation, a config field, an event, a channel and a graph — every one of which encodes as a key
/// on an IR object, never inside a schema. This walk asserts exactly that, at every depth, so it
/// stays as strong as the scan everywhere the scan was load-bearing and stops being wrong where it
/// was only ever counting characters.
///
/// It deliberately does **not** take a list of known IR positions: a new IR struct carrying a
/// `service` would then be unchecked until somebody remembered to add it here, which is the failure
/// mode this test was written to avoid in the first place.
fn service_key_outside_a_schema(value: &serde_json::Value, path: &str) -> Option<String> {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, child) in fields {
                if key == "service" || key == "services" {
                    return Some(format!("{path}.{key}"));
                }
                if SCHEMA_KEYS.contains(&key.as_str()) {
                    continue;
                }
                if let Some(found) = service_key_outside_a_schema(child, &format!("{path}.{key}")) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::Array(items) => items.iter().enumerate().find_map(|(index, item)| {
            service_key_outside_a_schema(item, &format!("{path}[{index}]"))
        }),
        _ => None,
    }
}

fn shipped() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no shipped providers found in {dir:?}");
    paths
}
