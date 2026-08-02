//! Exact operations selected from Zendesk's full Ticketing OpenAPI document.
//!
//! This is deliberately a per-provider test. It loads `zendesk` by name and never walks
//! `providers/`: the operation set is a closed premise about this connector.

use std::path::Path;

use connector_spec::{sha256_hex, HttpMethod, Idempotency, Risk, DEFAULT_SERVICE};

#[path = "support/shipped_provider.rs"]
mod shipped_provider;

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
        "753452bcac1bcb16bb03eec70c55491d779b82f849c37803e25e0c28618263c0",
    ),
    (
        "zendesk-incremental-ticket-list",
        "a9bb1e84d935251f12378839916f726b6b1598b4c9e404a51125eea1778d5bfc",
    ),
    (
        "zendesk-incremental-user-list",
        "fdddb2e407569ffb8d370c6305f44908864ce8d6d1cb4aa6c5bfa222e019bea7",
    ),
    (
        "zendesk-incremental-organization-list",
        "c505ca40dd42432b33ff5253a5d15b7cd46415fa8bb81a77e330e0ba1938a02b",
    ),
    (
        "zendesk-incremental-ticket-event-list",
        "a51207b84c61a8d4d073f5757cf0e379ef41fa908167d6575a85ceb6d47ea171",
    ),
    (
        "zendesk-custom-object-list",
        "3dbbc14f46c868dd4258f458309d67f32549cfa284ff5e756dd50e6e5a9b5218",
    ),
];

const EXISTING_NAMED_SERVICE_OPERATION_FLUX: [(&str, &str); 14] = [
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
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-conversation-create.flux",
        "265e0bf13d0ef2bc5e5cbf6bcf6185844776d522889f786b8b71ad639ec5fc15",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-conversation-get.flux",
        "547483e221d9a2d3edcbf0cacddfc403ecf57ba317e502a656921956cb1226fc",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-conversation-update.flux",
        "c1380b8eddfc76055fa4ea3d9f9ba3f0e9ecd8599f5476d159e439902a8507ae",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-participant-list.flux",
        "4f7c6d0b3895979284c5d57cdcac51cb85757b4c2ce760af71859de2b469d1df",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-user-create.flux",
        "2237e85740de910dbb277d788411eccb3d7eedf084f6ef0cf02a967fc57f4d6c",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-user-get.flux",
        "bfba2a7c5b737374806f0963a155bd48c45ced40f9086dbcc3d3f6fc68f1acbf",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-messaging-user-update.flux",
        "5bc9372ab636ed67aa197e2e155c82303c16da696f2fe9f4eecd61593ba529db",
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
