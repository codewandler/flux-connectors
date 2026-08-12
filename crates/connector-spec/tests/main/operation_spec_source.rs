//! C-481: source provenance belongs to the selected operation, never to its service by inference.

use connector_spec::{provider, Connector, OperationSpecSource};

use crate::shipped_provider;

fn source<'a>(connector: &'a Connector, id: &str) -> Option<&'a OperationSpecSource> {
    connector.provenance.operation_specs.get(id)
}

fn expected(
    loaded: &connector_spec::LoadedProvider,
    service: &str,
    operation_id: &str,
) -> OperationSpecSource {
    let source = loaded
        .specs
        .iter()
        .find(|source| source.service() == service)
        .unwrap_or_else(|| panic!("service {service:?} has no vendored source"));
    OperationSpecSource {
        operation_id: operation_id.to_owned(),
        source_url: source.source_url.clone(),
        upstream_version: source.upstream_version.clone().expect("pinned version"),
        sha256: source.sha256.clone().expect("pinned committed hash"),
    }
}

#[test]
fn a_fully_inline_provider_has_no_derived_operation_source() {
    let loaded = shipped_provider::load("freshdesk");
    assert!(
        loaded.connector.provenance.operation_specs.is_empty(),
        "an inline provider cannot acquire provenance from its service or provider"
    );
}

#[test]
fn a_patch_selected_operation_retains_its_exact_vendor_source() {
    let loaded = shipped_provider::load("github");
    assert_eq!(
        source(&loaded.connector, "github-issue-list").cloned(),
        Some(expected(
            &loaded,
            connector_spec::DEFAULT_SERVICE,
            "issues/list-for-repo",
        ))
    );
    assert_eq!(
        source(&loaded.connector, "github-repo-get"),
        None,
        "an inline operation beside selected operations stays inline"
    );
}

#[test]
fn mixed_zendesk_services_classify_each_operation_instead_of_the_service() {
    let loaded = shipped_provider::load("zendesk");

    assert_eq!(
        source(&loaded.connector, "zendesk-test").cloned(),
        Some(expected(&loaded, "default", "ShowCurrentUser"))
    );
    assert_eq!(
        source(&loaded.connector, "zendesk-ticket-audit-list").cloned(),
        Some(expected(&loaded, "default", "ListAuditsForTicket"))
    );

    assert_eq!(
        source(&loaded.connector, "zendesk-messaging-message-create").cloned(),
        Some(expected(&loaded, "messaging", "PostMessage")),
        "the bounded recursive response remains traceable to the vendor operation"
    );
    assert_eq!(
        source(&loaded.connector, "zendesk-messaging-conversation-get").cloned(),
        Some(expected(&loaded, "messaging", "GetConversation"))
    );
}

#[test]
fn an_inline_operation_cannot_author_the_derived_marker() {
    let definition = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.test"

[[operations]]
id = "acme-thing-get"
method = "GET"
direction = "read"
path = "/v1/things"
description = "Get things"
risk = "low"
idempotency = "idempotent"
spec_source = { operation_id = "forged", source_url = "https://example.test/openapi.json", upstream_version = "v1", sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
"#;

    let error = provider::load("providers/acme.toml", definition)
        .expect_err("operation provenance is derived, not provider-authored")
        .to_string();
    assert!(error.contains("spec_source"), "{error}");
    assert!(error.contains("unknown field"), "{error}");
}
