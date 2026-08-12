//! C-485: Asterisk ARI is one spec-backed HTTP connector, not a Flux plugin.

use std::collections::BTreeSet;

use connector_spec::{AuthScheme, Binding, HttpMethod, Idempotency, Risk, Transport};
use serde_json::Value;

use crate::shipped_provider;

#[test]
fn every_ari_operation_with_a_modelled_query_shape_is_published() {
    let loaded = shipped_provider::load("asterisk");
    let connector = &loaded.connector;

    assert_eq!(connector.operations.len(), 96);
    assert_eq!(connector.provenance.operation_specs.len(), 96);
    assert!(connector
        .provenance
        .operation_specs
        .values()
        .all(|source| source.operation_id != "events.eventWebsocket"));
    assert_eq!(connector.events.len(), 45);
    assert_eq!(connector.channels.len(), 1);

    let count = |method| {
        connector
            .operations
            .iter()
            .filter(|operation| operation.method == method)
            .count()
    };
    assert_eq!(count(HttpMethod::Get), 29);
    assert_eq!(count(HttpMethod::Post), 40);
    assert_eq!(count(HttpMethod::Put), 8);
    assert_eq!(count(HttpMethod::Delete), 19);

    let deferred = loaded
        .patch
        .operations
        .iter()
        .filter(|patch| patch.defer.is_some())
        .map(|patch| patch.select.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        deferred,
        BTreeSet::from([
            "applications-subscribe",
            "applications-unsubscribe",
            "asterisk-getInfo",
            "bridges-addChannel",
            "bridges-getBridgeVars",
            "bridges-play",
            "bridges-playWithId",
            "bridges-removeChannel",
            "channels-getChannelVars",
            "channels-play",
            "channels-playWithId",
            "events-userEvent",
        ]),
        "only operations with unmodelled array-valued query parameters may be deferred"
    );
}

#[test]
fn the_websocket_and_every_declared_event_subtype_are_accounted_for() {
    let connector = shipped_provider::load("asterisk").connector;
    let source: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../specs/asterisk/api-docs/events.json"),
        )
        .expect("vendored events document"),
    )
    .expect("events document is JSON");
    let source_events = source["models"]["Event"]["subTypes"]
        .as_array()
        .expect("Event subtype census")
        .iter()
        .map(|value| value.as_str().expect("subtype name"))
        .collect::<BTreeSet<_>>();
    let declared_wire_values = connector
        .events
        .iter()
        .map(|event| event.wire_value.as_deref().expect("exact ARI wire value"))
        .collect::<BTreeSet<_>>();
    assert_eq!(declared_wire_values, source_events);

    for event in &connector.events {
        let schema = event.schema.as_ref().expect("full event schema");
        assert_eq!(
            schema["properties"]["type"]["const"].as_str(),
            event.wire_value.as_deref()
        );
        assert!(schema["properties"].get("application").is_some());
        assert!(schema["properties"].get("timestamp").is_some());
        assert!(
            event
                .name
                .chars()
                .all(|character| character.is_ascii_lowercase() || character == '-'),
            "{}",
            event.name
        );
    }

    let channel = connector.channel("ari-events").expect("ARI event socket");
    assert_eq!(channel.transport, Transport::Socket);
    assert!(channel.payload_root);
    assert_eq!(
        channel
            .discriminator
            .as_ref()
            .map(|selector| selector.name.as_str()),
        Some("type")
    );
    assert_eq!(channel.events.len(), source_events.len());
    let connect = channel
        .connect
        .as_ref()
        .expect("declarative socket handshake");
    assert_eq!(connect.path, "/events");
    assert_eq!(connect.query["app"], "{app}");
    assert_eq!(connect.query["subscribeAll"], "{subscribe_all}");
    assert_eq!(connect.auth, connector.default_auth);

    let app = connector
        .config
        .iter()
        .find(|field| field.name == "app")
        .expect("required Stasis application");
    assert!(app.required);
    assert_eq!(
        app.binding(),
        Some(Binding::ChannelQuery {
            channel: "ari-events",
            parameter: "app",
        })
    );
    let subscribe_all = connector
        .config
        .iter()
        .find(|field| field.name == "subscribe_all")
        .expect("optional subscribe-all setting");
    assert!(!subscribe_all.required);
    assert_eq!(subscribe_all.default.as_deref(), Some("false"));
}

#[test]
fn safety_metadata_never_invites_an_automatic_write_replay() {
    let connector = shipped_provider::load("asterisk").connector;
    for operation in &connector.operations {
        let expected = match operation.method {
            HttpMethod::Get => (Risk::Low, Idempotency::Idempotent),
            HttpMethod::Post => (Risk::High, Idempotency::NonIdempotent),
            HttpMethod::Put => (Risk::High, Idempotency::NonIdempotent),
            HttpMethod::Delete => (Risk::Destructive, Idempotency::NonIdempotent),
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
fn the_complete_catalogue_does_not_become_a_96_tool_prompt() {
    let connector = shipped_provider::load("asterisk").connector;
    let exposed = connector
        .operations
        .iter()
        .filter(|operation| operation.expose)
        .count();
    assert!(exposed > 0, "a useful bounded model surface is required");
    assert!(exposed <= 20, "{exposed} Asterisk operations were exposed");
}
