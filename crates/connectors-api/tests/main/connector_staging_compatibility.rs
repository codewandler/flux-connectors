//! Connector direction remains compatible with Flux's canonical static gather admission (C-516).
//!
//! This belongs in the host graph rather than `connector-pack`'s tests: `flux-flow` reaches the
//! HTTP-capable provider graph, while the pack's transport-free dependency fence is a shipped
//! boundary. The two counterexamples are deliberately not method-shaped: one is an authored write
//! transported with GET, and the other is an authored read transported with POST.

use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{Configuration, Credentials, Egress, MemoryConfig, MemoryStore, Operation};

const TENANT: &str = "t-c516-staging";

fn http() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "C-516 staging compatibility transport".into(),
            input_schema: serde_json::json!({"type": "object"}),
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

fn projected(id: &str) -> Operation {
    let entry = catalog::operation(OperationKey::id(id))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{id}`"));
    Operation::project(
        entry,
        http(),
        Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("a valid tenant"),
        Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("a valid tenant"),
    )
    .unwrap_or_else(|error| panic!("`{id}` projects: {error}"))
}

#[test]
fn canonical_flux_gather_admission_uses_authored_direction_not_http_method() {
    let mutating_get = projected("babelforce-flush-dialer");
    assert!(mutating_get.entry().flux.contains("method: \"GET\""));
    assert!(!flux_flow::statically_gather_safe(&mutating_get));

    let read_post = projected("dropbox-user-me");
    assert!(read_post.entry().flux.contains("method: \"POST\""));
    assert!(flux_flow::statically_gather_safe(&read_post));
}
