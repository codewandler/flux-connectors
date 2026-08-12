//! C-469: GitHub's four frozen OpenAPI selections join the five published inline operations.
//!
//! This is provider-scoped evidence: it loads `github` by name and never walks `providers/`.

use std::collections::BTreeMap;
use std::path::Path;

use connector_spec::{sha256_hex, HttpMethod, Idempotency, Risk, DEFAULT_SERVICE};

use crate::shipped_provider;

const ORIGINAL_FLUX: [(&str, &str); 5] = [
    (
        "github-issue-comment-add",
        "6969b58893a080709aa1156e6960f51e8975c41d8d8a707fde91b9625c4ef477",
    ),
    (
        "github-issue-create",
        "c5b70072239b1294fd848493fb6bfe4f7cf8b3b902ed6eca1c26348b7850557a",
    ),
    (
        "github-issue-get",
        "f7d82d967f76e40a38aa22f9d873a8d5bf4e7adf5c515694a9ec4e73b5093a7c",
    ),
    (
        "github-pull-get",
        "f452b010aa9e9805bf1373a7ef3d0b03b471b2e1207dba41824dd37b359246e1",
    ),
    (
        "github-repo-get",
        "8ba138bd9e2a05ad12dbc3cb2ca6601cbb43b009913a77d8dac6fc987db21680",
    ),
];

const SELECTED: [(&str, &str, &str, &[&str]); 8] = [
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
    // The four discovery reads (C-527). They omit far less than the four above, and that is the
    // point rather than an inconsistency: the omissions above are frozen request bytes, while these
    // are new operations selected after C-30 made a scalar string query safe to carry.
    ("users/get-authenticated", "github-user-get", "/user", &[]),
    (
        "orgs/list-for-authenticated-user",
        "github-org-list",
        "/user/orgs",
        &[],
    ),
    (
        "repos/list-for-org",
        "github-org-repo-list",
        "/orgs/{org}/repos",
        &[],
    ),
    (
        "repos/list-for-authenticated-user",
        "github-user-repo-list",
        "/user/repos",
        &["since", "before"],
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
fn every_selected_operation_id_is_opted_in_one_at_a_time() {
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
            "github-user-get",
            "github-org-list",
            "github-org-repo-list",
            "github-user-repo-list",
        ],
        "an unselected GitHub operation leaked into the connector"
    );
}

/// The four frozen collection reads, whose request bytes are published and must not widen. The
/// discovery reads added by C-527 are deliberately not in this list — see the assertion below.
const FROZEN_PAGINATED_READS: [&str; 4] = [
    "github-issue-list",
    "github-pull-files-list",
    "github-workflow-run-list",
    "github-commit-list",
];

#[test]
fn every_selected_read_is_a_safe_get_with_a_real_response_shape() {
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

        // **Every selected read's query surface is optional and scalar.** Scalar is what C-30's
        // structured `query` map encodes; an array or object has no declared wire shape and is
        // refused at emission. This replaced an "integer only" rule that was a proxy for the same
        // property back when nothing percent-encoded a query value.
        const SCALARS: [&str; 4] = ["string", "integer", "number", "boolean"];
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
                let declared = parameter.schema["type"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{id}.{} declares no query type", parameter.name));
                assert!(
                    SCALARS.contains(&declared),
                    "{id}.{} is a {declared}, which has no declared query wire shape (C-30)",
                    parameter.name
                );
                parameter.name.as_str()
            })
            .collect();

        // The frozen four are held to their exact published surface; widening one is a change to
        // bytes already in the catalogue, not a side effect of selecting a new operation.
        if FROZEN_PAGINATED_READS.contains(&id) {
            assert_eq!(query, ["per_page", "page"], "{id} widened");
        }

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
