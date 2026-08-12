//! Exact operations selected from Zendesk's full Ticketing OpenAPI document.
//!
//! This is deliberately a per-provider test. It loads `zendesk` by name and never walks
//! `providers/`: the operation set is a closed premise about this connector.

use std::path::Path;

use connector_spec::{sha256_hex, HttpMethod, Idempotency, Risk, DEFAULT_SERVICE};

use crate::shipped_provider;

#[test]
fn every_zendesk_operation_is_derived_from_a_vendored_spec() {
    let loaded = shipped_provider::load("zendesk");
    let inline: Vec<_> = loaded
        .connector
        .operations
        .iter()
        .filter(|operation| {
            !loaded
                .connector
                .provenance
                .operation_specs
                .contains_key(&operation.id)
        })
        .map(|operation| operation.id.as_str())
        .collect();
    assert!(
        inline.is_empty(),
        "Zendesk still has inline operations: {inline:?}"
    );
}

const OPERATIONS: [(&str, HttpMethod, &str); 19] = [
    (
        "zendesk-ticket-audit-list",
        HttpMethod::Get,
        "/api/v2/tickets/{ticket_id}/audits",
    ),
    (
        "zendesk-incremental-ticket-list",
        HttpMethod::Get,
        "/api/v2/incremental/tickets",
    ),
    (
        "zendesk-incremental-user-list",
        HttpMethod::Get,
        "/api/v2/incremental/users",
    ),
    (
        "zendesk-incremental-organization-list",
        HttpMethod::Get,
        "/api/v2/incremental/organizations",
    ),
    (
        "zendesk-incremental-ticket-event-list",
        HttpMethod::Get,
        "/api/v2/incremental/ticket_events",
    ),
    (
        "zendesk-custom-object-list",
        HttpMethod::Get,
        "/api/v2/custom_objects",
    ),
    (
        "zendesk-ticket-recent-list",
        HttpMethod::Get,
        "/api/v2/tickets/recent",
    ),
    (
        "zendesk-view-ticket-list",
        HttpMethod::Get,
        "/api/v2/views/{view_id}/tickets",
    ),
    (
        "zendesk-user-show",
        HttpMethod::Get,
        "/api/v2/users/{user_id}",
    ),
    (
        "zendesk-organization-show",
        HttpMethod::Get,
        "/api/v2/organizations/{organization_id}",
    ),
    ("zendesk-group-list", HttpMethod::Get, "/api/v2/groups"),
    (
        "zendesk-ticket-field-list",
        HttpMethod::Get,
        "/api/v2/ticket_fields",
    ),
    (
        "zendesk-ticket-form-list",
        HttpMethod::Get,
        "/api/v2/ticket_forms",
    ),
    (
        "zendesk-custom-status-list",
        HttpMethod::Get,
        "/api/v2/custom_statuses",
    ),
    ("zendesk-test", HttpMethod::Get, "/api/v2/users/me"),
    ("zendesk-ticket-search", HttpMethod::Get, "/api/v2/search"),
    (
        "zendesk-ticket-show",
        HttpMethod::Get,
        "/api/v2/tickets/{ticket_id}",
    ),
    (
        "zendesk-ticket-comment-list",
        HttpMethod::Get,
        "/api/v2/tickets/{ticket_id}/comments",
    ),
    (
        "zendesk-ticket-update",
        HttpMethod::Put,
        "/api/v2/tickets/{ticket_id}",
    ),
];

const OMITTED_QUERY: [&str; 7] = [
    "page",
    "sort",
    "include",
    "include_boundary_indicators",
    "include_item_cursors",
    "filter_events",
    "sort_order",
];

const EXISTING_SUPPORT_FLUX: [(&str, &str); 6] = [
    (
        "zendesk-ticket-audit-list",
        "d5ef74bd520652c0f846ab339b9ac3084e9f5f12b907bba5af4874f7a96c07cc",
    ),
    (
        "zendesk-incremental-ticket-list",
        "de20e4b1b870ef91647f84a4af6fa465b94879f8d6287b9d3679a9331d5ed78b",
    ),
    (
        "zendesk-incremental-user-list",
        "00dc61414b0de225d9c5bb9360e370dd5a4d724d1bf0780e8ff201fad3b5fc24",
    ),
    (
        "zendesk-incremental-organization-list",
        "609ffa9cab07b83458360408f5665eae27849f3beb791f16df253f4403acfa13",
    ),
    (
        "zendesk-incremental-ticket-event-list",
        "03124812f9b6b1bef8bf59f89e2d5d2576ffc656c53bb61b4bf6a3fe1117a767",
    ),
    (
        "zendesk-custom-object-list",
        "020b6d65f12e98e6d0aca93fbb97d6b7308bdee47c8915b2c9e670949bded9d5",
    ),
];

