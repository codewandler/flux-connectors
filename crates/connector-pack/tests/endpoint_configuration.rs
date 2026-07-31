//! **The templated-host gap (C-193), asserted before it is closed.**
//!
//! Six shipped connectors declare a `base_url` carrying a `{placeholder}`, and nothing substitutes a
//! tenant's value into it. The request therefore goes out to a host containing a brace, and
//! `permission_subjects` declares that same unresolvable string as the subject the host's egress
//! allow-list is asked to match.

use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{Credentials, Egress, MemoryStore, Operation};
use flux_runtime::Tool;
use serde_json::json;

fn http() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "a stand-in".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            effects: vec![flux_spec::Effect::Network],
            risk: flux_spec::Risk::Medium,
            idempotency: flux_spec::Idempotency::NonIdempotent,
            access: vec![flux_spec::AccessKind::Network],
            group: None,
        },
        |params| async move { Ok(params) },
    ))
}

fn credentials() -> Credentials {
    Credentials::new(Arc::new(MemoryStore::new()), "t-endpoint").expect("a valid tenant id")
}

fn projected(id: &str) -> Operation {
    let entry = catalog::operation(OperationKey::id(id)).expect("the shipped catalogue carries it");
    Operation::project(entry, http(), credentials()).expect("the entry projects")
}

#[test]
fn a_templated_host_is_substituted_into_the_request_url() {
    let request = projected("zendesk-ticket-show")
        .build_request(&json!({ "ticket_id": 1 }))
        .expect("the request builds");

    assert!(
        !request.url.contains('{'),
        "the request URL still carries an unfilled placeholder: {}",
        request.url
    );
}

#[test]
fn the_permission_subject_is_the_host_the_request_reaches() {
    let subjects = projected("zendesk-ticket-show").permission_subjects(&json!({ "ticket_id": 1 }));

    for subject in &subjects {
        assert!(
            !subject.contains('{'),
            "the egress allow-list is asked to match a subject no host resolves: {subject}"
        );
    }
}
