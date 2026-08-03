//! C-470: Stripe's exact, read-only expansion from its pinned first-party OpenAPI document.
//!
//! This is provider-scoped: it loads Stripe by name and never walks the catalogue. The original
//! eight operations and their emitted bytes are a closed premise except where a later story records
//! an intentional metadata correction; the four list reads are the only new exposure C-470 permits.

use std::path::{Path, PathBuf};

use connector_spec::{sha256_hex, HttpMethod, Idempotency, Risk, DEFAULT_SERVICE};

#[path = "support/shipped_provider.rs"]
mod shipped_provider;

const SELECTED: [(&str, &str, &str, &[&str]); 4] = [
    (
        "GetCountrySpecs",
        "stripe-country-spec-list",
        "/v1/country_specs",
        &["ending_before", "expand", "starting_after"],
    ),
    (
        "GetEvents",
        "stripe-event-list",
        "/v1/events",
        &[
            "created",
            "delivery_success",
            "ending_before",
            "expand",
            "starting_after",
            "type",
            "types",
        ],
    ),
    (
        "GetExchangeRates",
        "stripe-exchange-rate-list",
        "/v1/exchange_rates",
        &["ending_before", "expand", "starting_after"],
    ),
    (
        "GetBillingMeters",
        "stripe-billing-meter-list",
        "/v1/billing/meters",
        &["ending_before", "expand", "starting_after", "status"],
    ),
];

const EXPANSION_DEFERRED: [(&str, &str); 4] = [
    ("GetCustomers", "/v1/customers"),
    ("GetPaymentIntents", "/v1/payment_intents"),
    ("GetInvoices", "/v1/invoices"),
    ("GetSubscriptions", "/v1/subscriptions"),
];