const EXISTING_NAMED_SERVICE_OPERATION_FLUX: [(&str, &str); 14] = [
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-article-create.flux",
        "8fbd63c007d2026bfa0d1426b3db49438d46a2166f620bc625785ffbbf98da12",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-article-get.flux",
        "eeaeb2aedb7ab36b3dd8f82f2d4a1319940c9b08420a039fb0a0c6b1a06f400e",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-article-incremental-list.flux",
        "e8cd4ace4ef9b76b26b3d27f38b8d84eee6dfa052698d2fc756e9b3070357679",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-article-list.flux",
        "7d9cbaa7a183a48f1082bc06491a6f29435ff37921e59129ad77b833b031cfa3",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-category-list.flux",
        "1a4a5a367c4f46c27448bdaf793df8af141f4d68f062d396bf894ecbc6e51bb9",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-section-list.flux",
        "8c9802c3e259b9ae7daa16c4dd6743d9f4e2028c7623bde7cb221e902f8245c6",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-help-center-translation-list.flux",
        "8e1906bf0049d34663bed04f1b86a4b2faf59c3218846ee840f6ba483506a2b7",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-conversation-create.flux",
        "56a273fe9cec8ef70073b347380f63632f632be55752bb31bc3ab574fcafca81",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-conversation-get.flux",
        "b06062a6e80b0a5fd3692473ced78ef22b2bae3e4b46dd9728e177d0260bbb89",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-conversation-update.flux",
        "67952f07853edfe6f5056e1e5e903a61fa3ea5b173134c191931c039922f73fb",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-participant-list.flux",
        "6d3b76a8d33af811f5d41a601e166225b64d1e3a4de2ef8e930eb1824ddd00a0",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-user-create.flux",
        "8c1325c28b406c24da273e89d4a9f70ca33bf294b643b80a0acf51a7ca35a8a9",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-user-get.flux",
        "aa46b10bd8de8fa95c966a15c4ea1d10998e221ddcf40dfa9bb90d730c1d3e3e",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-user-update.flux",
        "bf478b245f55dcbef6079d8a91ac2e6782312d2f82a74270c4b6c64010031200",
    ),
];

#[test]
fn nineteen_support_operations_are_selected_with_the_vendor_paths() {
    let loaded = shipped_provider::load("zendesk");
    let connector = &loaded.connector;
    let actual: Vec<&str> = connector
        .operations_of(DEFAULT_SERVICE)
        .map(|operation| operation.id.as_str())
        .collect();
    let expected: Vec<&str> = OPERATIONS.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(actual, expected, "the Zendesk operation set or order moved");

    for (id, method, path) in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("Zendesk must publish {id}"));
        assert_eq!(operation.method, method, "{id} changed method");
        assert_eq!(operation.path, path, "{id} does not carry the vendor path");
        assert_eq!(operation.service, DEFAULT_SERVICE, "{id} moved service");
        assert_eq!(
            connector
                .oip_of(operation)
                .unwrap_or_else(|| panic!("{id} must have an OIP"))
                .to_string(),
            format!("com.zendesk.api:v2#{id}"),
            "{id} has the wrong published address"
        );
    }

    for (id, source_id) in [
        ("zendesk-test", "ShowCurrentUser"),
        ("zendesk-ticket-search", "ListSearchResults"),
        ("zendesk-ticket-show", "ShowTicket"),
        ("zendesk-ticket-comment-list", "ListTicketComments"),
        ("zendesk-ticket-update", "UpdateTicket"),
    ] {
        assert_eq!(
            connector
                .provenance
                .operation_specs
                .get(id)
                .map(|source| source.operation_id.as_str()),
            Some(source_id),
            "{id} is not sourced from the official counterpart"
        );
    }
    for removed in ["zendesk-ticket-comment-add", "zendesk-ticket-tag-add"] {
        assert!(
            connector.operation(removed).is_none(),
            "the duplicate UpdateTicket alias {removed} survived"
        );
    }
}

