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
    let error = refuse(&AWS.replace(r#"name = "s3""#, r#"name = "  ""#));
    assert!(error.contains("empty"), "{error}");
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

/// Today's whole catalogue is single-service, which is why this story can be meaning-preserving for
/// it. Derived from the directory rather than a hard-coded list: a provider added tomorrow is covered
/// without anyone remembering to add it here.
#[test]
fn every_shipped_provider_is_single_service() {
    for path in shipped() {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let connector = provider::load(&format!("providers/{name}"), &source)
            .unwrap_or_else(|error| panic!("providers/{name} does not load: {error}"))
            .connector;
        let encoded = connector.canonical_json().expect("the IR encodes");
        assert!(
            !encoded.contains("\"services\""),
            "providers/{name} declares services; this test pins that today's catalogue does not"
        );
        assert!(
            !encoded.contains("\"service\""),
            "providers/{name} places an operation outside the `default` service"
        );
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
