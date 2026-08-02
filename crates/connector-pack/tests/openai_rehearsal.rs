//! C-472: the four OpenAPI-backed OpenAI reads compose before catalogue integration.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use connector_pack::{Configuration, MemoryConfig, Rehearsal, DEFAULT_USER_AGENT};
use serde_json::{json, Value};

const TENANT: &str = "openai-rehearsal";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn configuration() -> Configuration {
    Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a valid tenant")
}

#[test]
fn four_spec_backed_reads_compose_absolute_openai_requests() {
    let cases: [(&str, Value, &str); 4] = [
        (
            "openai-response-get",
            json!({"response_id": "resp_abc123"}),
            "https://api.openai.com/v1/responses/resp_abc123",
        ),
        (
            "openai-response-input-item-list",
            json!({"response_id": "resp_abc123", "limit": 25}),
            "https://api.openai.com/v1/responses/resp_abc123/input_items?limit=25",
        ),
        (
            "openai-file-list",
            json!({"limit": 25}),
            "https://api.openai.com/v1/files?limit=25",
        ),
        (
            "openai-batch-list",
            json!({"limit": 25}),
            "https://api.openai.com/v1/batches?limit=25",
        ),
    ];

    for (id, params, expected_url) in cases {
        let path = root().join(format!("crates/catalog/ops/openai/{id}.flux"));
        let flux = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let rehearsal = Rehearsal::of(id, "openai", "default", &flux)
            .unwrap_or_else(|error| panic!("{id} does not rehearse: {error}"));
        assert!(rehearsal.endpoint_variables().is_empty());

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
