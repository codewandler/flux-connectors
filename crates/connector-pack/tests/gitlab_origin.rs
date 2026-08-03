//! GitLab is the reference consumer of the generic operator-approved HTTPS-origin policy.

use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{
    ConfigStore, ConfigValue, Configuration, Credentials, Egress, Error, Field, MemoryConfig,
    MemoryStore, Operation, DEFAULT_SERVICE,
};
use connector_secrets::InstanceId;
use flux_runtime::{AllowApprover, Executor, PermissionManager, Tool, ToolContext, ToolRegistry};
use flux_system::{System, Workspace};
use serde_json::json;

const TENANT: &str = "t-gitlab-origin";
const CUSTOM: &str = "https://gitlab.company.example:8443";
const REPLACEMENT: &str = "https://gitlab.replacement.example";

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
    project_with(
        id,
        credentials(Arc::new(MemoryStore::new())),
        Configuration::new(Arc::new(config), TENANT).expect("valid tenant"),
    )
}

fn project_with(id: &str, credentials: Credentials, configuration: Configuration) -> Operation {
    let entry = catalog::operation(OperationKey::id(id)).expect("shipped GitLab operation");
    Operation::project(entry, egress(), credentials, configuration)
        .expect("GitLab operation projects")
}

fn assert_refused_and_concealed(operation: &Operation, proposal: &str) {
    let error = operation
        .build_request(&json!({}))
        .expect_err("an inert proposal is not an active endpoint");
    assert!(matches!(error, Error::UnapprovedConfig { .. }), "{error}");
    assert!(!error.to_string().contains(proposal), "{error}");

    let subjects = operation.permission_subjects(&json!({}));
    assert!(
        subjects.iter().all(|subject| !subject.contains(proposal)),
        "an inert proposal reached permission subjects: {subjects:?}"
    );
    let intents = operation.intents(&json!({}));
    assert!(
        !format!("{:?}", intents.intents).contains(proposal),
        "an inert proposal reached the approval/evidence intent: {:?}",
        intents.intents
    );
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
    assert_refused_and_concealed(&operation, CUSTOM);
}

#[tokio::test]
async fn an_unapproved_origin_is_absent_from_dispatch_evidence() {
    let proposed =
        MemoryConfig::new().with_endpoint(TENANT, "gitlab", DEFAULT_SERVICE, "origin", CUSTOM);
    let operation = project("gitlab-user-get", proposed);
    let name = operation.spec().name;
    let mut registry = ToolRegistry::new();
    registry
        .try_register(Arc::new(operation))
        .expect("the projected operation registers");
    let permissions = PermissionManager::from_rules(std::slice::from_ref(&name), &[]);
    let workspace = Workspace::new(env!("CARGO_MANIFEST_DIR")).expect("the crate root exists");
    let executor = Executor::new(
        registry,
        permissions,
        Arc::new(AllowApprover),
        ToolContext::new(Arc::new(System::new(workspace))),
    );

    let _ = executor.dispatch(&name, json!({})).await;

    let evidence = executor.evidence();
    let call = evidence
        .all()
        .iter()
        .find(|observation| observation.kind == "tool_call")
        .expect("dispatch records tool_call evidence");
    assert!(
        !call.data.to_string().contains(CUSTOM),
        "an inert proposal reached dispatch evidence: {}",
        call.data
    );
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
fn replacing_an_approved_origin_creates_a_new_inert_proposal() {
    let config = MemoryConfig::new()
        .with_approved_endpoint(TENANT, "gitlab", DEFAULT_SERVICE, "origin", CUSTOM)
        .with_endpoint(TENANT, "gitlab", DEFAULT_SERVICE, "origin", REPLACEMENT);
    assert_refused_and_concealed(&project("gitlab-user-get", config), REPLACEMENT);
}

#[test]
fn revoking_an_origin_keeps_its_proposal_inert_and_concealed() {
    let config = MemoryConfig::new()
        .with_approved_endpoint(TENANT, "gitlab", DEFAULT_SERVICE, "origin", CUSTOM)
        .with_revoked_endpoint(TENANT, "gitlab", DEFAULT_SERVICE, "origin", CUSTOM);
    assert_refused_and_concealed(&project("gitlab-user-get", config), CUSTOM);
}

#[test]
fn approval_is_scoped_to_the_selected_connection_instance() {
    const APPROVED: &str = "0d3f79ae-b6df-4f77-8f77-438436c3b2ef";
    const PROPOSED: &str = "742a9808-a155-4243-99b3-6bf907dc8ad8";

    struct Instances;

    impl ConfigStore for Instances {
        fn get(
            &self,
            _tenant: &str,
            _provider: &str,
            _service: &str,
            _field: Field<'_>,
        ) -> Option<String> {
            None
        }

        fn resolve_for_instance(
            &self,
            _tenant: &str,
            _provider: &str,
            instance: Option<&InstanceId>,
            _service: &str,
            _field: Field<'_>,
        ) -> Option<ConfigValue> {
            match instance.map(InstanceId::as_str) {
                Some(APPROVED) => Some(ConfigValue::operator_approved(CUSTOM.to_owned())),
                Some(PROPOSED) => Some(ConfigValue::proposed(REPLACEMENT.to_owned())),
                _ => None,
            }
        }
    }

    let store = Arc::new(Instances);
    let approved = project_with(
        "gitlab-user-get",
        Credentials::for_instance(Arc::new(MemoryStore::new()), TENANT, APPROVED)
            .expect("valid credential binding"),
        Configuration::for_instance(store.clone(), TENANT, APPROVED)
            .expect("valid configuration binding"),
    );
    assert_eq!(
        approved
            .build_request(&json!({}))
            .expect("this exact instance is approved")
            .url,
        "https://gitlab.company.example:8443/api/v4/user"
    );

    let proposed = project_with(
        "gitlab-user-get",
        Credentials::for_instance(Arc::new(MemoryStore::new()), TENANT, PROPOSED)
            .expect("valid credential binding"),
        Configuration::for_instance(store, TENANT, PROPOSED).expect("valid configuration binding"),
    );
    assert_refused_and_concealed(&proposed, REPLACEMENT);
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