const ORIGINAL_FLUX: [(&str, &str); 8] = [
    (
        "stripe-balance-get",
        "8aa021012daa4a8b769971349c0fc32243ee9d79fd49262347323863e547d4c9",
    ),
    (
        "stripe-charge-get",
        "437bc81655909aae320492ec4f86b2904ea38fcd3a7bfd36d22f513186116d86",
    ),
    (
        "stripe-charge-refund-create",
        "cff7fc446cbca6d6847012af000ebf8f820c106a3212d40bc9af2c9ab0064cb1",
    ),
    (
        "stripe-customer-get",
        "4b6b2cc2595c26400bc1eb21c54b9e1a8dacbcdc6328c90dae3f1f9817ad4b76",
    ),
    (
        "stripe-payment-intent-cancel",
        "48b5090c48f8316befe576b62375f44e1ce702e20087d6bfb2f9c44ae2a0f290",
    ),
    (
        "stripe-payment-intent-capture",
        // C-155 raises capture to destructive because it now declares the `money` semantic effect.
        "5b06259e1bb1f2e4b85c34009f5d3b4618308857e30870d92ec0479e7c3bef27",
    ),
    (
        "stripe-payment-intent-get",
        "c395f7eb33782f26e7dc7a58f1b2357fedbb8ed4befd51f91fbbc4a248e2c60a",
    ),
    (
        "stripe-refund-get",
        "520942e8c3df9c6fdccc250d4d4e24c148cc60794bde8550bb4cbe3c8a21124e",
    ),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn exactly_four_frozen_operation_ids_join_the_eight_existing_operations() {
    let loaded = shipped_provider::load("stripe");
    assert!(
        loaded.patch.select.is_empty(),
        "Stripe must use exact operationId patches, never a selector sweep"
    );
    assert_eq!(loaded.patch.operations.len(), SELECTED.len());

    for ((select, rename, path, omitted), patch) in SELECTED.iter().zip(&loaded.patch.operations) {
        assert_eq!(&patch.select, select);
        assert_eq!(patch.rename.as_deref(), Some(*rename));
        assert_eq!(&patch.omit.query, omitted);
        let operation = loaded
            .connector
            .operation(rename)
            .unwrap_or_else(|| panic!("Stripe must publish {rename}"));
        assert_eq!(operation.method, HttpMethod::Get);
        assert_eq!(operation.path, *path);
        assert_eq!(operation.service, DEFAULT_SERVICE);
        assert_eq!(operation.risk, Risk::Low);
        assert_eq!(operation.idempotency, Idempotency::Idempotent);
        assert_eq!(operation.params.query.len(), 1);
        assert_eq!(operation.params.query[0].name, "limit");
        assert_eq!(
            operation.params.query[0].schema["type"],
            serde_json::json!("integer")
        );
        let response = operation
            .response_schema
            .as_ref()
            .unwrap_or_else(|| panic!("{rename} retains Stripe's 200 JSON response"));
        assert_eq!(response["type"], serde_json::json!("object"));
        assert_eq!(
            response["properties"]["data"]["type"],
            serde_json::json!("array")
        );
    }

    assert_eq!(
        loaded.connector.operations.len(),
        12,
        "the eight existing operations plus exactly four frozen reads ship"
    );
    assert_eq!(
        shipped_provider::sources("stripe")
            .definition
            .matches("[[operations]]")
            .count(),
        8,
        "the original operations remain inline rather than being transcribed from OpenAPI"
    );
}

#[test]
fn the_eight_inline_operations_remain_byte_pinned() {
    for (id, expected) in ORIGINAL_FLUX {
        let path = root().join(format!("crates/catalog/ops/stripe/{id}.flux"));
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        assert_eq!(sha256_hex(&bytes), expected, "{} moved", path.display());
    }
}

#[test]
fn the_vendored_document_is_pinned_and_selected_gets_have_only_the_declared_normalization() {
    let loaded = shipped_provider::load("stripe");
    assert_eq!(loaded.specs.len(), 1);
    let source = &loaded.specs[0];
    assert_eq!(source.path, "specs/stripe/openapi-2026-08-02.json");
    assert_eq!(
        source.source_url.as_deref(),
        Some(
            "https://raw.githubusercontent.com/stripe/openapi/8da624f9b4f65178eb2e2c2b6fc80162a6c0dceb/latest/openapi.spec3.json"
        )
    );
    assert_eq!(
        source.upstream_version.as_deref(),
        Some("2026-07-29.dahlia")
    );
    assert_eq!(source.fetched_at.as_deref(), Some("2026-08-02T11:12:55Z"));

    let bytes = std::fs::read(root().join(&source.path)).expect("the Stripe document is vendored");
    assert_eq!(source.sha256.as_deref(), Some(sha256_hex(&bytes).as_str()));
    let document: serde_json::Value =
        serde_json::from_slice(&bytes).expect("the vendored Stripe document is JSON");
    for (select, _, path, _) in SELECTED {
        let get = &document["paths"][path]["get"];
        assert_eq!(get["operationId"], serde_json::json!(select));
        assert!(
            get.get("requestBody").is_none(),
            "{select} must have only its semantically empty optional GET body removed"
        );
    }
    assert!(
        document["paths"]["/v1/customers"]["post"]
            .get("requestBody")
            .is_some(),
        "a non-GET body must survive the normalization"
    );
    assert!(
        document["components"]["securitySchemes"]["bearerAuth"].is_object(),
        "normalization must retain the security declaration"
    );

    let provenance_path = root().join("specs/stripe.provenance.toml");
    let provenance: toml::Table = std::fs::read_to_string(&provenance_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", provenance_path.display()))
        .parse()
        .unwrap_or_else(|error| panic!("{} is not TOML: {error}", provenance_path.display()));
    let entry = provenance["spec"][0]
        .as_table()
        .expect("Stripe provenance has one [[spec]] entry");
    assert_eq!(
        entry["source_commit"].as_str(),
        Some("8da624f9b4f65178eb2e2c2b6fc80162a6c0dceb")
    );
    assert_eq!(
        entry["upstream_sha256"].as_str(),
        Some("6f3623aece40493eec2f5e3e631219f8c6bffa4f477e3807a4bf785ad377f237")
    );
    assert_eq!(entry["sha256"].as_str(), source.sha256.as_deref());
    let normalization = provenance["normalization"][0]
        .as_table()
        .expect("Stripe provenance declares its normalization");
    assert_eq!(normalization["count"].as_integer(), Some(4));
    let normalized: Vec<&str> = normalization["operations"]
        .as_array()
        .expect("normalization operations are listed")
        .iter()
        .map(|value| value.as_str().expect("an operationId string"))
        .collect();
    assert_eq!(
        normalized,
        SELECTED.map(|(operation_id, _, _, _)| operation_id)
    );
}

#[test]
fn the_original_billing_lists_remain_explicitly_deferred_at_the_expansion_bound() {
    let path = root().join("specs/stripe/openapi-2026-08-02.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let document: serde_json::Value = serde_json::from_str(&text).expect("Stripe JSON");
    let ingested = connector_spec::openapi::ingest(&text).expect("Stripe OpenAPI parses");

    for (operation_id, path) in EXPANSION_DEFERRED {
        assert_eq!(
            document["paths"][path]["get"]["operationId"],
            serde_json::json!(operation_id),
            "the vendor operation remains in the pinned evidence"
        );
        assert!(
            ingested.operation(operation_id).is_none(),
            "{operation_id} must not be hand-copied around an ingest refusal"
        );
        assert!(
            ingested.diagnostics.iter().any(|diagnostic| {
                diagnostic.location == format!("GET {path}")
                    && diagnostic.problem.contains("more than 50000 nodes")
            }),
            "{operation_id} must remain deferred rather than bypassing the resolver's expansion bound"
        );
    }
}
