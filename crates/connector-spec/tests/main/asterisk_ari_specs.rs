//! C-484: the official Asterisk 22.10.1 ARI Swagger bytes and their deterministic REST projection.
//!
//! This test is intentionally provider-independent. C-484 establishes source input for the existing
//! OpenAPI front-end; the provider and generated catalogue surface belong to later stories.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use connector_spec::openapi;
use connector_spec::sha256_hex;
use serde_json::Value;

const SOURCE_TAG: &str = "22.10.1";
const SOURCE_TAG_OBJECT: &str = "4f85d05889cf9fb9c9e2ae44cc3f4a825a74545a";
const SOURCE_COMMIT: &str = "f0e408a7b0d829c85bf15fa4b487870a50cb3000";
const SOURCE_REPOSITORY: &str = "https://github.com/asterisk/asterisk";
const DOCUMENTS: [&str; 11] = [
    "applications.json",
    "asterisk.json",
    "bridges.json",
    "channels.json",
    "deviceStates.json",
    "endpoints.json",
    "events.json",
    "mailboxes.json",
    "playbacks.json",
    "recordings.json",
    "sounds.json",
];
const HTTP_METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn spec_root() -> PathBuf {
    root().join("specs/asterisk")
}

fn json(path: &Path) -> Value {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("{} is not JSON: {error}", path.display()))
}

fn provenance() -> toml::Table {
    let path = spec_root().join("provenance.toml");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| {
            panic!(
                "absent Asterisk source contract at {}: {error}",
                path.display()
            )
        })
        .parse()
        .unwrap_or_else(|error| panic!("{} is not TOML: {error}", path.display()))
}

fn source_operations() -> BTreeMap<String, (String, String, String, Value)> {
    let mut operations = BTreeMap::new();
    for document_name in DOCUMENTS {
        let resource = document_name.trim_end_matches(".json");
        let document = json(&spec_root().join("api-docs").join(document_name));
        for api in document["apis"].as_array().expect("apis is an array") {
            let path = api["path"].as_str().expect("an API path");
            let path_description = api["description"].as_str().unwrap_or_default();
            for operation in api["operations"]
                .as_array()
                .expect("operations is an array")
            {
                let method = operation["httpMethod"]
                    .as_str()
                    .expect("an HTTP method")
                    .to_ascii_lowercase();
                let nickname = operation["nickname"].as_str().expect("a nickname");
                let id = format!("{resource}-{nickname}");
                assert!(
                    operations
                        .insert(
                            id.clone(),
                            (
                                path.to_owned(),
                                method,
                                path_description.to_owned(),
                                operation.clone(),
                            ),
                        )
                        .is_none(),
                    "duplicate source operation id {id}"
                );
            }
        }
    }
    operations
}

fn normalized_operations(document: &Value) -> BTreeMap<String, (String, String, Value)> {
    let mut operations = BTreeMap::new();
    for (path, item) in document["paths"].as_object().expect("paths is an object") {
        for method in HTTP_METHODS {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let id = operation["operationId"]
                .as_str()
                .expect("every normalized operation has an operationId")
                .to_owned();
            assert!(
                operations
                    .insert(
                        id.clone(),
                        (path.clone(), method.to_owned(), operation.clone())
                    )
                    .is_none(),
                "duplicate normalized operation id {id}"
            );
        }
    }
    operations
}

