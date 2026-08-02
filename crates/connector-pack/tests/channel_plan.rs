//! The generic socket seam composes declarations and tenant ports, but never owns transport.

use std::sync::Arc;

use connector_pack::{
    channel_plan, Configuration, CredentialRef, Credentials, MemoryConfig, MemoryStore, Secret,
    SecretStore, DEFAULT_SERVICE,
};

const TENANT: &str = "t-ari-channel";
const PASSWORD: &str = "SENTINEL-NOT-A-REAL-ARI-PASSWORD";

async fn ports() -> (Credentials, Configuration) {
    let store = MemoryStore::new();
    let reference = CredentialRef::new(TENANT, "org.asterisk.ari", DEFAULT_SERVICE, "password")
        .expect("Asterisk's generated credential address");
    store
        .put(&reference, &Secret::new(PASSWORD))
        .await
        .expect("memory store write");
    let configuration = MemoryConfig::new()
        .with_endpoint(
            TENANT,
            "asterisk",
            DEFAULT_SERVICE,
            "host",
            "pbx.example.com",
        )
        .with_username(
            TENANT,
            "asterisk",
            DEFAULT_SERVICE,
            "asterisk.password",
            "flux",
        )
        .with_channel_query(
            TENANT,
            "asterisk",
            DEFAULT_SERVICE,
            "ari-events",
            "app",
            "voice-app",
        );
    (
        Credentials::new(Arc::new(store), TENANT).expect("credential port"),
        Configuration::new(Arc::new(configuration), TENANT).expect("configuration port"),
    )
}

#[tokio::test]
async fn asterisk_composes_exact_query_defaults_and_basic_auth_without_transport() {
    let (credentials, configuration) = ports().await;
    let plan = channel_plan("asterisk", "ari-events", credentials, configuration)
        .await
        .expect("a complete declared handshake");

    assert_eq!(
        plan.url.expose_secret(),
        "wss://pbx.example.com:8089/ari/events?app=voice-app&subscribeAll=false"
    );
    assert_eq!(
        plan.headers["Authorization"].expose_secret(),
        "Basic Zmx1eDpTRU5USU5FTC1OT1QtQS1SRUFMLUFSSS1QQVNTV09SRA=="
    );
    assert_eq!(plan.events.len(), 45);
    assert!(plan.events.contains(&"channel-created"));
    assert_eq!(plan.declared_base_url, "https://{host}:8089/ari");
    assert_eq!(
        plan.wire_events.get("ChannelCreated"),
        Some(&"channel-created")
    );
    assert_eq!(plan.discriminator.expect("ARI discriminator").name, "type");
    assert!(plan.payload_root);
    assert!(plan.payload.is_empty());

    let debug = format!("{plan:?}");
    assert!(!debug.contains(PASSWORD), "{debug}");
    assert!(!debug.contains("Zmx1eDpTRU5USU5FTC"), "{debug}");
    assert!(!debug.contains("voice-app"), "{debug}");
    assert!(debug.contains("<redacted>"), "{debug}");
}

#[tokio::test]
async fn missing_channel_configuration_refuses_before_a_plan_exists() {
    let (credentials, _) = ports().await;
    let configuration = Configuration::new(
        Arc::new(
            MemoryConfig::new()
                .with_endpoint(
                    TENANT,
                    "asterisk",
                    DEFAULT_SERVICE,
                    "host",
                    "pbx.example.com",
                )
                .with_username(
                    TENANT,
                    "asterisk",
                    DEFAULT_SERVICE,
                    "asterisk.password",
                    "flux",
                ),
        ),
        TENANT,
    )
    .expect("configuration port");
    let error = channel_plan("asterisk", "ari-events", credentials, configuration)
        .await
        .expect_err("the required app cannot be guessed");
    assert!(
        error.to_string().contains("channel.ari-events.query.app"),
        "{error}"
    );
}

#[tokio::test]
async fn slack_socket_mode_is_not_misclassified_as_a_generic_socket() {
    let credentials = Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("port");
    let configuration = Configuration::new(Arc::new(MemoryConfig::new()), TENANT).expect("port");
    let error = channel_plan("slack", "socket", credentials, configuration)
        .await
        .expect_err("Socket Mode remains vendor-specific");
    assert!(error.to_string().contains("vendor-specific"), "{error}");
}
