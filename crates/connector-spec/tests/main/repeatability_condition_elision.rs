//! **Landing `repeatable_because` must not move a single existing `ir_sha256`.**
//!
//! `Operation::repeatable_because` carries `#[serde(default, skip_serializing_if =
//! "Option::is_none")]`, and its doc comment claims that keeps every unaffected connector hashing
//! exactly as it did before the field existed. That claim was **only a comment** in C-186's first
//! landing: deleting the `skip_serializing_if` turned nothing in the workspace red.
//!
//! The property matters because `Connector::hash_domain` feeds `LockEntry::ir_sha256`. A field that
//! serialized as `"repeatable_because": null` on all 299 operations would move every hash in the
//! repository and churn the lockfile for providers nobody edited — the exact reasoning
//! `tests/service_roles.rs` records for `Service::roles`, and the precedent this test copies.

use connector_spec::provider;

fn load(operations: &str) -> connector_spec::Connector {
    let source = format!(
        r#"
id = "acme"
vendor = "Acme"
description = "A fixture connector"
base_url = "https://api.acme.test"
{operations}"#
    );
    provider::load("acme", &source)
        .expect("the fixture loads")
        .connector
}

const A_READ: &str = r#"
[[operations]]
id = "acme-thing-get"
method = "GET"
direction = "read"
path = "/v1/things"
description = "List things."
risk = "low"
idempotency = "idempotent"
"#;

/// An operation stating no condition must not mention the field in its hash domain.
#[test]
fn an_operation_stating_no_condition_hashes_as_it_did_before_the_field_existed() {
    let domain = load(A_READ).hash_domain().expect("the hash domain encodes");

    assert!(
        !domain.contains("repeatable_because"),
        "an absent `repeatable_because` reached the hash domain, so landing C-186 moved the \
         `ir_sha256` of all 299 shipped operations and churned the lockfile for every provider \
         nobody edited: {domain}"
    );
}

/// The converse, so the test above cannot pass by the field being dropped from the domain entirely.
///
/// Without this, deleting the field from `Operation` would satisfy the assertion above perfectly.
#[test]
fn an_operation_stating_a_condition_does_carry_it_into_the_hash_domain() {
    let domain = load(
        r#"
[[operations]]
id = "acme-cache-purge"
method = "POST"
direction = "write"
path = "/cache/purge"
description = "Empty the cache."
risk = "high"
idempotency = "conditional"
repeatable_because = "purging an already-purged cache is a no-op"
"#,
    )
    .hash_domain()
    .expect("the hash domain encodes");

    assert!(
        domain.contains("repeatable_because") && domain.contains("already-purged"),
        "a stated condition must reach the hash domain — it is part of what the connector means, \
         so editing it has to be a change `diff` and the lockfile can both see: {domain}"
    );
}