#[test]
fn the_official_asterisk_source_contract_is_vendored_byte_for_byte() {
    let provenance = provenance();
    assert_eq!(
        provenance["source_repository"].as_str(),
        Some(SOURCE_REPOSITORY)
    );
    assert_eq!(provenance["source_tag"].as_str(), Some(SOURCE_TAG));
    assert_eq!(
        provenance["source_tag_object"].as_str(),
        Some(SOURCE_TAG_OBJECT)
    );
    assert_eq!(provenance["source_commit"].as_str(), Some(SOURCE_COMMIT));
    assert_eq!(
        provenance["upstream_license"].as_str(),
        Some("GPL-2.0-only")
    );

    let source_files = provenance["source_files"]
        .as_array()
        .expect("provenance has [[source_files]] entries");
    let expected_paths = std::iter::once("COPYING".to_owned())
        .chain(std::iter::once("resources.json".to_owned()))
        .chain(DOCUMENTS.map(|name| format!("api-docs/{name}")))
        .collect::<BTreeSet<_>>();
    let recorded_paths = source_files
        .iter()
        .map(|entry| entry["path"].as_str().expect("a source path").to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(recorded_paths, expected_paths);
    assert_eq!(source_files.len(), expected_paths.len());

    for entry in source_files {
        let relative = entry["path"].as_str().expect("a source path");
        let bytes = std::fs::read(spec_root().join(relative))
            .unwrap_or_else(|error| panic!("cannot read vendored {relative}: {error}"));
        assert_eq!(entry["bytes"].as_integer(), Some(bytes.len() as i64));
        assert_eq!(entry["sha256"].as_str(), Some(sha256_hex(&bytes).as_str()));
    }
}

#[test]
fn normalization_accounts_for_109_source_operations_and_only_defers_the_websocket() {
    let source = source_operations();
    assert_eq!(source.len(), 109);
    let deferred = source
        .iter()
        .filter(|(_, (_, _, _, operation))| operation["upgrade"] == "websocket")
        .map(|(id, _)| id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(deferred, ["events-eventWebsocket"]);

    let normalized_path = spec_root().join("ari-22.10.1.openapi.json");
    let normalized_bytes = std::fs::read(&normalized_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", normalized_path.display()));
    let normalized: Value = serde_json::from_slice(&normalized_bytes)
        .unwrap_or_else(|error| panic!("{} is not JSON: {error}", normalized_path.display()));
    assert_eq!(normalized["openapi"], "3.0.3");
    assert_eq!(normalized["info"]["version"], SOURCE_TAG);
    assert_eq!(normalized["servers"][0]["url"], "http://localhost:8088/ari");
    assert_eq!(normalized["x-ari-source"]["commit"], SOURCE_COMMIT);
    assert_eq!(normalized["x-ari-source"]["tag"], SOURCE_TAG);

    let emitted = normalized_operations(&normalized);
    assert_eq!(emitted.len(), 108);
    assert!(!emitted.contains_key("events-eventWebsocket"));
    assert_eq!(
        normalized["components"]["schemas"]
            .as_object()
            .map(serde_json::Map::len),
        Some(85)
    );
    for (id, (source_path, source_method, path_description, source_operation)) in source {
        if id == "events-eventWebsocket" {
            continue;
        }
        let (path, method, operation) = emitted
            .get(&id)
            .unwrap_or_else(|| panic!("normalized document lost {id}"));
        assert_eq!(path, &source_path, "{id} moved paths");
        assert_eq!(method, &source_method, "{id} moved methods");
        assert_eq!(operation["x-ari-resource"], id.split_once('-').unwrap().0);
        assert_eq!(operation["x-ari-nickname"], source_operation["nickname"]);
        assert_eq!(operation["summary"], source_operation["summary"]);
        assert_eq!(operation["x-ari-path-description"], path_description);
        match source_operation.get("notes") {
            Some(notes) => {
                assert_eq!(operation["description"], *notes, "{id} moved its notes");
                assert_eq!(operation["x-ari-notes"], *notes, "{id} lost its raw notes");
            }
            None => {
                assert!(operation.get("description").is_none());
                assert!(operation.get("x-ari-notes").is_none());
            }
        }
        assert_eq!(
            operation["x-ari-response-class"], source_operation["responseClass"],
            "{id} moved its response model"
        );
        let response_class = source_operation["responseClass"]
            .as_str()
            .expect("a response class");
        let success = &operation["responses"]["200"];
        if response_class == "void" {
            assert!(
                success.get("content").is_none(),
                "{id} invented a void body"
            );
        } else if response_class == "binary" {
            assert_eq!(
                success["content"]["application/octet-stream"]["schema"],
                serde_json::json!({"type": "string", "format": "binary"})
            );
        } else if let Some(item) = response_class
            .strip_prefix("List[")
            .and_then(|item| item.strip_suffix(']'))
        {
            assert_eq!(
                success["content"]["application/json"]["schema"],
                serde_json::json!({
                    "type": "array",
                    "items": {"$ref": format!("#/components/schemas/{item}")}
                }),
                "{id} moved its list response model"
            );
        } else {
            assert_eq!(
                success["content"]["application/json"]["schema"]["$ref"],
                format!("#/components/schemas/{response_class}"),
                "{id} moved its response model reference"
            );
        }
        assert_eq!(
            operation["x-ari-source-parameters"],
            source_operation
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
            "{id} moved its parameter contract"
        );
    }

    let provenance = provenance();
    assert_eq!(provenance["source_operation_count"].as_integer(), Some(109));
    assert_eq!(provenance["rest_operation_count"].as_integer(), Some(108));
    assert_eq!(
        provenance["deferred_operation_ids"].as_array(),
        Some(&vec![toml::Value::String(
            "events-eventWebsocket".to_owned()
        )])
    );
    assert_eq!(
        provenance["normalized_sha256"].as_str(),
        Some(sha256_hex(&normalized_bytes).as_str())
    );
}

#[test]
fn the_normalizer_is_a_deterministic_fixed_point() {
    let script = root().join("scripts/normalize-asterisk-ari.py");
    let output = Command::new("python3")
        .arg(&script)
        .arg("--check")
        .output()
        .unwrap_or_else(|error| panic!("cannot run {}: {error}", script.display()));
    assert!(
        output.status.success(),
        "{} --check failed: {}",
        script.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_generated_event_declarations_are_a_deterministic_fixed_point() {
    let script = root().join("scripts/generate-asterisk-events.py");
    let output = Command::new("python3")
        .arg(&script)
        .arg("--check")
        .output()
        .unwrap_or_else(|error| panic!("cannot run {}: {error}", script.display()));
    assert!(
        output.status.success(),
        "{} --check failed: {}",
        script.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn all_108_normalized_rest_operations_reach_the_existing_openapi_front_end() {
    let path = spec_root().join("ari-22.10.1.openapi.json");
    let document = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let ingested = openapi::ingest(&document)
        .unwrap_or_else(|error| panic!("{} does not ingest: {error}", path.display()));
    assert_eq!(ingested.version, "3.0.3");
    assert_eq!(ingested.operations.len(), 108);
    assert!(
        ingested.diagnostics.is_empty(),
        "normalization left front-end diagnostics: {:?}",
        ingested.diagnostics
    );
}

#[test]
fn the_normalizer_refuses_unknown_versions_inventories_types_placements_and_identities() {
    let script = root().join("scripts/normalize-asterisk-ari.py");
    let output = Command::new("python3")
        .arg(&script)
        .arg("--self-test")
        .output()
        .unwrap_or_else(|error| panic!("cannot run {}: {error}", script.display()));
    assert!(
        output.status.success(),
        "{} --self-test failed: {}",
        script.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}
