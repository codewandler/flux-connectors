//! C-465: Zendesk's Support Webhooks surface stays fail-closed.
//!
//! Zendesk publishes this family as prose, not in the first-party Ticketing OpenAPI document:
//!
//! - API contract: <https://developer.zendesk.com/api-reference/webhooks/webhooks-api/webhooks/>
//! - setup lifecycle: <https://developer.zendesk.com/documentation/webhooks/creating-and-monitoring-webhooks/>
//! - verification: <https://developer.zendesk.com/documentation/webhooks/verifying/>
//! - request headers: <https://developer.zendesk.com/documentation/webhooks/anatomy-of-a-webhook-request/>
//! - event vocabulary: <https://developer.zendesk.com/api-reference/webhooks/event-types/webhook-event-types/>
//!
//! This is provider-scoped evidence. It names Zendesk's provider, source, and generated metadata
//! directly and never walks `providers/`, so another connector landing in parallel cannot change
//! its premise.

use std::path::Path;

use connector_spec::{HttpMethod, Idempotency, Risk};
use serde_json::Value;

use crate::shipped_provider;

const TICKETING_OAS_URL: &str = "https://developer.zendesk.com/zendesk/oas.yaml";
const WEBHOOK_API_URL: &str =
    "https://developer.zendesk.com/api-reference/webhooks/webhooks-api/webhooks/";

#[derive(Clone, Copy)]
struct WithheldEndpoint {
    method: HttpMethod,
    path: &'static str,
    risk: Risk,
    idempotency: Idempotency,
}

// These are the five ordinary lifecycle requests documented by WEBHOOK_API_URL. They remain an
// accounting set, not connector operations: the generic Webhook response representation may carry
// `signing_secret`, and a narrowed response schema would not redact the raw result.
const ORDINARY_CRUD: [WithheldEndpoint; 5] = [
    WithheldEndpoint {
        method: HttpMethod::Get,
        path: "/api/v2/webhooks",
        risk: Risk::Low,
        idempotency: Idempotency::Idempotent,
    },
    WithheldEndpoint {
        method: HttpMethod::Get,
        path: "/api/v2/webhooks/{webhook_id}",
        risk: Risk::Low,
        idempotency: Idempotency::Idempotent,
    },
    WithheldEndpoint {
        method: HttpMethod::Post,
        path: "/api/v2/webhooks",
        risk: Risk::High,
        idempotency: Idempotency::NonIdempotent,
    },
    WithheldEndpoint {
        method: HttpMethod::Put,
        path: "/api/v2/webhooks/{webhook_id}",
        risk: Risk::High,
        idempotency: Idempotency::Conditional,
    },
    WithheldEndpoint {
        method: HttpMethod::Delete,
        path: "/api/v2/webhooks/{webhook_id}",
        risk: Risk::Destructive,
        // Zendesk documents 204 and 404 responses but no repeat guarantee. The final absent state
        // is not enough evidence to license an automatic retry.
        idempotency: Idempotency::NonIdempotent,
    },
];

// Both endpoints return the live HMAC credential. Reset returns a newly generated value; show
// returns the existing value. Neither is an ordinary connector result under C-430.
const SIGNING_SECRET_ENDPOINTS: [(HttpMethod, &str); 2] = [
    (
        HttpMethod::Get,
        "/api/v2/webhooks/{webhook_id}/signing_secret",
    ),
    (
        HttpMethod::Post,
        "/api/v2/webhooks/{webhook_id}/signing_secret",
    ),
];

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("connector-spec is two levels below the repository root")
}

#[test]
fn ticketing_openapi_declares_no_webhook_path() {
    let path = repo_root().join("specs/zendesk/ticketing-2026-08-02.openapi.yaml");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let document: Value = serde_norway::from_str(&source)
        .unwrap_or_else(|error| panic!("{} is not YAML: {error}", path.display()));
    let paths = document["paths"]
        .as_object()
        .unwrap_or_else(|| panic!("{} has no OpenAPI paths object", path.display()));
    let webhook_paths: Vec<_> = paths
        .keys()
        .filter(|candidate| candidate.starts_with("/api/v2/webhooks"))
        .collect();

    assert!(
        webhook_paths.is_empty(),
        "{TICKETING_OAS_URL} gained Webhooks paths {webhook_paths:?}; C-465's prose-only source and withholding decision must be reviewed"
    );
}

#[test]
fn five_crud_and_two_signing_secret_endpoints_remain_absent() {
    assert_eq!(ORDINARY_CRUD.len(), 5, "the reviewed CRUD accounting moved");
    assert_eq!(
        SIGNING_SECRET_ENDPOINTS.len(),
        2,
        "the credential-returning accounting moved"
    );

    let connector = shipped_provider::connector("zendesk");
    for endpoint in ORDINARY_CRUD {
        assert!(
            connector.operations.iter().all(|operation| {
                operation.method != endpoint.method || operation.path != endpoint.path
            }),
            "{:?} {} from {WEBHOOK_API_URL} must stay withheld ({:?}/{:?})",
            endpoint.method,
            endpoint.path,
            endpoint.risk,
            endpoint.idempotency,
        );
    }
    for (method, path) in SIGNING_SECRET_ENDPOINTS {
        assert!(
            connector
                .operations
                .iter()
                .all(|operation| operation.method != method || operation.path != path),
            "{method:?} {path} returns a live signing credential and must stay withheld under C-430"
        );
    }
}

#[test]
fn no_webhook_service_event_channel_or_generated_metadata_ships() {
    let connector = shipped_provider::connector("zendesk");
    assert!(
        connector.service("webhooks").is_none(),
        "C-465 must not publish an empty or partially usable Webhooks service"
    );
    assert!(
        connector.events.is_empty(),
        "Zendesk events wait for C-479's lossless wire discriminator"
    );
    assert!(
        connector.channels.is_empty(),
        "Zendesk channels wait for C-479 and C-480's complete verified setup lifecycle"
    );

    let generated_path = repo_root().join("crates/catalog/src/generated/zendesk.rs");
    let generated = std::fs::read_to_string(&generated_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", generated_path.display()));
    assert!(
        !generated.contains("name: \"webhooks\""),
        "{} publishes a withheld Webhooks service",
        generated_path.display()
    );
    for endpoint in ORDINARY_CRUD {
        assert!(
            !generated.contains(endpoint.path),
            "{} publishes withheld endpoint {:?} {}",
            generated_path.display(),
            endpoint.method,
            endpoint.path,
        );
    }
    for (method, path) in SIGNING_SECRET_ENDPOINTS {
        assert!(
            !generated.contains(path),
            "{} publishes credential-returning endpoint {method:?} {path}",
            generated_path.display()
        );
    }
}