#[test]
fn the_audit_read_is_selected_one_operation_id_at_a_time_and_is_query_free() {
    let loaded = shipped_provider::load("zendesk");
    assert!(
        loaded.patch.select.is_empty(),
        "no path or tag sweep is allowed"
    );
    let support_patches: Vec<_> = loaded
        .patch
        .operations
        .iter()
        .filter(|patch| patch.service.as_deref() == Some(DEFAULT_SERVICE))
        .collect();
    assert_eq!(
        support_patches.len(),
        19,
        "all nineteen Support operationIds are selected explicitly"
    );
    let patch = support_patches[0];
    assert_eq!(patch.select, "ListAuditsForTicket");
    assert_eq!(patch.rename.as_deref(), Some("zendesk-ticket-audit-list"));
    assert_eq!(patch.omit.query, OMITTED_QUERY);

    let spec = loaded
        .specs
        .iter()
        .find(|spec| spec.service() == DEFAULT_SERVICE)
        .expect("Ticketing remains the default service document");
    assert_eq!(spec.path, "specs/zendesk/ticketing-2026-08-02.openapi.yaml");
    assert_eq!(spec.service(), DEFAULT_SERVICE);
    assert_eq!(
        spec.source_url.as_deref(),
        Some("https://developer.zendesk.com/zendesk/oas.yaml")
    );
    assert_eq!(spec.upstream_version.as_deref(), Some("2.0.0"));
    assert_eq!(spec.fetched_at.as_deref(), Some("2026-08-02T08:47:26Z"));
    assert_eq!(
        spec.sha256.as_deref(),
        Some("338adeada0fc0e95bf1cea37320698e5a283fab9f48f496c28ab3bbac9ad0f88")
    );

    let audit = loaded
        .connector
        .operation("zendesk-ticket-audit-list")
        .expect("the selected audit operation");
    assert_eq!(audit.method, HttpMethod::Get);
    assert_eq!(audit.path, "/api/v2/tickets/{ticket_id}/audits");
    assert_eq!(audit.risk, Risk::Low);
    assert_eq!(audit.idempotency, Idempotency::Idempotent);
    assert!(
        audit.params.query.is_empty(),
        "the emitted URL must have no query"
    );
    assert_eq!(audit.params.path.len(), 1);
    let ticket_id = &audit.params.path[0];
    assert_eq!(ticket_id.name, "ticket_id");
    assert!(ticket_id.required);
    assert_eq!(ticket_id.schema["type"], serde_json::json!("integer"));
    assert_eq!(ticket_id.schema["format"], serde_json::json!("int64"));

    let response = audit
        .response_schema
        .as_ref()
        .expect("Zendesk documents the ticket audit response envelope");
    assert_eq!(response["type"], serde_json::json!("object"));
    assert_eq!(
        response["properties"]["audits"]["type"],
        serde_json::json!("array")
    );
}

#[test]
fn synchronization_selection_keeps_only_reviewed_integer_and_boolean_queries() {
    let loaded = shipped_provider::load("zendesk");
    let expected = [
        (
            "IncrementalTicketExportTime",
            "zendesk-incremental-ticket-list",
            &["start_time"][..],
            &["support_type_scope"][..],
        ),
        (
            "IncrementalUserExportTime",
            "zendesk-incremental-user-list",
            &["start_time", "per_page"][..],
            &[][..],
        ),
        (
            "IncrementalOrganizationExport",
            "zendesk-incremental-organization-list",
            &["start_time", "per_page"][..],
            &[][..],
        ),
        (
            "IncrementalTicketEvents",
            "zendesk-incremental-ticket-event-list",
            &["start_time"][..],
            &["include", "support_type_scope"][..],
        ),
        (
            "ListCustomObjects",
            "zendesk-custom-object-list",
            &["include_ui_path"][..],
            &[][..],
        ),
    ];

    for (select, published, kept, omitted) in expected {
        let patch = loaded
            .patch
            .operations
            .iter()
            .find(|patch| {
                patch.service.as_deref() == Some(DEFAULT_SERVICE) && patch.select == select
            })
            .unwrap_or_else(|| panic!("Zendesk must select {select} exactly"));
        assert_eq!(patch.rename.as_deref(), Some(published));
        assert_eq!(patch.omit.query, omitted);

        let operation = loaded
            .connector
            .operation(published)
            .unwrap_or_else(|| panic!("Zendesk must publish {published}"));
        assert_eq!(operation.risk, Risk::Low);
        assert_eq!(operation.idempotency, Idempotency::Idempotent);
        let query: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        assert_eq!(query, kept, "{published} widened its query contract");
        for parameter in &operation.params.query {
            assert!(
                matches!(
                    parameter.schema["type"].as_str(),
                    Some("integer" | "boolean")
                ),
                "{published} retained unsafe query parameter {}",
                parameter.name
            );
            assert_eq!(
                parameter.required,
                parameter.name == "start_time",
                "{published}.{} changed requiredness",
                parameter.name
            );
        }
        assert!(
            operation
                .response_schema
                .as_ref()
                .and_then(serde_json::Value::as_object)
                .is_some_and(|schema| !schema.is_empty()),
            "{published} lost the official response schema"
        );
    }

    let custom_objects = loaded
        .connector
        .operation("zendesk-custom-object-list")
        .expect("the custom-object definition read");
    assert!(
        custom_objects.params.path.is_empty(),
        "C-462 must not expose a caller-chosen custom-object key"
    );
}

