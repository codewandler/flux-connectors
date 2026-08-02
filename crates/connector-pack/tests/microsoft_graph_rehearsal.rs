//! C-471: the four OpenAPI-backed Graph reads compose before whole-catalogue integration.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use connector_pack::{Configuration, MemoryConfig, Rehearsal, DEFAULT_USER_AGENT};
use serde_json::{json, Value};

const TENANT: &str = "microsoft-graph-rehearsal";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn configuration() -> Configuration {
    Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a valid tenant")
}

#[test]
fn four_spec_backed_reads_compose_absolute_graph_requests() {
    let cases: [(&str, &str, Value, &str); 4] = [
        (
            "mail",
            "microsoft_graph-mail-message-list",
            json!({"_top": 25, "_skip": 2}),
            "https://graph.microsoft.com/v1.0/me/messages?$top=25&$skip=2",
        ),
        (
            "calendar",
            "microsoft_graph-calendar-category-list",
            json!({"_top": 25, "_skip": 2}),
            "https://graph.microsoft.com/v1.0/me/outlook/masterCategories?$top=25&$skip=2",
        ),
        (
            "calendar",
            "microsoft_graph-calendar-time-zone-list",
            json!({"_top": 25, "_skip": 2}),
            "https://graph.microsoft.com/v1.0/me/outlook/supportedTimeZones()?$top=25&$skip=2",
        ),
        (
            "calendar",
            "microsoft_graph-calendar-language-list",
            json!({"_top": 25, "_skip": 2}),
            "https://graph.microsoft.com/v1.0/me/outlook/supportedLanguages()?$top=25&$skip=2",
        ),
    ];

    for (service, id, params, expected_url) in cases {
        let path = root().join(format!("crates/catalog/ops/microsoft_graph/{id}.flux"));
        let flux = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let rehearsal = Rehearsal::of(id, "microsoft_graph", service, &flux)
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
