//! GitLab is the reference consumer of the generic operator-approved HTTPS-origin policy.

use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{
    Configuration, Credentials, Egress, Error, MemoryConfig, MemoryStore, Operation,
    DEFAULT_SERVICE,
};
use flux_runtime::Tool;
use serde_json::json;

const TENANT: &str = "t-gitlab-origin";
const CUSTOM: &str = "https://gitlab.company.example:8443";

fn egress() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "GitLab-shaped fixture".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            effects: vec![flux_spec::Effect::Network],
            risk: flux_spec::Risk::Medium,
            idempotency: flux_spec::Idempotency::NonIdempotent,
            access: vec![flux_spec::AccessKind::Network],
            group: None,
        },
        |params| async move { Ok(params) },
    ))
}

fn credentials(store: Arc<MemoryStore>) -> Credentials {
    Credentials::new(store, TENANT).expect("valid tenant")
}

fn project(id: &str, config: MemoryConfig) -> Operation {
    let entry = catalog::operation(OperationKey::id(id)).expect("shipped GitLab operation");
    Operation::project(
        entry,
        egress(),
        credentials(Arc::new(MemoryStore::new())),
        Configuration::new(Arc::new(config), TENANT).expect("valid tenant"),
    )
    .expect("GitLab operation projects")
}

#[test]
fn gitlab_com_remains_the_zero_configuration_request_and_subject() {
    let operation = project("gitlab-user-get", MemoryConfig::new());
    let request = operation.build_request(&json!({})).expect("default builds");
    assert_eq!(request.url, "https://gitlab.com/api/v4/user");
    assert_eq!(
        operation.permission_subjects(&json!({})),
        ["https://gitlab.com/api/v4/user"]
    );
}

#[test]
fn a_custom_origin_is_inert_until_operator_policy_approves_it() {
    let proposed =
        MemoryConfig::new().with_endpoint(TENANT, "gitlab", DEFAULT_SERVICE, "origin", CUSTOM);
    let operation = project("gitlab-user-get", proposed);
    let error = operation
        .build_request(&json!({}))
        .expect_err("a proposal is not an active endpoint");
    assert!(matches!(error, Error::UnapprovedConfig { .. }), "{error}");
    assert!(!error.to_string().contains(CUSTOM), "{error}");
}

#[test]
fn request_and_permission_subject_share_the_approved_origin_and_effective_port() {
    let config = MemoryConfig::new().with_approved_endpoint(
        TENANT,
        "gitlab",
        DEFAULT_SERVICE,
        "origin",
        CUSTOM,
    );
    let operation = project("gitlab-issue-list", config);
    let params = json!({"project_id": 7, "state": null, "page": null, "per_page": null});
    let request = operation
        .build_request(&params)
        .expect("approved origin builds");
    assert_eq!(
        request.url,
        "https://gitlab.company.example:8443/api/v4/projects/7/issues"
    );
    assert_eq!(operation.permission_subjects(&params), [request.url]);
    assert!(
        operation.spec().input_schema["properties"]
            .get("origin")
            .is_none(),
        "connection configuration must not become a model-visible operation argument"
    );
}

#[test]
fn invalid_stored_origin_is_refused_without_copying_it_into_the_refusal() {
    let value = "https://gitlab.company.example/api/v4";
    let config = MemoryConfig::new().with_approved_endpoint(
        TENANT,
        "gitlab",
        DEFAULT_SERVICE,
        "origin",
        value,
    );
    let error = project("gitlab-user-get", config)
        .build_request(&json!({}))
        .expect_err("an origin cannot smuggle the API path");
    assert!(matches!(error, Error::UnsafeOrigin { .. }), "{error}");
    assert!(!error.to_string().contains(value), "{error}");
}
