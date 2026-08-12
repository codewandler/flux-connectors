//! C-469: the four OpenAPI-backed GitHub reads compose callable requests before catalogue integration.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use connector_pack::{Configuration, MemoryConfig, Rehearsal};
use serde_json::{json, Value};

const TENANT: &str = "github-rehearsal";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn configuration() -> Configuration {
    Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a valid tenant")
}

#[test]
fn the_four_spec_backed_reads_compose_absolute_github_requests() {
    let cases: [(&str, Value, &str); 4] = [
        (
            "github-issue-list",
            json!({"owner": "octocat", "repo": "Hello-World", "per_page": 25, "page": 2}),
            "https://api.github.com/repos/octocat/Hello-World/issues?page=2&per_page=25",
        ),
        (
            "github-pull-files-list",
            json!({"owner": "octocat", "repo": "Hello-World", "pull_number": 7, "per_page": 25, "page": 2}),
            "https://api.github.com/repos/octocat/Hello-World/pulls/7/files?page=2&per_page=25",
        ),
        (
            "github-workflow-run-list",
            json!({"owner": "octocat", "repo": "Hello-World", "per_page": 25, "page": 2}),
            "https://api.github.com/repos/octocat/Hello-World/actions/runs?page=2&per_page=25",
        ),
        (
            "github-commit-list",
            json!({"owner": "octocat", "repo": "Hello-World", "per_page": 25, "page": 2}),
            "https://api.github.com/repos/octocat/Hello-World/commits?page=2&per_page=25",
        ),
    ];

    for (id, params, expected_url) in cases {
        let path = root().join(format!("crates/catalog/ops/github/{id}.flux"));
        let flux = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let rehearsal = Rehearsal::of(id, "github", "default", &flux)
            .unwrap_or_else(|error| panic!("{id} does not rehearse: {error}"));
        assert!(rehearsal.endpoint_variables().is_empty());

        let request = rehearsal
            .request(&configuration(), &params)
            .unwrap_or_else(|error| panic!("{id} does not compose: {error}"));
        assert_eq!(request.method, "GET");
        assert_eq!(request.url, expected_url);
        assert_eq!(
            request.headers.get("Accept").map(String::as_str),
            Some("application/vnd.github+json")
        );
        assert!(request.body.is_none(), "{id} gained a body");
    }
}
