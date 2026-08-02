//! C-464: Zendesk Messaging is a named, spec-backed sibling with its own app credential.
//!
//! This is provider-scoped evidence. It names Zendesk's files and operations directly and never
//! walks `providers/`, so another provider landing in parallel cannot change its premise.

use std::path::Path;

use connector_spec::{sha256_hex, AuthScheme, HttpMethod, Idempotency, Risk, Tag, DEFAULT_SERVICE};
use serde_json::json;

#[path = "support/shipped_provider.rs"]
mod shipped_provider;

const SUPPORT_OPERATIONS: [&str; 13] = [
    "zendesk-test",
    "zendesk-ticket-search",
    "zendesk-ticket-show",
    "zendesk-ticket-comment-list",
    "zendesk-ticket-update",
    "zendesk-ticket-comment-add",
    "zendesk-ticket-tag-add",
    "zendesk-ticket-audit-list",
    "zendesk-incremental-ticket-list",
    "zendesk-incremental-user-list",
    "zendesk-incremental-organization-list",
    "zendesk-incremental-ticket-event-list",
    "zendesk-custom-object-list",
];

const HELP_CENTER_OPERATIONS: [&str; 7] = [
    "zendesk-help-center-category-list",
    "zendesk-help-center-section-list",
    "zendesk-help-center-article-list",
    "zendesk-help-center-article-get",
    "zendesk-help-center-translation-list",
    "zendesk-help-center-article-incremental-list",
    "zendesk-help-center-article-create",
];

const MESSAGING_OPERATIONS: [(&str, &str, HttpMethod, &str); 9] = [
    (
        "CreateConversation",
        "zendesk-messaging-conversation-create",
        HttpMethod::Post,
        "/v2/apps/{appId}/conversations",
    ),
    (
        "GetConversation",
        "zendesk-messaging-conversation-get",
        HttpMethod::Get,
        "/v2/apps/{appId}/conversations/{conversationId}",
    ),
    (
        "UpdateConversation",
        "zendesk-messaging-conversation-update",
        HttpMethod::Patch,
        "/v2/apps/{appId}/conversations/{conversationId}",
    ),
    (
        "ListParticipants",
        "zendesk-messaging-participant-list",
        HttpMethod::Get,
        "/v2/apps/{appId}/conversations/{conversationId}/participants",
    ),
    (
        "PostMessage",
        "zendesk-messaging-message-create",
        HttpMethod::Post,
        "/v2/apps/{appId}/conversations/{conversationId}/messages",
    ),
    (
        "ListMessages",
        "zendesk-messaging-message-list",
        HttpMethod::Get,
        "/v2/apps/{appId}/conversations/{conversationId}/messages",
    ),
    (
        "CreateUser",
        "zendesk-messaging-user-create",
        HttpMethod::Post,
        "/v2/apps/{appId}/users",
    ),
    (
        "GetUser",
        "zendesk-messaging-user-get",
        HttpMethod::Get,
        "/v2/apps/{appId}/users/{userIdOrExternalId}",
    ),
    (
        "UpdateUser",
        "zendesk-messaging-user-update",
        HttpMethod::Patch,
        "/v2/apps/{appId}/users/{userIdOrExternalId}",
    ),
];

const HELP_CENTER_HASHES: [(&str, &str); 9] = [
    (
        "connectors/zendesk-help-center.flux",
        "13f2a3f9581a2a4eaec25945e5fc6fd8636ddb555ed3e7311f0b5186f2e434fc",
    ),
    (
        "connectors/zendesk-help-center.connector.toml",
        "b3711bc0c88a0438b3052ca17b1092343f209a946490b34124186119a8e26448",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-article-create.flux",
        "e3adcac4ed72f06d04001665206b9fd831f692059bf259c15656d29677d6cd54",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-article-get.flux",
        "00248cff54d214e199be9e76a6b9509f6fbbd1561e416e3dfa3245de981f3841",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-article-incremental-list.flux",
        "604b37ccbccffb7a23f3d6f2106fd416a2d3f224b3c88b0df92c55f3c5e88281",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-article-list.flux",
        "74f0a40369f03cbd85e8e20e393eeb4b3c8f14eee48bbd6c792192891b9a7617",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-category-list.flux",
        "9ad6cc411ec63da741ee9f2bacdaa6a52ed1280161a5d566d0f8f7d3aa81087f",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-section-list.flux",
        "467263505558d010cd564573c34d15e62d434999b3364d9b401a2761eec6b7b3",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-translation-list.flux",
        "3928cc18c7846b22b0c12dcba61ac203c7c7f61f989e2f7b55f9d0d043acb2b0",
    ),
];

