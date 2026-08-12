//! C-464: all nine Zendesk Messaging selections compose within their named service.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use connector_pack::{Configuration, Error, MemoryConfig, Rehearsal, DEFAULT_USER_AGENT};
use serde_json::{json, Value};

const TENANT: &str = "zendesk-messaging-rehearsal";
const SERVICE: &str = "messaging";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn configuration() -> Configuration {
    let values = MemoryConfig::new()
        .with_endpoint(TENANT, "zendesk", SERVICE, "subdomain", "acme")
        .with_endpoint(TENANT, "zendesk", SERVICE, "appId", "app_123");
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant")
}

fn rehearsal(id: &str) -> Rehearsal {
    let path = root().join(format!("crates/catalog/ops/zendesk/{id}.flux"));
    let flux = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    Rehearsal::of(id, "zendesk", SERVICE, &flux)
        .unwrap_or_else(|error| panic!("{id} does not rehearse: {error}"))
}

#[test]
fn nine_messaging_operations_compose_absolute_requests_with_narrow_json_bodies() {
    let cases: [(&str, &str, Value, &str, Option<Value>); 9] = [
        (
            "zendesk-messaging-conversation-create",
            "POST",
            json!({}),
            "https://acme.zendesk.com/sc/v2/apps/app_123/conversations",
            Some(json!({"type": "sdkGroup"})),
        ),
        (
            "zendesk-messaging-conversation-get",
            "GET",
            json!({"conversationId": "conv_123"}),
            "https://acme.zendesk.com/sc/v2/apps/app_123/conversations/conv_123",
            None,
        ),
        (
            "zendesk-messaging-conversation-update",
            "PATCH",
            json!({"conversationId": "conv_123", "displayName": "Escalations"}),
            "https://acme.zendesk.com/sc/v2/apps/app_123/conversations/conv_123",
            Some(json!({"displayName": "Escalations"})),
        ),
        (
            "zendesk-messaging-participant-list",
            "GET",
            json!({"conversationId": "conv_123"}),
            "https://acme.zendesk.com/sc/v2/apps/app_123/conversations/conv_123/participants",
            None,
        ),
        (
            "zendesk-messaging-message-create",
            "POST",
            json!({
                "conversationId": "conv_123",
                "author": {"type": "business"},
                "content": {"type": "text", "text": "Hello"}
            }),
            "https://acme.zendesk.com/sc/v2/apps/app_123/conversations/conv_123/messages",
            Some(json!({
                "author": {"type": "business"},
                "content": {"type": "text", "text": "Hello"}
            })),
        ),
        (
            "zendesk-messaging-message-list",
            "GET",
            json!({"conversationId": "conv_123"}),
            "https://acme.zendesk.com/sc/v2/apps/app_123/conversations/conv_123/messages",
            None,
        ),
        (
            "zendesk-messaging-user-create",
            "POST",
            json!({"externalId": "customer-123"}),
            "https://acme.zendesk.com/sc/v2/apps/app_123/users",
            Some(json!({"externalId": "customer-123"})),
        ),
        (
            "zendesk-messaging-user-get",
            "GET",
            json!({"userIdOrExternalId": "user_123"}),
            "https://acme.zendesk.com/sc/v2/apps/app_123/users/user_123",
            None,
        ),
        (
            "zendesk-messaging-user-update",
            "PATCH",
            json!({"userIdOrExternalId": "user_123", "toBeRetained": true}),
            "https://acme.zendesk.com/sc/v2/apps/app_123/users/user_123",
            Some(json!({"toBeRetained": true})),
        ),
    ];

    for (id, method, params, expected_url, expected_body) in cases {
        let rehearsal = rehearsal(id);
        let variables = rehearsal.endpoint_variables();
        assert!(variables.iter().any(|variable| variable == "subdomain"));
        assert!(variables.iter().any(|variable| variable == "appId"));

        let request = rehearsal
            .request(&configuration(), &params)
            .unwrap_or_else(|error| panic!("{id} does not compose: {error}"));
        assert_eq!(request.method, method);
        assert_eq!(request.url, expected_url);
        assert!(!request.url.contains('{') && !request.url.contains('}'));
        assert_eq!(
            request.headers.get("User-Agent").map(String::as_str),
            Some(DEFAULT_USER_AGENT)
        );
        assert_eq!(
            request
                .body
                .as_deref()
                .map(serde_json::from_str::<Value>)
                .transpose()
                .unwrap_or_else(|error| panic!("{id} body is not JSON: {error}")),
            expected_body,
            "{id} widened or nested its request body"
        );
    }
}

#[test]
fn absolute_state_updates_replay_to_byte_identical_requests_without_claiming_cacheability() {
    for (id, params) in [
        (
            "zendesk-messaging-conversation-update",
            json!({"conversationId": "conv_123", "displayName": "Escalations"}),
        ),
        (
            "zendesk-messaging-user-update",
            json!({"userIdOrExternalId": "user_123", "toBeRetained": true}),
        ),
    ] {
        let rehearsal = rehearsal(id);
        let first = rehearsal
            .request(&configuration(), &params)
            .unwrap_or_else(|error| panic!("{id} first request: {error}"));
        let replay = rehearsal
            .request(&configuration(), &params)
            .unwrap_or_else(|error| panic!("{id} replay request: {error}"));
        assert_eq!(
            first, replay,
            "{id} is not an absolute target-state request"
        );
    }
}

#[test]
fn caller_owned_messaging_ids_cannot_escape_their_path_segment() {
    let cases = [
        (
            "zendesk-messaging-conversation-get",
            "conversationId",
            json!({"conversationId": "safe"}),
        ),
        (
            "zendesk-messaging-conversation-update",
            "conversationId",
            json!({"conversationId": "safe", "displayName": "Escalations"}),
        ),
        (
            "zendesk-messaging-participant-list",
            "conversationId",
            json!({"conversationId": "safe"}),
        ),
        (
            "zendesk-messaging-message-create",
            "conversationId",
            json!({
                "conversationId": "safe",
                "author": {"type": "business"},
                "content": {"type": "text", "text": "Hello"}
            }),
        ),
        (
            "zendesk-messaging-message-list",
            "conversationId",
            json!({"conversationId": "safe"}),
        ),
        (
            "zendesk-messaging-user-get",
            "userIdOrExternalId",
            json!({"userIdOrExternalId": "safe"}),
        ),
        (
            "zendesk-messaging-user-update",
            "userIdOrExternalId",
            json!({"userIdOrExternalId": "safe", "toBeRetained": true}),
        ),
    ];

    for (id, parameter, safe_params) in cases {
        for hostile in ["a/b", "a?b", "a#b"] {
            let mut params = safe_params.clone();
            params[parameter] = Value::String(hostile.to_owned());
            let error = rehearsal(id)
                .request(&configuration(), &params)
                .expect_err("a caller-owned id may not escape its path segment");
            assert!(
                matches!(
                    &error,
                    Error::UnsafePathParameter {
                        operation,
                        parameter: refused,
                        ..
                    } if operation == id && refused == parameter
                ),
                "{id}.{parameter}={hostile:?}: {error}"
            );
        }
    }
}
