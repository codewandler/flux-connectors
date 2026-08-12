//! The generic socket seam composes declarations and tenant ports, but never owns transport.

use std::collections::BTreeMap;
use std::sync::Arc;

use catalog::ProviderKey;
use connector_pack::{
    channel_plan, Configuration, CredentialRef, Credentials, MemoryConfig, MemoryStore, Secret,
    SecretStore, SensitiveText, DEFAULT_SERVICE,
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
    assert_eq!(plan.delivery_id, None);
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
async fn endpoint_authority_drift_refuses_without_echoing_runtime_values() {
    const HOSTILE_HOST: &str = "SENTINEL-UNSAFE-HOST@elsewhere.invalid";

    let (credentials, _) = ports().await;
    let configuration = Configuration::new(
        Arc::new(
            MemoryConfig::new()
                .with_endpoint(TENANT, "asterisk", DEFAULT_SERVICE, "host", HOSTILE_HOST)
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
                ),
        ),
        TENANT,
    )
    .expect("configuration port");
    let error = channel_plan("asterisk", "ari-events", credentials, configuration)
        .await
        .expect_err("configured bytes cannot move the declared authority");
    let message = error.to_string();

    assert!(message.contains("asterisk#ari-events"), "{message}");
    assert!(message.contains("host"), "{message}");
    assert!(!message.contains(HOSTILE_HOST), "{message}");
    assert!(!message.contains(PASSWORD), "{message}");
    assert!(!message.contains("voice-app"), "{message}");
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

// ---------------------------------------------------------------------------------------------
// The channel differential gate (C-558): the engine-free producer against the flux-fed wrapper.
//
// `connector-pack`'s `channel_plan` now delegates to `connector_resolve::channel_plan`, adapting its
// `Configuration`/`Credentials` into the bound `ConfigPort`/`SecretStore` ports. This gate proves the
// relocation is faithful: for every channel binding the catalogue ships, the plan the flux-fed
// wrapper composes and the plan the engine-free producer composes from independently-built bare ports
// are byte-identical — or, for a binding that composes no handshake, both refuse with the same
// sentence. A divergence would be an adapter that mangled a field, a service, or a default.
// ---------------------------------------------------------------------------------------------

const PARITY_TENANT: &str = "t-c558-parity";
const PARITY_SENTINEL: &str = "SENTINEL-NOT-A-REAL-CHANNEL-CREDENTIAL";

/// A bare [`connector_resolve::ConfigPort`] over the same raw values the flux-fed [`Configuration`]
/// is seeded with — the seam X-156 supplies, with no `connector-pack` in it.
struct MapChannelPort(BTreeMap<String, String>);

impl connector_resolve::ConfigPort for MapChannelPort {
    fn resolve(
        &self,
        field: connector_resolve::ConfigField<'_>,
    ) -> Option<connector_resolve::ConfigValue> {
        let key = match field {
            connector_resolve::ConfigField::Endpoint(name) => format!("endpoint.{name}"),
            connector_resolve::ConfigField::Username(name) => format!("username.{name}"),
            connector_resolve::ConfigField::ChannelQuery { channel, parameter } => {
                format!("channel.{channel}.query.{parameter}")
            }
        };
        self.0
            .get(&key)
            .cloned()
            .map(connector_resolve::ConfigValue::proposed)
    }
}

/// The value one non-secret channel field is seeded with — an account for a Basic user half, a bare
/// hostname for an endpoint so the templated authority still validates, and a plain word otherwise.
fn parity_value(binds: &str) -> String {
    if binds.starts_with("username.") {
        "channel-user@example.test".to_owned()
    } else if binds.starts_with("endpoint.") {
        "pbx.example.test".to_owned()
    } else {
        "seed-value".to_owned()
    }
}

/// Seed the raw non-secret values for one binding, skipping any field with a declared default so the
/// producer's relocated default logic runs. Returns the flux-side [`MemoryConfig`] and the identical
/// raw map the engine-free [`MapChannelPort`] answers from.
fn parity_config(
    provider: &'static catalog::Provider,
    channel: &'static catalog::Channel,
) -> (MemoryConfig, BTreeMap<String, String>) {
    let mut memory = MemoryConfig::new();
    let mut raw = BTreeMap::new();
    let query_prefix = format!("channel.{}.query.", channel.name);
    for declaration in provider
        .config
        .iter()
        .filter(|field| field.service == channel.service)
    {
        if declaration.default.is_some() {
            continue;
        }
        let binds = declaration.binds;
        let value = parity_value(binds);
        if let Some(variable) = binds.strip_prefix("endpoint.") {
            memory = memory.with_endpoint(
                PARITY_TENANT,
                provider.id,
                channel.service,
                variable,
                &value,
            );
        } else if let Some(credential) = binds.strip_prefix("username.") {
            memory = memory.with_username(
                PARITY_TENANT,
                provider.id,
                channel.service,
                credential,
                &value,
            );
        } else if let Some(parameter) = binds.strip_prefix(&query_prefix) {
            memory = memory.with_channel_query(
                PARITY_TENANT,
                provider.id,
                channel.service,
                channel.name,
                parameter,
                &value,
            );
        } else {
            continue;
        }
        raw.insert(binds.to_owned(), value);
    }
    (memory, raw)
}

/// Seed the credential store with the sentinel at every address this connector declares.
async fn parity_store(provider: &'static catalog::Provider) -> Arc<MemoryStore> {
    let store = Arc::new(MemoryStore::new());
    if let Some(authority) = provider.authority {
        for credential in provider.auth {
            if let Ok(reference) =
                CredentialRef::new(PARITY_TENANT, authority, DEFAULT_SERVICE, credential.leaf)
            {
                store
                    .put(&reference, &Secret::new(PARITY_SENTINEL))
                    .await
                    .expect("an in-memory put cannot fail");
            }
        }
    }
    store
}

#[tokio::test]
async fn the_engine_free_channel_producer_matches_the_flux_fed_channel_plan() {
    let mut bindings = 0usize;
    let mut plans_built = 0usize;
    let mut refusals = 0usize;

    for &provider in catalog::providers() {
        for channel in provider.channels {
            bindings += 1;

            let (memory, raw) = parity_config(provider, channel);
            let store = parity_store(provider).await;
            let store_dyn: Arc<dyn SecretStore> = store.clone();
            let credentials =
                Credentials::new(store_dyn, PARITY_TENANT).expect("a valid tenant id");
            let configuration =
                Configuration::new(Arc::new(memory), PARITY_TENANT).expect("a valid tenant id");
            let port = MapChannelPort(raw);

            let flux_fed =
                channel_plan(provider.id, channel.name, credentials, configuration).await;
            let engine_free = connector_resolve::channel_plan(
                provider,
                channel,
                PARITY_TENANT,
                None,
                store.as_ref(),
                &port,
            )
            .await;

            match (flux_fed, engine_free) {
                (Ok(flux), Ok(free)) => {
                    assert_eq!(
                        flux, free,
                        "`{}#{}`: the flux-fed wrapper and the engine-free producer composed \
                         different plans",
                        provider.id, channel.name
                    );
                    plans_built += 1;
                }
                (Err(flux), Err(free)) => {
                    assert_eq!(
                        flux.to_string(),
                        free.to_string(),
                        "`{}#{}`: the two paths refused with different sentences",
                        provider.id,
                        channel.name
                    );
                    refusals += 1;
                }
                (flux, free) => panic!(
                    "`{}#{}`: one path composed and the other refused — flux `{flux:?}`, \
                     engine-free `{free:?}`",
                    provider.id, channel.name
                ),
            }
        }
    }

    // The embedded catalogue carries exactly five channel bindings today (asterisk's generic socket,
    // slack's Socket Mode + webhook, twilio's two webhooks). Pinned so a binding added or dropped is a
    // visible change here rather than a silently narrower gate.
    assert_eq!(bindings, 5, "the catalogue's channel-binding count moved");
    assert!(
        plans_built >= 1,
        "no binding composed a full handshake, so the byte comparison never ran on a real plan"
    );
    assert_eq!(
        bindings,
        plans_built + refusals,
        "a binding neither composed nor refused on both paths"
    );
}

/// **The control.** A gate green across the five bindings has said one of two things — the two paths
/// agree, or the comparison cannot tell. So a real engine-free plan is taken, both paths are confirmed
/// to agree, and then one URL is moved: the gate's own `==` must report the difference.
#[tokio::test]
async fn a_seeded_divergence_in_the_engine_free_channel_plan_is_caught() {
    let provider =
        catalog::provider(ProviderKey::id("asterisk")).expect("the shipped asterisk connector");
    let channel = provider
        .channel("ari-events")
        .expect("its generic socket binding");

    let (memory, raw) = parity_config(provider, channel);
    let store = parity_store(provider).await;
    let store_dyn: Arc<dyn SecretStore> = store.clone();
    let credentials = Credentials::new(store_dyn, PARITY_TENANT).expect("a valid tenant id");
    let configuration =
        Configuration::new(Arc::new(memory), PARITY_TENANT).expect("a valid tenant id");
    let port = MapChannelPort(raw);

    let flux = channel_plan(provider.id, channel.name, credentials, configuration)
        .await
        .expect("the flux-fed wrapper composes asterisk");
    let engine_free = connector_resolve::channel_plan(
        provider,
        channel,
        PARITY_TENANT,
        None,
        store.as_ref(),
        &port,
    )
    .await
    .expect("the engine-free producer composes asterisk");

    // The two agree before a seed means anything.
    assert_eq!(
        flux, engine_free,
        "the shipped catalogue must agree with itself before a seed means anything"
    );

    // The seed: one query argument appended to the composed URL. The byte comparison — the gate's own
    // `==` — must see it.
    let mut seeded = engine_free.clone();
    seeded.url = SensitiveText::new(format!("{}&seeded=divergence", seeded.url.expose_secret()));
    assert_ne!(
        flux, seeded,
        "a moved URL is not caught by the plan comparison"
    );
}
