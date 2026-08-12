//! C-472: OpenAI's exact, read-only expansion from its pinned first-party OpenAPI document.
//!
//! This file names OpenAI and its files explicitly. It never walks the provider directory: the
//! provider stories run in parallel, and another provider must not change this story's premise.

use std::path::{Path, PathBuf};

use connector_spec::{sha256_hex, HttpMethod, Idempotency, Risk, DEFAULT_SERVICE};

use crate::shipped_provider;

const SOURCE_COMMIT: &str = "117ce5680e4269f6656a4fd70d28f9755630d938";
const UPSTREAM_SHA256: &str = "ef36175ba6181b9d725a16b1eedcaa75a8a1268124896b10ccc5d9ddf0d543d3";

type SelectedOperation = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
);

const SELECTED: [SelectedOperation; 4] = [
    (
        "getResponse",
        "openai-response-get",
        "/v1/responses/{response_id}",
        &[],
        &["include", "stream", "starting_after", "include_obfuscation"],
    ),
    (
        "listInputItems",
        "openai-response-input-item-list",
        "/v1/responses/{response_id}/input_items",
        &["limit"],
        &["order", "after", "include"],
    ),
    (
        "listFiles",
        "openai-file-list",
        "/v1/files",
        &["limit"],
        &["purpose", "order", "after"],
    ),
    (
        "listBatches",
        "openai-batch-list",
        "/v1/batches",
        &["limit"],
        &["after"],
    ),
];

const ORIGINAL_FLUX: [(&str, &str); 4] = [
    (
        "openai-chat-completion",
        "7a1bb7ce50d457c4c2a758a0cf6b1f9af1ed559fcec5378054794efc644301ba",
    ),
    (
        "openai-embeddings-create",
        "1a593c975eaa14c1e974f381a5970b20ad354625d2e54ae7a522af317c2081b6",
    ),
    (
        "openai-model-get",
        "a2f4bd9f09c3d95fee8c574ec9921ff5d8366de11b8855b13fb15ce797267130",
    ),
    (
        "openai-models-list",
        "e41a013e728c2d49c1508ea9ffcb5816ce177a747e5c394e42d6addced4a892b",
    ),
];

