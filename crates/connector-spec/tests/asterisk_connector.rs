//! C-485: Asterisk ARI is one spec-backed HTTP connector, not a Flux plugin.

use connector_spec::{AuthScheme, HttpMethod, Idempotency, Risk};

#[path = "support/shipped_provider.rs"]
mod shipped_provider;

#[test]
fn every_non_websocket_ari_operation_is_selected_from_the_vendor_document() {
    let loaded = shipped_provider::load("asterisk");
    let connector = &loaded.connector;

    assert_eq!(connector.operations.len(), 108);
    assert_eq!(connector.provenance.operation_specs.len(), 108);
    assert!(connector
        .provenance
        .operation_specs
        .values()
        .all(|source| source.operation_id != "events.eventWebsocket"));
    assert!(
        connector.events.is_empty(),
        "eventing is future channel work"
    );
    assert!(
        connector.channels.is_empty(),
        "eventing is future channel work"
    );

    let count = |method| {
        connector
            .operations
            .iter()
            .filter(|operation| operation.method == method)
            .count()
    };
    assert_eq!(count(HttpMethod::Get), 32);
    assert_eq!(count(HttpMethod::Post), 48);
    assert_eq!(count(HttpMethod::Put), 8);
    assert_eq!(count(HttpMethod::Delete), 20);
}

#[test]
fn safety_metadata_is_conservative_by_http_method() {
    let connector = shipped_provider::load("asterisk").connector;
    for operation in &connector.operations {
        let expected = match operation.method {
            HttpMethod::Get => (Risk::Low, Idempotency::Idempotent),
            HttpMethod::Post => (Risk::High, Idempotency::NonIdempotent),
            HttpMethod::Put => (Risk::High, Idempotency::Idempotent),
            HttpMethod::Delete => (Risk::Destructive, Idempotency::Idempotent),
            other => panic!("ARI normalized an unsupported method {other:?}"),
        };
        assert_eq!(
            (operation.risk, operation.idempotency),
            expected,
            "{}",
            operation.id
        );
    }
}

#[test]
fn basic_auth_and_deployment_authority_use_the_existing_connector_ports() {
    let connector = shipped_provider::load("asterisk").connector;
    assert_eq!(connector.base_url, "https://{host}:8089/ari");

    let auth = connector
        .auth_method("asterisk.password")
        .expect("Basic credential");
    assert_eq!(auth.scheme, AuthScheme::Basic);
    assert_eq!(auth.user_env, ["ASTERISK_ARI_USERNAME"]);
    assert_eq!(auth.env, ["ASTERISK_ARI_PASSWORD"]);

    let host = connector
        .config
        .iter()
        .find(|field| field.binds == "endpoint.host")
        .expect("deployment host configuration");
    assert_eq!(host.example.as_deref(), Some("pbx.example.com"));
    assert!(!host.secret);
}

#[test]
fn the_complete_catalogue_does_not_become_a_108_tool_prompt() {
    let connector = shipped_provider::load("asterisk").connector;
    let exposed = connector
        .operations
        .iter()
        .filter(|operation| operation.expose)
        .count();
    assert!(exposed > 0, "a useful bounded model surface is required");
    assert!(exposed <= 20, "{exposed} Asterisk operations were exposed");
}
