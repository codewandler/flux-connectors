//! C-471's compatibility and selection fence for the Microsoft Graph OpenAPI expansion.
//!
//! This file names one provider and its files explicitly. It must never walk `providers/`: the
//! provider stories run in parallel, and a provider-specific assertion about the catalogue would
//! make another provider's disjoint change turn this story red.

use connector_spec::{HttpMethod, Idempotency, LoadedProvider, Risk};

#[path = "support/shipped_provider.rs"]
mod shipped_provider;

const DOCUMENTS: [(&str, &str); 2] = [
    ("mail", "specs/microsoft_graph/mail-2026-08-02.openapi.json"),
    (
        "calendar",
        "specs/microsoft_graph/calendar-2026-08-02.openapi.json",
    ),
];

const EXISTING_FLUX: [(&str, &str); 8] = [
    (
        "microsoft_graph-calendar-calendar-get.flux",
        "412213d36d8e0f5dee58e59c0aaa4c41d9c9db0e15f7030271444080f0f65285",
    ),
    (
        "microsoft_graph-calendar-event-create.flux",
        "8eb5af5bd1c98df7023fd6cd65cfde4a8500e4260c97dc868c77fdd74116f56c",
    ),
    (
        "microsoft_graph-calendar-event-get.flux",
        "4c9b9e030a7ca1c1e221a420a378f66989da4ed0796e9c59640a65371ba7838f",
    ),
    (
        "microsoft_graph-files-item-get.flux",
        "60bf6766f88e44762c82ded7867a482338a3bd6507de3ad04428b1f7e6b526d8",
    ),
    (
        "microsoft_graph-files-item-update.flux",
        "0373a662f297d0ef379f020df88471ba02a4d7d0f667724f53270b4c8c78f245",
    ),
    (
        "microsoft_graph-mail-folder-list.flux",
        "e42ea239a7c0d085512e0c027aa80f81aacd452a5f286854f40ba8d3d90216d7",
    ),
    (
        "microsoft_graph-mail-message-get.flux",
        "b010a7d1c9eddf59b961ec28467475681965b8b5c41e2f561e9035c05cc7dc3e",
    ),
    (
        "microsoft_graph-mail-message-reply.flux",
        "a604176356b9e320748d299e92261e81ae36f7e24f7380da0be0b1ecc2a0ac51",
    ),
];

const SELECTED: [(&str, &str, &str, &str, &str); 4] = [
    (
        "me.ListMessages",
        "microsoft_graph-mail-message-list",
        "mail",
        "/v1.0/me/messages",
        "Mail.Read",
    ),
    (
        "me.outlook.ListMasterCategories",
        "microsoft_graph-calendar-category-list",
        "calendar",
        "/v1.0/me/outlook/masterCategories",
        "MailboxSettings.Read",
    ),
    (
        "me.outlook.supportedTimeZones-5c4f",
        "microsoft_graph-calendar-time-zone-list",
        "calendar",
        "/v1.0/me/outlook/supportedTimeZones()",
        "MailboxSettings.Read",
    ),
    (
        "me.outlook.supportedLanguages",
        "microsoft_graph-calendar-language-list",
        "calendar",
        "/v1.0/me/outlook/supportedLanguages()",
        "MailboxSettings.Read",
    ),
];

fn load() -> LoadedProvider {
    shipped_provider::load("microsoft_graph")
}

#[test]
fn the_eight_existing_flux_files_do_not_move() {
    let root = shipped_provider::root().join("crates/catalog/ops/microsoft_graph");
    for (name, expected) in EXISTING_FLUX {
        let path = root.join(name);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        assert_eq!(
            connector_spec::sha256_hex(&bytes),
            expected,
            "the already-published Flux for {name} moved during the spec-backed expansion"
        );
    }
}

#[test]
fn two_reference_closed_documents_are_pinned_to_their_services() {
    let loaded = load();
    assert!(
        loaded.diagnostics().is_empty(),
        "the four selected operations must ingest without narrower diagnostics: {:?}",
        loaded.diagnostics()
    );
    let actual: Vec<(&str, &str)> = loaded
        .specs
        .iter()
        .map(|spec| (spec.service(), spec.path.as_str()))
        .collect();
    assert_eq!(actual, DOCUMENTS);

    for spec in &loaded.specs {
        let declared = spec
            .sha256
            .as_deref()
            .expect("each extracted document declares its vendored hash");
        let bytes = std::fs::read(shipped_provider::root().join(&spec.path))
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", spec.path));
        assert_eq!(declared, connector_spec::sha256_hex(&bytes));
    }
}

