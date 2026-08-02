//! C-469: GitHub's four frozen OpenAPI selections join the five published inline operations.
//!
//! This is provider-scoped evidence: it loads `github` by name and never walks `providers/`.

use std::collections::BTreeMap;
use std::path::Path;

use connector_spec::{sha256_hex, HttpMethod, Idempotency, Risk, DEFAULT_SERVICE};

#[path = "support/shipped_provider.rs"]
mod shipped_provider;

const ORIGINAL_FLUX: [(&str, &str); 5] = [
    (
        "github-issue-comment-add",
        "b9e8d697628f0ac1b39ae6057d51d2e1b9e1f3a3e475ef90fc6a0d32897f8156",
    ),
    (
        "github-issue-create",
        "d196b03b41d4ebf5b7a833f9f2b00e93b94cba0ed35c001bd298222269015a00",
    ),
    (
        "github-issue-get",
        "47b2a8dc8932ca5af3109e8e70bc739d77e2614d43ec7417666530fccea4eb69",
    ),
    (
        "github-pull-get",
        "b6ed8367268345da6400ad8b6696ff155c82c608ee129108bffbca50bd216402",
    ),
    (
        "github-repo-get",
        "be3c3669255f3caa7e1131a7fee9c0c42407e24cdcacc2b8f78b64d8fbc21183",
    ),
];

const SELECTED: [(&str, &str, &str, &[&str]); 4] = [
    (
        "issues/list-for-repo",
        "github-issue-list",
        "/repos/{owner}/{repo}/issues",
        &[
            "milestone",
            "state",
            "assignee",
            "type",
            "creator",
            "mentioned",
            "issue_field_values",
            "labels",
            "sort",
            "direction",
            "since",
        ],
    ),
    (
        "pulls/list-files",
        "github-pull-files-list",
        "/repos/{owner}/{repo}/pulls/{pull_number}/files",
        &[],
    ),
    (
        "actions/list-workflow-runs-for-repo",
        "github-workflow-run-list",
        "/repos/{owner}/{repo}/actions/runs",
        &[
            "actor",
            "branch",
            "event",
            "status",
            "created",
            "exclude_pull_requests",
            "check_suite_id",
            "head_sha",
        ],
    ),
    (
        "repos/list-commits",
        "github-commit-list",
        "/repos/{owner}/{repo}/commits",
        &["sha", "path", "author", "committer", "since", "until"],
    ),
];

#[test]
fn the_five_published_operations_keep_their_flux_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (id, expected) in ORIGINAL_FLUX {
        let path = root.join(format!("crates/catalog/ops/github/{id}.flux"));
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        assert_eq!(sha256_hex(&bytes), expected, "{} moved", path.display());
    }
}

#[test]
fn exactly_four_operation_ids_are_selected_without_a_sweep() {
    let loaded = shipped_provider::load("github");
    assert!(
        loaded.patch.select.is_empty(),
        "GitHub must opt in one operationId at a time"
    );
    assert_eq!(loaded.patch.operations.len(), SELECTED.len());

    for ((select, rename, _, omitted), patch) in SELECTED.iter().zip(&loaded.patch.operations) {
        assert_eq!(&patch.select, select);
        assert_eq!(patch.rename.as_deref(), Some(*rename));
        assert_eq!(patch.omit.query, *omitted);
    }

    let actual: Vec<&str> = loaded
        .connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        actual,
        [
            "github-repo-get",
            "github-issue-get",
            "github-pull-get",
            "github-issue-create",
            "github-issue-comment-add",
            "github-issue-list",
            "github-pull-files-list",
            "github-workflow-run-list",
            "github-commit-list",
        ],
        "an unselected GitHub operation leaked into the connector"
    );
}

#[test]
fn the_four_reads_keep_only_integer_pagination_and_real_response_shapes() {
    let loaded = shipped_provider::load("github");
    let expected: BTreeMap<&str, &str> = SELECTED
        .iter()
        .map(|(_, rename, path, _)| (*rename, *path))
        .collect();

    for (id, path) in expected {
        let operation = loaded
            .connector
            .operation(id)
            .unwrap_or_else(|| panic!("GitHub must publish {id}"));
        assert_eq!(operation.method, HttpMethod::Get);
        assert_eq!(operation.path, path);
        assert_eq!(operation.service, DEFAULT_SERVICE);
        assert_eq!(operation.risk, Risk::Low);
        assert_eq!(operation.idempotency, Idempotency::Idempotent);
        assert!(
            operation.params.body.is_empty(),
            "{id} gained a request body"
        );

        let query: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|parameter| {
                assert!(
                    !parameter.required,
                    "{id}.{} became required",
                    parameter.name
                );
                assert_eq!(
                    parameter.schema["type"],
                    serde_json::json!("integer"),
                    "{id}.{} is not injection-safe integer pagination",
                    parameter.name
                );
                parameter.name.as_str()
            })
            .collect();
        assert_eq!(query, ["per_page", "page"]);
        assert!(
            operation.response_schema.is_some(),
            "{id} lost GitHub's documented 200 JSON response"
        );
    }

    let issue_list = loaded
        .connector
        .operation("github-issue-list")
        .expect("the issue list");
    assert_eq!(
        issue_list.response_schema.as_ref().expect("response")["type"],
        serde_json::json!("array")
    );
    let workflow_runs = loaded
        .connector
        .operation("github-workflow-run-list")
        .expect("the workflow-run list");
    assert_eq!(
        workflow_runs.response_schema.as_ref().expect("response")["properties"]["workflow_runs"]
            ["type"],
        serde_json::json!("array")
    );
}
