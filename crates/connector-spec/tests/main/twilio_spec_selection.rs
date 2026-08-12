//! C-473: Twilio's exact, read-only expansion from its pinned first-party OpenAPI document.
//!
//! This file names Twilio and its files explicitly. It never walks the provider directory: the
//! provider stories run in parallel, and another provider must not change this story's premise.

use std::path::{Path, PathBuf};

use connector_spec::{sha256_hex, HttpMethod, Idempotency, Risk, DEFAULT_SERVICE};

use crate::shipped_provider;

const SOURCE_COMMIT: &str = "97418cf0e4d6cf35b02333dd624090a8c62fa25d";
const UPSTREAM_SHA256: &str = "a6753266b8b05a201e8658734e332ee51d07a0913f2d419335d87bdb287643a2";

type OriginalFlux = (&'static str, &'static str);
type SelectedOperation = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
);

const ORIGINAL_FLUX: [OriginalFlux; 5] = [
    (
        "twilio-account-get",
        "e06f47b11aa9c11e74bd22d8210ee2c715e608fd2a47bd1d2e5c41fb999065b1",
    ),
    (
        "twilio-call-get",
        "070af6c41bab9c8bdfa8905f9a0d03752cf8a88808b7cb9ba9a68e81cdde3163",
    ),
    (
        "twilio-call-list",
        // C-30 moved query values from URL interpolation into `http.request(query: ...)`.
        "b723a2150a07df66d353b4e452bd28841dfa132ef33b7b84f054d5479a7a7a91",
    ),
    (
        "twilio-message-get",
        "2cba087910b0c12078470e6abc4ccee70c95af692df7d2ebb71e579d6c42d6a0",
    ),
    (
        "twilio-message-list",
        "e7301d737b2b2c899fe428af36cb066a20e084299b9cd040fa8b2699dcaa8dd8",
    ),
];