const RESPONSE_REFS: [&str; 4] = [
    "#/components/schemas/Response",
    "#/components/schemas/ResponseItemList",
    "#/components/schemas/ListFilesResponse",
    "#/components/schemas/ListBatchesResponse",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_four_existing_flux_files_do_not_move() {
    for (id, expected) in ORIGINAL_FLUX {
        let path = root().join(format!("crates/catalog/ops/openai/{id}.flux"));
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        assert_eq!(sha256_hex(&bytes), expected, "{} moved", path.display());
    }
}

#[test]
fn four_exact_selectors_are_the_only_additions() {
    let loaded = shipped_provider::load("openai");
    assert!(
        loaded.patch.select.is_empty(),
        "OpenAI must use exact operationId patches, never a selector sweep"
    );
    assert_eq!(loaded.patch.operations.len(), SELECTED.len());

    for ((select, rename, path, kept, omitted), patch) in
        SELECTED.iter().zip(&loaded.patch.operations)
    {
        assert_eq!(&patch.select, select);
        assert_eq!(patch.rename.as_deref(), Some(*rename));
        assert_eq!(&patch.omit.query, omitted);

        let operation = loaded
            .connector
            .operation(rename)
            .unwrap_or_else(|| panic!("OpenAI must publish {rename}"));
        assert_eq!(operation.method, HttpMethod::Get);
        assert_eq!(operation.path, *path);
        assert_eq!(operation.service, DEFAULT_SERVICE);
        assert_eq!(operation.risk, Risk::Low);
        assert_eq!(operation.idempotency, Idempotency::Idempotent);
        let query: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect();
        assert_eq!(&query, kept, "{rename} widened its query surface");
        for parameter in &operation.params.query {
            assert_eq!(parameter.schema["type"], serde_json::json!("integer"));
            assert_eq!(parameter.name, "limit");
        }
        let response = operation
            .response_schema
            .as_ref()
            .unwrap_or_else(|| panic!("{rename} retains OpenAI's official JSON response"));
        assert!(
            response
                .as_object()
                .is_some_and(|schema| !schema.is_empty()),
            "{rename} lost its official JSON response schema"
        );
    }

    assert_eq!(loaded.connector.operations.len(), 8);
    assert_eq!(
        shipped_provider::sources("openai")
            .definition
            .matches("[[operations]]")
            .count(),
        4,
        "the original public operations remain inline"
    );
}

#[test]
fn extraction_is_pinned_scrubbed_reference_closed_and_path_normalized() {
    let loaded = shipped_provider::load("openai");
    assert!(
        loaded.diagnostics().is_empty(),
        "the four exact selectors must ingest without diagnostics: {:?}",
        loaded.diagnostics()
    );
    assert_eq!(loaded.specs.len(), 1);
    let source = &loaded.specs[0];
    assert_eq!(source.path, "specs/openai/selected-2026-08-02.openapi.json");
    assert_eq!(
        source.source_url.as_deref(),
        Some(
            "https://raw.githubusercontent.com/openai/openai-openapi/117ce5680e4269f6656a4fd70d28f9755630d938/openapi.json"
        )
    );
    assert_eq!(source.upstream_version.as_deref(), Some("2.3.0"));
    assert_eq!(source.fetched_at.as_deref(), Some("2026-08-02T11:28:56Z"));

    let bytes =
        std::fs::read(root().join(&source.path)).expect("the OpenAI extraction is vendored");
    assert_eq!(source.sha256.as_deref(), Some(sha256_hex(&bytes).as_str()));
    let document: serde_json::Value =
        serde_json::from_slice(&bytes).expect("the OpenAI extraction is JSON");
    assert_eq!(document["openapi"], serde_json::json!("3.1.0"));
    assert_eq!(
        document["info"]["license"]["identifier"],
        serde_json::json!("MIT")
    );
    assert_eq!(
        document["servers"][0]["url"],
        serde_json::json!("https://api.openai.com")
    );
    assert_eq!(
        document["paths"].as_object().map(|paths| paths.len()),
        Some(4)
    );

    fn assert_scrubbed(value: &serde_json::Value, at: &str) {
        match value {
            serde_json::Value::Object(object) => {
                assert!(!object.contains_key("example"), "example survived at {at}");
                assert!(
                    !object.contains_key("examples"),
                    "examples survived at {at}"
                );
                for (name, child) in object {
                    assert_scrubbed(child, &format!("{at}/{name}"));
                }
            }
            serde_json::Value::Array(array) => {
                for (index, child) in array.iter().enumerate() {
                    assert_scrubbed(child, &format!("{at}/{index}"));
                }
            }
            _ => {}
        }
    }
    assert_scrubbed(&document, "");

    for ((select, _, published_path, _, _), response_ref) in SELECTED.into_iter().zip(RESPONSE_REFS)
    {
        let operation = &document["paths"][published_path]["get"];
        assert_eq!(operation["operationId"], serde_json::json!(select));
        assert_eq!(
            operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            serde_json::json!(response_ref),
            "{select} did not retain its first-party JSON response reference"
        );
        let source_path = operation["x-flux-source-path"]
            .as_str()
            .expect("normalization records the original source path");
        assert_eq!(
            format!("https://api.openai.com/v1{source_path}"),
            format!("https://api.openai.com{published_path}")
        );
    }

    let provenance_path = root().join("specs/openai.provenance.toml");
    let provenance: toml::Table = std::fs::read_to_string(&provenance_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", provenance_path.display()))
        .parse()
        .unwrap_or_else(|error| panic!("{} is not TOML: {error}", provenance_path.display()));
    assert_eq!(provenance["source_commit"].as_str(), Some(SOURCE_COMMIT));
    assert_eq!(
        provenance["upstream_sha256"].as_str(),
        Some(UPSTREAM_SHA256)
    );
    assert_eq!(provenance["upstream_bytes"].as_integer(), Some(3_244_309));
    let scrubbed_path = provenance["scrubbed_path"]
        .as_str()
        .expect("the full scrubbed source path is provenanced");
    let scrubbed_bytes = std::fs::read(root().join(scrubbed_path))
        .unwrap_or_else(|error| panic!("cannot read {scrubbed_path}: {error}"));
    assert_eq!(
        provenance["scrubbed_sha256"].as_str(),
        Some(sha256_hex(&scrubbed_bytes).as_str())
    );
    let scrubbed: serde_json::Value =
        serde_json::from_slice(&scrubbed_bytes).expect("the scrubbed full source is JSON");
    assert_eq!(
        scrubbed["paths"].as_object().map(|paths| paths.len()),
        Some(182),
        "the full pinned source remains available as scrubbed drift evidence"
    );
    assert_scrubbed(&scrubbed, "");
    assert_eq!(
        provenance["selected_operation_ids"]
            .as_array()
            .expect("the exact selectors are provenanced")
            .iter()
            .map(|value| value.as_str().expect("an operationId"))
            .collect::<Vec<_>>(),
        SELECTED.map(|(select, _, _, _, _)| select)
    );
    assert_eq!(
        provenance["extraction_sha256"].as_str(),
        source.sha256.as_deref()
    );
}