#[test]
fn eight_support_foundation_reads_are_selected_exactly_and_query_free() {
    let loaded = shipped_provider::load("zendesk");
    let expected = [
        (
            "ListRecentTickets",
            "zendesk-ticket-recent-list",
            "/api/v2/tickets/recent",
            "tickets",
            &[][..],
        ),
        (
            "ListTicketsFromView",
            "zendesk-view-ticket-list",
            "/api/v2/views/{view_id}/tickets",
            "tickets",
            &["sort_by", "sort_order"][..],
        ),
        (
            "ShowUser",
            "zendesk-user-show",
            "/api/v2/users/{user_id}",
            "user",
            &["include"][..],
        ),
        (
            "ShowOrganization",
            "zendesk-organization-show",
            "/api/v2/organizations/{organization_id}",
            "organization",
            &[
                "include",
                "include_boundary_indicators",
                "include_item_cursors",
            ][..],
        ),
        (
            "ListGroups",
            "zendesk-group-list",
            "/api/v2/groups",
            "groups",
            &[
                "exclude_deleted",
                "include",
                "page",
                "per_page",
                "sort",
                "include_boundary_indicators",
                "include_item_cursors",
            ][..],
        ),
        (
            "ListTicketFields",
            "zendesk-ticket-field-list",
            "/api/v2/ticket_fields",
            "ticket_fields",
            &[
                "locale",
                "creator",
                "page",
                "sort",
                "include_boundary_indicators",
                "include_item_cursors",
            ][..],
        ),
        (
            "ListTicketForms",
            "zendesk-ticket-form-list",
            "/api/v2/ticket_forms",
            "ticket_forms",
            &[
                "active",
                "end_user_visible",
                "fallback_to_default",
                "form_type",
                "associated_to_brand",
                "page",
                "per_page",
                "sort",
                "include_boundary_indicators",
                "include_item_cursors",
                "locale",
            ][..],
        ),
        (
            "ListCustomStatuses",
            "zendesk-custom-status-list",
            "/api/v2/custom_statuses",
            "custom_statuses",
            &["status_categories", "active", "default"][..],
        ),
    ];

    for (select, published, path, envelope, omitted) in expected {
        let patch = loaded
            .patch
            .operations
            .iter()
            .find(|patch| {
                patch.service.as_deref() == Some(DEFAULT_SERVICE) && patch.select == select
            })
            .unwrap_or_else(|| panic!("Zendesk must select {select} exactly"));
        assert_eq!(patch.rename.as_deref(), Some(published));
        assert_eq!(patch.omit.query, omitted, "{select} omission drifted");

        let operation = loaded
            .connector
            .operation(published)
            .unwrap_or_else(|| panic!("Zendesk must publish {published}"));
        assert_eq!(operation.method, HttpMethod::Get);
        assert_eq!(operation.path, path);
        assert_eq!(operation.risk, Risk::Low);
        assert_eq!(operation.idempotency, Idempotency::Idempotent);
        assert!(
            operation.params.query.is_empty(),
            "{published} must stay query-free"
        );

        let response = operation
            .response_schema
            .as_ref()
            .unwrap_or_else(|| panic!("{published} keeps its pinned response envelope"));
        assert!(
            response["properties"].get(envelope).is_some(),
            "{published} lost response envelope member {envelope:?}: {response}"
        );
        assert!(
            response
                .get("required")
                .and_then(serde_json::Value::as_array)
                .is_none_or(Vec::is_empty),
            "{published} invented required response members absent from the pinned document"
        );
    }

    let view = loaded
        .connector
        .operation("zendesk-view-ticket-list")
        .expect("the built-in view read");
    assert_eq!(view.params.path.len(), 1);
    assert_eq!(view.params.path[0].name, "view_id");
    assert_eq!(
        view.params.path[0].schema,
        serde_json::json!({"type": "string", "enum": ["incoming", "my", "my_groups"]}),
        "the source integer-or-string union must not regress to uncallable Flux Any"
    );
}