#[test]
fn recursive_message_responses_remain_an_explicit_mixed_front_end_boundary() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("specs/zendesk/messaging-2026-08-02.openapi.yaml");
    let source = std::fs::read_to_string(&path).expect("Messaging source");
    let ingested = connector_spec::openapi::ingest(&source).expect("Messaging ingests");
    let diagnostics = ingested
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    for expected in ["PostMessage", "ListMessages"] {
        assert!(
            ingested.operation(expected).is_none(),
            "{expected} became patch-selectable; replace its hand-authored bounded operation with an exact selection"
        );
    }
    assert!(
        diagnostics.contains("$ref` cycle: #/components/schemas/message")
            && diagnostics.contains("quotedMessageMessage")
            && diagnostics.contains("operation was skipped"),
        "the mixed-front-end rationale changed:\n{diagnostics}"
    );
}

#[test]
fn messaging_is_a_named_service_without_moving_support_or_help_center() {
    let loaded = shipped_provider::load("zendesk");
    let connector = &loaded.connector;
    assert_eq!(
        connector.service_names(),
        [DEFAULT_SERVICE, "help-center", "messaging"]
    );
    assert_eq!(connector.verify.as_deref(), Some("zendesk-test"));

    let messaging = connector.service("messaging").expect("Messaging service");
    assert_eq!(
        messaging.description,
        "Zendesk Messaging conversations, messages, participants and users"
    );
    assert_eq!(
        messaging.base_url.as_deref(),
        Some("https://{subdomain}.zendesk.com/sc")
    );
    assert_eq!(messaging.api_version.as_deref(), Some("v2"));
    assert_eq!(messaging.tags, [Tag::Messaging]);
    assert_eq!(
        connector.gid_of("messaging").map(|gid| gid.to_string()),
        Some("com.zendesk.api/messaging:v2".to_owned())
    );

    assert!(
        connector
            .operations_of(DEFAULT_SERVICE)
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>()
            .starts_with(&SUPPORT_OPERATIONS),
        "the pre-Messaging Support operation prefix moved"
    );
    assert_eq!(
        connector
            .operations_of("help-center")
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
        HELP_CENTER_OPERATIONS
    );

    for (_, id, method, path) in MESSAGING_OPERATIONS {
        let operation = connector.operation(id).expect("published Messaging op");
        assert_eq!(operation.service, "messaging");
        assert_eq!(operation.method, method);
        assert_eq!(operation.path, path);
        assert_eq!(
            connector.oip_of(operation).map(|oip| oip.to_string()),
            Some(format!("com.zendesk.api/messaging:v2#{id}"))
        );
        assert!(
            operation.response_schema.is_some(),
            "{id} lost its documented JSON response"
        );
    }
    let mut actual: Vec<_> = connector
        .operations_of("messaging")
        .map(|operation| operation.id.as_str())
        .collect();
    actual.sort_unstable();
    let mut expected: Vec<_> = MESSAGING_OPERATIONS
        .iter()
        .map(|(_, id, _, _)| *id)
        .collect();
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "Messaging widened beyond the approved nine"
    );
}

#[test]
fn nine_exact_operation_ids_are_selected_and_all_webhook_lifecycle_operations_stay_absent() {
    let loaded = shipped_provider::load("zendesk");
    let selected: Vec<_> = loaded
        .patch
        .operations
        .iter()
        .filter(|patch| patch.service.as_deref() == Some("messaging"))
        .collect();
    let patch_selected = MESSAGING_OPERATIONS
        .iter()
        .filter(|(select, _, _, _)| !matches!(*select, "PostMessage" | "ListMessages"));
    assert_eq!(selected.len(), 7);
    for (patch, (select, rename, _, _)) in selected.into_iter().zip(patch_selected) {
        assert_eq!(patch.select, *select);
        assert_eq!(patch.rename.as_deref(), Some(*rename));
        assert_eq!(patch.omit.path, ["appId"]);
    }

    for cycle_bound in ["PostMessage", "ListMessages"] {
        assert!(
            loaded
                .patch
                .operations
                .iter()
                .all(|patch| patch.select != cycle_bound),
            "{cycle_bound} cannot be selected while its response graph is recursive"
        );
    }

    for withheld in [
        "CreateWebhook",
        "ListWebhooks",
        "GetWebhook",
        "UpdateWebhook",
        "DeleteWebhook",
    ] {
        assert!(
            loaded
                .patch
                .operations
                .iter()
                .all(|patch| patch.select != withheld),
            "{withheld} must remain absent from the selected surface"
        );
    }

    let document = loaded
        .ingested_for("messaging")
        .expect("the pinned Messaging document");
    for credential_returning in [
        "CreateWebhook",
        "ListWebhooks",
        "GetWebhook",
        "UpdateWebhook",
    ] {
        let response = document
            .ingested
            .operation(credential_returning)
            .and_then(|operation| operation.response_schema.as_ref())
            .unwrap_or_else(|| panic!("{credential_returning} still has a response schema"));
        assert!(
            response.to_string().contains("secret"),
            "{credential_returning} no longer exposes the signing-secret field that justifies withholding it"
        );
    }
}

