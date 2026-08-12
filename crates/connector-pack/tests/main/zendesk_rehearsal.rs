//! The OpenAPI-backed Zendesk Support operations compose before catalogue integration.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use connector_pack::{Configuration, Error, MemoryConfig, Rehearsal, DEFAULT_USER_AGENT};
use serde_json::{json, Value};

const TENANT: &str = "zendesk-rehearsal";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn configuration() -> Configuration {
    let values =
        MemoryConfig::new().with_endpoint(TENANT, "zendesk", "default", "subdomain", "acme");
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant")
}

#[test]
fn thirteen_spec_backed_reads_compose_absolute_zendesk_requests() {
    let cases: [(&str, Value, &str); 13] = [
        (
            "zendesk-incremental-ticket-list",
            json!({"start_time": 1_700_000_000}),
            "https://acme.zendesk.com/api/v2/incremental/tickets?start_time=1700000000",
        ),
        (
            "zendesk-incremental-user-list",
            json!({"start_time": 1_700_000_000, "per_page": 100}),
            "https://acme.zendesk.com/api/v2/incremental/users?per_page=100&start_time=1700000000",
        ),
        (
            "zendesk-incremental-organization-list",
            json!({"start_time": 1_700_000_000, "per_page": 100}),
            "https://acme.zendesk.com/api/v2/incremental/organizations?per_page=100&start_time=1700000000",
        ),
        (
            "zendesk-incremental-ticket-event-list",
            json!({"start_time": 1_700_000_000}),
            "https://acme.zendesk.com/api/v2/incremental/ticket_events?start_time=1700000000",
        ),
        (
            "zendesk-custom-object-list",
            json!({"include_ui_path": true}),
            "https://acme.zendesk.com/api/v2/custom_objects?include_ui_path=true",
        ),
        (
            "zendesk-ticket-recent-list",
            json!({}),
            "https://acme.zendesk.com/api/v2/tickets/recent",
        ),
        (
            "zendesk-view-ticket-list",
            json!({"view_id": "incoming"}),
            "https://acme.zendesk.com/api/v2/views/incoming/tickets",
        ),
        (
            "zendesk-user-show",
            json!({"user_id": 35436}),
            "https://acme.zendesk.com/api/v2/users/35436",
        ),
        (
            "zendesk-organization-show",
            json!({"organization_id": 509974}),
            "https://acme.zendesk.com/api/v2/organizations/509974",
        ),
        (
            "zendesk-group-list",
            json!({}),
            "https://acme.zendesk.com/api/v2/groups",
        ),
        (
            "zendesk-ticket-field-list",
            json!({}),
            "https://acme.zendesk.com/api/v2/ticket_fields",
        ),
        (
            "zendesk-ticket-form-list",
            json!({}),
            "https://acme.zendesk.com/api/v2/ticket_forms",
        ),
        (
            "zendesk-custom-status-list",
            json!({}),
            "https://acme.zendesk.com/api/v2/custom_statuses",
        ),
    ];

    for (id, params, expected_url) in cases {
        let path = root().join(format!("crates/catalog/ops/zendesk/{id}.flux"));
        let flux = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let rehearsal = Rehearsal::of(id, "zendesk", "default", &flux)
            .unwrap_or_else(|error| panic!("{id} does not rehearse: {error}"));
        assert_eq!(rehearsal.endpoint_variables(), ["subdomain"]);

        let request = rehearsal
            .request(&configuration(), &params)
            .unwrap_or_else(|error| panic!("{id} does not compose: {error}"));
        assert_eq!(request.method, "GET");
        assert_eq!(request.url, expected_url);
        assert_eq!(
            request.headers,
            [("User-Agent".to_owned(), DEFAULT_USER_AGENT.to_owned())].into(),
            "{id} gained a header beyond the host identity"
        );
        assert!(request.body.is_none(), "{id} gained a body");
    }
}

#[test]
fn five_greenfield_support_replacements_compose_the_vendor_requests() {
    let cases: [(&str, &str, Value, &str, Option<Value>); 5] = [
        (
            "zendesk-test",
            "GET",
            json!({}),
            "https://acme.zendesk.com/api/v2/users/me",
            None,
        ),
        (
            "zendesk-ticket-search",
            "GET",
            json!({"query": "type:ticket"}),
            "https://acme.zendesk.com/api/v2/search?query=type%3Aticket",
            None,
        ),
        (
            "zendesk-ticket-show",
            "GET",
            json!({"ticket_id": 35436}),
            "https://acme.zendesk.com/api/v2/tickets/35436",
            None,
        ),
        (
            "zendesk-ticket-comment-list",
            "GET",
            json!({"ticket_id": 35436, "include_inline_images": false, "per_page": 100}),
            "https://acme.zendesk.com/api/v2/tickets/35436/comments?include_inline_images=false&per_page=100",
            None,
        ),
        (
            "zendesk-ticket-update",
            "PUT",
            json!({"ticket_id": 35436, "ticket": {"status": "open"}}),
            "https://acme.zendesk.com/api/v2/tickets/35436",
            Some(json!({"ticket": {"status": "open"}})),
        ),
    ];

    for (id, method, params, expected_url, expected_body) in cases {
        let path = root().join(format!("crates/catalog/ops/zendesk/{id}.flux"));
        let flux = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let rehearsal = Rehearsal::of(id, "zendesk", "default", &flux)
            .unwrap_or_else(|error| panic!("{id} does not rehearse: {error}"));
        let request = rehearsal
            .request(&configuration(), &params)
            .unwrap_or_else(|error| panic!("{id} does not compose: {error}"));
        assert_eq!(request.method, method);
        assert_eq!(request.url, expected_url);
        assert_eq!(
            request
                .body
                .as_deref()
                .map(serde_json::from_str::<Value>)
                .transpose()
                .unwrap_or_else(|error| panic!("{id} body is not JSON: {error}")),
            expected_body
        );
    }
}

#[test]
fn the_string_view_id_cannot_escape_its_path_segment() {
    let id = "zendesk-view-ticket-list";
    let path = root().join(format!("crates/catalog/ops/zendesk/{id}.flux"));
    let flux = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let rehearsal = Rehearsal::of(id, "zendesk", "default", &flux)
        .unwrap_or_else(|error| panic!("{id} does not rehearse: {error}"));

    for hostile in ["a/b", "a?b", "a#b", "a%b", "a\\b", "a b", ".", ".."] {
        let error = rehearsal
            .request(&configuration(), &json!({"view_id": hostile}))
            .expect_err("a caller-owned view id may not escape its path segment");
        assert!(
            matches!(
                &error,
                Error::UnsafePathParameter {
                    operation,
                    parameter,
                    ..
                } if operation == id && parameter == "view_id"
            ),
            "{id}.view_id={hostile:?}: {error}"
        );
    }
}
