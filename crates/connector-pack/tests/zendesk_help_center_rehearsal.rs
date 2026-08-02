//! C-463: all seven Help Center selections compose inside their named service.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use connector_pack::{Configuration, MemoryConfig, Rehearsal, DEFAULT_USER_AGENT};
use serde_json::{json, Value};

const TENANT: &str = "zendesk-help-center-rehearsal";
const SERVICE: &str = "help-center";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn configuration() -> Configuration {
    let values = MemoryConfig::new().with_endpoint(TENANT, "zendesk", SERVICE, "subdomain", "acme");
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant")
}

#[test]
fn seven_help_center_operations_compose_absolute_requests_in_the_named_service() {
    let cases: [(&str, &str, Value, &str); 7] = [
        (
            "zendesk-help-center-category-list",
            "GET",
            json!({}),
            "https://acme.zendesk.com/api/v2/help_center/categories",
        ),
        (
            "zendesk-help-center-section-list",
            "GET",
            json!({}),
            "https://acme.zendesk.com/api/v2/help_center/sections",
        ),
        (
            "zendesk-help-center-article-list",
            "GET",
            json!({"start_time": 1_700_000_000}),
            "https://acme.zendesk.com/api/v2/help_center/articles?start_time=1700000000",
        ),
        (
            "zendesk-help-center-article-get",
            "GET",
            json!({"article_id": 360026053753_u64}),
            "https://acme.zendesk.com/api/v2/help_center/articles/360026053753",
        ),
        (
            "zendesk-help-center-translation-list",
            "GET",
            json!({"article_id": 360026053753_u64}),
            "https://acme.zendesk.com/api/v2/help_center/articles/360026053753/translations",
        ),
        (
            "zendesk-help-center-article-incremental-list",
            "GET",
            json!({"start_time": 1_700_000_000}),
            "https://acme.zendesk.com/api/v2/help_center/incremental/articles?start_time=1700000000",
        ),
        (
            "zendesk-help-center-article-create",
            "POST",
            json!({
                "section_id": 360004785313_u64,
                "article": {
                    "title": "Taking photos in low light",
                    "locale": "en-us",
                    "body": "Use a tripod"
                }
            }),
            "https://acme.zendesk.com/api/v2/help_center/sections/360004785313/articles",
        ),
    ];

    for (id, method, params, expected_url) in cases {
        let path = root().join(format!("crates/catalog/ops/zendesk/{id}.flux"));
        let flux = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let rehearsal = Rehearsal::of(id, "zendesk", SERVICE, &flux)
            .unwrap_or_else(|error| panic!("{id} does not rehearse: {error}"));
        assert_eq!(rehearsal.endpoint_variables(), ["subdomain"]);

        let request = rehearsal
            .request(&configuration(), &params)
            .unwrap_or_else(|error| panic!("{id} does not compose: {error}"));
        assert_eq!(request.method, method);
        assert_eq!(request.url, expected_url);
        assert!(!request.url.contains('{') && !request.url.contains('}'));
        if method == "POST" {
            assert_eq!(
                request.headers,
                [
                    ("content-type".to_owned(), "application/json".to_owned()),
                    ("User-Agent".to_owned(), DEFAULT_USER_AGENT.to_owned()),
                ]
                .into()
            );
            assert_eq!(
                request
                    .body
                    .as_deref()
                    .map(serde_json::from_str::<Value>)
                    .transpose()
                    .expect("the create body is JSON"),
                Some(json!({
                    "article": {
                        "title": "Taking photos in low light",
                        "locale": "en-us",
                        "body": "Use a tripod"
                    }
                }))
            );
        } else {
            assert_eq!(
                request.headers,
                [("User-Agent".to_owned(), DEFAULT_USER_AGENT.to_owned())].into()
            );
            assert!(request.body.is_none(), "{id} gained a body");
        }
    }
}