#[test]
fn three_unrepresentable_support_writes_remain_negatively_accounted() {
    let loaded = shipped_provider::load("zendesk");
    for withheld in [
        "CreateTicket",
        "CreateOrUpdateOrganization",
        "CreateOrUpdateUser",
    ] {
        assert!(
            loaded
                .patch
                .operations
                .iter()
                .all(|patch| patch.select != withheld),
            "{withheld} must remain outside the selected Support surface"
        );
    }

    let document = loaded
        .ingested_for(DEFAULT_SERVICE)
        .expect("the pinned Ticketing document");
    let create_ticket = document
        .ingested
        .operation("CreateTicket")
        .expect("the pinned document still declares CreateTicket");
    assert!(
        create_ticket
            .params
            .header
            .iter()
            .all(|parameter| parameter.name != "Idempotency-Key"),
        "the pinned operation unexpectedly gained its prose-documented idempotency header"
    );
    assert!(
        create_ticket
            .params
            .body
            .iter()
            .find(|parameter| parameter.name == "ticket")
            .is_some_and(|parameter| !parameter.required),
        "CreateTicket's document gap changed: the ticket input is no longer optional"
    );
    let create_ticket_response = create_ticket
        .response_schema
        .as_ref()
        .expect("CreateTicket keeps its incomplete response");
    assert!(
        create_ticket_response["properties"].get("audit").is_none(),
        "CreateTicket's pinned response gained the prose-documented audit"
    );

    let organization = document
        .ingested
        .operation("CreateOrUpdateOrganization")
        .expect("the pinned document still declares CreateOrUpdateOrganization");
    assert!(
        organization.params.body.is_empty() && organization.params.body_schema.is_none(),
        "CreateOrUpdateOrganization gained the missing request body"
    );

    let user = document
        .ingested
        .operation("CreateOrUpdateUser")
        .expect("the pinned document still declares CreateOrUpdateUser");
    let user_union = &user
        .params
        .body
        .iter()
        .find(|parameter| parameter.name == "user")
        .expect("CreateOrUpdateUser keeps its required user body")
        .schema;
    let variants = user_union["anyOf"]
        .as_array()
        .expect("the user body remains a create-or-merge union");
    assert_eq!(variants.len(), 2);
    assert!(
        variants
            .iter()
            .all(|variant| variant["properties"].get("password").is_some()),
        "both user variants must keep exposing the nested password that blocks selection"
    );
    assert!(
        variants[1]
            .get("required")
            .and_then(serde_json::Value::as_array)
            .is_none_or(Vec::is_empty),
        "the merge variant gained a stable required identity"
    );
}

#[test]
fn unchanged_spec_backed_support_operation_flux_is_byte_identical() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (id, expected) in EXISTING_SUPPORT_FLUX {
        let path = root.join(format!("crates/catalog/ops/zendesk/{id}.flux"));
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        assert_eq!(sha256_hex(&bytes), expected, "{} moved", path.display());
    }
}

#[test]
fn help_center_and_messaging_operation_flux_is_byte_identical() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Service modules and manifests carry the provider-wide source hash and generator version;
    // both legitimately move for a sibling spec update or a release. The per-operation Flux is
    // the published request contract this regression test owns.
    for (relative, expected) in EXISTING_NAMED_SERVICE_OPERATION_FLUX {
        let path = root.join(relative);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        assert_eq!(sha256_hex(&bytes), expected, "{} moved", path.display());
    }
}