#[test]
fn provenance_keeps_the_complete_upstream_identity() {
    let path = shipped_provider::root().join("specs/microsoft_graph.provenance.toml");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let provenance: toml::Value =
        toml::from_str(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));

    assert_eq!(
        provenance["source_commit"].as_str(),
        Some("60b50e2e5b23612aac74ecdf65d35d566c5a4031")
    );
    assert_eq!(
        provenance["upstream_sha256"].as_str(),
        Some("2749e51f363a471cdaa4835493c2c57198aa834262666da39c03a2e7f9f9d831")
    );
    assert_eq!(provenance["upstream_bytes"].as_integer(), Some(38_050_122));

    let entries = provenance["spec"].as_array().expect("two spec entries");
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|entry| {
        entry["upstream_sha256"].as_str()
            == Some("2749e51f363a471cdaa4835493c2c57198aa834262666da39c03a2e7f9f9d831")
    }));
}

#[test]
fn four_exact_selectors_publish_only_the_frozen_reads() {
    let loaded = load();
    let patches: Vec<(&str, &str, &str)> = loaded
        .patch
        .operations
        .iter()
        .map(|patch| {
            (
                patch.select.as_str(),
                patch.rename.as_deref().expect("every public id is pinned"),
                patch
                    .service
                    .as_deref()
                    .expect("two documents require a service"),
            )
        })
        .collect();
    let expected: Vec<(&str, &str, &str)> = SELECTED
        .iter()
        .map(|(selector, public, service, _, _)| (*selector, *public, *service))
        .collect();
    assert_eq!(patches, expected, "selection widened or a selector moved");

    for (_, public, service, path, required_scope) in SELECTED {
        let operation = loaded
            .connector
            .operations
            .iter()
            .find(|operation| operation.id == public)
            .unwrap_or_else(|| panic!("{public} was not published"));
        assert_eq!(operation.service, service);
        assert_eq!(operation.method, HttpMethod::Get);
        assert_eq!(operation.path, path);
        assert_eq!(operation.risk, Risk::Low);
        assert_eq!(operation.idempotency, Idempotency::Idempotent);
        assert!(
            operation.description.contains(required_scope),
            "{public} must state its least-privilege Microsoft Graph permission scope"
        );
    }
}

#[test]
fn selected_odata_queries_are_integer_paging_only() {
    let loaded = load();
    for (_, public, _, _, _) in SELECTED {
        let operation = loaded
            .connector
            .operations
            .iter()
            .find(|operation| operation.id == public)
            .unwrap_or_else(|| panic!("{public} was not published"));
        let query: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        assert_eq!(
            query,
            ["$top", "$skip"],
            "{public} widened its OData query surface"
        );

        for param in operation.params.query.iter() {
            assert_eq!(
                param.schema.get("type").and_then(|value| value.as_str()),
                Some("integer"),
                "{} on {public} is not integer-shaped",
                param.name
            );
        }
    }
}

#[test]
fn extracted_paths_materialize_only_the_source_server_prefix() {
    for (_, path) in DOCUMENTS {
        let bytes = std::fs::read(shipped_provider::root().join(path))
            .unwrap_or_else(|error| panic!("cannot read {path}: {error}"));
        let document: serde_json::Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(
            document
                .pointer("/x-flux-source/server")
                .and_then(|v| v.as_str()),
            Some("https://graph.microsoft.com/v1.0")
        );
        let paths = document["paths"].as_object().expect("paths object");
        for (published_path, item) in paths {
            let source_path = item["x-flux-source-path"]
                .as_str()
                .expect("the extractor records the unprefixed source path");
            assert_eq!(published_path, &format!("/v1.0{source_path}"));
            assert_eq!(
                format!("https://graph.microsoft.com{published_path}"),
                format!("https://graph.microsoft.com/v1.0{source_path}")
            );
        }
    }
}