const SELECTED: [SelectedOperation; 4] = [
    (
        "ListRecording",
        "twilio-recording-list",
        "/Accounts/{AccountSid}/Recordings.json",
        &["IncludeSoftDeleted", "PageSize", "Page"],
        &[
            "DateCreated",
            "DateCreated<",
            "DateCreated>",
            "CallSid",
            "ConferenceSid",
            "PageToken",
        ],
    ),
    (
        "FetchRecording",
        "twilio-recording-get",
        "/Accounts/{AccountSid}/Recordings/{Sid}.json",
        &["IncludeSoftDeleted"],
        &[],
    ),
    (
        "ListUsageRecord",
        "twilio-usage-record-list",
        "/Accounts/{AccountSid}/Usage/Records.json",
        &["IncludeSubaccounts", "PageSize", "Page"],
        &["Category", "StartDate", "EndDate", "PageToken"],
    ),
    (
        "ListConference",
        "twilio-conference-list",
        "/Accounts/{AccountSid}/Conferences.json",
        &["PageSize", "Page"],
        &[
            "DateCreated",
            "DateCreated<",
            "DateCreated>",
            "DateUpdated",
            "DateUpdated<",
            "DateUpdated>",
            "FriendlyName",
            "Status",
            "PageToken",
        ],
    ),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_five_existing_flux_files_do_not_move() {
    for (id, expected) in ORIGINAL_FLUX {
        let path = root().join(format!("crates/catalog/ops/twilio/{id}.flux"));
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        assert_eq!(sha256_hex(&bytes), expected, "{} moved", path.display());
    }
}

#[test]
fn four_exact_selectors_are_the_only_additions() {
    let loaded = shipped_provider::load("twilio");
    assert!(
        loaded.patch.select.is_empty(),
        "Twilio must use exact operationId patches, never a selector sweep"
    );
    assert_eq!(loaded.patch.operations.len(), SELECTED.len());

    for ((select, rename, path, kept, omitted), patch) in
        SELECTED.iter().zip(&loaded.patch.operations)
    {
        assert_eq!(&patch.select, select);
        assert_eq!(patch.rename.as_deref(), Some(*rename));
        assert_eq!(patch.omit.path, ["AccountSid"]);
        assert_eq!(&patch.omit.query, omitted);

        let operation = loaded
            .connector
            .operation(rename)
            .unwrap_or_else(|| panic!("Twilio must publish {rename}"));
        assert_eq!(operation.method, HttpMethod::Get);
        assert_eq!(operation.path, *path);
        assert_eq!(operation.service, DEFAULT_SERVICE);
        assert_eq!(operation.risk, Risk::Low);
        assert_eq!(operation.idempotency, Idempotency::Idempotent);
        assert!(
            operation
                .params
                .path
                .iter()
                .all(|parameter| parameter.name != "AccountSid"),
            "{rename} exposes the operator-pinned AccountSid as a caller parameter"
        );
        let query: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect();
        assert_eq!(&query, kept, "{rename} widened its query surface");
        for parameter in &operation.params.query {
            assert!(
                matches!(
                    parameter.schema["type"].as_str(),
                    Some("integer" | "boolean")
                ),
                "{rename} retained an unsafe non-integer/non-boolean query: {}",
                parameter.name
            );
        }
        assert!(
            operation
                .response_schema
                .as_ref()
                .and_then(serde_json::Value::as_object)
                .is_some_and(|schema| !schema.is_empty()),
            "{rename} lost its official JSON response schema"
        );
    }

    assert_eq!(loaded.connector.operations.len(), 9);
    assert_eq!(
        shipped_provider::sources("twilio")
            .definition
            .matches("[[operations]]")
            .count(),
        5,
        "the original public operations remain inline"
    );
}

#[test]
fn extraction_is_pinned_scrubbed_reference_closed_licensed_and_path_normalized() {
    let selected_path = root().join("specs/twilio/selected-2026-08-02.openapi.json");
    let selected_bytes = std::fs::read(&selected_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", selected_path.display()));
    assert_eq!(
        sha256_hex(&selected_bytes),
        "2ab609f9daeddae92f339c6c8d646dae9058f8f11baa53fab12e98fd3f37700d"
    );
    let selected: serde_json::Value =
        serde_json::from_slice(&selected_bytes).expect("the Twilio extraction is JSON");
    assert_eq!(selected["openapi"], serde_json::json!("3.0.1"));
    assert_eq!(selected["info"]["version"], serde_json::json!("1.0.0"));
    assert_eq!(
        selected["info"]["license"]["name"],
        serde_json::json!("Apache 2.0")
    );
    assert_eq!(
        selected["servers"][0]["url"],
        serde_json::json!("https://api.twilio.com/2010-04-01")
    );
    assert_eq!(
        selected["paths"].as_object().map(|paths| paths.len()),
        Some(4)
    );
    assert_eq!(
        selected["components"]["securitySchemes"]["accountSid_authToken"]["scheme"],
        serde_json::json!("basic"),
        "the scrub must retain Twilio's security declaration"
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
    assert_scrubbed(&selected, "");

    for (source_operation, _, published_path, _, _) in SELECTED {
        let operation = &selected["paths"][published_path]["get"];
        assert_eq!(
            operation["operationId"],
            serde_json::json!(source_operation)
        );
        let source_path = operation["x-flux-source-path"]
            .as_str()
            .expect("normalization records the original source path");
        assert_eq!(
            format!("https://api.twilio.com{source_path}"),
            format!("https://api.twilio.com/2010-04-01{published_path}"),
            "{source_operation} moved during version-prefix normalization"
        );
        assert!(
            operation["responses"]["200"]["content"]["application/json"]["schema"]
                .as_object()
                .is_some_and(|schema| !schema.is_empty()),
            "{source_operation} lost its official 200 JSON response"
        );
    }

    let provenance_path = root().join("specs/twilio.provenance.toml");
    let provenance: toml::Table = std::fs::read_to_string(&provenance_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", provenance_path.display()))
        .parse()
        .unwrap_or_else(|error| panic!("{} is not TOML: {error}", provenance_path.display()));
    assert_eq!(provenance["source_commit"].as_str(), Some(SOURCE_COMMIT));
    assert_eq!(
        provenance["upstream_sha256"].as_str(),
        Some(UPSTREAM_SHA256)
    );
    assert_eq!(provenance["upstream_bytes"].as_integer(), Some(1_869_905));
    assert_eq!(provenance["document_license"].as_str(), Some("Apache-2.0"));
    assert_eq!(provenance["repository_license"].as_str(), Some("MIT"));
    assert_eq!(
        provenance["selected_operation_ids"]
            .as_array()
            .expect("the exact selectors are provenanced")
            .iter()
            .map(|value| value.as_str().expect("an operationId"))
            .collect::<Vec<_>>(),
        SELECTED.map(|(select, _, _, _, _)| select)
    );

    let upstream_path = provenance["scrubbed_path"]
        .as_str()
        .expect("the full scrubbed source path is provenanced");
    let upstream_bytes = std::fs::read(root().join(upstream_path))
        .unwrap_or_else(|error| panic!("cannot read {upstream_path}: {error}"));
    assert_eq!(
        provenance["scrubbed_sha256"].as_str(),
        Some(sha256_hex(&upstream_bytes).as_str())
    );
    let upstream: serde_json::Value =
        serde_json::from_slice(&upstream_bytes).expect("the full scrubbed source is JSON");
    assert_eq!(
        upstream["paths"].as_object().map(|paths| paths.len()),
        Some(121),
        "the full pinned source remains available as scrubbed drift evidence"
    );
    assert_scrubbed(&upstream, "");

    let license_path = provenance["repository_license_path"]
        .as_str()
        .expect("the repository license path is provenanced");
    let license_bytes = std::fs::read(root().join(license_path))
        .unwrap_or_else(|error| panic!("cannot read {license_path}: {error}"));
    assert_eq!(
        provenance["repository_license_sha256"].as_str(),
        Some(sha256_hex(&license_bytes).as_str())
    );
    assert!(
        std::str::from_utf8(&license_bytes)
            .expect("the repository license is UTF-8")
            .starts_with("MIT License"),
        "the first-party repository's MIT notice was not retained"
    );
}

#[test]
fn message_and_call_creation_remain_high_send_external_deferrals() {
    let source = shipped_provider::sources("twilio").definition;
    assert!(!source.contains("select = \"CreateMessage\""));
    assert!(!source.contains("select = \"CreateCall\""));
    assert!(source.contains("`risk = \"high\"`"));
    assert!(source.contains("`idempotency = \"non_idempotent\"`"));
    assert!(source.contains("`effects = [\"network\", \"send_external\"]`"));
    assert!(source.contains("structured form encoder"));
}