#[test]
fn auth_configuration_path_pins_and_queries_are_bounded() {
    let loaded = shipped_provider::load("zendesk");
    let connector = &loaded.connector;
    let config: Vec<_> = connector
        .config_of("messaging")
        .map(|field| (field.name.as_str(), field.binds.as_str(), field.secret))
        .collect();
    assert_eq!(
        config,
        [
            ("messaging_subdomain", "endpoint.subdomain", false),
            ("messaging_app_id", "path.appId", false),
            ("messaging_key_id", "username.zendesk.messaging_key", false,),
            (
                "messaging_key_secret",
                "credential.zendesk.messaging_key",
                true,
            ),
        ]
    );

    let auth = connector
        .auth_method("zendesk.messaging_key")
        .expect("app-scoped Basic key");
    assert_eq!(auth.scheme, AuthScheme::Basic);
    for operation in connector.operations_of("messaging") {
        let requirements = connector.effective_auth(operation);
        assert_eq!(requirements.len(), 1, "{} auth widened", operation.id);
        assert!(requirements[0].contains("zendesk.messaging_key"));
        assert!(
            operation
                .params
                .path
                .iter()
                .all(|parameter| parameter.name != "appId"),
            "{} exposed the operator-pinned appId",
            operation.id
        );
    }

    for id in [
        "zendesk-messaging-participant-list",
        "zendesk-messaging-message-list",
        "zendesk-messaging-user-get",
        "zendesk-messaging-user-update",
    ] {
        assert!(
            connector
                .operation(id)
                .expect("bounded query op")
                .params
                .query
                .is_empty(),
            "{id} retained an unencoded deep-object or brand query"
        );
    }

    let diagnostic = connector
        .operation("zendesk-messaging-conversation-get")
        .expect("Messaging diagnostic read");
    assert_eq!(diagnostic.risk, Risk::Low);
    assert_eq!(diagnostic.idempotency, Idempotency::Idempotent);
    assert_eq!(diagnostic.params.path[0].name, "conversationId");
}

fn body_names(operation: &connector_spec::Operation) -> Vec<&str> {
    operation
        .params
        .body
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect()
}

#[test]
fn every_write_has_a_nonvacuous_narrow_body_and_an_explicit_retry_contract() {
    let loaded = shipped_provider::load("zendesk");
    let connector = &loaded.connector;

    let create_conversation = connector
        .operation("zendesk-messaging-conversation-create")
        .expect("conversation create");
    assert_eq!(create_conversation.risk, Risk::High);
    assert_eq!(create_conversation.idempotency, Idempotency::NonIdempotent);
    assert_eq!(body_names(create_conversation), ["type"]);
    assert!(create_conversation.params.body[0].required);
    assert_eq!(
        create_conversation.params.body[0].schema,
        json!({"type": "string", "const": "sdkGroup"})
    );

    let update_conversation = connector
        .operation("zendesk-messaging-conversation-update")
        .expect("conversation update");
    assert_eq!(update_conversation.risk, Risk::Medium);
    assert_eq!(update_conversation.idempotency, Idempotency::NonIdempotent);
    assert_eq!(body_names(update_conversation), ["displayName"]);
    assert!(update_conversation.params.body[0].required);
    assert_eq!(
        update_conversation.params.body[0].schema,
        json!({"type": "string", "minLength": 1, "maxLength": 100})
    );

    let post_message = connector
        .operation("zendesk-messaging-message-create")
        .expect("message create");
    assert_eq!(post_message.risk, Risk::High);
    assert_eq!(post_message.idempotency, Idempotency::NonIdempotent);
    assert_eq!(body_names(post_message), ["author", "content"]);
    assert!(post_message
        .params
        .body
        .iter()
        .all(|parameter| parameter.required));

    let create_user = connector
        .operation("zendesk-messaging-user-create")
        .expect("user create");
    assert_eq!(create_user.risk, Risk::Medium);
    assert_eq!(create_user.idempotency, Idempotency::NonIdempotent);
    assert_eq!(body_names(create_user), ["externalId"]);
    assert!(create_user.params.body[0].required);

    let update_user = connector
        .operation("zendesk-messaging-user-update")
        .expect("user update");
    assert_eq!(update_user.risk, Risk::Medium);
    assert_eq!(update_user.idempotency, Idempotency::NonIdempotent);
    assert_eq!(body_names(update_user), ["toBeRetained"]);
    assert!(update_user.params.body[0].required);
    assert_eq!(
        update_user.params.body[0].schema,
        json!({"type": "boolean"})
    );
}

#[test]
fn every_existing_help_center_rendering_and_artifact_is_byte_identical() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (relative, expected) in HELP_CENTER_HASHES {
        let path = root.join(relative);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        assert_eq!(sha256_hex(&bytes), expected, "{} moved", path.display());
    }
}
