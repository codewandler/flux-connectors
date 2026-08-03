//! The runtime half of the HTTPS-origin grammar and its runtime-only value boundary.

use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{
    Configuration, CredentialRef, Credentials, Egress, Error, MemoryConfig, MemoryStore, Operation,
    Secret, SecretStore, DEFAULT_SERVICE,
};
use flux_runtime::{Tool, ToolContext};
use flux_system::{System, Workspace};
use serde_json::{json, Value};

#[path = "../../connector-spec/tests/fixtures/origin_grammar.rs"]
mod origin_grammar;

const TENANT: &str = "t-origin-parity";
const CONFIGURED_ORIGIN: &str = "https://configured-origin-sentinel.example:8443";
const CONNECTION_LABEL: &str = "company-connection-label-sentinel";
const CREDENTIAL: &str = "SENTINEL-NOT-A-REAL-GITLAB-CREDENTIAL";

fn egress() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "request-reflecting test transport".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            effects: vec![flux_spec::Effect::Network],
            risk: flux_spec::Risk::Medium,
            idempotency: flux_spec::Idempotency::NonIdempotent,
            access: vec![flux_spec::AccessKind::Network],
            group: None,
        },
        |params: Value| async move { Ok(params) },
    ))
}

fn configuration(origin: &str) -> Configuration {
    Configuration::new(
        Arc::new(MemoryConfig::new().with_approved_endpoint(
            TENANT,
            "gitlab",
            DEFAULT_SERVICE,
            "origin",
            origin,
        )),
        TENANT,
    )
    .expect("valid tenant")
}

fn project(origin: &str, credentials: Credentials) -> Operation {
    let entry = catalog::operation(OperationKey::id("gitlab-user-get"))
        .expect("the shipped GitLab verification read");
    Operation::project(entry, egress(), credentials, configuration(origin))
        .expect("GitLab operation projects")
}

fn empty_credentials() -> Credentials {
    Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("valid tenant")
}

#[test]
fn the_runtime_accepts_exactly_the_shared_origin_grammar_cases() {
    for case in origin_grammar::ORIGIN_CASES {
        let outcome = project(case.value, empty_credentials()).build_request(&json!({}));
        assert_eq!(
            outcome.is_ok(),
            case.accepted,
            "runtime classified {:?} differently from the shared contract: {outcome:?}",
            case.value
        );
        if !case.accepted {
            let error = outcome.expect_err("the shared case is refused");
            assert!(matches!(error, Error::UnsafeOrigin { .. }), "{error}");
            assert!(
                !error.to_string().contains(case.value),
                "an invalid configured origin reached refusal evidence: {error}"
            );
        }
    }
}

/// A connection label is Exchange-owned metadata. The closest connector seam receives only the
/// resolved origin, stable tenant/instance addressing and credential store; the mutable label has
/// no parameter to enter through. Keeping it in this fixture while projecting the operation makes
/// that boundary explicit rather than relying on a search for the word `label`.
struct LabelledConnection<'a> {
    label: &'a str,
    origin: &'a str,
    credential: &'a str,
}

#[tokio::test]
async fn runtime_values_stay_out_of_model_permission_and_refusal_metadata() {
    let connection = LabelledConnection {
        label: CONNECTION_LABEL,
        origin: CONFIGURED_ORIGIN,
        credential: CREDENTIAL,
    };
    let store = Arc::new(MemoryStore::new());
    let reference = CredentialRef::new(TENANT, "com.gitlab.api", DEFAULT_SERVICE, "token")
        .expect("GitLab credential address");
    store
        .put(&reference, &Secret::new(connection.credential))
        .await
        .expect("in-memory credential write");
    let operation = project(
        connection.origin,
        Credentials::new(store, TENANT).expect("valid tenant"),
    );

    let params = json!({});
    let model = serde_json::to_string(&operation.spec()).expect("ToolSpec serializes");
    let subjects = operation.permission_subjects(&params);
    let intents = serde_json::to_string(&operation.intents(&params)).expect("intents serialize");

    assert!(
        subjects
            .iter()
            .any(|subject| subject.starts_with(connection.origin)),
        "the configured origin must reach the network gate, or transport and permission diverge"
    );
    assert!(
        intents.contains(connection.origin),
        "the configured origin must reach the mirrored network intent"
    );
    assert!(
        !model.contains(connection.origin),
        "a configured origin reached model-visible tool metadata: {model}"
    );
    for (surface, rendered) in [
        ("model metadata", model.as_str()),
        ("permission subjects", &subjects.join("\n")),
        ("intents", intents.as_str()),
    ] {
        assert!(
            !rendered.contains(connection.label),
            "the Exchange-owned connection label reached {surface}: {rendered}"
        );
        assert!(
            !rendered.contains(connection.credential),
            "the credential value reached {surface}: {rendered}"
        );
    }

    let context = ToolContext::new(Arc::new(System::new(
        Workspace::new(env!("CARGO_MANIFEST_DIR")).expect("crate root"),
    )));
    let request = operation
        .build_authenticated_request(&context, &json!({}))
        .await
        .expect("the credential and origin compose at the last responsible moment");
    let authorization = request
        .headers
        .get("Authorization")
        .expect("GitLab bearer authorization");
    assert!(
        authorization.contains(connection.credential),
        "the non-leak assertions would be vacuous if the credential never reached the request"
    );
    assert!(
        !context
            .redactor
            .redact(authorization)
            .contains(connection.credential),
        "the host redactor was not told the Authorization value"
    );
    assert!(
        !request.url.contains(connection.label),
        "a mutable connection label became part of the request URL"
    );

    let invalid = format!("{}/api/v4", connection.origin);
    let refusal = project(&invalid, empty_credentials())
        .build_request(&json!({}))
        .expect_err("a configured origin cannot smuggle the connector-owned path");
    let refusal = refusal.to_string();
    for sentinel in [connection.origin, connection.label, connection.credential] {
        assert!(
            !refusal.contains(sentinel),
            "runtime-only value {sentinel:?} reached refusal evidence: {refusal}"
        );
    }
}
